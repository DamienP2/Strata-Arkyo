//! Exploitation du site : mises à jour OTA, sauvegardes planifiées, redémarrage.
//!
//! Le moteur ne se redémarre pas lui-même : il s'arrête proprement avec le code 75
//! et systemd le relance sur la version que désigne le lien `current`.

use chrono::{DateTime, Local, NaiveTime, TimeZone, Utc};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use strata_backup::{Backups, Kind};
use strata_store::Store;
use strata_update::{Layout, ReleaseManifest, TrustedKeys, fetch, manifest, package, version};
use tokio::sync::{Mutex, watch};

pub const EXIT_RESTART: i32 = 75;

pub const META_CHANNEL: &str = "update.channel";
pub const META_AUTO_CHECK: &str = "update.auto_check";
pub const META_LAST_CHECK: &str = "update.last_check";
pub const META_BACKUP_DIR: &str = "backup.dir";
pub const META_BACKUP_ERROR: &str = "backup.last_error";
pub const META_BACKUP_LAST: &str = "backup.last_success";

/// Heure de la sauvegarde quotidienne (heure locale du serveur).
pub const BACKUP_TIME: (u32, u32) = (3, 0);
/// Heure d'installation « cette nuit ».
pub const NIGHT_INSTALL_TIME: (u32, u32) = (3, 30);

#[derive(Debug)]
pub enum OpsError {
    Conflict(String),
    Invalid(String),
    Unavailable(String),
    Internal(String),
}

impl<E: std::fmt::Display> From<E> for OpsError
where
    E: std::error::Error,
{
    fn from(e: E) -> Self {
        OpsError::Internal(e.to_string())
    }
}

pub type OpsResult<T> = Result<T, OpsError>;

fn update_err(e: strata_update::UpdateError) -> OpsError {
    use strata_update::UpdateError as U;
    match e {
        U::Io(_) | U::Json(_) => OpsError::Internal(e.to_string()),
        U::Fetch(_) => OpsError::Unavailable(e.to_string()),
        other => OpsError::Invalid(other.to_string()),
    }
}

fn backup_err(e: strata_backup::BackupError) -> OpsError {
    use strata_backup::BackupError as B;
    match e {
        B::NotFound(_) | B::Invalid(_) | B::NothingPending | B::Integrity => {
            OpsError::Invalid(e.to_string())
        }
        other => OpsError::Internal(other.to_string()),
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    Downloading,
    Testing,
    Scheduled,
    Restarting,
}

#[derive(Debug, Clone, Serialize)]
pub struct Scheduled {
    pub version: String,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdateStatus {
    pub phase: Phase,
    /// Version plus récente annoncée par le canal, s'il y en a une.
    pub available: Option<ReleaseManifest>,
    pub last_check_error: Option<String>,
    /// Version préparée et installation planifiée.
    pub scheduled: Option<Scheduled>,
    /// Heure du redémarrage annoncé aux utilisateurs connectés.
    pub restart_at: Option<String>,
    pub last_error: Option<String>,
}

pub struct Ops {
    pub data_dir: PathBuf,
    pub layout: Layout,
    pub keys: TrustedKeys,
    /// Adresse de base du serveur de publication (`https://…` ou `file://…`).
    pub update_url: Option<String>,
    /// Délai entre l'annonce du redémarrage et la bascule.
    pub countdown: Duration,
    pub version: String,
    status: Mutex<UpdateStatus>,
    busy: Mutex<()>,
    restart_tx: watch::Sender<bool>,
}

fn rfc3339(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Prochaine occurrence de l'heure locale `hh:mm` après `now`.
pub fn next_local(now: DateTime<Local>, (h, m): (u32, u32)) -> DateTime<Local> {
    let time = NaiveTime::from_hms_opt(h, m, 0).expect("heure valide");
    let mut day = now.date_naive();
    loop {
        if let Some(t) = Local.from_local_datetime(&day.and_time(time)).earliest() {
            if t > now {
                return t;
            }
        }
        day = day.succ_opt().expect("date valide");
    }
}

impl Ops {
    pub fn new(
        data_dir: PathBuf,
        layout: Layout,
        keys: TrustedKeys,
        update_url: Option<String>,
        countdown: Duration,
        version: &str,
    ) -> (Arc<Ops>, watch::Receiver<bool>) {
        let (tx, rx) = watch::channel(false);
        let ops = Ops {
            data_dir,
            layout,
            keys,
            update_url: update_url.filter(|u| !u.trim().is_empty()),
            countdown,
            version: version.into(),
            status: Mutex::new(UpdateStatus::default()),
            busy: Mutex::new(()),
            restart_tx: tx,
        };
        (Arc::new(ops), rx)
    }

    pub fn request_restart(&self) {
        let _ = self.restart_tx.send(true);
    }

    pub async fn status(&self) -> UpdateStatus {
        self.status.lock().await.clone()
    }

    async fn set_phase(&self, p: Phase) {
        self.status.lock().await.phase = p;
    }

    pub fn updates_dir(&self) -> PathBuf {
        self.data_dir.join("updates")
    }

    // ----- Réglages -----

    pub async fn backup_dir(&self, store: &Store) -> OpsResult<PathBuf> {
        Ok(store
            .meta(META_BACKUP_DIR)
            .await?
            .map(PathBuf::from)
            .unwrap_or_else(|| self.data_dir.join("backups")))
    }

    pub async fn backups(&self, store: &Store) -> OpsResult<Backups> {
        Ok(Backups::new(&self.data_dir, self.backup_dir(store).await?))
    }

    pub async fn channel(&self, store: &Store) -> OpsResult<String> {
        Ok(store
            .meta(META_CHANNEL)
            .await?
            .unwrap_or_else(|| "stable".into()))
    }

    pub async fn auto_check(&self, store: &Store) -> OpsResult<bool> {
        Ok(store.meta(META_AUTO_CHECK).await?.as_deref() != Some("false"))
    }

    fn require_managed(&self) -> OpsResult<()> {
        if !self.layout.is_managed() {
            return Err(OpsError::Unavailable(
                "Ce serveur n'a pas été installé par le paquet Strata Arkyo : les mises à jour sont désactivées.".into(),
            ));
        }
        if self.keys.is_empty() {
            return Err(OpsError::Unavailable(
                "Cette version n'a pas de clé de publication : les mises à jour signées sont désactivées.".into(),
            ));
        }
        Ok(())
    }

    // ----- Sauvegardes -----

    /// Sauvegarde complète du site. Une sauvegarde automatique ratée reste signalée jusqu'au succès suivant.
    pub async fn backup(
        &self,
        store: &Store,
        kind: Kind,
        label: Option<String>,
    ) -> OpsResult<strata_backup::BackupInfo> {
        let backups = self.backups(store).await?;
        match backups.create(store, kind, label, &self.version).await {
            Ok(info) => {
                store.set_meta(META_BACKUP_ERROR, None).await?;
                store
                    .set_meta(META_BACKUP_LAST, Some(&info.manifest.created_at))
                    .await?;
                Ok(info)
            }
            Err(e) => {
                let msg = format!("{} : {e}", rfc3339(Utc::now()));
                store.set_meta(META_BACKUP_ERROR, Some(&msg)).await?;
                tracing::error!(error = %e, "sauvegarde du site en échec");
                Err(backup_err(e))
            }
        }
    }

    pub async fn stage_restore(
        &self,
        store: &Store,
        name: &str,
    ) -> OpsResult<strata_backup::Manifest> {
        self.backups(store)
            .await?
            .stage_restore(name)
            .map_err(backup_err)
    }

    // ----- Mises à jour -----

    /// Interroge le canal. Ne fait que signaler : rien n'est téléchargé.
    pub async fn check(&self, store: &Store) -> OpsResult<Option<ReleaseManifest>> {
        self.require_managed()?;
        let base = self.update_url.clone().ok_or_else(|| {
            OpsError::Unavailable(
                "Aucun serveur de publication n'est configuré (STRATA_UPDATE_URL).".into(),
            )
        })?;
        let channel = self.channel(store).await?;
        self.set_phase(Phase::Checking).await;
        let result = manifest::fetch_manifest(&base, &channel, &self.keys).await;
        store
            .set_meta(META_LAST_CHECK, Some(&rfc3339(Utc::now())))
            .await?;
        let mut st = self.status.lock().await;
        if st.phase == Phase::Checking {
            st.phase = if st.scheduled.is_some() {
                Phase::Scheduled
            } else {
                Phase::Idle
            };
        }
        match result {
            Ok(m) => {
                st.last_check_error = None;
                st.available = (version::is_newer(&m.version, &self.version)
                    && m.arch == strata_update::host_arch())
                .then_some(m);
                Ok(st.available.clone())
            }
            Err(e) => {
                st.last_check_error = Some(e.to_string());
                Err(update_err(e))
            }
        }
    }

    /// Télécharge la version annoncée, la vérifie et la prépare (sans basculer).
    pub async fn prepare_online(&self, store: &Store) -> OpsResult<String> {
        self.require_managed()?;
        let base = self
            .update_url
            .clone()
            .ok_or_else(|| OpsError::Unavailable("Aucun serveur de publication.".into()))?;
        let m = match self.status().await.available {
            Some(m) => m,
            None => self
                .check(store)
                .await?
                .ok_or_else(|| OpsError::Conflict("Aucune nouvelle version disponible.".into()))?,
        };
        let _guard = self
            .busy
            .try_lock()
            .map_err(|_| OpsError::Conflict("Une mise à jour est déjà en cours.".into()))?;
        self.set_phase(Phase::Downloading).await;
        let result = async {
            let bytes = fetch::get(&manifest::package_url(&base, &m))
                .await
                .map_err(update_err)?;
            if strata_update::signing::sha256_hex(&bytes) != m.sha256 {
                return Err(update_err(strata_update::UpdateError::HashMismatch));
            }
            std::fs::create_dir_all(self.updates_dir())?;
            let path = self
                .updates_dir()
                .join(package::file_name(&m.version, &m.arch));
            std::fs::write(&path, bytes)?;
            self.prepare_file_locked(store, &path).await
        }
        .await;
        self.after_prepare(&result).await;
        result
    }

    /// Prépare un paquet déposé (mise à jour hors ligne).
    pub async fn prepare_file(&self, store: &Store, path: &Path) -> OpsResult<String> {
        self.require_managed()?;
        let _guard = self
            .busy
            .try_lock()
            .map_err(|_| OpsError::Conflict("Une mise à jour est déjà en cours.".into()))?;
        let result = self.prepare_file_locked(store, path).await;
        self.after_prepare(&result).await;
        result
    }

    async fn after_prepare(&self, result: &OpsResult<String>) {
        let mut st = self.status.lock().await;
        st.phase = if st.scheduled.is_some() {
            Phase::Scheduled
        } else {
            Phase::Idle
        };
        if let Err(e) = result {
            st.last_error = Some(match e {
                OpsError::Conflict(m)
                | OpsError::Invalid(m)
                | OpsError::Unavailable(m)
                | OpsError::Internal(m) => m.clone(),
            });
        }
    }

    async fn prepare_file_locked(&self, store: &Store, path: &Path) -> OpsResult<String> {
        self.set_phase(Phase::Testing).await;
        let pkg = package::open(path, &self.keys).map_err(update_err)?;
        let v = pkg.info.version.clone();
        if !version::is_newer(&v, &self.version) {
            return Err(OpsError::Conflict(format!(
                "La version {v} n'est pas plus récente que la version installée {} : utilisez le retour arrière.",
                self.version
            )));
        }
        self.layout.install_binary(&pkg).map_err(update_err)?;
        // Essai à blanc sur une copie de la base.
        let test_dir = self.updates_dir().join("selftest");
        let _ = std::fs::remove_dir_all(&test_dir);
        std::fs::create_dir_all(&test_dir)?;
        store.vacuum_into(&test_dir.join("strata.db")).await?;
        let outcome = self.layout.selftest(&v, &test_dir).await;
        let _ = std::fs::remove_dir_all(&test_dir);
        outcome.map_err(update_err)?;
        tracing::info!(version = %v, "nouvelle version prête");
        Ok(v)
    }

    /// Planifie la bascule : maintenant (après le compte à rebours) ou à l'heure donnée.
    pub async fn schedule(
        self: &Arc<Self>,
        store: &Store,
        v: String,
        at: Option<DateTime<Utc>>,
    ) -> OpsResult<Scheduled> {
        let at = at.unwrap_or_else(|| {
            Utc::now() + chrono::Duration::from_std(self.countdown).unwrap_or_default()
        });
        let s = Scheduled {
            version: v,
            at: rfc3339(at),
        };
        {
            let mut st = self.status.lock().await;
            st.scheduled = Some(s.clone());
            st.phase = Phase::Scheduled;
            st.last_error = None;
            st.restart_at = Some(s.at.clone());
        }
        // Une bascule proche est suivie de près ; une bascule lointaine passe par la boucle de fond.
        let delay = (at - Utc::now()).to_std().unwrap_or_default();
        if delay <= Duration::from_secs(120) {
            let ops = self.clone();
            let store = store.clone();
            tokio::spawn(async move {
                tokio::time::sleep(delay).await;
                ops.run_due(&store).await;
            });
        }
        Ok(s)
    }

    pub async fn cancel_schedule(&self) -> OpsResult<()> {
        let mut st = self.status.lock().await;
        if st.phase == Phase::Restarting {
            return Err(OpsError::Conflict("La bascule est déjà lancée.".into()));
        }
        if st.scheduled.take().is_none() {
            return Err(OpsError::Conflict("Aucune installation planifiée.".into()));
        }
        st.restart_at = None;
        st.phase = Phase::Idle;
        Ok(())
    }

    /// Lance la bascule planifiée si son heure est venue.
    async fn run_due(&self, store: &Store) {
        let due = {
            let st = self.status.lock().await;
            st.scheduled.clone().filter(|s| {
                DateTime::parse_from_rfc3339(&s.at)
                    .map(|t| t <= Utc::now())
                    .unwrap_or(false)
            })
        };
        let Some(s) = due else { return };
        if let Err(e) = self.finish(store, &s.version).await {
            let mut st = self.status.lock().await;
            st.phase = Phase::Idle;
            st.scheduled = None;
            st.restart_at = None;
            st.last_error = Some(match e {
                OpsError::Conflict(m)
                | OpsError::Invalid(m)
                | OpsError::Unavailable(m)
                | OpsError::Internal(m) => m,
            });
        }
    }

    /// Point de restauration du site, bascule du lien, puis arrêt pour redémarrage.
    pub async fn finish(&self, store: &Store, v: &str) -> OpsResult<()> {
        self.set_phase(Phase::Restarting).await;
        self.backup(
            store,
            Kind::PreUpdate,
            Some(format!("Avant la mise à jour {v}")),
        )
        .await?;
        self.layout.switch_to(v).map_err(update_err)?;
        store
            .audit::<(), _>(
                None,
                "update_switch",
                "site",
                "engine",
                None,
                Some(&serde_json::json!({ "from": self.version, "to": v })),
            )
            .await?;
        tracing::warn!(from = %self.version, to = %v, "bascule effectuée : redémarrage");
        self.request_restart();
        Ok(())
    }

    /// Retour manuel à la version précédente, puis redémarrage.
    pub async fn rollback(&self, store: &Store) -> OpsResult<String> {
        if !self.layout.is_managed() {
            return Err(OpsError::Unavailable(
                "Ce serveur n'a pas été installé par le paquet Strata Arkyo.".into(),
            ));
        }
        let target = self.layout.rollback().map_err(update_err)?;
        store
            .audit::<(), _>(
                None,
                "update_rollback",
                "site",
                "engine",
                None,
                Some(&serde_json::json!({ "from": self.version, "to": target })),
            )
            .await?;
        self.set_phase(Phase::Restarting).await;
        self.request_restart();
        Ok(target)
    }

    /// Tâches de fond : sauvegarde quotidienne, recherche quotidienne de mise à jour, bascules planifiées.
    pub fn spawn_background(self: &Arc<Self>, store: Store) {
        let ops = self.clone();
        tokio::spawn(async move {
            let mut next_backup = next_local(Local::now(), BACKUP_TIME);
            let mut next_check = Instant::now() + Duration::from_secs(600);
            let mut tick = tokio::time::interval(Duration::from_secs(30));
            loop {
                tick.tick().await;
                if Local::now() >= next_backup {
                    let _ = ops.backup(&store, Kind::Auto, None).await;
                    next_backup = next_local(Local::now(), BACKUP_TIME);
                }
                if Instant::now() >= next_check {
                    next_check = Instant::now() + Duration::from_secs(24 * 3600);
                    let enabled = ops.auto_check(&store).await.unwrap_or(false);
                    if enabled
                        && ops.update_url.is_some()
                        && ops.layout.is_managed()
                        && !ops.keys.is_empty()
                    {
                        match ops.check(&store).await {
                            Ok(Some(m)) => {
                                tracing::info!(version = %m.version, "nouvelle version disponible")
                            }
                            Ok(None) => {}
                            Err(e) => tracing::warn!(?e, "recherche de mise à jour"),
                        }
                    }
                }
                ops.run_due(&store).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_local_time_is_always_ahead() {
        let at = Local.with_ymd_and_hms(2026, 11, 11, 2, 0, 0).unwrap();
        assert_eq!(
            next_local(at, (3, 0)),
            Local.with_ymd_and_hms(2026, 11, 11, 3, 0, 0).unwrap()
        );
        let at = Local.with_ymd_and_hms(2026, 11, 11, 3, 0, 0).unwrap();
        assert_eq!(
            next_local(at, (3, 0)),
            Local.with_ymd_and_hms(2026, 11, 12, 3, 0, 0).unwrap()
        );
    }
}
