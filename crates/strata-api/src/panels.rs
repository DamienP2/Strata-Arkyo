//! Panels de brassage (sections 6.3 et 9.5).
//!
//! La bibliothèque du site garde des panels ; en ajouter un à un contenant de projet le
//! recopie dans l'article (accessoire « panel »), qui se modifie ensuite sans toucher la
//! bibliothèque. Gérer la bibliothèque demande « Catalogue : écriture ».

use crate::AppState;
use crate::auth::Member;
use crate::error::{ApiError, ApiResult};
use crate::live::Origin;
use crate::projects::{fields, one, record, write};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use strata_domain::catalog::RefKind;
use strata_domain::panel::{ConnectorInfo, PanelData, validate_panel};
use strata_domain::project::{Accessory, unique_name};
use strata_domain::{Domain, Level};
use strata_store::panels::Panel;
use strata_store::{Store, StoreError};

const LIB_WRITE: &str = "Gérer la bibliothèque de panels demande « Catalogue : écriture ».";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/panels", get(list).post(create))
        .route("/panels/{id}", get(show).put(update).delete(remove))
        .route("/panels/{id}/duplicate", post(duplicate))
        .route("/articles/{id}/panel", put(update_article_panel))
}

/// Valide un panel contre le référentiel des connecteurs et le normalise.
pub(crate) async fn check_panel(store: &Store, p: &mut PanelData) -> ApiResult<()> {
    let refs = store.list_refs(Some(RefKind::Connector)).await?;
    let lookup = |id: &str| {
        refs.iter().find(|r| r.id == id).map(|r| ConnectorInfo {
            variants: r
                .data
                .get("variants")
                .and_then(Value::as_array)
                .map(|v| {
                    v.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            d_format: r
                .data
                .get("d_format")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    };
    fields(validate_panel(p, &lookup))
}

fn view(p: &Panel) -> Value {
    json!({
        "id": p.id, "panel": p.panel, "rev": p.rev, "stats": p.panel.stats(),
        "created_at": p.created_at, "updated_at": p.updated_at,
    })
}

fn name_taken(e: StoreError) -> ApiError {
    match e {
        StoreError::Conflict(_) => one("name", "Un panel de la bibliothèque porte déjà ce nom."),
        e => e.into(),
    }
}

async fn list(State(st): State<AppState>, _m: Member) -> ApiResult<Json<Value>> {
    let panels: Vec<Value> = st.store.panels().await?.iter().map(view).collect();
    Ok(Json(json!({ "panels": panels })))
}

async fn show(
    State(st): State<AppState>,
    _m: Member,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let p = st
        .store
        .panel(&id)
        .await?
        .ok_or(ApiError::NotFound("Panel"))?;
    Ok(Json(view(&p)))
}

#[derive(Deserialize)]
struct NewPanel {
    panel: PanelData,
}

async fn create(
    State(st): State<AppState>,
    m: Member,
    Json(mut b): Json<NewPanel>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    m.0.require(Domain::Catalogue, Level::Ecriture, LIB_WRITE)?;
    check_panel(&st.store, &mut b.panel).await?;
    let p = st
        .store
        .insert_panel(&b.panel, Some(&m.0.account.id))
        .await
        .map_err(name_taken)?;
    st.store
        .audit(
            Some(&m.0.account.id),
            "create",
            "panel",
            &p.id,
            None::<&()>,
            Some(&json!({ "name": p.panel.name })),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(view(&p))))
}

#[derive(Deserialize)]
struct PanelPut {
    rev: i64,
    panel: PanelData,
}

async fn update(
    State(st): State<AppState>,
    m: Member,
    Path(id): Path<String>,
    Json(mut b): Json<PanelPut>,
) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Catalogue, Level::Ecriture, LIB_WRITE)?;
    let before = st
        .store
        .panel(&id)
        .await?
        .ok_or(ApiError::NotFound("Panel"))?;
    check_panel(&st.store, &mut b.panel).await?;
    let p = st
        .store
        .update_panel(&id, b.rev, &b.panel)
        .await
        .map_err(name_taken)?;
    st.store
        .audit(
            Some(&m.0.account.id),
            "update",
            "panel",
            &id,
            Some(&json!({ "name": before.panel.name, "slots": before.panel.slots.len() })),
            Some(&json!({ "name": p.panel.name, "slots": p.panel.slots.len() })),
        )
        .await?;
    Ok(Json(view(&p)))
}

async fn duplicate(
    State(st): State<AppState>,
    m: Member,
    Path(id): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    m.0.require(Domain::Catalogue, Level::Ecriture, LIB_WRITE)?;
    let src = st
        .store
        .panel(&id)
        .await?
        .ok_or(ApiError::NotFound("Panel"))?;
    let taken: Vec<String> = st
        .store
        .panels()
        .await?
        .into_iter()
        .map(|p| p.panel.name)
        .collect();
    let mut panel = src.panel.clone();
    panel.name = unique_name(&format!("{} (copie)", src.panel.name), &taken);
    let p = st
        .store
        .insert_panel(&panel, Some(&m.0.account.id))
        .await
        .map_err(name_taken)?;
    st.store
        .audit(
            Some(&m.0.account.id),
            "create",
            "panel",
            &p.id,
            None::<&()>,
            Some(&json!({ "name": p.panel.name, "from": src.id })),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(view(&p))))
}

async fn remove(
    State(st): State<AppState>,
    m: Member,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    m.0.require(Domain::Catalogue, Level::Ecriture, LIB_WRITE)?;
    let p = st
        .store
        .panel(&id)
        .await?
        .ok_or(ApiError::NotFound("Panel"))?;
    st.store.delete_panel(&id).await?;
    st.store
        .audit(
            Some(&m.0.account.id),
            "delete",
            "panel",
            &id,
            Some(&json!({ "name": p.panel.name })),
            None::<&()>,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Modifie le panel posé dans un projet : la bibliothèque n'est pas touchée.
async fn update_article_panel(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
    Json(mut b): Json<PanelPut>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    let before = st
        .store
        .article(&id)
        .await?
        .ok_or(ApiError::NotFound("Article"))?;
    let Some(Accessory::Panel { panel: old }) = &before.accessory else {
        return Err(ApiError::Invalid("Cet article n'est pas un panel.".into()));
    };
    check_panel(&st.store, &mut b.panel).await?;
    if b.panel.height_u != old.height_u {
        if let Some(cid) = &before.container_id {
            if st
                .store
                .placements(cid)
                .await?
                .iter()
                .any(|p| p.article_id == id)
            {
                return Err(one(
                    "height_u",
                    "Retirez d'abord le panel de la baie pour changer sa hauteur.",
                ));
            }
        }
    }
    let after = st
        .store
        .set_article_accessory(&id, b.rev, &Accessory::Panel { panel: b.panel })
        .await?;
    record(
        &st,
        &m,
        &origin,
        &after.project_id,
        "article",
        &id,
        "update",
        Some(after.rev),
        Some(&json!({ "panel": old.name, "slots": old.slots.len() })),
        Some(&after),
    )
    .await?;
    Ok(Json(json!({ "article": after })))
}
