//! Mode tableau des câbles (section 10.9) et liste des câbles exportée (section 10.8).
//!
//! Tous les câbles du projet, avec leurs deux extrémités et le signal. Écran collaboratif :
//! la saisie en série (numéros, couleurs, notes) est diffusée en direct ; chaque ligne garde
//! sa révision, une ligne modifiée entre-temps par quelqu'un d'autre est rendue en conflit.

use crate::AppState;
use crate::auth::Member;
use crate::catalog_io::{Cell, file_response, to_csv, to_xlsx_named};
use crate::error::{ApiError, ApiResult};
use crate::live::Origin;
use crate::projects::{one, read, room};
use crate::synoptic::{Node, Port, cable_warnings, estimate, project_nodes};
use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use strata_domain::catalog::RefKind;
use strata_domain::synoptic::validate_cable;
use strata_domain::{Domain, Level};
use strata_store::{Store, StoreError};

const DESIGN: &str = "Vous n'avez pas le droit de modifier la conception.";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects/{id}/cables/table", get(table))
        .route("/projects/{id}/cables/bulk", post(bulk))
        .route("/projects/{id}/cables/export", get(export))
}

#[derive(Debug, Clone, Serialize)]
struct End {
    port_ref: String,
    element: String,
    container_id: Option<String>,
    location: String,
    location_id: Option<String>,
    equipment: String,
    tag: String,
    port: String,
    connector: String,
    /// L'équipement ou le port visé n'existe plus.
    missing: bool,
}

#[derive(Debug, Clone, Serialize)]
struct Row {
    id: String,
    rev: i64,
    number: Option<String>,
    color: Option<String>,
    notes: Option<String>,
    length_m: Option<f64>,
    estimate_m: Option<f64>,
    signal: String,
    family: Option<String>,
    a: End,
    b: End,
    warnings: Vec<String>,
}

async fn rows(store: &Store, pid: &str) -> ApiResult<Vec<Row>> {
    let nodes = project_nodes(store, pid).await?;
    let port_of: HashMap<&str, (&Node, &Port)> = nodes
        .iter()
        .flat_map(|n| n.ports.iter().map(move |p| (p.r.as_str(), (n, p))))
        .collect();
    let inv = store.inventory(pid).await?;
    let routes = crate::plans::route_lengths(store, pid).await?;
    let containers: HashMap<&str, &strata_store::Container> =
        inv.containers.iter().map(|c| (c.id.as_str(), c)).collect();
    let locations: HashMap<&str, &str> = inv
        .locations
        .iter()
        .map(|l| (l.id.as_str(), l.name.as_str()))
        .collect();
    let refs = store.list_refs(None).await?;
    let name_of = |k: RefKind, id: &str| {
        refs.iter()
            .find(|r| r.kind == k && r.id == id)
            .map(|r| r.name.clone())
            .unwrap_or_else(|| id.to_string())
    };
    let end = |r: &str| match port_of.get(r) {
        Some((n, p)) => End {
            port_ref: r.to_string(),
            element: n
                .container_id
                .as_deref()
                .and_then(|c| containers.get(c))
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "Équipement libre".into()),
            container_id: n.container_id.clone(),
            location: locations
                .get(n.location_id.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default(),
            location_id: Some(n.location_id.clone()),
            equipment: n.label.clone(),
            tag: n.sub.rsplit(" · ").next().unwrap_or("").to_string(),
            port: p.label.clone(),
            connector: name_of(RefKind::Connector, &p.connector),
            missing: false,
        },
        None => End {
            port_ref: r.to_string(),
            element: String::new(),
            container_id: None,
            location: String::new(),
            location_id: None,
            equipment: "Extrémité supprimée".into(),
            tag: String::new(),
            port: String::new(),
            connector: String::new(),
            missing: true,
        },
    };
    let mut out = Vec::new();
    for c in store
        .cables(pid)
        .await?
        .into_iter()
        .filter(|c| !c.fields.attach)
    {
        let ea = port_of.get(c.a.as_str()).copied();
        let eb = port_of.get(c.b.as_str()).copied();
        let sig = ea
            .and_then(|(_, p)| p.signal.clone())
            .or_else(|| eb.and_then(|(_, p)| p.signal.clone()));
        let family = ea
            .and_then(|(_, p)| p.family.clone())
            .or_else(|| eb.and_then(|(_, p)| p.family.clone()));
        out.push(Row {
            id: c.id.clone(),
            rev: c.rev,
            number: c.fields.number.clone(),
            color: c.fields.color.clone(),
            notes: c.fields.notes.clone(),
            length_m: c.fields.length_m,
            estimate_m: estimate(ea.as_ref(), eb.as_ref(), &containers, &routes),
            signal: sig
                .as_deref()
                .map(|s| name_of(RefKind::Signal, s))
                .unwrap_or_default(),
            family,
            a: end(&c.a),
            b: match &c.fields.external {
                Some(x) => End {
                    port_ref: c.b.clone(),
                    element: "Port externe".into(),
                    container_id: None,
                    location: x
                        .location_id
                        .as_deref()
                        .and_then(|l| locations.get(l))
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "Hors projet".into()),
                    location_id: x.location_id.clone(),
                    equipment: match (&x.location_id, &x.label) {
                        (Some(l), _) => format!(
                            "→ {}",
                            locations
                                .get(l.as_str())
                                .copied()
                                .unwrap_or("Lieu supprimé")
                        ),
                        (None, Some(t)) => format!("→ {t}"),
                        _ => "→ Hors projet".into(),
                    },
                    tag: String::new(),
                    port: String::new(),
                    connector: String::new(),
                    missing: false,
                },
                None => end(&c.b),
            },
            warnings: cable_warnings(ea, eb, &c.fields),
        });
    }
    // Ordre de lecture : numéro, puis câbles sans numéro.
    out.sort_by(|x, y| match (&x.number, &y.number) {
        (Some(a), Some(b)) => natord(a, b),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        _ => {
            x.a.element
                .cmp(&y.a.element)
                .then(x.a.equipment.cmp(&y.a.equipment))
        }
    });
    Ok(out)
}

/// Tri « naturel » : V-2 avant V-10.
fn natord(a: &str, b: &str) -> std::cmp::Ordering {
    let key = |s: &str| {
        let mut parts: Vec<(String, u64)> = Vec::new();
        let mut txt = String::new();
        let mut num = String::new();
        for c in s.chars() {
            if c.is_ascii_digit() {
                num.push(c);
            } else {
                if !num.is_empty() {
                    parts.push((std::mem::take(&mut txt), num.parse().unwrap_or(0)));
                    num.clear();
                }
                txt.push(c);
            }
        }
        parts.push((txt, num.parse().unwrap_or(0)));
        parts
    };
    key(a).cmp(&key(b))
}

async fn table(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    st.store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    Ok(Json(json!({ "rows": rows(&st.store, &pid).await? })))
}

#[derive(Deserialize)]
struct Change {
    id: String,
    rev: i64,
    /// Champs modifiés : `number`, `color`, `notes`, `length_m` ; `null` efface.
    #[serde(flatten)]
    set: serde_json::Map<String, Value>,
}

#[derive(Deserialize)]
struct BulkBody {
    changes: Vec<Change>,
}

async fn bulk(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<BulkBody>,
) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    if b.changes.is_empty() || b.changes.len() > 500 {
        return Err(one("changes", "De 1 à 500 lignes par envoi."));
    }
    let mut updated = Vec::new();
    let mut conflicts = Vec::new();
    for ch in b.changes {
        let Some(c) = st
            .store
            .cable(&ch.id)
            .await?
            .filter(|c| c.project_id == pid)
        else {
            conflicts.push(json!({ "id": ch.id, "reason": "gone" }));
            continue;
        };
        let mut f = c.fields.clone();
        for (k, v) in &ch.set {
            let txt = || v.as_str().map(str::to_string);
            match k.as_str() {
                "number" => f.number = txt(),
                "color" => f.color = txt(),
                "notes" => f.notes = txt(),
                "length_m" => f.length_m = v.as_f64(),
                _ => {}
            }
        }
        let errs = validate_cable(&mut f);
        if !errs.is_empty() {
            return Err(ApiError::Fields(
                errs.into_iter()
                    .map(|mut e| {
                        e.field = format!("{}.{}", ch.id, e.field);
                        e
                    })
                    .collect(),
            ));
        }
        match st.store.update_cable(&ch.id, ch.rev, &f).await {
            Ok(after) => updated.push(json!({ "id": after.id, "rev": after.rev })),
            Err(StoreError::Stale) => conflicts.push(json!({ "id": ch.id, "reason": "stale" })),
            Err(e) => return Err(e.into()),
        }
    }
    if !updated.is_empty() {
        st.store
            .audit_project(
                &pid,
                Some(&m.0.account.id),
                "update",
                "cable",
                "table",
                None::<&()>,
                Some(&json!({ "cables": updated.len() })),
            )
            .await?;
        st.hub.change(
            room(&pid),
            "cable",
            &pid,
            "update",
            None,
            &m.0.account.id,
            origin.0.as_deref(),
        );
    }
    Ok(Json(json!({ "updated": updated, "conflicts": conflicts })))
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
    let list = rows(&st.store, &pid).await?;
    let headers: Vec<String> = [
        "Numéro",
        "Signal",
        "De : élément",
        "De : équipement",
        "De : repère",
        "De : port",
        "De : connecteur",
        "Vers : élément",
        "Vers : équipement",
        "Vers : repère",
        "Vers : port",
        "Vers : connecteur",
        "Longueur (m)",
        "Estimée (m)",
        "Couleur",
        "Notes",
        "À vérifier",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let t = |s: &str| Cell::Text(s.to_string());
    let data: Vec<Vec<Cell>> = list
        .iter()
        .map(|r| {
            vec![
                t(r.number.as_deref().unwrap_or("")),
                t(&r.signal),
                t(&r.a.element),
                t(&r.a.equipment),
                t(&r.a.tag),
                t(&r.a.port),
                t(&r.a.connector),
                t(&r.b.element),
                t(&r.b.equipment),
                t(&r.b.tag),
                t(&r.b.port),
                t(&r.b.connector),
                Cell::Number(r.length_m),
                Cell::Number(r.estimate_m),
                t(r.color.as_deref().unwrap_or("")),
                t(r.notes.as_deref().unwrap_or("")),
                t(&r.warnings.join(" ")),
            ]
        })
        .collect();
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
    let stem = format!("Cables_{safe}_{}", chrono::Local::now().format("%Y-%m-%d"));
    match q.format.as_str() {
        "csv" => Ok(file_response(
            to_csv(&headers, &data).map_err(ApiError::Internal)?,
            "text/csv; charset=utf-8",
            &format!("{stem}.csv"),
        )),
        "xlsx" => Ok(file_response(
            to_xlsx_named(&headers, &data, "Câbles")
                .map_err(|e| ApiError::Internal(e.to_string()))?,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            &format!("{stem}.xlsx"),
        )),
        _ => Err(ApiError::Invalid("Format d'export : csv ou xlsx.".into())),
    }
}
