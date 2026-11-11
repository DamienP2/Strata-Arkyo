//! Réglages mémorisés par projet et identité du site (nom pour les cartouches).

use crate::AppState;
use crate::auth::Member;
use crate::error::{ApiError, ApiResult};
use crate::projects::{one, read, write};
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use strata_domain::{Domain, Level};

const KEYS: [&str; 1] = ["rack_sheet"];
const SITE_NAME: &str = "site_name";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects/{id}/settings/{key}", get(show).put(put))
        .route("/site", get(site).put(set_site))
}

fn key_ok(k: &str) -> ApiResult<()> {
    if KEYS.contains(&k) {
        Ok(())
    } else {
        Err(ApiError::NotFound("Réglage"))
    }
}

async fn show(
    State(st): State<AppState>,
    m: Member,
    Path((pid, key)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    key_ok(&key)?;
    st.store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    let (data, rev) = st.store.project_setting(&pid, &key).await?;
    Ok(Json(json!({ "data": data, "rev": rev })))
}

#[derive(Deserialize)]
struct Put {
    rev: i64,
    data: Value,
}

async fn put(
    State(st): State<AppState>,
    m: Member,
    Path((pid, key)): Path<(String, String)>,
    Json(b): Json<Put>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    key_ok(&key)?;
    st.store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    if !b.data.is_object() || b.data.to_string().len() > 512 * 1024 {
        return Err(one("data", "Réglages illisibles ou trop volumineux."));
    }
    let rev = st
        .store
        .put_project_setting(&pid, &key, b.rev, &b.data)
        .await?;
    Ok(Json(json!({ "data": b.data, "rev": rev })))
}

async fn site(State(st): State<AppState>, _m: Member) -> ApiResult<Json<Value>> {
    let name = st.store.meta(SITE_NAME).await?;
    Ok(Json(
        json!({ "name": name.unwrap_or_else(|| "Strata Arkyo".into()) }),
    ))
}

#[derive(Deserialize)]
struct Site {
    name: String,
}

async fn set_site(
    State(st): State<AppState>,
    m: Member,
    Json(b): Json<Site>,
) -> ApiResult<Json<Value>> {
    m.0.require(
        Domain::Parametres,
        Level::Ecriture,
        "Renommer le site demande « Paramètres : écriture ».",
    )?;
    let name = b.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(one("name", "Le nom compte de 1 à 80 caractères."));
    }
    st.store.set_meta(SITE_NAME, Some(name)).await?;
    st.store
        .audit(
            Some(&m.0.account.id),
            "update",
            "site",
            "name",
            None::<&()>,
            Some(&json!({ "name": name })),
        )
        .await?;
    Ok(Json(json!({ "name": name })))
}
