//! Régies (section 6.2) : ensembles ordonnés de contenants.
//!
//! Dans un projet, une régie assemble des contenants du projet ; chaque projet a sa régie
//! principale, créée au premier contenant. Dans la bibliothèque, un modèle de régie assemble
//! des modèles de contenants ; le copier dans un projet recrée chaque contenant avec son
//! contenu. Une régie liée à un projet n'est jamais un modèle : « Enregistrer comme modèle »
//! en fait une copie dans la bibliothèque.

use crate::AppState;
use crate::auth::Member;
use crate::error::{ApiError, ApiResult};
use crate::live::Origin;
use crate::projects::{fields, inventory_view, one, read, record, write};
use crate::templates::{instantiate, snapshot};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use strata_domain::project::{RegieFields, unique_name, validate_regie};
use strata_domain::{Domain, Level};
use strata_store::Store;
use strata_store::regies::Regie;

const LIB_WRITE: &str = "Gérer la bibliothèque de régies demande « Catalogue : écriture ».";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/{id}/regies",
            get(project_list).post(project_create),
        )
        .route("/projects/{id}/regies/from-template", post(from_template))
        .route("/regies", get(library_list).post(library_create))
        .route("/regies/{id}", get(show).patch(update).delete(remove))
        .route("/regies/{id}/save-as-template", post(save_as))
}

fn lib_read(m: &Member) -> ApiResult<()> {
    if m.0.actor.allows(Domain::Catalogue, Level::Lecture)
        || m.0.actor.allows(Domain::Projets, Level::Lecture)
    {
        Ok(())
    } else {
        Err(ApiError::Forbidden(
            "Vous n'avez pas accès à la bibliothèque.".into(),
        ))
    }
}

/// Droit de lire (`writing` faux) ou de modifier une régie, selon qu'elle est liée à un projet.
fn allowed(m: &Member, r: &Regie, writing: bool) -> ApiResult<()> {
    match (&r.project_id, writing) {
        (Some(_), false) => read(m),
        (Some(_), true) => write(m),
        (None, false) => lib_read(m),
        (None, true) => m.0.require(Domain::Catalogue, Level::Ecriture, LIB_WRITE),
    }
}

async fn load(store: &Store, id: &str) -> ApiResult<Regie> {
    store.regie(id).await?.ok_or(ApiError::NotFound("Régie"))
}

fn sum_view(r: &Regie, members: Vec<Value>) -> Value {
    let f = |m: &Value, k: &str| m["totals"][k].as_f64().unwrap_or(0.0);
    let total_u: f64 = members.iter().map(|m| f(m, "total_u")).sum();
    let mut v = serde_json::to_value(r).unwrap_or_default();
    v["totals"] = json!({
        "containers": members.len(),
        "used_u": members.iter().map(|m| f(m, "used_u")).sum::<f64>() + 0.0,
        "total_u": total_u + 0.0,
        "weight_kg": (members.iter().map(|m| f(m, "weight_kg")).sum::<f64>() * 10.0).round() / 10.0 + 0.0,
        "power_w": members.iter().map(|m| f(m, "power_w")).sum::<f64>().round() + 0.0,
        "alerts": members.iter().filter(|m| m["totals"]["over_weight"] == true || m["totals"]["over_power"] == true || m["totals"]["over_u"] == true).count(),
    });
    v["members"] = Value::Array(members);
    v
}

/// Régies d'un projet, avec les contenants et leurs totaux.
async fn project_views(store: &Store, pid: &str, only: Option<&str>) -> ApiResult<Vec<Value>> {
    let (_, _, inv) = inventory_view(store, pid).await?;
    let by_id: HashMap<&str, &Value> = inv["containers"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|c| c["id"].as_str().map(|id| (id, c)))
                .collect()
        })
        .unwrap_or_default();
    let loc_name: HashMap<&str, &str> = inv["locations"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|l| Some((l["id"].as_str()?, l["name"].as_str()?)))
                .collect()
        })
        .unwrap_or_default();
    Ok(store
        .regies(Some(pid))
        .await?
        .iter()
        .filter(|r| only.is_none_or(|o| o == r.id))
        .map(|r| {
            let members = r
                .members
                .iter()
                .filter_map(|id| by_id.get(id.as_str()))
                .map(|c| {
                    json!({
                        "id": c["id"], "name": c["name"], "data": c, "totals": c["totals"],
                        "location": c["location_id"].as_str().and_then(|l| loc_name.get(l)),
                    })
                })
                .collect();
            sum_view(r, members)
        })
        .collect())
}

/// Modèles de régies, avec les modèles de contenants et leurs résumés.
async fn library_views(store: &Store, only: Option<&str>) -> ApiResult<Vec<Value>> {
    let templates = crate::templates::views(store).await?;
    let by_id: HashMap<&str, &Value> = templates
        .iter()
        .filter_map(|t| t["id"].as_str().map(|id| (id, t)))
        .collect();
    Ok(store
        .regies(None)
        .await?
        .iter()
        .filter(|r| only.is_none_or(|o| o == r.id))
        .map(|r| {
            let members = r
                .members
                .iter()
                .filter_map(|id| by_id.get(id.as_str()))
                .map(|t| {
                    let s = &t["summary"];
                    json!({
                        "id": t["id"], "name": t["name"], "data": t["container"], "status": t["status"],
                        "totals": {
                            "used_u": s["used_front_u"], "total_u": if t["container"]["rackable"] == true { t["container"]["height_u"].clone() } else { json!(0) },
                            "weight_kg": s["weight_kg"], "power_w": s["power_w"],
                            "over_weight": s["over_weight"], "over_power": s["over_power"],
                        },
                    })
                })
                .collect();
            sum_view(r, members)
        })
        .collect())
}

async fn view_of(store: &Store, r: &Regie) -> ApiResult<Value> {
    let v = match &r.project_id {
        Some(pid) => project_views(store, pid, Some(&r.id)).await?,
        None => library_views(store, Some(&r.id)).await?,
    };
    v.into_iter().next().ok_or(ApiError::NotFound("Régie"))
}

/// Les membres doivent être des contenants du projet, ou des modèles de contenants, sans doublon.
async fn check_members(store: &Store, pid: Option<&str>, members: &[String]) -> ApiResult<()> {
    let known: HashSet<String> = match pid {
        Some(pid) => store
            .inventory(pid)
            .await?
            .containers
            .into_iter()
            .map(|c| c.id)
            .collect(),
        None => store.templates().await?.into_iter().map(|t| t.id).collect(),
    };
    if members.iter().any(|m| !known.contains(m)) {
        return Err(one(
            "members",
            if pid.is_some() {
                "Un élément n'appartient pas au projet."
            } else {
                "Un modèle d'élément est introuvable."
            },
        ));
    }
    if pid.is_some() && members.iter().collect::<HashSet<_>>().len() != members.len() {
        return Err(one(
            "members",
            "Un élément ne figure qu'une fois dans une régie.",
        ));
    }
    Ok(())
}

// ----- Projet -----

async fn project_list(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    st.store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    Ok(Json(
        json!({ "regies": project_views(&st.store, &pid, None).await? }),
    ))
}

#[derive(Deserialize)]
struct NewRegie {
    #[serde(flatten)]
    fields: RegieFields,
    #[serde(default)]
    members: Vec<String>,
}

async fn project_create(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(mut b): Json<NewRegie>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    write(&m)?;
    st.store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    fields(validate_regie(&mut b.fields))?;
    check_members(&st.store, Some(&pid), &b.members).await?;
    let r = st
        .store
        .insert_regie(Some(&pid), &b.fields, &b.members, Some(&m.0.account.id))
        .await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "regie",
        &r.id,
        "create",
        Some(r.rev),
        None::<&()>,
        Some(&r),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(view_of(&st.store, &r).await?)))
}

#[derive(Deserialize)]
struct FromTemplate {
    regie_id: String,
    location_id: String,
}

/// Copie un modèle de régie : chaque modèle de contenant devient un contenant du projet.
async fn from_template(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<FromTemplate>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    write(&m)?;
    st.store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    let l = st
        .store
        .location(&b.location_id)
        .await?
        .filter(|l| l.project_id == pid)
        .ok_or_else(|| one("location_id", "Choisissez un emplacement du projet."))?;
    let tpl = load(&st.store, &b.regie_id).await?;
    if tpl.project_id.is_some() {
        return Err(ApiError::NotFound("Modèle de régie"));
    }
    if tpl.members.is_empty() {
        return Err(ApiError::Conflict(
            "Ce modèle de régie ne contient aucun élément.".into(),
        ));
    }
    let mut made = Vec::new();
    let mut skipped = 0;
    for tid in &tpl.members {
        let Some(t) = st.store.template(tid).await? else {
            continue;
        };
        let short = t
            .name
            .strip_prefix(&format!("{} · ", tpl.name))
            .map(str::to_string);
        let (c, s) = instantiate(&st.store, &pid, &l.id, &t, short.as_deref()).await?;
        skipped += s;
        st.hub.change(
            crate::projects::room(&pid),
            "container",
            &c.id,
            "create",
            Some(c.rev),
            &m.0.account.id,
            origin.0.as_deref(),
        );
        made.push(c.id);
    }
    let taken: Vec<String> = st
        .store
        .regies(Some(&pid))
        .await?
        .into_iter()
        .map(|r| r.name)
        .collect();
    let f = RegieFields {
        name: unique_name(&tpl.name, &taken),
        kind: tpl.kind,
        service_type: tpl.service_type.clone(),
        notes: tpl.notes.clone(),
    };
    let r = st
        .store
        .insert_regie(Some(&pid), &f, &made, Some(&m.0.account.id))
        .await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "regie",
        &r.id,
        "create",
        Some(r.rev),
        None::<&()>,
        Some(&json!({ "name": r.name, "template": tpl.name })),
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "regie": view_of(&st.store, &r).await?, "skipped_items": skipped })),
    ))
}

// ----- Bibliothèque -----

async fn library_list(State(st): State<AppState>, m: Member) -> ApiResult<Json<Value>> {
    lib_read(&m)?;
    Ok(Json(
        json!({ "regies": library_views(&st.store, None).await? }),
    ))
}

async fn library_create(
    State(st): State<AppState>,
    m: Member,
    Json(mut b): Json<NewRegie>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    m.0.require(Domain::Catalogue, Level::Ecriture, LIB_WRITE)?;
    fields(validate_regie(&mut b.fields))?;
    check_members(&st.store, None, &b.members).await?;
    let r = st
        .store
        .insert_regie(None, &b.fields, &b.members, Some(&m.0.account.id))
        .await?;
    st.store
        .audit(
            Some(&m.0.account.id),
            "create",
            "regie_template",
            &r.id,
            None::<&()>,
            Some(&json!({ "name": r.name })),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(view_of(&st.store, &r).await?)))
}

#[derive(Deserialize)]
struct SaveAs {
    name: Option<String>,
}

/// Enregistre une régie de projet comme modèle : chaque contenant devient un modèle de
/// contenant (en brouillon), et le modèle de régie les assemble dans le même ordre.
async fn save_as(
    State(st): State<AppState>,
    m: Member,
    Path(id): Path<String>,
    Json(b): Json<SaveAs>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    m.0.require(Domain::Catalogue, Level::Ecriture, LIB_WRITE)?;
    read(&m)?;
    let r = load(&st.store, &id).await?;
    if r.project_id.is_none() {
        return Err(ApiError::Conflict(
            "Cette régie est déjà un modèle de la bibliothèque.".into(),
        ));
    }
    if r.members.is_empty() {
        return Err(ApiError::Conflict(
            "Ajoutez au moins un élément avant d'en faire un modèle.".into(),
        ));
    }
    let mut taken: Vec<String> = st
        .store
        .templates()
        .await?
        .into_iter()
        .map(|t| t.name)
        .collect();
    let mut ids = Vec::new();
    // Les modèles de contenants portent le nom de la régie, pour qu'on les retrouve.
    let prefix = b
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| r.name.replace(" — Régie principale", ""));
    for cid in &r.members {
        let Some(c) = st.store.container(cid).await? else {
            continue;
        };
        let data = snapshot(&st.store, &c).await?;
        let base: String = format!("{prefix} · {}", c.name).chars().take(120).collect();
        let name = unique_name(&base, &taken);
        let t = st
            .store
            .insert_template(
                &name,
                &serde_json::to_value(&data).unwrap_or_default(),
                Some(&m.0.account.id),
            )
            .await?;
        taken.push(t.name.clone());
        ids.push(t.id);
    }
    let lib_taken: Vec<String> = st
        .store
        .regies(None)
        .await?
        .into_iter()
        .map(|x| x.name)
        .collect();
    let base = b
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            if r.main {
                r.name.replace(" — Régie principale", "")
            } else {
                r.name.clone()
            }
        });
    let f = RegieFields {
        name: unique_name(&base, &lib_taken),
        kind: r.kind,
        service_type: r.service_type.clone(),
        notes: r.notes.clone(),
    };
    let lib = st
        .store
        .insert_regie(None, &f, &ids, Some(&m.0.account.id))
        .await?;
    st.store
        .audit(
            Some(&m.0.account.id),
            "create",
            "regie_template",
            &lib.id,
            None::<&()>,
            Some(&json!({ "name": lib.name, "from": r.id, "containers": ids.len() })),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(view_of(&st.store, &lib).await?)))
}

// ----- Les deux -----

async fn show(
    State(st): State<AppState>,
    m: Member,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let r = load(&st.store, &id).await?;
    allowed(&m, &r, false)?;
    Ok(Json(view_of(&st.store, &r).await?))
}

#[derive(Deserialize)]
struct Patch {
    rev: i64,
    name: Option<String>,
    kind: Option<strata_domain::project::Regie>,
    /// Absent : inchangé ; chaîne vide : effacé.
    service_type: Option<String>,
    notes: Option<String>,
    members: Option<Vec<String>>,
}

async fn update(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
    Json(b): Json<Patch>,
) -> ApiResult<Json<Value>> {
    let r = load(&st.store, &id).await?;
    allowed(&m, &r, true)?;
    let mut f = RegieFields {
        name: b.name.unwrap_or(r.name.clone()),
        kind: b.kind.unwrap_or(r.kind),
        service_type: b.service_type.or(r.service_type.clone()),
        notes: b.notes.or(r.notes.clone()),
    };
    fields(validate_regie(&mut f))?;
    if let Some(ms) = &b.members {
        check_members(&st.store, r.project_id.as_deref(), ms).await?;
    }
    let after = st
        .store
        .update_regie(&id, b.rev, &f, b.members.as_deref())
        .await?;
    match &r.project_id {
        Some(pid) => {
            record(
                &st,
                &m,
                &origin,
                pid,
                "regie",
                &id,
                "update",
                Some(after.rev),
                Some(&r),
                Some(&after),
            )
            .await?
        }
        None => {
            st.store
                .audit(
                    Some(&m.0.account.id),
                    "update",
                    "regie_template",
                    &id,
                    Some(&json!({ "name": r.name, "members": r.members.len() })),
                    Some(&json!({ "name": after.name, "members": after.members.len() })),
                )
                .await?
        }
    }
    Ok(Json(view_of(&st.store, &after).await?))
}

async fn remove(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let r = load(&st.store, &id).await?;
    allowed(&m, &r, true)?;
    if r.main {
        return Err(ApiError::Conflict(
            "La régie principale du projet ne se supprime pas.".into(),
        ));
    }
    st.store.delete_regie(&id).await?;
    match &r.project_id {
        Some(pid) => {
            record(
                &st,
                &m,
                &origin,
                pid,
                "regie",
                &id,
                "delete",
                None,
                Some(&r),
                None::<&()>,
            )
            .await?
        }
        None => {
            st.store
                .audit(
                    Some(&m.0.account.id),
                    "delete",
                    "regie_template",
                    &id,
                    Some(&json!({ "name": r.name })),
                    None::<&()>,
                )
                .await?
        }
    }
    Ok(StatusCode::NO_CONTENT)
}
