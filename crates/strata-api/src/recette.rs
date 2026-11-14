//! Recette (section 11.5) : une par projet, calculée en direct. Écran collaboratif.

use crate::AppState;
use crate::auth::Member;
use crate::error::{ApiError, ApiResult};
use crate::live::Origin;
use crate::projects::{article_label, item_views, one, read, room, write};
use crate::synoptic::{Node, Port, project_nodes};
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use strata_domain::project::ArticleNature;
use strata_store::Store;

const STATUSES: [&str; 4] = ["to_test", "ok", "reserve", "ko"];

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects/{id}/recette", get(show))
        .route("/projects/{id}/recette/verdict", post(verdict))
        .route("/projects/{id}/recette/comment", post(comment))
        .route("/projects/{id}/recette/log", get(log))
        .route(
            "/projects/{id}/recette/free",
            post(free_add).patch(free_rename),
        )
        .route("/projects/{id}/recette/free/delete", post(free_delete))
        .route("/projects/{id}/recette/free/order", post(free_order))
}

/// Groupe des liaisons dont une extrémité n'existe plus (équipement ou port supprimé).
pub(crate) const MISSING_GROUP: &str = "Extrémité supprimée";

struct Live {
    key: String,
    kind: &'static str,
    group: String,
    label: String,
    detail: String,
}

/// Points calculés : une ligne par liaison (par élément, ou « Inter-éléments ») et une par
/// équipement posé dans un élément.
async fn live_points(store: &Store, pid: &str) -> ApiResult<Vec<Live>> {
    let inv = store.inventory(pid).await?;
    let names: HashMap<&str, &str> = inv
        .containers
        .iter()
        .map(|c| (c.id.as_str(), c.name.as_str()))
        .collect();
    let nodes = project_nodes(store, pid).await?;
    let port_of: HashMap<&str, (&Node, &Port)> = nodes
        .iter()
        .flat_map(|n| n.ports.iter().map(move |p| (p.r.as_str(), (n, p))))
        .collect();
    let end = |r: &str| {
        port_of
            .get(r)
            .map(|(n, p)| format!("{} › {}", n.label, p.label))
    };
    let mut out = Vec::new();
    for c in store.cables(pid).await?.iter().filter(|c| !c.fields.attach) {
        let ca = port_of
            .get(c.a.as_str())
            .and_then(|(n, _)| n.container_id.clone());
        let cb = port_of
            .get(c.b.as_str())
            .and_then(|(n, _)| n.container_id.clone());
        let gone = !port_of.contains_key(c.a.as_str())
            || (c.fields.external.is_none() && !port_of.contains_key(c.b.as_str()));
        let group = match (&ca, &cb, &c.fields.external) {
            // Rangés à part : un câble dont l'autre bout a été supprimé n'est pas « inter-éléments ».
            _ if gone => MISSING_GROUP.into(),
            (Some(a), _, Some(_)) => names.get(a.as_str()).unwrap_or(&"").to_string(),
            (Some(a), Some(b), None) if a == b => names.get(a.as_str()).unwrap_or(&"").to_string(),
            _ => "Inter-éléments".into(),
        };
        let to = match &c.fields.external {
            Some(x) => format!(
                "port externe{}",
                x.label
                    .as_ref()
                    .map(|l| format!(" · {l}"))
                    .unwrap_or_default()
            ),
            None => end(&c.b).unwrap_or_else(|| "extrémité supprimée".into()),
        };
        out.push(Live {
            key: format!("cable:{}", c.id),
            kind: "cable",
            group,
            label: c
                .fields
                .number
                .clone()
                .unwrap_or_else(|| "Câble non numéroté".into()),
            detail: format!(
                "{} → {to}",
                end(&c.a).unwrap_or_else(|| "extrémité supprimée".into())
            ),
        });
    }
    let ids: HashSet<String> = inv
        .articles
        .iter()
        .filter_map(|a| a.item_id.clone())
        .collect();
    let items = item_views(store, &ids).await?;
    for a in inv
        .articles
        .iter()
        .filter(|a| a.nature == ArticleNature::Equipment)
    {
        let Some(cid) = &a.container_id else { continue };
        for i in inv.instances.iter().filter(|i| i.article_id == a.id) {
            out.push(Live {
                key: format!("equip:{}", i.id),
                kind: "equipment",
                group: names.get(cid.as_str()).unwrap_or(&"").to_string(),
                label: format!("{} {}", article_label(a, &items), i.tag),
                detail: i
                    .serial
                    .clone()
                    .map(|s| format!("N° de série {s}"))
                    .unwrap_or_default(),
            });
        }
    }
    Ok(out)
}

pub(crate) async fn recette_view(store: &Store, pid: &str) -> ApiResult<Value> {
    let live = live_points(store, pid).await?;
    let saved: HashMap<String, strata_store::recette::Point> = store
        .recette_points(pid)
        .await?
        .into_iter()
        .map(|p| (p.key.clone(), p))
        .collect();
    let names: HashMap<String, String> = store
        .list_accounts(true)
        .await?
        .into_iter()
        .map(|a| (a.id, a.name))
        .collect();
    let point = |key: &str, label: &str, detail: &str, deleted: bool| {
        let s = saved.get(key);
        json!({
            "key": key, "label": label, "detail": detail, "deleted": deleted,
            "status": s.map(|s| s.status.as_str()).unwrap_or("to_test"), "comment": s.and_then(|s| s.comment.clone()),
            "by": s.and_then(|s| s.by_account.as_ref()).and_then(|b| names.get(b)), "at": s.and_then(|s| s.at.clone()),
        })
    };
    let mut cables: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut equipment: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let keys: HashSet<&str> = live.iter().map(|l| l.key.as_str()).collect();
    for l in &live {
        let target = if l.kind == "cable" {
            &mut cables
        } else {
            &mut equipment
        };
        target
            .entry(l.group.clone())
            .or_default()
            .push(point(&l.key, &l.label, &l.detail, false));
    }
    let free: Vec<Value> = saved
        .values()
        .filter(|p| p.kind == "free")
        .map(|p| (p.position, point(&p.key, &p.label, "", false)))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect();
    let deleted: Vec<Value> = saved
        .values()
        .filter(|p| p.kind != "free" && !keys.contains(p.key.as_str()))
        .map(|p| point(&p.key, &p.label, "Élément supprimé du projet", true))
        .collect();
    let mut all: Vec<&Value> = cables
        .values()
        .flatten()
        .chain(equipment.values().flatten())
        .chain(free.iter())
        .collect();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for p in &all {
        *counts
            .entry(p["status"].as_str().unwrap_or("to_test"))
            .or_default() += 1;
    }
    let total = all.len();
    let tested = total - counts.get("to_test").copied().unwrap_or(0);
    all.retain(|p| p["status"] == "reserve" || p["status"] == "ko");
    let reserves: Vec<Value> = all.into_iter().cloned().collect();
    let group = |m: BTreeMap<String, Vec<Value>>| {
        m.into_iter()
            .map(|(g, pts)| json!({ "title": g, "points": pts }))
            .collect::<Vec<_>>()
    };
    Ok(json!({
        "progress": if total == 0 { 0 } else { ((100 * tested) as f64 / total as f64).round() as u64 },
        "total": total,
        "counts": { "to_test": counts.get("to_test").copied().unwrap_or(0), "ok": counts.get("ok").copied().unwrap_or(0), "reserve": counts.get("reserve").copied().unwrap_or(0), "ko": counts.get("ko").copied().unwrap_or(0) },
        "cables": group(cables), "equipment": group(equipment), "free": free, "deleted": deleted, "reserves": reserves,
    }))
}

async fn show(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    st.store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    Ok(Json(recette_view(&st.store, &pid).await?))
}

#[derive(Deserialize)]
struct VerdictBody {
    key: String,
    status: String,
    comment: Option<String>,
}

async fn verdict(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<VerdictBody>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    if !STATUSES.contains(&b.status.as_str()) {
        return Err(one("status", "Statut : à tester, OK, réserve ou KO."));
    }
    let comment = b
        .comment
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty());
    if comment.is_some_and(|c| c.chars().count() > 2000) {
        return Err(one("comment", "2 000 caractères au plus."));
    }
    let live = live_points(&st.store, &pid).await?;
    let (kind, label) = match live.iter().find(|l| l.key == b.key) {
        Some(l) => (l.kind, l.label.clone()),
        None => match st
            .store
            .recette_points(&pid)
            .await?
            .into_iter()
            .find(|p| p.key == b.key && p.kind == "free")
        {
            Some(p) => ("free", p.label),
            None => return Err(one("key", "Point de recette inconnu.")),
        },
    };
    st.store
        .set_verdict(
            &pid,
            &b.key,
            kind,
            &label,
            &b.status,
            comment,
            Some(&m.0.account.id),
        )
        .await?;
    st.hub.change(
        room(&pid),
        "recette",
        &b.key,
        "verdict",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct CommentBody {
    key: String,
    comment: String,
}

async fn comment(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<CommentBody>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    let c = b.comment.trim();
    if c.is_empty() || c.chars().count() > 2000 {
        return Err(one("comment", "De 1 à 2 000 caractères."));
    }
    st.store
        .add_recette_comment(&pid, &b.key, c, Some(&m.0.account.id))
        .await?;
    st.hub.change(
        room(&pid),
        "recette",
        &b.key,
        "comment",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct LogQuery {
    key: String,
}

async fn log(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
    Query(q): Query<LogQuery>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    let names: HashMap<String, String> = st
        .store
        .list_accounts(true)
        .await?
        .into_iter()
        .map(|a| (a.id, a.name))
        .collect();
    let entries: Vec<Value> = st.store.recette_log(&pid, &q.key).await?.into_iter().map(|e| json!({ "at": e.at, "by": e.by_account.as_ref().and_then(|b| names.get(b)), "status": e.status, "comment": e.comment })).collect();
    Ok(Json(json!({ "entries": entries })))
}

#[derive(Deserialize)]
struct FreeBody {
    key: Option<String>,
    label: String,
}

fn free_label(l: &str) -> ApiResult<String> {
    let l = l.trim();
    if l.is_empty() || l.chars().count() > 200 {
        return Err(one("label", "De 1 à 200 caractères."));
    }
    Ok(l.to_string())
}

async fn free_add(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<FreeBody>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    let key = st
        .store
        .add_free_point(&pid, &free_label(&b.label)?)
        .await?;
    st.hub.change(
        room(&pid),
        "recette",
        &key,
        "create",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(Json(json!({ "key": key })))
}

async fn free_rename(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<FreeBody>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    let key = b.key.ok_or_else(|| one("key", "Point inconnu."))?;
    st.store
        .rename_free_point(&pid, &key, &free_label(&b.label)?)
        .await?;
    st.hub.change(
        room(&pid),
        "recette",
        &key,
        "update",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct KeyBody {
    key: String,
}

async fn free_delete(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<KeyBody>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    st.store.delete_free_point(&pid, &b.key).await?;
    st.hub.change(
        room(&pid),
        "recette",
        &b.key,
        "delete",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct OrderBody {
    keys: Vec<String>,
}

async fn free_order(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<OrderBody>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    st.store.order_free_points(&pid, &b.keys).await?;
    st.hub.change(
        room(&pid),
        "recette",
        &pid,
        "order",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(Json(json!({ "ok": true })))
}
