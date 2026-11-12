//! Administration du site (cahier des charges, section 13) : système, sauvegardes,
//! restauration, mises à jour. Toutes les routes exigent « Paramètres et site : admin ».

use crate::auth::Member;
use crate::error::{ApiError, ApiResult};
use crate::ops::{
    META_AUTO_CHECK, META_BACKUP_DIR, META_BACKUP_ERROR, META_BACKUP_LAST, META_CHANNEL,
    META_LAST_CHECK, NIGHT_INSTALL_TIME, Ops, OpsError, next_local,
};
use crate::{AppState, VERSION};
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::Response;
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use strata_backup::Kind;
use strata_domain::{Domain, Level};

const SITE_ADMIN: &str = "Vous n'avez pas le droit d'administrer le site.";
/// Taille maximale d'un paquet de mise à jour déposé.
const MAX_UPLOAD: usize = 600 * 1024 * 1024;

impl From<OpsError> for ApiError {
    fn from(e: OpsError) -> Self {
        match e {
            OpsError::Conflict(m) => ApiError::Conflict(m),
            OpsError::Invalid(m) => ApiError::Invalid(m),
            OpsError::Unavailable(m) => ApiError::Unavailable(m),
            OpsError::Internal(m) => ApiError::Internal(m),
        }
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/system", get(system))
        .route("/settings", patch(settings))
        .route("/update", get(update_status))
        .route("/update/check", post(update_check))
        .route("/update/install", post(update_install))
        .route(
            "/update/upload",
            post(update_upload).layer(DefaultBodyLimit::max(MAX_UPLOAD)),
        )
        .route("/update/schedule", delete(update_cancel))
        .route("/update/rollback", post(update_rollback))
        .route("/backups", get(backups_list).post(backups_create))
        .route("/backups/{name}", delete(backups_delete))
        .route("/backups/{name}/download", get(backups_download))
        .route("/backups/{name}/restore", post(backups_restore))
        .route("/restore", delete(restore_cancel))
}

/// Personne autorisée et moteur d'exploitation disponible.
fn admin(st: &AppState, m: &Member) -> ApiResult<Arc<Ops>> {
    m.0.require(Domain::Parametres, Level::Admin, SITE_ADMIN)?;
    st.ops.clone().ok_or_else(|| {
        ApiError::Unavailable("L'administration du site n'est pas disponible ici.".into())
    })
}

async fn system(State(st): State<AppState>, m: Member) -> ApiResult<Json<Value>> {
    let ops = admin(&st, &m)?;
    Ok(Json(json!({
        "version": VERSION,
        "schema": st.store.schema_version().await?,
        "arch": strata_update::host_arch(),
        "data_dir": ops.data_dir,
        "install_dir": ops.layout.root(),
        "managed": ops.layout.is_managed(),
        "release_keys": ops.keys.ids(),
        "update_url": ops.update_url,
    })))
}

#[derive(Deserialize)]
struct SettingsBody {
    backup_dir: Option<String>,
    update_channel: Option<String>,
    update_auto_check: Option<bool>,
}

async fn settings(
    State(st): State<AppState>,
    m: Member,
    Json(b): Json<SettingsBody>,
) -> ApiResult<Json<Value>> {
    let ops = admin(&st, &m)?;
    if let Some(dir) = b.backup_dir {
        let dir = dir.trim();
        if dir.is_empty() {
            st.store.set_meta(META_BACKUP_DIR, None).await?;
        } else {
            let path = std::path::Path::new(dir);
            if !path.is_absolute() {
                return Err(ApiError::Invalid(
                    "Indiquez un chemin absolu, par exemple /mnt/nas/strata-arkyo.".into(),
                ));
            }
            // Le dossier doit exister ou pouvoir être créé, et accepter l'écriture.
            let probe = path.join(".strata-probe");
            std::fs::create_dir_all(path)
                .and_then(|_| std::fs::write(&probe, b"ok"))
                .and_then(|_| std::fs::remove_file(&probe))
                .map_err(|e| {
                    ApiError::Invalid(format!(
                        "Le service ne peut pas écrire dans {dir} ({e}). Vérifiez le montage et les droits de l'utilisateur strata."
                    ))
                })?;
            st.store.set_meta(META_BACKUP_DIR, Some(dir)).await?;
        }
    }
    if let Some(c) = b.update_channel {
        if !matches!(c.as_str(), "stable" | "beta") {
            return Err(ApiError::Invalid(
                "Canal inconnu : choisissez stable ou beta.".into(),
            ));
        }
        st.store.set_meta(META_CHANNEL, Some(&c)).await?;
    }
    if let Some(a) = b.update_auto_check {
        st.store
            .set_meta(META_AUTO_CHECK, Some(if a { "true" } else { "false" }))
            .await?;
    }
    st.store
        .audit::<(), ()>(
            Some(&m.0.account.id),
            "update",
            "site",
            "settings",
            None,
            None,
        )
        .await?;
    Ok(Json(json!({
        "backup_dir": ops.backup_dir(&st.store).await?,
        "update_channel": ops.channel(&st.store).await?,
        "update_auto_check": ops.auto_check(&st.store).await?,
    })))
}

// ----- Mises à jour -----

async fn update_status(State(st): State<AppState>, m: Member) -> ApiResult<Json<Value>> {
    let ops = admin(&st, &m)?;
    Ok(Json(json!({
        "current": VERSION,
        "managed": ops.layout.is_managed(),
        "signed_updates": !ops.keys.is_empty(),
        "source": ops.update_url,
        "channel": ops.channel(&st.store).await?,
        "auto_check": ops.auto_check(&st.store).await?,
        "last_check": st.store.meta(META_LAST_CHECK).await?,
        "status": ops.status().await,
        "installed": ops.layout.installed(),
        "history": ops.layout.state(),
    })))
}

async fn update_check(State(st): State<AppState>, m: Member) -> ApiResult<Json<Value>> {
    let ops = admin(&st, &m)?;
    let available = ops.check(&st.store).await?;
    Ok(Json(json!({ "available": available })))
}

#[derive(Deserialize, Default)]
struct InstallBody {
    /// `now` (par défaut) ou `night`.
    when: Option<String>,
}

fn install_time(when: Option<&str>) -> ApiResult<Option<chrono::DateTime<chrono::Utc>>> {
    match when.unwrap_or("now") {
        "now" => Ok(None),
        "night" => Ok(Some(
            next_local(chrono::Local::now(), NIGHT_INSTALL_TIME).with_timezone(&chrono::Utc),
        )),
        _ => Err(ApiError::Invalid("Choisissez « now » ou « night ».".into())),
    }
}

/// Télécharge, vérifie et essaie la version annoncée, puis planifie la bascule.
async fn update_install(
    State(st): State<AppState>,
    m: Member,
    body: Option<Json<InstallBody>>,
) -> ApiResult<Json<Value>> {
    let ops = admin(&st, &m)?;
    let b = body.map(|Json(b)| b).unwrap_or_default();
    let at = install_time(b.when.as_deref())?;
    let v = ops.prepare_online(&st.store).await?;
    let s = ops.schedule(&st.store, v, at).await?;
    st.store
        .audit::<(), _>(
            Some(&m.0.account.id),
            "update_schedule",
            "site",
            "engine",
            None,
            Some(&s),
        )
        .await?;
    Ok(Json(json!({ "scheduled": s })))
}

#[derive(Deserialize)]
struct UploadQuery {
    when: Option<String>,
}

/// Mise à jour hors ligne : le corps de la requête est le paquet `.saupd`.
async fn update_upload(
    State(st): State<AppState>,
    m: Member,
    Query(q): Query<UploadQuery>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    let ops = admin(&st, &m)?;
    let at = install_time(q.when.as_deref())?;
    if body.is_empty() {
        return Err(ApiError::Invalid("Le paquet déposé est vide.".into()));
    }
    std::fs::create_dir_all(ops.updates_dir()).map_err(|e| ApiError::Internal(e.to_string()))?;
    let path = ops.updates_dir().join(format!(
        "upload-{}.saupd",
        chrono::Utc::now().format("%Y%m%d%H%M%S")
    ));
    std::fs::write(&path, &body).map_err(|e| ApiError::Internal(e.to_string()))?;
    let prepared = ops.prepare_file(&st.store, &path).await;
    let _ = std::fs::remove_file(&path);
    let s = ops.schedule(&st.store, prepared?, at).await?;
    st.store
        .audit::<(), _>(
            Some(&m.0.account.id),
            "update_schedule",
            "site",
            "engine",
            None,
            Some(&s),
        )
        .await?;
    Ok(Json(json!({ "scheduled": s })))
}

async fn update_cancel(State(st): State<AppState>, m: Member) -> ApiResult<StatusCode> {
    admin(&st, &m)?.cancel_schedule().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn update_rollback(State(st): State<AppState>, m: Member) -> ApiResult<Json<Value>> {
    let ops = admin(&st, &m)?;
    let target = ops.rollback(&st.store).await?;
    Ok(Json(json!({ "restarting_on": target })))
}

// ----- Sauvegardes -----

async fn backups_list(State(st): State<AppState>, m: Member) -> ApiResult<Json<Value>> {
    let ops = admin(&st, &m)?;
    let backups = ops.backups(&st.store).await?;
    Ok(Json(json!({
        "dir": backups.dir(),
        "backups": backups.list(),
        "last_success": st.store.meta(META_BACKUP_LAST).await?,
        "last_error": st.store.meta(META_BACKUP_ERROR).await?,
        "next_auto": next_local(chrono::Local::now(), crate::ops::BACKUP_TIME).to_rfc3339(),
        "pending_restore": strata_backup::pending_restore(&ops.data_dir),
    })))
}

#[derive(Deserialize, Default)]
struct BackupBody {
    label: Option<String>,
}

async fn backups_create(
    State(st): State<AppState>,
    m: Member,
    body: Option<Json<BackupBody>>,
) -> ApiResult<(StatusCode, Json<strata_backup::BackupInfo>)> {
    let ops = admin(&st, &m)?;
    let label = body
        .and_then(|Json(b)| b.label)
        .filter(|l| !l.trim().is_empty());
    let info = ops.backup(&st.store, Kind::Manual, label).await?;
    st.store
        .audit::<(), _>(
            Some(&m.0.account.id),
            "backup",
            "site",
            &info.name,
            None,
            Some(&info.manifest),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(info)))
}

async fn backups_delete(
    State(st): State<AppState>,
    m: Member,
    Path(name): Path<String>,
) -> ApiResult<StatusCode> {
    let ops = admin(&st, &m)?;
    ops.backups(&st.store)
        .await?
        .delete(&name)
        .map_err(|e| ApiError::Missing(e.to_string()))?;
    st.store
        .audit::<(), ()>(Some(&m.0.account.id), "delete", "backup", &name, None, None)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn backups_download(
    State(st): State<AppState>,
    m: Member,
    Path(name): Path<String>,
) -> ApiResult<Response> {
    let ops = admin(&st, &m)?;
    let path = ops
        .backups(&st.store)
        .await?
        .path_of(&name)
        .map_err(|e| ApiError::Missing(e.to_string()))?;
    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let len = file.metadata().await.map(|m| m.len()).unwrap_or(0);
    Response::builder()
        .header(header::CONTENT_TYPE, "application/zstd")
        .header(header::CONTENT_LENGTH, len)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{name}\""),
        )
        .body(Body::from_stream(tokio_util::io::ReaderStream::new(file)))
        .map_err(|e| ApiError::Internal(e.to_string()))
}

#[derive(Deserialize, Default)]
struct RestoreBody {
    /// Redémarrer tout de suite pour appliquer la restauration.
    #[serde(default)]
    restart: bool,
}

/// Prépare la restauration ; elle s'applique au prochain démarrage, annulable d'ici là.
async fn backups_restore(
    State(st): State<AppState>,
    m: Member,
    Path(name): Path<String>,
    body: Option<Json<RestoreBody>>,
) -> ApiResult<Json<Value>> {
    let ops = admin(&st, &m)?;
    let manifest = ops.stage_restore(&st.store, &name).await?;
    st.store
        .audit::<(), _>(
            Some(&m.0.account.id),
            "restore_stage",
            "site",
            &name,
            None,
            Some(&manifest),
        )
        .await?;
    let restart = body.map(|Json(b)| b.restart).unwrap_or(false);
    if restart {
        let ops = ops.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            ops.request_restart();
        });
    }
    Ok(Json(
        json!({ "pending_restore": manifest, "restarting": restart }),
    ))
}

async fn restore_cancel(State(st): State<AppState>, m: Member) -> ApiResult<StatusCode> {
    let ops = admin(&st, &m)?;
    strata_backup::cancel_restore(&ops.data_dir).map_err(|e| ApiError::Invalid(e.to_string()))?;
    st.store
        .audit::<(), ()>(
            Some(&m.0.account.id),
            "restore_cancel",
            "site",
            "restore",
            None,
            None,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
