//! Liens entre projets : un port externe d'un projet est relié au port externe d'un autre
//! (car régie ↔ installation fixe, plateau ↔ diffuseur…). Les deux projets voient le lien.

use crate::AppState;
use crate::auth::Member;
use crate::error::{ApiError, ApiResult};
use crate::live::Origin;
use crate::projects::{one, read};
use crate::synoptic::{Node, Port, project_nodes};
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use strata_domain::synoptic::PortRef;
use strata_domain::{Domain, Level};
use strata_store::Store;
use strata_store::cables::Cable;

const DESIGN: &str = "Vous n'avez pas le droit de modifier la conception.";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects/{id}/links", get(list).post(create))
        .route("/projects/{id}/links/delete", post(delete))
}

/// Ports externes d'un projet : câble, extrémité locale, connecteur et signal.
struct Ext {
    cable: Cable,
    label: String,
    connector: Option<String>,
    family: Option<String>,
    signal: Option<String>,
}

async fn externals(store: &Store, pid: &str) -> ApiResult<Vec<Ext>> {
    let nodes = project_nodes(store, pid).await?;
    let port_of: HashMap<&str, (&Node, &Port)> = nodes
        .iter()
        .flat_map(|n| n.ports.iter().map(move |p| (p.r.as_str(), (n, p))))
        .collect();
    Ok(store
        .cables(pid)
        .await?
        .into_iter()
        .filter(|c| {
            c.fields.external.is_some()
                && matches!(c.b.parse::<PortRef>(), Ok(PortRef::External { .. }))
        })
        .map(|c| {
            let np = port_of.get(c.a.as_str());
            let dest = c.fields.external.as_ref().and_then(|x| x.label.clone());
            Ext {
                label: format!(
                    "{}{}",
                    np.map(|(n, p)| format!(
                        "{} {} › {}",
                        n.label,
                        n.sub.rsplit(" · ").next().unwrap_or(""),
                        p.label
                    ))
                    .unwrap_or_else(|| "Port introuvable".into()),
                    dest.map(|d| format!(" (vers {d})")).unwrap_or_default()
                ),
                connector: np.map(|(_, p)| p.connector.clone()),
                family: np.and_then(|(_, p)| p.family.clone()),
                signal: np.and_then(|(_, p)| p.signal.clone()),
                cable: c,
            }
        })
        .collect())
}

fn ext_json(e: &Ext) -> Value {
    json!({ "cable_id": e.cable.id, "number": e.cable.fields.number, "label": e.label, "connector": e.connector, "signal": e.signal })
}

fn warnings(a: &Ext, b: &Ext) -> Vec<&'static str> {
    let mut w = Vec::new();
    if a.connector.is_some() && b.connector.is_some() && a.connector != b.connector {
        w.push("Connecteurs différents aux deux bouts.");
    }
    if a.family.is_some() && b.family.is_some() && a.family != b.family {
        w.push("Familles de signal différentes.");
    }
    w
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    candidates: bool,
}

async fn list(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    st.store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    let mine = externals(&st.store, &pid).await?;
    let names: HashMap<String, String> = st
        .store
        .list_projects()
        .await?
        .into_iter()
        .map(|p| (p.id.clone(), p.data.name.clone()))
        .collect();
    let mut cache: HashMap<String, Vec<Ext>> = HashMap::new();
    let mut links = Vec::new();
    for l in st.store.project_links(&pid).await? {
        let (lc, rp, rc) = if l.a_project == pid {
            (&l.a_cable, &l.b_project, &l.b_cable)
        } else {
            (&l.b_cable, &l.a_project, &l.a_cable)
        };
        if !cache.contains_key(rp) {
            cache.insert(rp.clone(), externals(&st.store, rp).await?);
        }
        let local = mine.iter().find(|e| &e.cable.id == lc);
        let remote = cache[rp].iter().find(|e| &e.cable.id == rc);
        links.push(json!({
            "id": l.id,
            "local": local.map(ext_json),
            "remote": { "project_id": rp, "project_name": names.get(rp), "port": remote.map(ext_json) },
            "warnings": match (local, remote) { (Some(a), Some(b)) => warnings(a, b), _ => vec!["Une extrémité a disparu."] },
            "note": l.note, "created_at": l.created_at,
        }));
    }
    let linked: HashSet<String> = st.store.linked_cables().await?.into_iter().collect();
    let mut candidates = Vec::new();
    if q.candidates {
        for p in st
            .store
            .list_projects()
            .await?
            .into_iter()
            .filter(|p| p.id != pid)
        {
            let ports: Vec<Value> = externals(&st.store, &p.id)
                .await?
                .iter()
                .filter(|e| !linked.contains(&e.cable.id))
                .map(ext_json)
                .collect();
            if !ports.is_empty() {
                candidates.push(
                    json!({ "project_id": p.id, "project_name": p.data.name, "ports": ports }),
                );
            }
        }
    }
    Ok(Json(json!({
        "links": links,
        "free": mine.iter().filter(|e| !linked.contains(&e.cable.id)).map(ext_json).collect::<Vec<_>>(),
        "candidates": candidates,
    })))
}

#[derive(Deserialize)]
struct CreateBody {
    cable_id: String,
    remote_project: String,
    remote_cable: String,
    #[serde(default)]
    note: Option<String>,
}

async fn create(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<CreateBody>,
) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    if b.remote_project == pid {
        return Err(one(
            "remote_project",
            "Un lien relie deux projets différents.",
        ));
    }
    let a = externals(&st.store, &pid)
        .await?
        .into_iter()
        .find(|e| e.cable.id == b.cable_id)
        .ok_or_else(|| one("cable_id", "Ce câble n'est pas un port externe du projet."))?;
    st.store
        .project(&b.remote_project)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    let r = externals(&st.store, &b.remote_project)
        .await?
        .into_iter()
        .find(|e| e.cable.id == b.remote_cable)
        .ok_or_else(|| {
            one(
                "remote_cable",
                "Ce câble n'est pas un port externe de l'autre projet.",
            )
        })?;
    let note = b.note.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if note.is_some_and(|n| n.chars().count() > 500) {
        return Err(one("note", "500 caractères au plus."));
    }
    let id = st
        .store
        .insert_project_link(
            (&pid, &a.cable.id),
            (&b.remote_project, &r.cable.id),
            note,
            Some(&m.0.account.id),
        )
        .await
        .map_err(|e| match e {
            strata_store::StoreError::Conflict(_) => {
                ApiError::Conflict("Un de ces ports externes est déjà lié.".into())
            }
            e => e.into(),
        })?;
    for p in [&pid, &b.remote_project] {
        st.store
            .audit_project(
                p,
                Some(&m.0.account.id),
                "create",
                "link",
                &id,
                None::<&Value>,
                Some(&json!({ "a": a.cable.id, "b": r.cable.id })),
            )
            .await?;
        st.hub.change(
            crate::projects::room(p),
            "link",
            &id,
            "create",
            None,
            &m.0.account.id,
            origin.0.as_deref(),
        );
    }
    Ok(Json(json!({ "id": id, "warnings": warnings(&a, &r) })))
}

#[derive(Deserialize)]
struct IdBody {
    id: String,
}

async fn delete(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<IdBody>,
) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    let l = st
        .store
        .project_links(&pid)
        .await?
        .into_iter()
        .find(|l| l.id == b.id)
        .ok_or(ApiError::NotFound("Lien"))?;
    st.store.delete_project_link(&l.id).await?;
    for p in [&l.a_project, &l.b_project] {
        st.store
            .audit_project(
                p,
                Some(&m.0.account.id),
                "delete",
                "link",
                &l.id,
                Some(&json!({ "a": l.a_cable, "b": l.b_cable })),
                None::<&Value>,
            )
            .await?;
        st.hub.change(
            crate::projects::room(p),
            "link",
            &l.id,
            "delete",
            None,
            &m.0.account.id,
            origin.0.as_deref(),
        );
    }
    Ok(Json(json!({ "ok": true })))
}
