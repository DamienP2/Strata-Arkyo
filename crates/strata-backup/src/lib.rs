//! Sauvegardes du site et restauration (cahier des charges, section 13.2).
//!
//! Une sauvegarde est une archive `strata-backup-<date>.tar.zst` contenant :
//! `manifest.json`, `strata.db` (copie cohérente faite à chaud) et le dossier `files/`.
//! Une restauration est **préparée** (archive vérifiée puis extraite dans
//! `restore-pending/`), puis **appliquée au démarrage suivant** : les données
//! actuelles sont mises de côté dans `set-aside/<date>/`, jamais effacées.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use strata_store::Store;

pub const KEEP_AUTO: usize = 7;
const PREFIX: &str = "strata-backup-";
const SUFFIX: &str = ".tar.zst";

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("sauvegarde introuvable : {0}")]
    NotFound(String),
    #[error("archive invalide : {0}")]
    Invalid(String),
    #[error("la base copiée ne passe pas le contrôle d'intégrité")]
    Integrity,
    #[error("aucune restauration en attente")]
    NothingPending,
    #[error(
        "la restauration a échoué et les données d'origine n'ont pas pu être remises en place. \
         Elles sont dans {0} : remettez strata.db et files/ dans le dossier de données, puis relancez le service"
    )]
    Fatal(PathBuf),
    #[error(transparent)]
    Store(#[from] strata_store::StoreError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, BackupError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Sauvegarde quotidienne : seules les 7 dernières sont gardées.
    Auto,
    /// Sauvegarde manuelle : gardée jusqu'à suppression.
    Manual,
    /// Prise juste avant une mise à jour : gardée jusqu'à suppression.
    PreUpdate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub format: u32,
    pub kind: Kind,
    pub label: Option<String>,
    pub created_at: String,
    pub app_version: String,
    pub schema: i64,
    pub db_sha256: String,
    pub db_size: u64,
    pub files: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BackupInfo {
    pub name: String,
    pub size: u64,
    pub manifest: Manifest,
}

pub struct Backups {
    data_dir: PathBuf,
    dir: PathBuf,
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

/// Chemin d'entrée d'archive sûr : relatif, sans « .. ».
fn safe_entry(path: &Path) -> bool {
    path.components().all(|c| matches!(c, Component::Normal(_)))
}

fn count_files(dir: &Path) -> u64 {
    let Ok(rd) = fs::read_dir(dir) else { return 0 };
    rd.flatten()
        .map(|e| {
            if e.path().is_dir() {
                count_files(&e.path())
            } else {
                1
            }
        })
        .sum()
}

impl Backups {
    /// `dir` : dossier des sauvegardes, de préférence un autre disque ou un partage réseau.
    pub fn new(data_dir: impl Into<PathBuf>, dir: impl Into<PathBuf>) -> Backups {
        Backups {
            data_dir: data_dir.into(),
            dir: dir.into(),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Crée une sauvegarde complète, la vérifie, puis élague les sauvegardes automatiques.
    pub async fn create(
        &self,
        store: &Store,
        kind: Kind,
        label: Option<String>,
        app_version: &str,
    ) -> Result<BackupInfo> {
        let stamp = Utc::now();
        let work = self
            .data_dir
            .join("tmp")
            .join(format!("backup-{}", stamp.format("%Y%m%d%H%M%S%f")));
        fs::create_dir_all(&work)?;
        let result = self
            .create_in(&work, store, kind, label, app_version, stamp)
            .await;
        let _ = fs::remove_dir_all(&work);
        let info = result?;
        if kind == Kind::Auto {
            self.prune_auto(KEEP_AUTO)?;
        }
        Ok(info)
    }

    async fn create_in(
        &self,
        work: &Path,
        store: &Store,
        kind: Kind,
        label: Option<String>,
        app_version: &str,
        stamp: chrono::DateTime<Utc>,
    ) -> Result<BackupInfo> {
        let db = work.join("strata.db");
        store.vacuum_into(&db).await?;
        let (ok, schema) = strata_store::check_database_file(&db).await?;
        if !ok {
            return Err(BackupError::Integrity);
        }
        let files_dir = self.data_dir.join("files");
        let manifest = Manifest {
            format: 1,
            kind,
            label: label.map(|l| l.chars().take(120).collect()),
            created_at: stamp.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            app_version: app_version.into(),
            schema,
            db_sha256: sha256_file(&db)?,
            db_size: fs::metadata(&db)?.len(),
            files: count_files(&files_dir),
        };

        fs::create_dir_all(&self.dir)?;
        let name = format!("{PREFIX}{}{SUFFIX}", stamp.format("%Y%m%d-%H%M%S"));
        let dest = self.dir.join(&name);
        let partial = self.dir.join(format!(".{name}.partial"));
        {
            let enc = zstd::Encoder::new(fs::File::create(&partial)?, 9)?;
            let mut tar = tar::Builder::new(enc);
            let text = serde_json::to_vec_pretty(&manifest)?;
            let mut h = tar::Header::new_gnu();
            h.set_size(text.len() as u64);
            h.set_mode(0o640);
            h.set_mtime(stamp.timestamp() as u64);
            h.set_cksum();
            tar.append_data(&mut h, "manifest.json", text.as_slice())?;
            tar.append_path_with_name(&db, "strata.db")?;
            if files_dir.is_dir() {
                tar.append_dir_all("files", &files_dir)?;
            }
            tar.into_inner()?.finish()?;
        }
        fs::rename(&partial, &dest)?;
        fs::write(
            self.dir.join(format!("{name}.json")),
            serde_json::to_vec_pretty(&manifest)?,
        )?;
        // Une sauvegarde n'existe que si elle se relit.
        if let Err(e) = verify(&dest) {
            let _ = fs::remove_file(&dest);
            let _ = fs::remove_file(self.dir.join(format!("{name}.json")));
            return Err(e);
        }
        Ok(BackupInfo {
            size: fs::metadata(&dest)?.len(),
            name,
            manifest,
        })
    }

    /// Sauvegardes présentes, de la plus récente à la plus ancienne.
    pub fn list(&self) -> Vec<BackupInfo> {
        let mut out: Vec<BackupInfo> = fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if !(name.starts_with(PREFIX) && name.ends_with(SUFFIX)) {
                    return None;
                }
                let manifest = fs::read(self.dir.join(format!("{name}.json")))
                    .ok()
                    .and_then(|b| serde_json::from_slice(&b).ok())
                    .or_else(|| verify(&e.path()).ok())?;
                Some(BackupInfo {
                    size: e.metadata().map(|m| m.len()).unwrap_or(0),
                    name,
                    manifest,
                })
            })
            .collect();
        out.sort_by(|a, b| b.name.cmp(&a.name));
        out
    }

    pub fn path_of(&self, name: &str) -> Result<PathBuf> {
        let valid = name.starts_with(PREFIX)
            && name.ends_with(SUFFIX)
            && !name.contains(['/', '\\'])
            && !name.contains("..");
        let path = self.dir.join(name);
        if valid && path.is_file() {
            Ok(path)
        } else {
            Err(BackupError::NotFound(name.into()))
        }
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        let path = self.path_of(name)?;
        fs::remove_file(path)?;
        let _ = fs::remove_file(self.dir.join(format!("{name}.json")));
        Ok(())
    }

    fn prune_auto(&self, keep: usize) -> Result<()> {
        for old in self
            .list()
            .into_iter()
            .filter(|b| b.manifest.kind == Kind::Auto)
            .skip(keep)
        {
            self.delete(&old.name)?;
        }
        Ok(())
    }

    /// Prépare la restauration d'une sauvegarde : elle sera appliquée au prochain démarrage.
    pub fn stage_restore(&self, name: &str) -> Result<Manifest> {
        let archive = self.path_of(name)?;
        let manifest = verify(&archive)?;
        let pending = pending_dir(&self.data_dir);
        if pending.exists() {
            fs::remove_dir_all(&pending)?;
        }
        let staging = self.data_dir.join("restore-pending.partial");
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        fs::create_dir_all(&staging)?;
        let mut ar = tar::Archive::new(zstd::Decoder::new(fs::File::open(&archive)?)?);
        for entry in ar.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.to_path_buf();
            if !safe_entry(&path) {
                return Err(BackupError::Invalid(format!(
                    "chemin refusé : {}",
                    path.display()
                )));
            }
            entry.unpack_in(&staging)?;
        }
        if sha256_file(&staging.join("strata.db"))? != manifest.db_sha256 {
            return Err(BackupError::Invalid(
                "empreinte de la base différente".into(),
            ));
        }
        fs::rename(&staging, &pending)?;
        Ok(manifest)
    }
}

fn pending_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("restore-pending")
}

/// Relit une archive : manifeste lisible, base présente, empreinte et en-tête SQLite conformes.
pub fn verify(archive: &Path) -> Result<Manifest> {
    let mut ar = tar::Archive::new(zstd::Decoder::new(fs::File::open(archive)?)?);
    let mut manifest: Option<Manifest> = None;
    let mut db: Option<(String, Vec<u8>)> = None;
    for entry in ar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        if !safe_entry(&path) {
            return Err(BackupError::Invalid(format!(
                "chemin refusé : {}",
                path.display()
            )));
        }
        match path.to_string_lossy().as_ref() {
            "manifest.json" => {
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf)?;
                manifest = Some(serde_json::from_slice(&buf)?);
            }
            "strata.db" => {
                let mut h = Sha256::new();
                let mut head = Vec::new();
                let mut buf = vec![0u8; 1 << 16];
                loop {
                    let n = entry.read(&mut buf)?;
                    if n == 0 {
                        break;
                    }
                    if head.len() < 16 {
                        head.extend_from_slice(&buf[..n.min(16)]);
                    }
                    h.update(&buf[..n]);
                }
                db = Some((hex::encode(h.finalize()), head));
            }
            _ => {}
        }
    }
    let manifest = manifest.ok_or_else(|| BackupError::Invalid("manifeste absent".into()))?;
    let (sha, head) = db.ok_or_else(|| BackupError::Invalid("base absente".into()))?;
    if sha != manifest.db_sha256 {
        return Err(BackupError::Invalid(
            "empreinte de la base différente".into(),
        ));
    }
    if !head.starts_with(b"SQLite format 3\0") {
        return Err(BackupError::Invalid(
            "la base n'est pas un fichier SQLite".into(),
        ));
    }
    Ok(manifest)
}

/// Restauration en attente, s'il y en a une.
pub fn pending_restore(data_dir: &Path) -> Option<Manifest> {
    let text = fs::read(pending_dir(data_dir).join("manifest.json")).ok()?;
    serde_json::from_slice(&text).ok()
}

/// Annule une restauration en attente.
pub fn cancel_restore(data_dir: &Path) -> Result<()> {
    let dir = pending_dir(data_dir);
    if !dir.exists() {
        return Err(BackupError::NothingPending);
    }
    fs::remove_dir_all(dir)?;
    Ok(())
}

const LIVE: [&str; 4] = ["strata.db", "strata.db-wal", "strata.db-shm", "files"];

const FAILED_MARKER: &str = "restore-failed.json";

/// Si une restauration a échoué sans pouvoir remettre les données d'origine, le dossier
/// où elles attendent. Le moteur refuse alors de démarrer sur une base vide.
pub fn failed_restore(data_dir: &Path) -> Option<PathBuf> {
    let text = fs::read(data_dir.join(FAILED_MARKER)).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&text).ok()?;
    v["set_aside"].as_str().map(PathBuf::from)
}

/// Applique la restauration en attente, avant l'ouverture de la base.
/// Renvoie le dossier où les données d'origine ont été mises de côté.
pub fn apply_pending_restore(data_dir: &Path) -> Result<Option<PathBuf>> {
    let pending = pending_dir(data_dir);
    if !pending.join("strata.db").is_file() {
        return Ok(None);
    }
    let result = apply_pending_inner(data_dir, &pending);
    match &result {
        Err(BackupError::Fatal(aside)) => {
            let marker = serde_json::json!({ "set_aside": aside, "at": Utc::now().to_rfc3339() });
            let _ = fs::write(data_dir.join(FAILED_MARKER), marker.to_string());
        }
        Err(_) => {
            // Données d'origine remises : on écarte la restauration ratée pour ne pas la rejouer à chaque démarrage.
            let _ = fs::rename(
                &pending,
                data_dir.join(format!(
                    "restore-failed-{}",
                    Utc::now().format("%Y%m%d-%H%M%S")
                )),
            );
        }
        Ok(_) => {}
    }
    result
}

fn apply_pending_inner(data_dir: &Path, pending: &Path) -> Result<Option<PathBuf>> {
    let aside = data_dir
        .join("set-aside")
        .join(Utc::now().format("%Y%m%d-%H%M%S").to_string());
    fs::create_dir_all(&aside)?;

    // 1. Mettre de côté les données actuelles.
    let mut moved = Vec::new();
    for name in LIVE {
        let src = data_dir.join(name);
        if src.exists() {
            if let Err(e) = fs::rename(&src, aside.join(name)) {
                return Err(undo(data_dir, &aside, &moved, e));
            }
            moved.push(name);
        }
    }
    // 2. Mettre en place les données restaurées.
    let mut placed = Vec::new();
    for name in ["strata.db", "files"] {
        let src = pending.join(name);
        if src.exists() {
            if let Err(e) = fs::rename(&src, data_dir.join(name)) {
                for p in &placed {
                    let _ = fs::rename(data_dir.join(p), pending.join(p));
                }
                return Err(undo(data_dir, &aside, &moved, e));
            }
            placed.push(name);
        }
    }
    let _ = fs::copy(
        pending.join("manifest.json"),
        aside.join("restored-from.json"),
    );
    let _ = fs::remove_dir_all(pending);
    Ok(Some(aside))
}

/// Remet les données d'origine ; si c'est impossible, erreur fatale qui nomme le dossier.
fn undo(data_dir: &Path, aside: &Path, moved: &[&str], cause: std::io::Error) -> BackupError {
    for name in moved {
        if fs::rename(aside.join(name), data_dir.join(name)).is_err() {
            return BackupError::Fatal(aside.to_path_buf());
        }
    }
    BackupError::Io(cause)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (tempfile::TempDir, Store, Backups) {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        fs::create_dir_all(data.join("files/ab")).unwrap();
        fs::write(data.join("files/ab/plan.pdf"), b"%PDF-1.7 plan").unwrap();
        let store = Store::open(&data.join("strata.db")).await.unwrap();
        let b = Backups::new(&data, tmp.path().join("backups"));
        (tmp, store, b)
    }

    async fn add_account(store: &Store, email: &str) {
        store
            .insert_account(strata_store::NewAccount {
                email,
                username: None,
                name: "x",
                password_hash: "h",
                must_change_password: false,
                role_id: "technician",
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn backup_is_verified_and_listed() {
        let (_t, store, b) = setup().await;
        let info = b
            .create(&store, Kind::Manual, Some("avant tournée".into()), "0.1.0")
            .await
            .unwrap();
        assert_eq!(info.manifest.schema, 5);
        assert_eq!(info.manifest.files, 1);
        assert_eq!(
            verify(&b.path_of(&info.name).unwrap()).unwrap(),
            info.manifest
        );
        assert_eq!(b.list().len(), 1);
        assert!(b.path_of("../../etc/passwd").is_err());
    }

    #[tokio::test]
    async fn only_seven_automatic_backups_are_kept() {
        let (_t, store, b) = setup().await;
        let manual = b.create(&store, Kind::Manual, None, "0.1.0").await.unwrap();
        for _ in 0..9 {
            // Les noms sont horodatés à la seconde.
            tokio::time::sleep(std::time::Duration::from_millis(1010)).await;
            b.create(&store, Kind::Auto, None, "0.1.0").await.unwrap();
        }
        let list = b.list();
        assert_eq!(
            list.iter()
                .filter(|x| x.manifest.kind == Kind::Auto)
                .count(),
            KEEP_AUTO
        );
        assert!(
            list.iter().any(|x| x.name == manual.name),
            "la manuelle reste"
        );
    }

    #[tokio::test]
    async fn restore_is_staged_cancelable_then_applied_with_data_set_aside() {
        let (tmp, store, b) = setup().await;
        let data = tmp.path().join("data");
        add_account(&store, "avant@studio.fr").await;
        let info = b.create(&store, Kind::Manual, None, "0.1.0").await.unwrap();
        add_account(&store, "apres@studio.fr").await;
        fs::write(data.join("files/ab/nouveau.pdf"), b"nouveau").unwrap();

        b.stage_restore(&info.name).unwrap();
        assert!(pending_restore(&data).is_some());
        cancel_restore(&data).unwrap();
        assert!(pending_restore(&data).is_none());

        b.stage_restore(&info.name).unwrap();
        store.close().await;
        let aside = apply_pending_restore(&data).unwrap().unwrap();
        // Les données d'avant sont gardées de côté.
        assert!(aside.join("strata.db").is_file());
        assert!(aside.join("files/ab/nouveau.pdf").is_file());
        // La base restaurée ne connaît pas le compte créé après la sauvegarde.
        let store = Store::open(&data.join("strata.db")).await.unwrap();
        assert!(
            store
                .account_by_email("avant@studio.fr")
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            store
                .account_by_email("apres@studio.fr")
                .await
                .unwrap()
                .is_none()
        );
        assert!(!data.join("files/ab/nouveau.pdf").exists());
        assert!(data.join("files/ab/plan.pdf").exists());
        assert!(apply_pending_restore(&data).unwrap().is_none());
    }

    #[tokio::test]
    async fn a_corrupted_archive_is_refused() {
        let (_t, store, b) = setup().await;
        let info = b.create(&store, Kind::Manual, None, "0.1.0").await.unwrap();
        let path = b.path_of(&info.name).unwrap();
        let mut bytes = fs::read(&path).unwrap();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0xFF;
        fs::write(&path, bytes).unwrap();
        assert!(verify(&path).is_err());
        assert!(b.stage_restore(&info.name).is_err());
    }
}
