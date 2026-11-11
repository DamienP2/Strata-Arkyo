//! Bibliothèques d'impression du site (sections 6.4, 11.6, 11.7) : cartouches, planches
//! d'étiquettes, modèles de dossier et de colonnes, logo du site.

use crate::AppState;
use crate::auth::Member;
use crate::error::{ApiError, ApiResult};
use crate::projects::{one, read, write};
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::header;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use base64::Engine;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use strata_domain::catalog::FieldError;
use strata_domain::print::{
    ColumnModel, DOCS, DossierTemplate, LabelSheet, TitleBlock, VARIABLES, default_label_sheets,
    default_title_blocks, repair, restore, validate_column_model, validate_dossier_template,
    validate_label_sheets, validate_title_blocks,
};
use strata_domain::{Domain, Level};
use strata_store::Store;

const TITLE_BLOCKS: &str = "title_blocks";
const LABEL_SHEETS: &str = "label_sheets";
const DOSSIERS: &str = "dossier_templates";
const COLUMNS: &str = "column_models";
pub const SITE_LOGO: &str = "site_logo";
const SETTINGS: &str = "Modifier les modèles d'impression demande « Paramètres : écriture ».";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/print/docs", get(docs))
        .route("/print/title-blocks", get(tb_list).put(tb_put))
        .route("/print/title-blocks/restore", post(tb_restore))
        .route("/print/label-sheets", get(ls_list).put(ls_put))
        .route("/print/label-sheets/restore", post(ls_restore))
        .route(
            "/print/dossier-templates",
            get(dt_list).post(dt_create).patch(dt_update),
        )
        .route("/print/dossier-templates/delete", post(dt_delete))
        .route("/print/dossier-templates/duplicate", post(dt_duplicate))
        .route("/print/column-models", get(cm_list).post(cm_create))
        .route("/print/column-models/delete", post(cm_delete))
        .route(
            "/site/logo",
            put(logo_put)
                .delete(logo_delete)
                .layer(DefaultBodyLimit::max(600 * 1024)),
        )
}

fn to_fields(e: Vec<(&'static str, String)>) -> ApiResult<()> {
    if e.is_empty() {
        return Ok(());
    }
    Err(ApiError::Fields(
        e.into_iter()
            .map(|(f, m)| FieldError {
                field: f.into(),
                message: m,
            })
            .collect(),
    ))
}

async fn load<T: DeserializeOwned>(store: &Store, key: &str) -> ApiResult<Option<Vec<T>>> {
    Ok(store
        .meta(key)
        .await?
        .and_then(|s| serde_json::from_str(&s).ok()))
}

async fn keep<T: Serialize>(store: &Store, key: &str, v: &[T]) -> ApiResult<()> {
    store
        .set_meta(key, Some(&serde_json::to_string(v).unwrap_or_default()))
        .await?;
    Ok(())
}

async fn audit(st: &AppState, m: &Member, kind: &str, id: &str, after: &Value) -> ApiResult<()> {
    st.store
        .audit(
            Some(&m.0.account.id),
            "update",
            kind,
            id,
            None::<&()>,
            Some(after),
        )
        .await?;
    Ok(())
}

async fn docs(m: Member) -> ApiResult<Json<Value>> {
    read(&m)?;
    Ok(Json(json!({
        "docs": DOCS.iter().map(|(k, g, t, c)| json!({ "key": k, "group": g, "title": t, "columns": c })).collect::<Vec<_>>(),
        "variables": VARIABLES.iter().map(|(k, l)| json!({ "key": k, "label": l })).collect::<Vec<_>>(),
    })))
}

// ----- Cartouches -----

pub(crate) async fn title_blocks(store: &Store) -> ApiResult<Vec<TitleBlock>> {
    Ok(load(store, TITLE_BLOCKS)
        .await?
        .unwrap_or_else(default_title_blocks))
}

async fn tb_list(State(st): State<AppState>, m: Member) -> ApiResult<Json<Value>> {
    read(&m)?;
    Ok(Json(json!({ "items": title_blocks(&st.store).await? })))
}

#[derive(Deserialize)]
struct List<T> {
    items: Vec<T>,
}

async fn tb_put(
    State(st): State<AppState>,
    m: Member,
    Json(mut b): Json<List<TitleBlock>>,
) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Parametres, Level::Ecriture, SETTINGS)?;
    to_fields(validate_title_blocks(&mut b.items))?;
    keep(&st.store, TITLE_BLOCKS, &b.items).await?;
    audit(
        &st,
        &m,
        "title_blocks",
        "site",
        &json!({ "count": b.items.len() }),
    )
    .await?;
    Ok(Json(json!({ "items": b.items })))
}

async fn tb_restore(State(st): State<AppState>, m: Member) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Parametres, Level::Ecriture, SETTINGS)?;
    let mut list = title_blocks(&st.store).await?;
    let before = list.len();
    restore(&mut list, default_title_blocks(), |t| t.key.clone());
    keep(&st.store, TITLE_BLOCKS, &list).await?;
    Ok(Json(
        json!({ "items": list, "restored": list.len() - before }),
    ))
}

// ----- Planches d'étiquettes -----

async fn label_sheets(store: &Store) -> ApiResult<Vec<LabelSheet>> {
    Ok(load(store, LABEL_SHEETS)
        .await?
        .unwrap_or_else(default_label_sheets))
}

async fn ls_list(State(st): State<AppState>, m: Member) -> ApiResult<Json<Value>> {
    read(&m)?;
    Ok(Json(json!({ "items": label_sheets(&st.store).await? })))
}

async fn ls_put(
    State(st): State<AppState>,
    m: Member,
    Json(mut b): Json<List<LabelSheet>>,
) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Parametres, Level::Ecriture, SETTINGS)?;
    to_fields(validate_label_sheets(&mut b.items))?;
    keep(&st.store, LABEL_SHEETS, &b.items).await?;
    audit(
        &st,
        &m,
        "label_sheets",
        "site",
        &json!({ "count": b.items.len() }),
    )
    .await?;
    Ok(Json(json!({ "items": b.items })))
}

async fn ls_restore(State(st): State<AppState>, m: Member) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Parametres, Level::Ecriture, SETTINGS)?;
    let mut list = label_sheets(&st.store).await?;
    let before = list.len();
    restore(&mut list, default_label_sheets(), |t| t.key.clone());
    keep(&st.store, LABEL_SHEETS, &list).await?;
    Ok(Json(
        json!({ "items": list, "restored": list.len() - before }),
    ))
}

// ----- Modèles de dossier -----

async fn dossiers(store: &Store) -> ApiResult<Vec<DossierTemplate>> {
    let mut list: Vec<DossierTemplate> = load(store, DOSSIERS).await?.unwrap_or_default();
    // Un modèle ancien se répare à la lecture.
    for t in list.iter_mut() {
        t.docs = repair(&t.docs);
    }
    Ok(list)
}

async fn dt_list(State(st): State<AppState>, m: Member) -> ApiResult<Json<Value>> {
    read(&m)?;
    Ok(Json(json!({ "items": dossiers(&st.store).await? })))
}

fn unique_name(list: &[DossierTemplate], t: &DossierTemplate) -> ApiResult<()> {
    if list
        .iter()
        .any(|x| x.id != t.id && x.name.to_lowercase() == t.name.to_lowercase())
    {
        return Err(one("name", "Un modèle de dossier porte déjà ce nom."));
    }
    Ok(())
}

async fn dt_create(
    State(st): State<AppState>,
    m: Member,
    Json(mut t): Json<DossierTemplate>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    t.id = uuid::Uuid::now_v7().to_string();
    to_fields(validate_dossier_template(&mut t))?;
    let mut list = dossiers(&st.store).await?;
    unique_name(&list, &t)?;
    list.push(t.clone());
    keep(&st.store, DOSSIERS, &list).await?;
    audit(
        &st,
        &m,
        "dossier_template",
        &t.id,
        &json!({ "name": t.name }),
    )
    .await?;
    Ok(Json(json!(t)))
}

async fn dt_update(
    State(st): State<AppState>,
    m: Member,
    Json(mut t): Json<DossierTemplate>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    to_fields(validate_dossier_template(&mut t))?;
    let mut list = dossiers(&st.store).await?;
    unique_name(&list, &t)?;
    let slot = list
        .iter_mut()
        .find(|x| x.id == t.id)
        .ok_or(ApiError::NotFound("Modèle de dossier"))?;
    *slot = t.clone();
    keep(&st.store, DOSSIERS, &list).await?;
    audit(
        &st,
        &m,
        "dossier_template",
        &t.id,
        &json!({ "name": t.name }),
    )
    .await?;
    Ok(Json(json!(t)))
}

#[derive(Deserialize)]
struct IdBody {
    id: String,
}

async fn dt_delete(
    State(st): State<AppState>,
    m: Member,
    Json(b): Json<IdBody>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    let mut list = dossiers(&st.store).await?;
    let n = list.len();
    list.retain(|x| x.id != b.id);
    if list.len() == n {
        return Err(ApiError::NotFound("Modèle de dossier"));
    }
    keep(&st.store, DOSSIERS, &list).await?;
    audit(
        &st,
        &m,
        "dossier_template",
        &b.id,
        &json!({ "deleted": true }),
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn dt_duplicate(
    State(st): State<AppState>,
    m: Member,
    Json(b): Json<IdBody>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    let mut list = dossiers(&st.store).await?;
    let src = list
        .iter()
        .find(|x| x.id == b.id)
        .cloned()
        .ok_or(ApiError::NotFound("Modèle de dossier"))?;
    let base = format!("{} (copie)", src.name);
    let mut name = base.clone();
    let mut i = 2;
    while list
        .iter()
        .any(|x| x.name.to_lowercase() == name.to_lowercase())
    {
        name = format!("{} (copie {i})", src.name);
        i += 1;
    }
    let mut t = DossierTemplate {
        id: uuid::Uuid::now_v7().to_string(),
        name: name.chars().take(100).collect(),
        ..src
    };
    to_fields(validate_dossier_template(&mut t))?;
    list.push(t.clone());
    keep(&st.store, DOSSIERS, &list).await?;
    Ok(Json(json!(t)))
}

// ----- Modèles de colonnes -----

async fn cm_list(State(st): State<AppState>, m: Member) -> ApiResult<Json<Value>> {
    read(&m)?;
    let list: Vec<ColumnModel> = load(&st.store, COLUMNS).await?.unwrap_or_default();
    Ok(Json(json!({ "items": list })))
}

async fn cm_create(
    State(st): State<AppState>,
    m: Member,
    Json(mut c): Json<ColumnModel>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    c.id = uuid::Uuid::now_v7().to_string();
    to_fields(validate_column_model(&mut c))?;
    let mut list: Vec<ColumnModel> = load(&st.store, COLUMNS).await?.unwrap_or_default();
    if list
        .iter()
        .any(|x| x.table == c.table && x.name.to_lowercase() == c.name.to_lowercase())
    {
        return Err(one(
            "name",
            "Un modèle de colonnes de ce tableau porte déjà ce nom.",
        ));
    }
    list.push(c.clone());
    keep(&st.store, COLUMNS, &list).await?;
    Ok(Json(json!(c)))
}

async fn cm_delete(
    State(st): State<AppState>,
    m: Member,
    Json(b): Json<IdBody>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    let mut list: Vec<ColumnModel> = load(&st.store, COLUMNS).await?.unwrap_or_default();
    list.retain(|x| x.id != b.id);
    keep(&st.store, COLUMNS, &list).await?;
    Ok(Json(json!({ "ok": true })))
}

// ----- Logo du site -----

async fn logo_put(
    State(st): State<AppState>,
    m: Member,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Parametres, Level::Ecriture, SETTINGS)?;
    let ct = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let ok = match ct.as_str() {
        "image/png" => body.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => body.starts_with(&[0xFF, 0xD8, 0xFF]),
        _ => false,
    };
    if !ok || body.len() > 512 * 1024 {
        return Err(ApiError::Invalid(
            "Logo : image PNG ou JPG de 512 Ko au plus.".into(),
        ));
    }
    let url = format!(
        "data:{ct};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&body)
    );
    st.store.set_meta(SITE_LOGO, Some(&url)).await?;
    audit(&st, &m, "site", "logo", &json!({ "bytes": body.len() })).await?;
    Ok(Json(json!({ "logo": url })))
}

async fn logo_delete(State(st): State<AppState>, m: Member) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Parametres, Level::Ecriture, SETTINGS)?;
    st.store.set_meta(SITE_LOGO, None).await?;
    Ok(Json(json!({ "logo": null })))
}
