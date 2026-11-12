//! Points de restauration de projet et fichiers portables (section 13.2).
//!
//! Automatiques (toutes les 1, 6 ou 24 h, seulement si le projet a changé ; 48 gardés) ou
//! manuels (gardés jusqu'à suppression). Restaurer prend d'abord un point de l'état courant
//! et marche aussi pour un projet supprimé. L'export portable joint les fiches du catalogue
//! utilisées ; l'import remplace (avec copie de sécurité) ou crée une copie, tout ou rien.

use crate::AppState;
use crate::auth::Member;
use crate::catalog_io::file_response;
use crate::error::{ApiError, ApiResult};
use crate::live::Origin;
use crate::projects::{one, read, room, write};
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use strata_domain::{Domain, Level};
use strata_store::Store;
use strata_store::points::{FORMAT, pack, remap_copy, unpack};

const INTERVAL: &str = "points_interval_h";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects/{id}/points", get(list).post(create))
        .route("/projects/{id}/export", get(export))
        .route(
            "/projects/import",
            post(import).layer(DefaultBodyLimit::max(200 * 1024 * 1024)),
        )
        .route("/points/orphans", get(orphans))
        .route("/points/{pt}/restore", post(restore))
        .route("/points/{pt}/download", get(download))
        .route("/points/{pt}/delete", post(delete))
        .route("/site/points", get(interval).put(set_interval))
}

pub async fn interval_h(store: &Store) -> i64 {
    store
        .meta(INTERVAL)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
        .filter(|v| [1, 6, 24].contains(v))
        .unwrap_or(1)
}

/// Tâche de fond : un point automatique pour chaque projet modifié depuis l'intervalle.
pub async fn auto_points(store: &Store) {
    let h = interval_h(store).await;
    let Ok(due) = store.projects_due(h).await else {
        return;
    };
    for (pid, name) in due {
        match store.project_snapshot(&pid).await {
            Ok(s) => {
                if let Err(e) = store.insert_point(&s, &name, None, true, None).await {
                    tracing::warn!(error = %e, project = %pid, "point de restauration automatique");
                }
            }
            Err(e) => tracing::warn!(error = %e, project = %pid, "instantané de projet"),
        }
    }
}

async fn list(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    Ok(Json(
        json!({ "points": st.store.points_of(&pid).await?, "interval_h": interval_h(&st.store).await }),
    ))
}

#[derive(Deserialize, Default)]
struct LabelBody {
    #[serde(default)]
    label: Option<String>,
}

async fn create(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
    Json(b): Json<LabelBody>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    let p = st
        .store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    let label = b.label.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if label.is_some_and(|l| l.chars().count() > 120) {
        return Err(one("label", "120 caractères au plus."));
    }
    let snap = st.store.project_snapshot(&pid).await?;
    let pt = st
        .store
        .insert_point(&snap, &p.data.name, label, false, Some(&m.0.account.id))
        .await?;
    st.store
        .audit_project(
            &pid,
            Some(&m.0.account.id),
            "create",
            "point",
            &pt.id,
            None::<&Value>,
            Some(&json!({ "label": label })),
        )
        .await?;
    Ok(Json(json!(pt)))
}

async fn orphans(State(st): State<AppState>, m: Member) -> ApiResult<Json<Value>> {
    read(&m)?;
    Ok(Json(json!({ "points": st.store.orphan_points().await? })))
}

async fn restore(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pt): Path<String>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    let (p, data) = st
        .store
        .point(&pt)
        .await?
        .ok_or(ApiError::NotFound("Point de restauration"))?;
    let snap = unpack(&data)
        .ok_or_else(|| ApiError::Invalid("Point de restauration illisible.".into()))?;
    // L'état courant est d'abord mis de côté.
    if let Some(cur) = st.store.project(&p.project_id).await? {
        let now = st.store.project_snapshot(&p.project_id).await?;
        st.store
            .insert_point(
                &now,
                &cur.data.name,
                Some("Avant restauration"),
                false,
                Some(&m.0.account.id),
            )
            .await?;
    }
    let pid = st.store.restore_snapshot(&snap).await?;
    st.store
        .audit_project(
            &pid,
            Some(&m.0.account.id),
            "restore",
            "project",
            &pid,
            None::<&Value>,
            Some(&json!({ "point": pt, "taken_at": p.created_at })),
        )
        .await?;
    st.hub.change(
        room(&pid),
        "project",
        &pid,
        "restore",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(Json(json!({ "project_id": pid })))
}

fn stem(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("Projet_{safe}_{}", chrono::Local::now().format("%Y-%m-%d"))
}

async fn portable(store: &Store, snap: Value) -> ApiResult<Vec<u8>> {
    let catalog = store.catalog_rows_for(&snap).await?;
    let mut v = snap;
    v["catalog"] = json!(catalog);
    Ok(pack(&v))
}

async fn download(
    State(st): State<AppState>,
    m: Member,
    Path(pt): Path<String>,
) -> ApiResult<Response> {
    read(&m)?;
    let (p, data) = st
        .store
        .point(&pt)
        .await?
        .ok_or(ApiError::NotFound("Point de restauration"))?;
    let snap = unpack(&data)
        .ok_or_else(|| ApiError::Invalid("Point de restauration illisible.".into()))?;
    Ok(file_response(
        portable(&st.store, snap).await?,
        "application/octet-stream",
        &format!("{}.strata", stem(&p.project_name)),
    ))
}

async fn delete(
    State(st): State<AppState>,
    m: Member,
    Path(pt): Path<String>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    let (p, _) = st
        .store
        .point(&pt)
        .await?
        .ok_or(ApiError::NotFound("Point de restauration"))?;
    st.store.delete_point(&pt).await?;
    if st.store.project(&p.project_id).await?.is_some() {
        st.store
            .audit_project(
                &p.project_id,
                Some(&m.0.account.id),
                "delete",
                "point",
                &pt,
                None::<&Value>,
                None::<&Value>,
            )
            .await?;
    }
    Ok(Json(json!({ "ok": true })))
}

async fn export(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
) -> ApiResult<Response> {
    read(&m)?;
    let p = st
        .store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    let snap = st.store.project_snapshot(&pid).await?;
    Ok(file_response(
        portable(&st.store, snap).await?,
        "application/octet-stream",
        &format!("{}.strata", stem(&p.data.name)),
    ))
}

#[derive(Deserialize)]
struct ImportQuery {
    /// preview, replace ou copy.
    mode: String,
}

/// Import d'un fichier portable : aperçu, puis Remplacer (copie de sécurité) ou Copie
/// (nouveaux identifiants, « (copie) »). Le catalogue existant n'est jamais modifié.
async fn import(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Query(q): Query<ImportQuery>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    let snap = unpack(&body).ok_or_else(|| {
        ApiError::Invalid("Fichier illisible : ce n'est pas un projet Strata Arkyo.".into())
    })?;
    if snap["format"] != FORMAT {
        return Err(ApiError::Invalid(
            "Fichier illisible : ce n'est pas un projet Strata Arkyo.".into(),
        ));
    }
    let pid = snap["project_id"].as_str().unwrap_or_default().to_string();
    let name = snap["tables"]["projects"][0]["name"]
        .as_str()
        .unwrap_or("Projet")
        .to_string();
    let exists = st.store.project(&pid).await?;
    let mut warnings: Vec<String> = Vec::new();
    let schema = snap["schema"].as_i64().unwrap_or(0);
    let current: i64 = st.store.schema_version().await.unwrap_or(0);
    if schema > current {
        warnings.push(format!("Fichier d'une version plus récente (schéma {schema}, ici {current}) : les données inconnues seront ignorées."));
    } else if schema < current {
        warnings.push(format!("Fichier d'une version plus ancienne (schéma {schema}) : les nouveautés prendront leurs valeurs par défaut."));
    }
    let counts: Value = snap["tables"]
        .as_object()
        .map(|t| {
            t.iter()
                .filter(|(_, v)| v.as_array().is_some_and(|a| !a.is_empty()))
                .map(|(k, v)| (k.clone(), json!(v.as_array().map(|a| a.len()).unwrap_or(0))))
                .collect::<serde_json::Map<_, _>>()
                .into()
        })
        .unwrap_or(json!({}));
    let catalog: Vec<Value> = snap["catalog"].as_array().cloned().unwrap_or_default();
    match q.mode.as_str() {
        "preview" => Ok(Json(
            json!({ "name": name, "exists": exists.is_some(), "counts": counts, "catalog": catalog.len(), "warnings": warnings, "schema": schema }),
        )),
        "replace" | "copy" => {
            let added = st.store.add_missing_catalog(&catalog).await?;
            let (target, snap) = if q.mode == "copy" {
                let mut nm = format!("{name} (copie)");
                let mut i = 2;
                while st
                    .store
                    .list_projects()
                    .await?
                    .iter()
                    .any(|p| p.data.name == nm)
                {
                    nm = format!("{name} (copie {i})");
                    i += 1;
                }
                let s = remap_copy(&snap, &nm);
                (s["project_id"].as_str().unwrap_or_default().to_string(), s)
            } else {
                if let Some(cur) = &exists {
                    let now = st.store.project_snapshot(&pid).await?;
                    st.store
                        .insert_point(
                            &now,
                            &cur.data.name,
                            Some("Avant import"),
                            false,
                            Some(&m.0.account.id),
                        )
                        .await?;
                }
                (pid.clone(), snap)
            };
            let restored = st.store.restore_snapshot(&snap).await?;
            debug_assert_eq!(restored, target);
            st.store
                .audit_project(
                    &restored,
                    Some(&m.0.account.id),
                    "import",
                    "project",
                    &restored,
                    None::<&Value>,
                    Some(&json!({ "mode": q.mode, "catalog_added": added })),
                )
                .await?;
            st.hub.change(
                room(&restored),
                "project",
                &restored,
                "import",
                None,
                &m.0.account.id,
                origin.0.as_deref(),
            );
            Ok(Json(
                json!({ "project_id": restored, "counts": counts, "catalog_added": added, "warnings": warnings }),
            ))
        }
        _ => Err(one("mode", "Mode : preview, replace ou copy.")),
    }
}

async fn interval(State(st): State<AppState>, m: Member) -> ApiResult<Json<Value>> {
    read(&m)?;
    Ok(Json(json!({ "interval_h": interval_h(&st.store).await })))
}

#[derive(Deserialize)]
struct IntervalBody {
    interval_h: i64,
}

async fn set_interval(
    State(st): State<AppState>,
    m: Member,
    Json(b): Json<IntervalBody>,
) -> ApiResult<Json<Value>> {
    m.0.require(
        Domain::Parametres,
        Level::Ecriture,
        "Régler les points de restauration demande « Paramètres : écriture ».",
    )?;
    if ![1, 6, 24].contains(&b.interval_h) {
        return Err(one("interval_h", "Toutes les 1, 6 ou 24 heures."));
    }
    st.store
        .set_meta(INTERVAL, Some(&b.interval_h.to_string()))
        .await?;
    Ok(Json(json!({ "interval_h": b.interval_h })))
}
