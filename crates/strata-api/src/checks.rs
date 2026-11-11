//! Contrôles de construction en atelier (section 9.7).
//!
//! La liste à cocher d'un rack se déduit de son inventaire : la baie, chaque groupe
//! d'équipements (un article et ses exemplaires), les panels, les accessoires et les
//! adaptateurs. Les groupes de câbles, les connexions et les attaches d'adaptateurs s'y
//! ajouteront avec le câblage (lot 6). Seules les coches sont gardées : une coche dont
//! l'élément a disparu est ignorée. Écran collaboratif : chaque coche est diffusée en direct.

use crate::AppState;
use crate::auth::Member;
use crate::error::{ApiError, ApiResult};
use crate::live::Origin;
use crate::projects::{article_label, item_views, read, room, write};
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use strata_domain::project::{Accessory, ArticleNature};
use strata_store::{Container, Store};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/containers/{id}/checks", get(show).post(set).delete(clear))
        .route("/projects/{id}/checks", get(summary))
}

const FAMILIES: [(&str, &str); 3] = [
    ("materiel", "Matériel"),
    ("cables", "Câbles"),
    ("connexions", "Connexions"),
];

struct Line {
    key: String,
    family: &'static str,
    kind: &'static str,
    label: String,
    detail: String,
    /// Ordre de montage : la baie, puis de haut en bas, puis le reste.
    order: (i64, i64, String),
}

/// Éléments à contrôler d'un contenant, dans l'ordre de montage.
async fn lines(store: &Store, c: &Container) -> ApiResult<Vec<Line>> {
    let inv = store.inventory(&c.project_id).await?;
    let arts: Vec<_> = inv
        .articles
        .iter()
        .filter(|a| a.container_id.as_deref() == Some(&c.id))
        .collect();
    let ids: HashSet<String> = arts.iter().filter_map(|a| a.item_id.clone()).collect();
    let items = item_views(store, &ids).await?;
    let placements = store.placements(&c.id).await?;
    let mut out = vec![Line {
        key: "rack".into(),
        family: "materiel",
        kind: "rack",
        label: c.name.clone(),
        detail: element_detail(&c.data),
        order: (0, 0, String::new()),
    }];
    for a in arts {
        let mine: Vec<_> = placements.iter().filter(|p| p.article_id == a.id).collect();
        let top = mine.iter().map(|p| p.u_start).max();
        let us = |iid: Option<&str>| {
            mine.iter()
                .find(|p| p.instance_id.as_deref() == iid)
                .map(|p| format!("U{}", p.u_start))
        };
        let label = article_label(a, &items);
        let (family, kind, detail) = match (a.nature, &a.accessory) {
            (ArticleNature::Equipment, _) => {
                let tags: Vec<String> = inv
                    .instances
                    .iter()
                    .filter(|i| i.article_id == a.id)
                    .map(|i| match us(Some(&i.id)) {
                        Some(u) => format!("{} {u}", i.tag),
                        None => i.tag.clone(),
                    })
                    .collect();
                let shown = tags.iter().take(6).cloned().collect::<Vec<_>>().join(", ");
                let more = if tags.len() > 6 {
                    format!(" et {} autres", tags.len() - 6)
                } else {
                    String::new()
                };
                (
                    "materiel",
                    "equipment",
                    format!("× {} · {shown}{more}", a.quantity),
                )
            }
            (ArticleNature::Adapter, _) => ("materiel", "adapter", format!("× {}", a.quantity)),
            (_, Some(Accessory::Panel { panel })) => (
                "materiel",
                "panel",
                format!(
                    "{} connecteur{} · {} groupe{}{}",
                    panel.slots.len(),
                    if panel.slots.len() > 1 { "s" } else { "" },
                    panel.groups.len(),
                    if panel.groups.len() > 1 { "s" } else { "" },
                    us(None).map(|u| format!(" · {u}")).unwrap_or_default()
                ),
            ),
            _ => (
                "materiel",
                "accessory",
                us(None).unwrap_or_else(|| "À placer".into()),
            ),
        };
        out.push(Line {
            key: format!("article:{}", a.id),
            family,
            kind,
            label,
            detail,
            order: (
                if kind == "adapter" {
                    3
                } else if top.is_some() {
                    1
                } else {
                    2
                },
                -top.unwrap_or(0),
                a.id.clone(),
            ),
        });
    }
    out.sort_by(|a, b| a.order.cmp(&b.order));
    Ok(out)
}

/// Ce qu'on vérifie sur l'élément lui-même, selon son type.
fn element_detail(d: &strata_domain::project::ContainerData) -> String {
    use strata_domain::project::ContainerKind as K;
    let kind = d.kind.label();
    match d.kind {
        _ if d.rackable => format!(
            "{kind} {} U · roulettes, rails, portes et visserie",
            d.height_u.unwrap_or(0.0)
        ),
        K::MonitorWall => {
            let w = d.details.wall.clone().unwrap_or_default();
            format!(
                "{kind} {} × {} · {} tuile(s) · structure, fixations et alimentation",
                w.rows,
                w.cols,
                w.tiles.len()
            )
        }
        K::Desk => format!(
            "{kind} · {} poste(s) · mobilier, éclairage et câblage des postes",
            d.details.positions.len()
        ),
        K::Zone => format!("{kind} · surface prête, protégée et balisée"),
        K::CameraPosition => format!("{kind} · support, sécurité et passage des câbles"),
        K::RfMast => format!("{kind} · haubanage, antennes et terre"),
        K::Power => format!("{kind} · raccordement, protections et terre"),
        K::ExternalFeed => format!("{kind} · liaison livrée et testée"),
        K::CableDrum => format!("{kind} · longueur et état du câble"),
        _ => format!("{kind} : état, fermetures et fixations"),
    }
}

async fn view(st: &AppState, c: &Container) -> ApiResult<Value> {
    let lines = lines(&st.store, c).await?;
    let checks: HashMap<String, strata_store::checks::Check> = st
        .store
        .checks(&c.id)
        .await?
        .into_iter()
        .map(|k| (k.key.clone(), k))
        .collect();
    let names: HashMap<String, String> = st
        .store
        .list_accounts(true)
        .await?
        .into_iter()
        .map(|a| (a.id, a.name))
        .collect();
    let items: Vec<Value> = lines
        .iter()
        .map(|l| {
            let ch = checks.get(&l.key).map(|k| json!({
                "by": k.checked_by, "name": k.checked_by.as_ref().and_then(|b| names.get(b)), "at": k.checked_at,
            }));
            json!({ "key": l.key, "family": l.family, "kind": l.kind, "label": l.label, "detail": l.detail, "checked": ch })
        })
        .collect();
    let count = |f: Option<&str>| {
        let all: Vec<_> = lines
            .iter()
            .filter(|l| f.is_none_or(|f| l.family == f))
            .collect();
        let done = all.iter().filter(|l| checks.contains_key(&l.key)).count();
        json!({ "done": done, "total": all.len() })
    };
    Ok(json!({
        "container": { "id": c.id, "name": c.name, "project_id": c.project_id },
        "items": items,
        "families": FAMILIES.iter().map(|(k, l)| { let mut v = count(Some(k)); v["key"] = json!(k); v["label"] = json!(l); v }).collect::<Vec<_>>(),
        "progress": count(None),
    }))
}

async fn container(store: &Store, id: &str) -> ApiResult<Container> {
    store
        .container(id)
        .await?
        .ok_or(ApiError::NotFound("Contenant"))
}

async fn show(
    State(st): State<AppState>,
    m: Member,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    let c = container(&st.store, &id).await?;
    Ok(Json(view(&st, &c).await?))
}

#[derive(Deserialize)]
struct SetBody {
    keys: Vec<String>,
    checked: bool,
}

async fn set(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
    Json(b): Json<SetBody>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    let c = container(&st.store, &id).await?;
    let known: HashSet<String> = lines(&st.store, &c)
        .await?
        .into_iter()
        .map(|l| l.key)
        .collect();
    let keys: Vec<String> = b.keys.into_iter().filter(|k| known.contains(k)).collect();
    if !keys.is_empty() {
        st.store
            .set_checks(&id, &keys, b.checked, Some(&m.0.account.id))
            .await?;
        st.hub.change(
            room(&c.project_id),
            "check",
            &id,
            if b.checked { "check" } else { "uncheck" },
            None,
            &m.0.account.id,
            origin.0.as_deref(),
        );
    }
    Ok(Json(view(&st, &c).await?))
}

async fn clear(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    let c = container(&st.store, &id).await?;
    let n = st.store.clear_checks(&id).await?;
    st.store
        .audit_project(
            &c.project_id,
            Some(&m.0.account.id),
            "clear",
            "check",
            &id,
            Some(&json!({ "container": c.name, "checked": n })),
            None::<&()>,
        )
        .await?;
    st.hub.change(
        room(&c.project_id),
        "check",
        &id,
        "clear",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(Json(view(&st, &c).await?))
}

/// Avancement de chaque contenant du projet.
async fn summary(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    st.store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    let mut out = Vec::new();
    for c in st.store.inventory(&pid).await?.containers {
        let keys: HashSet<String> = lines(&st.store, &c)
            .await?
            .into_iter()
            .map(|l| l.key)
            .collect();
        let done = st
            .store
            .checks(&c.id)
            .await?
            .iter()
            .filter(|k| keys.contains(&k.key))
            .count();
        out.push(json!({ "id": c.id, "name": c.name, "location_id": c.location_id, "rackable": c.data.rackable, "done": done, "total": keys.len() }));
    }
    Ok(Json(json!({ "containers": out })))
}
