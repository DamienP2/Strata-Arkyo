//! Versions côte à côte, bascule atomique et retour arrière.
//!
//! ```text
//! /opt/strata-arkyo/
//! ├── versions/0.1.0/strata
//! ├── versions/0.2.0/strata
//! ├── current -> versions/0.2.0     (lien lu par le service systemd)
//! └── update-state.json             (version en attente, version précédente, compteur)
//! ```

use crate::package::VerifiedPackage;
use crate::{Result, UpdateError, version};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Au-delà de trois démarrages ratés, la version précédente est rétablie.
pub const MAX_FAILED_BOOTS: u32 = 3;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateState {
    /// Version installée par la dernière bascule et pas encore confirmée.
    pub pending: Option<String>,
    /// Version qui tournait avant la dernière bascule.
    pub previous: Option<String>,
    /// Démarrages de la version en attente depuis la bascule.
    pub attempts: u32,
    pub switched_at: Option<String>,
    pub confirmed_at: Option<String>,
    /// Dernier retour arrière automatique ou manuel.
    pub rolled_back_from: Option<String>,
    pub rolled_back_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InstalledVersion {
    pub version: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootDecision {
    /// Continuer le démarrage.
    Continue,
    /// Trop d'échecs : le lien désigne de nouveau `to`. Le programme doit s'arrêter
    /// pour que systemd relance l'ancienne version.
    RolledBack { from: String, to: String },
}

#[derive(Debug, Clone)]
pub struct Layout {
    root: PathBuf,
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

impl Layout {
    pub fn new(root: impl Into<PathBuf>) -> Layout {
        Layout { root: root.into() }
    }

    /// `STRATA_INSTALL_DIR`, ou `/opt/strata-arkyo`.
    pub fn from_env() -> Layout {
        Layout::new(
            std::env::var_os("STRATA_INSTALL_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| "/opt/strata-arkyo".into()),
        )
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn versions_dir(&self) -> PathBuf {
        self.root.join("versions")
    }

    fn current_link(&self) -> PathBuf {
        self.root.join("current")
    }

    fn state_path(&self) -> PathBuf {
        self.root.join("update-state.json")
    }

    pub fn binary(&self, v: &str) -> PathBuf {
        self.versions_dir().join(v).join("strata")
    }

    /// Vrai si le programme tourne depuis une installation par paquet (le lien `current` existe).
    pub fn is_managed(&self) -> bool {
        self.current_link().symlink_metadata().is_ok()
    }

    pub fn current_version(&self) -> Option<String> {
        let target = std::fs::read_link(self.current_link()).ok()?;
        Some(target.file_name()?.to_string_lossy().to_string())
    }

    pub fn installed(&self) -> Vec<InstalledVersion> {
        let current = self.current_version();
        let mut out: Vec<InstalledVersion> = std::fs::read_dir(self.versions_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().join("strata").is_file())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .map(|v| InstalledVersion {
                active: Some(&v) == current.as_ref(),
                version: v,
            })
            .collect();
        out.sort_by(|a, b| version::compare(&b.version, &a.version));
        out
    }

    pub fn state(&self) -> UpdateState {
        std::fs::read(self.state_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn save_state(&self, s: &UpdateState) -> Result<()> {
        let tmp = self.root.join("update-state.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(s)?)?;
        std::fs::rename(tmp, self.state_path())?;
        Ok(())
    }

    /// Écrit le programme du paquet dans `versions/<version>/strata`, sans toucher au lien.
    pub fn install_binary(&self, pkg: &VerifiedPackage) -> Result<PathBuf> {
        let dir = self.versions_dir().join(&pkg.info.version);
        std::fs::create_dir_all(&dir)?;
        let dest = dir.join("strata");
        let tmp = dir.join(".strata.partial");
        std::fs::write(&tmp, &pkg.binary)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
        }
        std::fs::rename(&tmp, &dest)?;
        Ok(dest)
    }

    /// Essai à blanc : le nouveau programme annonce sa version, puis ouvre une copie
    /// de la base, applique ses migrations et contrôle l'intégrité.
    pub async fn selftest(&self, v: &str, data_copy: &Path) -> Result<String> {
        let bin = self.binary(v);
        let fail = |detail: String| UpdateError::SelftestFailed {
            version: v.into(),
            detail,
        };
        let run = |args: Vec<String>| {
            let bin = bin.clone();
            async move {
                let out = tokio::time::timeout(
                    Duration::from_secs(120),
                    tokio::process::Command::new(&bin)
                        .args(&args)
                        .env_remove("STRATA_DATA_DIR")
                        .output(),
                )
                .await
                .map_err(|_| "délai dépassé".to_string())?
                .map_err(|e| e.to_string())?;
                let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if out.status.success() {
                    Ok(text)
                } else {
                    Err(format!(
                        "{} {}",
                        text,
                        String::from_utf8_lossy(&out.stderr).trim()
                    ))
                }
            }
        };
        let announced = run(vec!["version".into(), "--short".into()])
            .await
            .map_err(fail)?;
        if announced != v {
            return Err(fail(format!("le programme annonce la version {announced}")));
        }
        run(vec![
            "selftest".into(),
            "--data-dir".into(),
            data_copy.display().to_string(),
        ])
        .await
        .map_err(fail)
    }

    fn point_current_to(&self, v: &str) -> Result<()> {
        if !self.binary(v).is_file() {
            return Err(UpdateError::UnknownVersion(v.into()));
        }
        let tmp = self.root.join("current.new");
        let _ = std::fs::remove_file(&tmp);
        #[cfg(unix)]
        std::os::unix::fs::symlink(self.versions_dir().join(v), &tmp)?;
        #[cfg(not(unix))]
        return Err(UpdateError::Io(std::io::Error::other(
            "bascule prise en charge sous Linux seulement",
        )));
        std::fs::rename(&tmp, self.current_link())?;
        Ok(())
    }

    /// Bascule le lien sur `v` et arme le compteur de démarrages.
    pub fn switch_to(&self, v: &str) -> Result<()> {
        let previous = self.current_version();
        self.point_current_to(v)?;
        let mut s = self.state();
        s.pending = Some(v.into());
        s.previous = previous;
        s.attempts = 0;
        s.switched_at = Some(now());
        s.confirmed_at = None;
        self.save_state(&s)
    }

    /// Retour manuel à la version précédente.
    pub fn rollback(&self) -> Result<String> {
        let mut s = self.state();
        let current = self.current_version();
        let target = s
            .previous
            .clone()
            .filter(|p| Some(p) != current.as_ref() && self.binary(p).is_file())
            .ok_or(UpdateError::NoPrevious)?;
        self.point_current_to(&target)?;
        s.rolled_back_from = current;
        s.rolled_back_at = Some(now());
        s.previous = s.rolled_back_from.clone();
        s.pending = None;
        s.attempts = 0;
        self.save_state(&s)?;
        Ok(target)
    }

    /// À appeler tout au début du démarrage du service.
    pub fn on_boot(&self, my_version: &str) -> Result<BootDecision> {
        let mut s = self.state();
        if s.pending.as_deref() != Some(my_version) {
            return Ok(BootDecision::Continue);
        }
        s.attempts += 1;
        if s.attempts > MAX_FAILED_BOOTS {
            if let Some(prev) = s.previous.clone().filter(|p| self.binary(p).is_file()) {
                self.point_current_to(&prev)?;
                s.rolled_back_from = Some(my_version.into());
                s.rolled_back_at = Some(now());
                s.pending = None;
                s.attempts = 0;
                s.previous = Some(my_version.into());
                self.save_state(&s)?;
                return Ok(BootDecision::RolledBack {
                    from: my_version.into(),
                    to: prev,
                });
            }
        }
        self.save_state(&s)?;
        Ok(BootDecision::Continue)
    }

    /// Garde de démarrage, lancée par systemd avant chaque démarrage (`ExecStartPre`)
    /// avec le programme du paquet Debian, que les mises à jour OTA ne remplacent jamais :
    /// elle compte aussi les démarrages d'une version qui plante avant d'avoir pu le faire.
    pub fn guard_boot(&self) -> Result<BootDecision> {
        match self.current_version() {
            Some(v) => self.on_boot(&v),
            None => Ok(BootDecision::Continue),
        }
    }

    /// À appeler une fois le service prêt : la nouvelle version est confirmée.
    pub fn confirm_boot(&self, my_version: &str) -> Result<()> {
        let mut s = self.state();
        if s.pending.as_deref() == Some(my_version) {
            s.pending = None;
            s.attempts = 0;
            s.confirmed_at = Some(now());
            self.save_state(&s)?;
        }
        Ok(())
    }
}
