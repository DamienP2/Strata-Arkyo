//! Préparation et livraison (section 11.4). Écran collaboratif, diffusé en direct.

use crate::AppState;
use crate::auth::Member;
use crate::catalog_io::{Cell, file_response, to_csv, to_xlsx_named};
use crate::error::{ApiError, ApiResult};
use crate::live::Origin;
use crate::projects::{article_label, fields, item_views, one, read, room, write};
use crate::synoptic::{Node, Port, estimate, project_nodes};
use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use strata_domain::catalog::RefKind;
use strata_domain::prep::{Family, LineState, plug_gender, validate_line};
use strata_domain::project::{Accessory, ArticleNature};
use strata_store::Store;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects/{id}/prep", get(show))
        .route("/projects/{id}/prep/line", patch(set_line))
        .route("/projects/{id}/prep/cable-model", post(cable_model))
        .route("/projects/{id}/prep/cabled", post(cabled))
        .route("/projects/{id}/prep/export", get(export))
}

struct Line {
    key: String,
    family: Family,
    label: String,
    detail: String,
    min: i64,
    cables: Vec<String>,
    model: Option<String>,
}

async fn lines(store: &Store, pid: &str) -> ApiResult<(Vec<Line>, Value)> {
    let inv = store.inventory(pid).await?;
    let ids: HashSet<String> = inv
        .articles
        .iter()
        .filter_map(|a| a.item_id.clone())
        .collect();
    let items = item_views(store, &ids).await?;
    let locs: HashMap<&str, &str> = inv
        .locations
        .iter()
        .map(|l| (l.id.as_str(), l.name.as_str()))
        .collect();
    let routes = crate::plans::route_lengths(store, pid).await?;
    let cont: HashMap<&str, &strata_store::Container> =
        inv.containers.iter().map(|c| (c.id.as_str(), c)).collect();
    let mut out = Vec::new();
    for c in &inv.containers {
        out.push(Line {
            key: format!("element:{}", c.id),
            family: Family::Elements,
            label: c.name.clone(),
            detail: format!(
                "{} · {}",
                c.data.kind.label(),
                locs.get(c.location_id.as_str()).unwrap_or(&"")
            ),
            min: 1,
            cables: vec![],
            model: None,
        });
    }
    let mut groups: BTreeMap<String, (Family, String, i64, BTreeMap<String, i64>)> =
        BTreeMap::new();
    for a in &inv.articles {
        let where_ = a
            .container_id
            .as_deref()
            .and_then(|c| cont.get(c))
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "libre".into());
        let (key, fam, label) = match (a.nature, &a.accessory) {
            (ArticleNature::Equipment, _) => (
                format!("equipment:{}", a.item_id.clone().unwrap_or_default()),
                Family::Equipment,
                article_label(a, &items),
            ),
            (ArticleNature::Adapter, _) => (
                format!("adapter:{}", a.item_id.clone().unwrap_or_default()),
                Family::Others,
                article_label(a, &items),
            ),
            (_, Some(Accessory::Panel { panel })) => (
                format!(
                    "panel:{}:{}:{}",
                    strata_domain::text::normalize(&panel.name),
                    panel.height_u,
                    panel.per_u
                ),
                Family::Others,
                format!("Panel {}", panel.name),
            ),
            _ => (
                format!(
                    "accessory:{}",
                    strata_domain::text::normalize(&article_label(a, &items))
                ),
                Family::Others,
                article_label(a, &items),
            ),
        };
        let g = groups
            .entry(key)
            .or_insert((fam, label, 0, BTreeMap::new()));
        g.2 += a.quantity;
        *g.3.entry(where_).or_default() += a.quantity;
    }
    for (key, (family, label, n, wh)) in groups {
        out.push(Line {
            key,
            family,
            label,
            detail: wh
                .iter()
                .map(|(w, q)| format!("{q} × {w}"))
                .collect::<Vec<_>>()
                .join(" · "),
            min: n,
            cables: vec![],
            model: None,
        });
    }
    // Câbles : modèle, connecteurs et genres des fiches, longueur.
    let nodes = project_nodes(store, pid).await?;
    let port_of: HashMap<&str, (&Node, &Port)> = nodes
        .iter()
        .flat_map(|n| n.ports.iter().map(move |p| (p.r.as_str(), (n, p))))
        .collect();
    let refs = store.list_refs(Some(RefKind::Connector)).await?;
    let cname = |c: &str| {
        refs.iter()
            .find(|r| r.id == c)
            .map(|r| r.name.clone())
            .unwrap_or_else(|| c.to_string())
    };
    let models: HashMap<String, String> = {
        let mut m = HashMap::new();
        for c in store.cables(pid).await? {
            if let Some(id) = &c.fields.cable_model_id {
                if !m.contains_key(id) {
                    if let Some(it) = store.item(id).await? {
                        m.insert(id.clone(), it.name);
                    }
                }
            }
        }
        m
    };
    let mut cg: BTreeMap<String, (Option<String>, String, Vec<String>)> = BTreeMap::new();
    for c in store.cables(pid).await?.iter().filter(|c| !c.fields.attach) {
        let ea = port_of.get(c.a.as_str()).copied();
        let eb = port_of.get(c.b.as_str()).copied();
        let end = |e: Option<(&Node, &Port)>| {
            e.map(|(_, p)| {
                (
                    p.connector.clone(),
                    plug_gender(p.gender.as_deref()).to_string(),
                )
            })
            .unwrap_or_else(|| {
                (
                    ea.map(|(_, p)| p.connector.clone()).unwrap_or_default(),
                    "?".into(),
                )
            })
        };
        let mut ends = [end(ea), end(eb)];
        ends.sort();
        let len = c
            .fields
            .length_m
            .or_else(|| estimate(ea.as_ref(), eb.as_ref(), &cont, &routes));
        let len_s = len
            .map(|l| format!("{:.1}", (l * 2.0).ceil() / 2.0))
            .unwrap_or_else(|| "?".into());
        let key = format!(
            "cable:{}:{}{}-{}{}:{}",
            c.fields
                .cable_model_id
                .clone()
                .unwrap_or_else(|| "none".into()),
            ends[0].0,
            ends[0].1,
            ends[1].0,
            ends[1].1,
            len_s
        );
        let detail = format!(
            "{} {} ↔ {} {} · {}",
            cname(&ends[0].0),
            ends[0].1,
            cname(&ends[1].0),
            ends[1].1,
            if len_s == "?" {
                "longueur à définir".to_string()
            } else {
                format!("{} m", len_s.replace('.', ","))
            }
        );
        cg.entry(key)
            .or_insert((c.fields.cable_model_id.clone(), detail, vec![]))
            .2
            .push(c.id.clone());
    }
    for (key, (model, detail, ids)) in cg {
        out.push(Line {
            label: model
                .as_deref()
                .and_then(|m| models.get(m))
                .cloned()
                .unwrap_or_else(|| "Type de câble à choisir".into()),
            key,
            family: Family::Cables,
            detail,
            min: ids.len() as i64,
            cables: ids,
            model,
        });
    }
    // Colisage : l'élément est l'unité de colisage.
    let packing: Vec<Value> = inv
        .containers
        .iter()
        .map(|c| {
            let content: Vec<Value> = inv.articles.iter().filter(|a| a.container_id.as_deref() == Some(&c.id)).map(|a| json!({ "label": article_label(a, &items), "quantity": a.quantity })).collect();
            json!({ "id": c.id, "name": c.name, "location": locs.get(c.location_id.as_str()), "kind": c.data.kind.label(), "content": content })
        })
        .collect();
    Ok((out, Value::Array(packing)))
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
    let (ls, packing) = lines(&st.store, &pid).await?;
    let saved: HashMap<String, strata_store::prep::PrepLine> = st
        .store
        .prep_lines(&pid)
        .await?
        .into_iter()
        .map(|l| (l.key.clone(), l))
        .collect();
    let names: HashMap<String, String> = st
        .store
        .list_accounts(true)
        .await?
        .into_iter()
        .map(|a| (a.id, a.name))
        .collect();
    let done: HashMap<String, strata_store::prep::CableDone> = st
        .store
        .cables_done(&pid)
        .await?
        .into_iter()
        .map(|d| (d.cable_id.clone(), d))
        .collect();
    let cables: HashMap<String, strata_store::cables::Cable> = st
        .store
        .cables(&pid)
        .await?
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();
    let nodes = project_nodes(&st.store, &pid).await?;
    let port_of: HashMap<&str, (&Node, &Port)> = nodes
        .iter()
        .flat_map(|n| n.ports.iter().map(move |p| (p.r.as_str(), (n, p))))
        .collect();
    let endname = |r: &str| {
        port_of
            .get(r)
            .map(|(n, p)| format!("{} › {}", n.label, p.label))
            .unwrap_or_else(|| "port externe".into())
    };
    let mut fams: Vec<Value> = Vec::new();
    for f in [
        Family::Elements,
        Family::Equipment,
        Family::Others,
        Family::Cables,
    ] {
        let mut rows = Vec::new();
        let mut finished = 0;
        for l in ls.iter().filter(|l| l.family == f) {
            let s = saved.get(&l.key);
            let status = s
                .and_then(|s| s.status.clone())
                .unwrap_or_else(|| f.first().to_string());
            if status == f.done() {
                finished += 1;
            }
            let reserve = s.map(|s| s.reserve).unwrap_or(0);
            let items: Vec<Value> = l.cables.iter().filter_map(|id| cables.get(id)).map(|c| {
                let d = done.get(&c.id);
                json!({ "id": c.id, "number": c.fields.number, "from": endname(&c.a), "to": c.fields.external.as_ref().map(|_| "port externe".to_string()).unwrap_or_else(|| endname(&c.b)),
                        "done_at": d.map(|d| &d.done_at), "done_by": d.and_then(|d| d.done_by.as_ref()).and_then(|b| names.get(b)), "note": d.and_then(|d| d.note.clone()) })
            }).collect();
            rows.push(json!({
                "key": l.key, "label": l.label, "detail": l.detail, "min": l.min, "reserve": reserve, "total": l.min + reserve,
                "status": status, "supply": s.and_then(|s| s.supply), "note": s.and_then(|s| s.note.clone()),
                "updated_by": s.and_then(|s| s.updated_by.as_ref()).and_then(|b| names.get(b)), "updated_at": s.map(|s| &s.updated_at),
                "model_id": l.model, "missing_model": f == Family::Cables && l.model.is_none(), "cables": items,
                "cabled": l.cables.iter().filter(|id| done.contains_key(*id)).count(),
            }));
        }
        fams.push(json!({ "key": f, "label": f.label(), "statuses": f.statuses().iter().map(|(k, l)| json!({ "key": k, "label": l })).collect::<Vec<_>>(), "lines": rows, "done": finished }));
    }
    Ok(Json(json!({ "families": fams, "packing": packing })))
}

#[derive(Deserialize)]
struct LineBody {
    key: String,
    #[serde(flatten)]
    state: LineState,
}

async fn set_line(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(mut b): Json<LineBody>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    let fam = Family::of_key(&b.key).ok_or_else(|| one("key", "Ligne inconnue."))?;
    fields(validate_line(fam, &mut b.state))?;
    let (ls, _) = lines(&st.store, &pid).await?;
    if !ls.iter().any(|l| l.key == b.key) {
        return Err(one(
            "key",
            "Ligne inconnue : l'inventaire ou le câblage a changé.",
        ));
    }
    st.store
        .put_prep_line(&pid, &b.key, &b.state, Some(&m.0.account.id))
        .await?;
    st.hub.change(
        room(&pid),
        "prep",
        &b.key,
        "update",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct ModelBody {
    key: String,
    model_id: Option<String>,
}

/// Affecte un modèle de câble à toute une ligne.
async fn cable_model(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<ModelBody>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    if let Some(id) = &b.model_id {
        st.store
            .item(id)
            .await?
            .filter(|i| i.kind == strata_domain::catalog::ItemKind::CableModel)
            .ok_or_else(|| one("model_id", "Type de câble introuvable."))?;
    }
    let (ls, _) = lines(&st.store, &pid).await?;
    let l = ls
        .iter()
        .find(|l| l.key == b.key && l.family == Family::Cables)
        .ok_or_else(|| one("key", "Ligne de câbles inconnue."))?;
    let n = st
        .store
        .set_cable_model(&pid, &l.cables, b.model_id.as_deref())
        .await?;
    st.hub.change(
        room(&pid),
        "prep",
        &b.key,
        "update",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(Json(json!({ "changed": n })))
}

#[derive(Deserialize)]
struct CabledBody {
    cable_id: String,
    done: bool,
    note: Option<String>,
}

async fn cabled(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<CabledBody>,
) -> ApiResult<Json<Value>> {
    write(&m)?;
    st.store
        .cable(&b.cable_id)
        .await?
        .filter(|c| c.project_id == pid)
        .ok_or(ApiError::NotFound("Câble"))?;
    let note = b.note.as_deref().map(str::trim).filter(|n| !n.is_empty());
    if note.is_some_and(|n| n.chars().count() > 1000) {
        return Err(one("note", "1 000 caractères au plus."));
    }
    st.store
        .set_cable_done(&pid, &b.cable_id, b.done, note, Some(&m.0.account.id))
        .await?;
    st.hub.change(
        room(&pid),
        "prep",
        &b.cable_id,
        "cabled",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct ExportQuery {
    format: String,
}

async fn export(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
    Query(q): Query<ExportQuery>,
) -> ApiResult<Response> {
    read(&m)?;
    let p = st
        .store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    let Json(v) = show(State(st.clone()), m, Path(pid.clone())).await?;
    let headers: Vec<String> = [
        "Famille",
        "Désignation",
        "Détail",
        "Minimum",
        "Réserve",
        "Quantité",
        "Approvisionnement",
        "Statut",
        "Note",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let mut rows = Vec::new();
    for f in v["families"].as_array().cloned().unwrap_or_default() {
        let label = |k: &str| {
            f["statuses"]
                .as_array()
                .and_then(|a| a.iter().find(|s| s["key"] == k))
                .and_then(|s| s["label"].as_str())
                .unwrap_or(k)
                .to_string()
        };
        for l in f["lines"].as_array().cloned().unwrap_or_default() {
            let t = |k: &str| Cell::Text(l[k].as_str().unwrap_or("").to_string());
            rows.push(vec![
                Cell::Text(f["label"].as_str().unwrap_or("").into()),
                t("label"),
                t("detail"),
                Cell::Number(l["min"].as_f64()),
                Cell::Number(l["reserve"].as_f64()),
                Cell::Number(l["total"].as_f64()),
                Cell::Text(
                    match l["supply"].as_str() {
                        Some("purchase") => "Achat",
                        Some("rental") => "Location",
                        Some("stock") => "Stock",
                        _ => "",
                    }
                    .into(),
                ),
                Cell::Text(label(l["status"].as_str().unwrap_or(""))),
                t("note"),
            ]);
        }
    }
    let safe: String = p
        .data
        .name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stem = format!(
        "Preparation_{safe}_{}",
        chrono::Local::now().format("%Y-%m-%d")
    );
    match q.format.as_str() {
        "csv" => Ok(file_response(
            to_csv(&headers, &rows).map_err(ApiError::Internal)?,
            "text/csv; charset=utf-8",
            &format!("{stem}.csv"),
        )),
        "xlsx" => Ok(file_response(
            to_xlsx_named(&headers, &rows, "Préparation")
                .map_err(|e| ApiError::Internal(e.to_string()))?,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            &format!("{stem}.xlsx"),
        )),
        _ => Err(ApiError::Invalid("Format d'export : csv ou xlsx.".into())),
    }
}
