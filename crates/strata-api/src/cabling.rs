//! Numérotation des câbles (section 10.7) et validation du câblage (section 10.6).

use crate::AppState;
use crate::auth::Member;
use crate::error::{ApiError, ApiResult};
use crate::live::Origin;
use crate::projects::{fields, one, read, room};
use crate::synoptic::{Node, Port, editing, project_nodes};
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use strata_domain::catalog::RefKind;
use strata_domain::numbering::{
    NumberingRules, Parts, bump, dir_digit, format, rack_code, validate_rules,
};
use strata_domain::synoptic::{Dir, PortInfo, compat};
use strata_domain::{Domain, Level};
use strata_store::Store;

pub const SITE_RULES: &str = "numbering_rules";
const MAX_BATCH: usize = 500;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects/{id}/numbering", post(number))
        .route("/projects/{id}/synoptic/validate", get(validate))
        .route("/site/numbering", get(site_rules).put(set_site_rules))
}

pub(crate) async fn site_rules_of(store: &Store) -> ApiResult<NumberingRules> {
    Ok(store
        .meta(SITE_RULES)
        .await?
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default())
}

async fn site_rules(State(st): State<AppState>, _m: Member) -> ApiResult<Json<NumberingRules>> {
    Ok(Json(site_rules_of(&st.store).await?))
}

async fn set_site_rules(
    State(st): State<AppState>,
    m: Member,
    Json(mut r): Json<NumberingRules>,
) -> ApiResult<Json<NumberingRules>> {
    m.0.require(
        Domain::Parametres,
        Level::Ecriture,
        "Les règles du site demandent « Paramètres : écriture ».",
    )?;
    fields(validate_rules(&mut r))?;
    st.store
        .set_meta(
            SITE_RULES,
            Some(&serde_json::to_string(&r).unwrap_or_default()),
        )
        .await?;
    Ok(Json(r))
}

fn in_view(view: &str, n: &Node) -> bool {
    match view.split_once(':') {
        Some(("container", id)) => n.container_id.as_deref() == Some(id),
        Some(("location", id)) => n.location_id == id,
        _ => true,
    }
}

#[derive(Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum Mode {
    /// Seulement les câbles sans numéro (défaut).
    Missing,
    /// Tout renuméroter dans la portée.
    All,
    /// Les câbles choisis.
    Selection,
    /// Saisie manuelle par lot, à partir d'un premier numéro.
    Manual,
    /// Effacer (tout ou la sélection).
    Clear,
}

#[derive(Deserialize)]
struct NumberBody {
    mode: Mode,
    /// Portée : `container:{id}`, `location:{id}` ; vide : tout le projet.
    view: Option<String>,
    #[serde(default)]
    ids: Vec<String>,
    first: Option<String>,
    /// Réglages ponctuels : priment sur ceux de l'élément et du site.
    rules: Option<NumberingRules>,
}

struct Ref<'a> {
    node: &'a Node,
    port: &'a Port,
    other: Option<(&'a Node, &'a Port)>,
}

async fn number(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(mut b): Json<NumberBody>,
) -> ApiResult<Json<Value>> {
    editing(&st, &m, &pid).await?;
    if let Some(r) = &mut b.rules {
        fields(validate_rules(r))?;
    }
    let nodes = project_nodes(&st.store, &pid).await?;
    let port_of: HashMap<&str, (&Node, &Port)> = nodes
        .iter()
        .flat_map(|n| n.ports.iter().map(move |p| (p.r.as_str(), (n, p))))
        .collect();
    let cables = st.store.cables(&pid).await?;
    let view = b.view.clone().unwrap_or_default();
    let ids: HashSet<&str> = b.ids.iter().map(String::as_str).collect();
    let in_scope = |c: &strata_store::cables::Cable| {
        [&c.a, &c.b].iter().any(|p| {
            port_of
                .get(p.as_str())
                .is_some_and(|(n, _)| in_view(&view, n))
        })
    };
    let chosen: Vec<&strata_store::cables::Cable> = cables
        .iter()
        .filter(|c| !c.fields.attach)
        .filter(|c| match b.mode {
            Mode::Missing => in_scope(c) && c.fields.number.is_none(),
            Mode::All => in_scope(c),
            Mode::Selection | Mode::Manual => ids.contains(c.id.as_str()),
            Mode::Clear => {
                if ids.is_empty() {
                    in_scope(c)
                } else {
                    ids.contains(c.id.as_str())
                }
            }
        })
        .collect();
    if chosen.len() > 5000 || (b.mode == Mode::Manual && chosen.len() > MAX_BATCH) {
        return Err(one("ids", "Saisie manuelle : de 1 à 500 câbles."));
    }
    let mut out: Vec<(String, Option<String>)> = Vec::new();
    let mut counters: HashMap<(String, String, i64), i64> = HashMap::new();
    match b.mode {
        Mode::Clear => out = chosen.iter().map(|c| (c.id.clone(), None)).collect(),
        Mode::Manual => {
            let first = b
                .first
                .as_deref()
                .map(str::trim)
                .filter(|f| !f.is_empty())
                .ok_or_else(|| one("first", "Indiquez le premier numéro, par exemple V-0100."))?;
            if chosen.is_empty() {
                return Err(one("ids", "Choisissez de 1 à 500 câbles."));
            }
            for (k, c) in chosen.iter().enumerate() {
                let n = bump(first, k as u64).ok_or_else(|| {
                    one("first", "Le premier numéro doit finir par des chiffres.")
                })?;
                out.push((c.id.clone(), Some(n)));
            }
        }
        _ => {
            let refs = st.store.list_refs(None).await?;
            let sig = |id: &str| {
                refs.iter()
                    .find(|r| r.kind == RefKind::Signal && r.id == id)
            };
            let fun = |id: &str| {
                refs.iter()
                    .find(|r| r.kind == RefKind::Function && r.id == id)
            };
            let inv = st.store.inventory(&pid).await?;
            let containers: HashMap<&str, &strata_store::Container> =
                inv.containers.iter().map(|c| (c.id.as_str(), c)).collect();
            let site = site_rules_of(&st.store).await?;
            // Extrémité de référence : la sortie d'un équipement, sinon un équipement, sinon A.
            let pick = |c: &strata_store::cables::Cable| -> Option<Ref> {
                let a = port_of.get(c.a.as_str()).copied();
                let bb = port_of.get(c.b.as_str()).copied();
                let score = |e: Option<(&Node, &Port)>| match e {
                    Some((n, p)) if n.kind == "equipment" && p.dir == Dir::Out => 3,
                    Some((n, _)) if n.kind == "equipment" => 2,
                    Some(_) => 1,
                    None => 0,
                };
                let (main, other) = if score(bb) > score(a) {
                    (bb?, a)
                } else {
                    (a?, bb)
                };
                Some(Ref {
                    node: main.0,
                    port: main.1,
                    other,
                })
            };
            struct Job {
                id: String,
                scope: String,
                prefix: String,
                suffix: String,
                dir: u8,
                rack: String,
                rules: NumberingRules,
                key: (String, u8, i64, String, usize),
            }
            let mut jobs = Vec::new();
            for c in &chosen {
                let Some(r) = pick(c) else { continue };
                let cont = r
                    .node
                    .container_id
                    .as_deref()
                    .and_then(|id| containers.get(id));
                // Un port externe se numérote avec son élément ; une liaison entre éléments, au projet.
                let same = c.fields.external.is_some()
                    || r.other
                        .is_some_and(|(n, _)| n.container_id == r.node.container_id);
                let scope = if same {
                    r.node
                        .container_id
                        .clone()
                        .unwrap_or_else(|| format!("loc:{}", r.node.location_id))
                } else {
                    "project".into()
                };
                let rules = b
                    .rules
                    .clone()
                    .or_else(|| cont.and_then(|c| c.data.numbering.clone()))
                    .unwrap_or_else(|| site.clone());
                let signal = r
                    .port
                    .signal
                    .clone()
                    .or_else(|| r.other.and_then(|(_, p)| p.signal.clone()));
                let sref = signal.as_deref().and_then(sig);
                let sdata = |k: &str| {
                    sref.and_then(|s| s.data.get(k))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string()
                };
                let family = sref
                    .and_then(|s| s.data.get("function"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| r.port.family.clone());
                let mut prefix = sdata("prefix");
                if prefix.is_empty() {
                    prefix = family
                        .as_deref()
                        .and_then(fun)
                        .and_then(|f| f.data.get("cable_prefix"))
                        .and_then(Value::as_str)
                        .unwrap_or("X")
                        .to_string();
                }
                let order_y = r.node.spot.map(|s| s.center_mm).unwrap_or(0.0);
                let y = (if rules.top_down { -order_y } else { order_y }) as i64;
                let idx = r
                    .node
                    .ports
                    .iter()
                    .position(|p| p.r == r.port.r)
                    .unwrap_or(0);
                jobs.push(Job {
                    id: c.id.clone(),
                    scope,
                    prefix,
                    suffix: sdata("suffix"),
                    dir: dir_digit(r.port.dir),
                    rack: cont
                        .map(|c| rack_code(&c.data.cable_code, &c.name))
                        .unwrap_or_default(),
                    key: (
                        sref.map(|s| s.name.clone()).unwrap_or_default(),
                        if r.port.side == 'l' { 0 } else { 1 },
                        y,
                        if rules.left_right {
                            r.node.key.clone()
                        } else {
                            format!("~{}", r.node.key)
                        },
                        idx,
                    ),
                    rules,
                });
            }
            jobs.sort_by(|a, b| {
                a.key
                    .partial_cmp(&b.key)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let renumbered: HashSet<&str> = jobs.iter().map(|j| j.id.as_str()).collect();
            let mut used: HashSet<String> = cables
                .iter()
                .filter(|c| !renumbered.contains(c.id.as_str()))
                .filter_map(|c| c.fields.number.clone())
                .collect();
            let mut last = st.store.cable_counters(&pid).await?;
            for j in jobs {
                let k = (j.scope.clone(), j.prefix.clone(), j.dir as i64);
                let mut seq = last.get(&k).copied().unwrap_or(0) + 1;
                let mut s = format(
                    &j.rules,
                    &Parts {
                        rack: &j.rack,
                        prefix: &j.prefix,
                        dir: j.dir,
                        seq: seq as u64,
                        suffix: &j.suffix,
                    },
                );
                while used.contains(&s) {
                    seq += 1;
                    s = format(
                        &j.rules,
                        &Parts {
                            rack: &j.rack,
                            prefix: &j.prefix,
                            dir: j.dir,
                            seq: seq as u64,
                            suffix: &j.suffix,
                        },
                    );
                }
                used.insert(s.clone());
                last.insert(k.clone(), seq);
                counters.insert(k, seq);
                out.push((j.id, Some(s)));
            }
        }
    }
    let n = st
        .store
        .apply_numbers(&pid, &out, &counters.into_iter().collect::<Vec<_>>())
        .await?;
    if n > 0 {
        st.store
            .audit_project(
                &pid,
                Some(&m.0.account.id),
                "number",
                "cable",
                out.first().map(|o| o.0.as_str()).unwrap_or(""),
                None::<&()>,
                Some(&json!({ "cables": n })),
            )
            .await?;
        st.hub.change(
            room(&pid),
            "cable",
            &pid,
            "number",
            None,
            &m.0.account.id,
            origin.0.as_deref(),
        );
    }
    Ok(Json(
        json!({ "changed": n, "numbers": out.iter().map(|(id, n)| json!({ "id": id, "number": n })).collect::<Vec<_>>() }),
    ))
}

// ----- Validation : 11 contrôles -----

pub(crate) struct Issue {
    pub(crate) code: u8,
    pub(crate) severity: &'static str,
    pub(crate) title: &'static str,
    pub(crate) detail: String,
    cables: Vec<String>,
    nodes: Vec<String>,
    view: Option<String>,
}

fn view_of(n: &Node) -> String {
    match &n.container_id {
        Some(c) => format!("container:{c}"),
        None => format!("location:{}", n.location_id),
    }
}

/// Anomalies de câblage du projet (onze contrôles), triées par gravité.
pub(crate) async fn issues_of(store: &Store, pid: &str) -> ApiResult<Vec<Issue>> {
    let pid = pid.to_string();
    let nodes = project_nodes(store, &pid).await?;
    let port_of: HashMap<&str, (&Node, &Port)> = nodes
        .iter()
        .flat_map(|n| n.ports.iter().map(move |p| (p.r.as_str(), (n, p))))
        .collect();
    let cables = store.cables(&pid).await?;
    let inv = store.inventory(&pid).await?;
    let mut out: Vec<Issue> = Vec::new();
    let name = |c: &strata_store::cables::Cable| {
        c.fields
            .number
            .clone()
            .unwrap_or_else(|| "Sans numéro".into())
    };
    let where_ = |c: &strata_store::cables::Cable| {
        [&c.a, &c.b]
            .iter()
            .find_map(|p| port_of.get(p.as_str()))
            .map(|(n, _)| view_of(n))
    };
    let info = |p: &Port| PortInfo {
        connector: p.connector.clone(),
        signal: p.signal.clone(),
        family: p.family.clone(),
        ip: false,
        dir: p.dir,
    };

    let mut uses: HashMap<&str, Vec<String>> = HashMap::new();
    let mut numbers: HashMap<&str, Vec<String>> = HashMap::new();
    for c in &cables {
        // 1. Extrémité manquante
        if !port_of.contains_key(c.a.as_str())
            || (c.fields.external.is_none() && !port_of.contains_key(c.b.as_str()))
        {
            out.push(Issue {
                code: 1,
                severity: "error",
                title: "Extrémité manquante",
                detail: format!("{} vise un équipement ou un port disparu.", name(c)),
                cables: vec![c.id.clone()],
                nodes: vec![],
                view: where_(c),
            });
        }
        for p in [&c.a, &c.b] {
            uses.entry(p.as_str()).or_default().push(c.id.clone());
        }
        if let Some(n) = &c.fields.number {
            numbers.entry(n.as_str()).or_default().push(c.id.clone());
        }
        // 4. Incompatibilité (familles, à défaut connecteurs ; hors IP et transit) ; pour une
        // attache d'adaptateur, connecteur et genres.
        if c.fields.attach {
            let w = crate::synoptic::cable_warnings(
                port_of.get(c.a.as_str()).copied(),
                port_of.get(c.b.as_str()).copied(),
                &c.fields,
            );
            if let Some(m) = w.into_iter().find(|m| !m.starts_with("Extrémité")) {
                out.push(Issue {
                    code: 4,
                    severity: "error",
                    title: "Attache incompatible",
                    detail: m,
                    cables: vec![c.id.clone()],
                    nodes: vec![],
                    view: where_(c),
                });
            }
        } else if let (Some((_, pa)), Some((_, pb))) =
            (port_of.get(c.a.as_str()), port_of.get(c.b.as_str()))
        {
            let mut a = info(pa);
            let mut bb = info(pb);
            a.dir = Dir::Bidir;
            bb.dir = Dir::Bidir;
            a.ip = pa.ip;
            bb.ip = pb.ip;
            let w: Vec<String> = compat(&a, &bb)
                .into_iter()
                .filter(|m| !m.starts_with("Deux"))
                .collect();
            if !w.is_empty() {
                out.push(Issue {
                    code: 4,
                    severity: "error",
                    title: "Incompatibilité",
                    detail: format!("{} : {}", name(c), w.join(" ")),
                    cables: vec![c.id.clone()],
                    nodes: vec![],
                    view: where_(c),
                });
            }
        }
        // 5. Câble sans numéro
        if c.fields.number.is_none() && !c.fields.attach {
            out.push(Issue {
                code: 5,
                severity: "warning",
                title: "Câble sans numéro",
                detail: [&c.a, &c.b]
                    .iter()
                    .map(|p| {
                        port_of
                            .get(p.as_str())
                            .map(|(n, p)| format!("{} › {}", n.label, p.label))
                            .unwrap_or_else(|| "?".into())
                    })
                    .collect::<Vec<_>>()
                    .join(" → "),
                cables: vec![c.id.clone()],
                nodes: vec![],
                view: where_(c),
            });
        }
    }
    // 2. Numéro en double
    for (n, ids) in &numbers {
        if ids.len() > 1 {
            out.push(Issue {
                code: 2,
                severity: "error",
                title: "Numéro en double",
                detail: format!("{n} sur {} câbles.", ids.len()),
                cables: ids.clone(),
                nodes: vec![],
                view: None,
            });
        }
    }
    // 3. Port surutilisé
    for (p, ids) in &uses {
        if ids.len() > 1 {
            let label = port_of
                .get(p)
                .map(|(n, pp)| format!("{} › {}", n.label, pp.label))
                .unwrap_or_else(|| p.to_string());
            out.push(Issue {
                code: 3,
                severity: "error",
                title: "Port surutilisé",
                detail: format!("{label} porte {} câbles.", ids.len()),
                cables: ids.clone(),
                nodes: vec![],
                view: None,
            });
        }
    }
    // 6. Emplacement vide
    for l in &inv.locations {
        if !nodes
            .iter()
            .any(|n| n.location_id == l.id && (n.kind == "equipment" || n.kind == "panel"))
        {
            out.push(Issue {
                code: 6,
                severity: "info",
                title: "Emplacement vide",
                detail: format!("{} : ni équipement ni panel.", l.name),
                cables: vec![],
                nodes: vec![],
                view: Some(format!("location:{}", l.id)),
            });
        }
    }
    let cabled = |n: &Node| n.ports.iter().any(|p| uses.contains_key(p.r.as_str()));
    for n in &nodes {
        match n.kind {
            // 7. Équipement non câblé
            "equipment" if !n.ports.is_empty() && !cabled(n) => out.push(Issue {
                code: 7,
                severity: "info",
                title: "Équipement non câblé",
                detail: format!("{} ({})", n.label, n.sub),
                cables: vec![],
                nodes: vec![n.key.clone()],
                view: Some(view_of(n)),
            }),
            // 8. Panel non ou mal câblé
            "panel" if !n.ports.is_empty() => {
                if !cabled(n) {
                    out.push(Issue {
                        code: 8,
                        severity: "info",
                        title: "Panel non câblé",
                        detail: n.label.clone(),
                        cables: vec![],
                        nodes: vec![n.key.clone()],
                        view: Some(view_of(n)),
                    });
                } else {
                    let rears: Vec<&Port> = n.ports.iter().filter(|p| p.side == 'l').collect();
                    let half: Vec<String> = rears
                        .iter()
                        .filter_map(|r| {
                            let front = r.r.trim_end_matches(":r").to_string() + ":f";
                            (uses.contains_key(r.r.as_str()) != uses.contains_key(front.as_str()))
                                .then(|| r.label.clone())
                        })
                        .collect();
                    if !half.is_empty() {
                        out.push(Issue {
                            code: 8,
                            severity: "warning",
                            title: "Panel mal câblé",
                            detail: format!(
                                "{} : un seul côté câblé ({}).",
                                n.label,
                                half.join(", ")
                            ),
                            cables: vec![],
                            nodes: vec![n.key.clone()],
                            view: Some(view_of(n)),
                        });
                    }
                }
            }
            // 9. Adaptateur orphelin
            "adapter" if !cabled(n) => out.push(Issue {
                code: 9,
                severity: "info",
                title: "Adaptateur orphelin",
                detail: format!("{} ({})", n.label, n.sub),
                cables: vec![],
                nodes: vec![n.key.clone()],
                view: Some(view_of(n)),
            }),
            _ => {}
        }
    }
    // 10. Zone sans nom ; 11. Zone vide (aucun nœud dont la position enregistrée y tombe).
    for z in store
        .annotations(&pid, None)
        .await?
        .into_iter()
        .filter(|a| a.kind == "zone")
    {
        let f = |k: &str| z.data[k].as_f64().unwrap_or(0.0);
        let name = z.data["name"].as_str().unwrap_or("").to_string();
        let view = Some(z.view.clone()).filter(|v| v != "overview");
        if name.is_empty() {
            out.push(Issue {
                code: 10,
                severity: "info",
                title: "Zone sans nom",
                detail: "Une zone n'a pas de nom.".into(),
                cables: vec![],
                nodes: vec![],
                view: view.clone(),
            });
        }
        let layout = store.layout(&pid, &z.view).await?;
        let inside = layout.values().any(|(x, y)| {
            *x >= f("x") && *x <= f("x") + f("w") && *y >= f("y") && *y <= f("y") + f("h")
        });
        if !inside {
            out.push(Issue {
                code: 11,
                severity: "info",
                title: "Zone vide",
                detail: format!(
                    "{} : aucun nœud dedans.",
                    if name.is_empty() {
                        "Zone sans nom"
                    } else {
                        name.as_str()
                    }
                ),
                cables: vec![],
                nodes: vec![],
                view,
            });
        }
    }
    out.sort_by_key(|i| {
        (
            match i.severity {
                "error" => 0,
                "warning" => 1,
                _ => 2,
            },
            i.code,
        )
    });
    Ok(out)
}

async fn validate(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    st.store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    let out = issues_of(&st.store, &pid).await?;
    let count = |s: &str| out.iter().filter(|i| i.severity == s).count();
    Ok(Json(json!({
        "counts": { "error": count("error"), "warning": count("warning"), "info": count("info") },
        "issues": out.iter().map(|i| json!({ "code": i.code, "severity": i.severity, "title": i.title, "detail": i.detail, "cables": i.cables, "nodes": i.nodes, "view": i.view })).collect::<Vec<_>>(),
    })))
}
