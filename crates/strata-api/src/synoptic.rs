//! Synoptique (section 10) : nœuds et ports d'une vue, câbles, disposition, verrou.
//!
//! Vues : `container:{id}` (un élément) et `location:{id}` (un lieu : ses éléments et ses
//! équipements libres). Nœuds : chaque exemplaire d'équipement, chaque rangée de panel, chaque
//! unité d'adaptateur. Un câble dont une seule extrémité est dans la vue y figure comme
//! départ vers l'extérieur. Écran de conception : une seule personne modifie à la fois.

use crate::AppState;
use crate::auth::Member;
use crate::error::{ApiError, ApiResult};
use crate::live::Origin;
use crate::projects::{fields, one, read, record, room};
use crate::racks::LOCK_TTL_SECS;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use strata_domain::catalog::RefKind;
use strata_domain::project::{Accessory, ArticleNature};
use strata_domain::rack::RackFace;
use strata_domain::synoptic::{
    CableFields, Dir, ExternalEnd, PortInfo, PortRef, RackSpot, attach_ok, compat,
    estimate_in_rack, validate_cable,
};
use strata_domain::{Domain, Level};
use strata_store::cables::{NewCable, PlaceCables};
use strata_store::{LockOutcome, Store};

const DESIGN: &str = "Vous n'avez pas le droit de modifier la conception.";
const MAX_BATCH: usize = 500;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects/{id}/synoptic", get(show))
        .route("/projects/{id}/synoptic/layout", put(layout))
        .route("/projects/{id}/synoptic/lock", post(take).delete(release))
        .route("/projects/{id}/synoptic/lock/request", post(request))
        .route("/projects/{id}/cables", post(create))
        .route("/projects/{id}/cables/delete", post(remove))
        .route("/cables/{id}", patch(update))
        .route("/projects/{id}/external", post(external))
        .route("/projects/{id}/attach", post(attach))
        .route("/projects/{id}/harnesses", post(harness_create))
        .route("/projects/{id}/harnesses/assign", post(harness_assign))
        .route(
            "/harnesses/{id}",
            patch(harness_update).delete(harness_delete),
        )
        .route("/projects/{id}/annotations", post(annotation_create))
        .route("/projects/{id}/annotations/delete", post(annotation_delete))
        .route("/annotations/{id}", patch(annotation_update))
}

fn lock_key(pid: &str) -> String {
    format!("syn:{pid}")
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Port {
    #[serde(rename = "ref")]
    pub r: String,
    pub label: String,
    pub connector: String,
    pub signal: Option<String>,
    pub family: Option<String>,
    pub dir: Dir,
    pub gender: Option<String>,
    pub variant: Option<String>,
    /// Côté du nœud : entrées à gauche, sorties et bidirectionnels à droite ; un panel
    /// montre sa face arrière à gauche et sa face avant à droite.
    pub side: char,
    #[serde(skip)]
    pub ip: bool,
    /// Port de commutation (switch) : sert à dessiner la carte réseau.
    #[serde(skip)]
    pub switching: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Node {
    pub key: String,
    pub kind: &'static str,
    pub label: String,
    pub sub: String,
    pub function: Option<String>,
    pub container_id: Option<String>,
    pub location_id: String,
    pub ports: Vec<Port>,
    /// Position dans le rack (centre en mm depuis le bas, face avant), pour les longueurs.
    #[serde(skip)]
    pub spot: Option<RackSpot>,
}

impl Port {
    fn info(&self) -> PortInfo {
        PortInfo {
            connector: self.connector.clone(),
            signal: self.signal.clone(),
            family: self.family.clone(),
            ip: self.ip,
            dir: self.dir,
        }
    }
}

/// Tous les nœuds du projet et leurs ports, tirés de l'inventaire et du catalogue.
pub(crate) async fn project_nodes(store: &Store, pid: &str) -> ApiResult<Vec<Node>> {
    let inv = store.inventory(pid).await?;
    let refs = store.list_refs(None).await?;
    let sig: HashMap<&str, (Option<String>, bool, String)> = refs
        .iter()
        .filter(|r| r.kind == RefKind::Signal)
        .map(|r| {
            (
                r.id.as_str(),
                (
                    r.data
                        .get("function")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    r.data.get("ip").and_then(Value::as_bool).unwrap_or(false),
                    r.name.clone(),
                ),
            )
        })
        .collect();
    let conn_name = |id: &str| {
        refs.iter()
            .find(|r| r.kind == RefKind::Connector && r.id == id)
            .map(|r| r.name.clone())
            .unwrap_or_else(|| id.to_string())
    };
    let containers: HashMap<&str, &strata_store::Container> =
        inv.containers.iter().map(|c| (c.id.as_str(), c)).collect();
    let mut placements = Vec::new();
    for c in &inv.containers {
        if c.data.rackable {
            placements.extend(store.placements(&c.id).await?);
        }
    }
    let spot = |article: &str, instance: Option<&str>, h: f64| {
        placements
            .iter()
            .find(|p| {
                p.article_id == article
                    && p.instance_id.as_deref() == instance
                    && p.shelf_id.is_none()
            })
            .map(|p| RackSpot {
                center_mm: ((p.u_start - 1) as f64 + h / 2.0) * 44.45,
                front: p.face == RackFace::Front,
            })
    };
    let mut items: HashMap<String, strata_store::CatalogItem> = HashMap::new();
    for a in &inv.articles {
        if let Some(id) = &a.item_id {
            if !items.contains_key(id) {
                if let Some(it) = store.item(id).await? {
                    items.insert(id.clone(), it);
                }
            }
        }
    }
    // Modules glissés dans un châssis : leurs ports rejoignent le nœud du châssis.
    let mut modules_of: HashMap<&str, Vec<&strata_store::Article>> = HashMap::new();
    for a in inv.articles.iter() {
        if let Some(h) = &a.host {
            modules_of.entry(h.article_id.as_str()).or_default().push(a);
        }
    }
    for v in modules_of.values_mut() {
        v.sort_by_key(|a| a.host.as_ref().map(|h| h.slot).unwrap_or(0));
    }
    let mut out = Vec::new();
    for a in &inv.articles {
        if a.host.is_some() {
            continue;
        }
        let where_ = a
            .container_id
            .as_deref()
            .and_then(|c| containers.get(c))
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "Équipement libre".into());
        match (a.nature, &a.accessory) {
            (ArticleNature::Equipment, _) => {
                let Some(it) = a.item_id.as_ref().and_then(|i| items.get(i)) else {
                    continue;
                };
                let groups: Vec<strata_domain::catalog::PortGroup> = it
                    .data
                    .get("ports")
                    .cloned()
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default();
                let function = it
                    .data
                    .get("functions")
                    .and_then(|f| f.get(0))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let h = it
                    .data
                    .get("format")
                    .and_then(|f| f.get("height_u"))
                    .and_then(Value::as_f64)
                    .unwrap_or(1.0);
                let item_groups = |id: &str| -> Vec<strata_domain::catalog::PortGroup> {
                    items
                        .get(id)
                        .and_then(|it| it.data.get("ports").cloned())
                        .and_then(|v| serde_json::from_value(v).ok())
                        .unwrap_or_default()
                };
                for inst in inv.instances.iter().filter(|i| i.article_id == a.id) {
                    // Le châssis d'abord, puis chaque module dans l'ordre des emplacements.
                    let mut sources: Vec<(String, Vec<strata_domain::catalog::PortGroup>, String)> =
                        vec![(inst.id.clone(), groups.clone(), String::new())];
                    for mo in modules_of
                        .get(a.id.as_str())
                        .map(|v| v.as_slice())
                        .unwrap_or(&[])
                    {
                        let Some(mi) = inv.instances.iter().find(|i| i.article_id == mo.id) else {
                            continue;
                        };
                        let slot = mo.host.as_ref().map(|h| h.slot).unwrap_or(0);
                        sources.push((
                            mi.id.clone(),
                            item_groups(mo.item_id.as_deref().unwrap_or_default()),
                            format!("S{slot} · "),
                        ));
                    }
                    let mut ports = Vec::new();
                    for (src_inst, groups, prefix) in &sources {
                        for (g, pg) in groups.iter().enumerate() {
                            let (family, ip, sname) = pg
                                .signal_id
                                .as_deref()
                                .and_then(|s| sig.get(s).cloned())
                                .unwrap_or((None, false, String::new()));
                            let base = pg
                                .label
                                .clone()
                                .filter(|l| !l.trim().is_empty())
                                .unwrap_or_else(|| {
                                    if sname.is_empty() {
                                        conn_name(&pg.connector_id)
                                    } else {
                                        sname.clone()
                                    }
                                });
                            let q = pg.quantity.clamp(1, 1024);
                            for n in 1..=q {
                                let dir = match pg.direction {
                                    strata_domain::catalog::Direction::In => Dir::In,
                                    strata_domain::catalog::Direction::Out => Dir::Out,
                                    strata_domain::catalog::Direction::Bidir => Dir::Bidir,
                                };
                                ports.push(Port {
                                    r: PortRef::Instance {
                                        instance: src_inst.clone(),
                                        group: g as u32,
                                        n,
                                    }
                                    .to_string(),
                                    label: if q > 1 {
                                        format!("{prefix}{base} {n}")
                                    } else {
                                        format!("{prefix}{base}")
                                    },
                                    connector: pg.connector_id.clone(),
                                    signal: pg.signal_id.clone(),
                                    family: family.clone(),
                                    dir,
                                    gender: Some(format!("{:?}", pg.gender)),
                                    variant: pg.variant.clone(),
                                    side: if dir == Dir::In { 'l' } else { 'r' },
                                    ip,
                                    switching: pg.network_role
                                        == Some(strata_domain::catalog::NetworkRole::Switching),
                                });
                            }
                        }
                    }
                    out.push(Node {
                        key: format!("i:{}", inst.id),
                        kind: "equipment",
                        label: it.name.clone(),
                        sub: format!("{where_} · {}", inst.tag),
                        function: function.clone(),
                        container_id: a.container_id.clone(),
                        location_id: a.location_id.clone(),
                        ports,
                        spot: spot(&a.id, Some(&inst.id), h),
                    });
                }
            }
            (_, Some(Accessory::Panel { panel })) => {
                for row in 1..=panel.height_u {
                    let mut ports = Vec::new();
                    for s in panel
                        .slots
                        .iter()
                        .filter(|s| (s.index - 1) / panel.per_u + 1 == row)
                    {
                        let label = if s.label.is_empty() {
                            format!("{}", s.index)
                        } else {
                            s.label.clone()
                        };
                        for rear in [true, false] {
                            ports.push(Port {
                                r: PortRef::Panel {
                                    article: a.id.clone(),
                                    slot: s.index,
                                    rear,
                                }
                                .to_string(),
                                label: label.clone(),
                                connector: s.connector.clone(),
                                signal: None,
                                family: s.function.clone(),
                                dir: Dir::Through,
                                gender: Some("F".into()),
                                variant: s.variant.clone(),
                                side: if rear { 'l' } else { 'r' },
                                ip: false,
                                switching: false,
                            });
                        }
                    }
                    let mut sp = spot(&a.id, None, panel.height_u as f64);
                    if let Some(s) = &mut sp {
                        s.center_mm += ((panel.height_u - row) as f64
                            - (panel.height_u as f64 - 1.0) / 2.0)
                            * 44.45;
                    }
                    out.push(Node {
                        key: format!("p:{}:{row}", a.id),
                        kind: "panel",
                        label: if panel.height_u > 1 {
                            format!("{} · rangée {row}", panel.name)
                        } else {
                            panel.name.clone()
                        },
                        sub: format!("{where_} · arrière ← → avant"),
                        function: None,
                        container_id: a.container_id.clone(),
                        location_id: a.location_id.clone(),
                        ports,
                        spot: sp,
                    });
                }
            }
            (ArticleNature::Adapter, _) => {
                let Some(it) = a.item_id.as_ref().and_then(|i| items.get(i)) else {
                    continue;
                };
                let ad: strata_domain::catalog::Adapter =
                    serde_json::from_value(it.data.clone()).unwrap_or_default();
                for unit in 1..=a.quantity.clamp(1, 200) as u32 {
                    let mut ports = vec![Port {
                        r: PortRef::Adapter {
                            article: a.id.clone(),
                            unit,
                            out: None,
                        }
                        .to_string(),
                        label: ad
                            .input
                            .label
                            .clone()
                            .unwrap_or_else(|| conn_name(&ad.input.connector_id)),
                        connector: ad.input.connector_id.clone(),
                        signal: None,
                        family: ad.function.clone(),
                        dir: Dir::Through,
                        gender: Some(ad.input.gender.letter().into()),
                        variant: None,
                        side: 'l',
                        ip: false,
                        switching: false,
                    }];
                    for (k, o) in ad.outputs.iter().enumerate() {
                        for n in 1..=o.quantity.clamp(1, 64) {
                            let base = o
                                .label
                                .clone()
                                .unwrap_or_else(|| conn_name(&o.connector_id));
                            ports.push(Port {
                                r: PortRef::Adapter {
                                    article: a.id.clone(),
                                    unit,
                                    out: Some((k as u32, n)),
                                }
                                .to_string(),
                                label: if o.quantity > 1 {
                                    format!("{base} {n}")
                                } else {
                                    base
                                },
                                connector: o.connector_id.clone(),
                                signal: None,
                                family: ad.function.clone(),
                                dir: Dir::Through,
                                gender: Some(o.gender.letter().into()),
                                variant: None,
                                side: 'r',
                                ip: false,
                                switching: false,
                            });
                        }
                    }
                    out.push(Node {
                        key: format!("a:{}:{unit}", a.id),
                        kind: "adapter",
                        label: it.name.clone(),
                        sub: format!("{where_} · unité {unit}"),
                        function: ad.function.clone(),
                        container_id: a.container_id.clone(),
                        location_id: a.location_id.clone(),
                        ports,
                        spot: None,
                    });
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

#[derive(Deserialize)]
struct ViewQuery {
    view: Option<String>,
}

fn in_view(view: &str, n: &Node) -> bool {
    match view.split_once(':') {
        Some(("container", id)) => n.container_id.as_deref() == Some(id),
        Some(("location", id)) => n.location_id == id,
        _ => false,
    }
}

/// Constat d'un câble dont une extrémité (ou les deux) ne correspond plus à aucun port.
pub(crate) fn missing_end(a_ok: bool, b_ok: bool) -> String {
    if !a_ok && !b_ok {
        "Les deux extrémités n'existent plus (équipement ou port supprimé).".into()
    } else {
        "L'autre extrémité n'existe plus (équipement ou port supprimé).".into()
    }
}

/// Avertissements d'un câble : extrémité disparue, compatibilité (ou règle d'attache).
pub(crate) fn cable_warnings(
    ea: Option<(&Node, &Port)>,
    eb: Option<(&Node, &Port)>,
    f: &CableFields,
) -> Vec<String> {
    let mut w = Vec::new();
    if f.external.is_some() {
        if ea.is_none() {
            w.push(missing_end(false, true));
        }
        return w;
    }
    if ea.is_none() || eb.is_none() {
        w.push(missing_end(ea.is_some(), eb.is_some()));
    }
    if let (Some((_, pa)), Some((_, pb))) = (ea, eb) {
        if f.attach {
            let (ad, port) = if pa.r.starts_with("a:") {
                (pa, pb)
            } else {
                (pb, pa)
            };
            if let Err(e) = attach_ok(
                &port.connector,
                port.gender.as_deref(),
                &ad.connector,
                ad.gender.as_deref().unwrap_or("N"),
            ) {
                w.push(e);
            }
        } else {
            w.extend(compat(&pa.info(), &pb.info()));
        }
    }
    w.dedup();
    w
}

/// Longueur estimée quand les deux extrémités sont posées dans le même rack.
pub(crate) fn estimate(
    ea: Option<&(&Node, &Port)>,
    eb: Option<&(&Node, &Port)>,
    containers: &HashMap<&str, &strata_store::Container>,
    routes: &HashMap<String, f64>,
) -> Option<f64> {
    let ((na, pa), (nb, pb)) = (ea?, eb?);
    // Entre deux contenants : la longueur du tracé sur la carte, s'il existe.
    if let (Some(a), Some(b)) = (&na.container_id, &nb.container_id) {
        if a != b {
            let key = if a < b {
                format!("{a}|{b}")
            } else {
                format!("{b}|{a}")
            };
            return routes.get(&key).copied();
        }
    }
    if na.container_id.is_none() || na.container_id != nb.container_id {
        return None;
    }
    let ct = containers.get(na.container_id.as_deref()?)?;
    Some(estimate_in_rack(
        na.spot?,
        nb.spot?,
        ct.data.depth_mm.unwrap_or(600.0),
        &pa.connector,
        &pb.connector,
    ))
}

async fn show(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
    Query(q): Query<ViewQuery>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    st.store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    let inv = st.store.inventory(&pid).await?;
    let view = q.view.unwrap_or_else(|| {
        inv.containers
            .first()
            .map(|c| format!("container:{}", c.id))
            .or_else(|| inv.locations.first().map(|l| format!("location:{}", l.id)))
            .unwrap_or_default()
    });
    let nodes = project_nodes(&st.store, &pid).await?;
    let port_of: HashMap<&str, (&Node, &Port)> = nodes
        .iter()
        .flat_map(|n| n.ports.iter().map(move |p| (p.r.as_str(), (n, p))))
        .collect();
    let all_cables = st.store.cables(&pid).await?;
    let loc_name: HashMap<&str, &str> = inv
        .locations
        .iter()
        .map(|l| (l.id.as_str(), l.name.as_str()))
        .collect();
    // Port externe « résolu » : le lieu visé a un port externe qui revient vers le lieu d'origine.
    let ext_pairs: HashSet<(String, String)> = all_cables
        .iter()
        .filter_map(|c| {
            let to = c.fields.external.as_ref()?.location_id.clone()?;
            let (n, _) = port_of.get(c.a.as_str())?;
            Some((n.location_id.clone(), to))
        })
        .collect();
    let overview = if view == "overview" {
        Some(overview(&st.store, &pid, &nodes, &port_of, &all_cables, &inv).await?)
    } else {
        None
    };
    let shown: Vec<&Node> = nodes.iter().filter(|n| in_view(&view, n)).collect();
    let keys: HashSet<&str> = shown.iter().map(|n| n.key.as_str()).collect();
    let containers: HashMap<&str, &strata_store::Container> =
        inv.containers.iter().map(|c| (c.id.as_str(), c)).collect();
    let routes = crate::plans::route_lengths(&st.store, &pid).await?;
    let mut cables = Vec::new();
    for c in all_cables.iter().filter(|_| overview.is_none()) {
        let ea = port_of.get(c.a.as_str());
        let eb = port_of.get(c.b.as_str());
        let ia = ea.is_some_and(|(n, _)| keys.contains(n.key.as_str()));
        let ib = eb.is_some_and(|(n, _)| keys.contains(n.key.as_str()));
        if !ia && !ib {
            continue;
        }
        let warnings = cable_warnings(ea.copied(), eb.copied(), &c.fields);
        let estimate = estimate(ea, eb, &containers, &routes);
        let far = |e: Option<&(&Node, &Port)>| {
            e.map(|(n, p)| json!({ "node": n.key, "label": n.label, "sub": n.sub, "port": p.label, "connector": p.connector }))
        };
        let b_info = match &c.fields.external {
            Some(x) => {
                let origin = ea.map(|(n, _)| n.location_id.clone()).unwrap_or_default();
                let resolved = x
                    .location_id
                    .as_ref()
                    .map(|l| ext_pairs.contains(&(l.clone(), origin.clone())));
                Some(json!({
                    "external": true, "node": null, "sub": "", "port": "", "connector": "",
                    "label": match (&x.location_id, &x.label) {
                        (Some(l), _) => format!("{}{}", loc_name.get(l.as_str()).copied().unwrap_or("Emplacement supprimé"), if resolved == Some(true) { "" } else { " · non rattaché" }),
                        (None, Some(t)) => format!("Hors projet · {t}"),
                        _ => "Hors projet".into(),
                    },
                    "location_id": x.location_id, "resolved": resolved,
                }))
            }
            None => far(eb),
        };
        cables.push(json!({
            "id": c.id, "a": c.a, "b": c.b, "rev": c.rev, "fields": c.fields,
            "a_in_view": ia, "b_in_view": ib, "a_info": far(ea), "b_info": b_info,
            "warnings": warnings, "estimate_m": estimate,
        }));
    }
    let layout = st.store.layout(&pid, &view).await?;
    let lock = st.store.lock(&lock_key(&pid)).await?;
    let fresh = lock.as_ref().is_some_and(|l| {
        chrono::DateTime::parse_from_rfc3339(&l.beat_at)
            .map(|t| {
                (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_seconds() < LOCK_TTL_SECS
            })
            .unwrap_or(false)
    });
    Ok(Json(json!({
        "view": view,
        "overview": overview,
        "harnesses": st.store.harnesses(&pid).await?,
        "annotations": st.store.annotations(&pid, Some(&view)).await?,
        "nodes": if view == "overview" { Vec::new() } else { shown },
        "cables": cables,
        "layout": layout.into_iter().map(|(k, (x, y))| (k, json!({ "x": x, "y": y }))).collect::<serde_json::Map<String, Value>>(),
        "lock": lock.filter(|_| fresh).map(|l| json!({ "account_id": l.account_id, "account_name": l.account_name, "taken_at": l.taken_at, "mine": l.token_hash == m.0.token_hash })),
        "views": {
            "containers": inv.containers.iter().map(|c| json!({ "id": c.id, "name": c.name, "location_id": c.location_id, "kind": c.data.kind })).collect::<Vec<_>>(),
            "locations": inv.locations.iter().map(|l| json!({ "id": l.id, "name": l.name, "code": l.code })).collect::<Vec<_>>(),
        },
    })))
}

/// Écriture : droit « Conception : écriture » et main tenue sur le synoptique du projet.
pub(crate) async fn editing(st: &AppState, m: &Member, pid: &str) -> ApiResult<()> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    if !st
        .store
        .holds_lock(&lock_key(pid), &m.0.token_hash, LOCK_TTL_SECS)
        .await?
    {
        let who = st.store.lock(&lock_key(pid)).await?.map(|l| l.account_name);
        return Err(ApiError::Conflict(match who {
            Some(n) => format!("{n} a la main sur le synoptique : vous le voyez en lecture seule."),
            None => "Prenez la main sur le synoptique pour le modifier.".into(),
        }));
    }
    Ok(())
}

async fn take(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    st.store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    match st
        .store
        .take_lock(
            &lock_key(&pid),
            &pid,
            &m.0.token_hash,
            &m.0.account.id,
            &m.0.account.name,
            LOCK_TTL_SECS,
        )
        .await?
    {
        LockOutcome::Taken(l) => {
            if l.taken_at == l.beat_at {
                st.hub.change(
                    room(&pid),
                    "lock",
                    &lock_key(&pid),
                    "taken",
                    None,
                    &m.0.account.id,
                    origin.0.as_deref(),
                );
            }
            Ok(Json(
                json!({ "mine": true, "account_name": l.account_name, "taken_at": l.taken_at }),
            ))
        }
        LockOutcome::Held(l) => Ok(Json(
            json!({ "mine": false, "account_id": l.account_id, "account_name": l.account_name, "taken_at": l.taken_at }),
        )),
    }
}

async fn release(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
) -> ApiResult<StatusCode> {
    if st
        .store
        .release_lock(&lock_key(&pid), &m.0.token_hash)
        .await?
    {
        st.hub.change(
            room(&pid),
            "lock",
            &lock_key(&pid),
            "released",
            None,
            &m.0.account.id,
            origin.0.as_deref(),
        );
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn request(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
) -> ApiResult<StatusCode> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    st.hub.change(
        room(&pid),
        "lock",
        &lock_key(&pid),
        "request",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct Pos {
    node: String,
    x: f64,
    y: f64,
}

#[derive(Deserialize)]
struct LayoutBody {
    view: String,
    positions: Vec<Pos>,
}

async fn layout(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
    Json(b): Json<LayoutBody>,
) -> ApiResult<StatusCode> {
    editing(&st, &m, &pid).await?;
    if b.positions.len() > 5000
        || b.positions
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite())
    {
        return Err(one("positions", "Positions illisibles."));
    }
    let list: Vec<(String, f64, f64)> = b
        .positions
        .into_iter()
        .map(|p| (p.node, p.x.round(), p.y.round()))
        .collect();
    st.store.put_layout(&pid, &b.view, &list).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct NewOne {
    a: String,
    b: String,
    #[serde(flatten)]
    fields: CableFields,
}

#[derive(Deserialize)]
struct CreateBody {
    cables: Vec<NewOne>,
    #[serde(default)]
    replace: bool,
}

async fn create(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<CreateBody>,
) -> ApiResult<Response> {
    editing(&st, &m, &pid).await?;
    if b.cables.is_empty() || b.cables.len() > MAX_BATCH {
        return Err(one("cables", "De 1 à 500 câbles par lot."));
    }
    let nodes = project_nodes(&st.store, &pid).await?;
    let ports: HashMap<&str, &Port> = nodes
        .iter()
        .flat_map(|n| n.ports.iter().map(|p| (p.r.as_str(), p)))
        .collect();
    let mut seen = HashSet::new();
    let mut list = Vec::new();
    let mut warnings = Vec::new();
    for (k, mut c) in b.cables.into_iter().enumerate() {
        let (Some(pa), Some(pb)) = (ports.get(c.a.as_str()), ports.get(c.b.as_str())) else {
            return Err(one(
                &format!("cables.{k}"),
                "Port introuvable dans le projet.",
            ));
        };
        if c.a == c.b || !seen.insert(c.a.clone()) || !seen.insert(c.b.clone()) {
            return Err(one(&format!("cables.{k}"), "Un port ne porte qu'un câble."));
        }
        fields(validate_cable(&mut c.fields))?;
        warnings.push(compat(&pa.info(), &pb.info()));
        list.push(NewCable {
            a: c.a,
            b: c.b,
            fields: c.fields,
        });
    }
    match st.store.insert_cables(&pid, &list, b.replace, Some(&m.0.account.id)).await? {
        PlaceCables::Occupied(busy) => Ok((
            StatusCode::CONFLICT,
            Json(json!({ "code": "occupied", "message": if busy.len() > 1 { format!("{} ports déjà câblés : les câbles existants seront remplacés.", busy.len()) } else { "1 port déjà câblé : le câble existant sera remplacé.".to_string() }, "ports": busy })),
        )
            .into_response()),
        PlaceCables::Done { created, replaced } => {
            for c in &created {
                record(&st, &m, &origin, &pid, "cable", &c.id, "create", Some(c.rev), None::<&()>, Some(c)).await?;
            }
            Ok((
                StatusCode::CREATED,
                Json(json!({ "created": created, "replaced": replaced, "warnings": warnings })),
            )
                .into_response())
        }
    }
}

#[derive(Deserialize)]
struct PatchBody {
    rev: i64,
    #[serde(flatten)]
    fields: CableFields,
}

async fn update(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
    Json(mut b): Json<PatchBody>,
) -> ApiResult<Json<Value>> {
    let before = st
        .store
        .cable(&id)
        .await?
        .ok_or(ApiError::NotFound("Câble"))?;
    editing(&st, &m, &before.project_id).await?;
    fields(validate_cable(&mut b.fields))?;
    let after = st.store.update_cable(&id, b.rev, &b.fields).await?;
    record(
        &st,
        &m,
        &origin,
        &before.project_id,
        "cable",
        &id,
        "update",
        Some(after.rev),
        Some(&before),
        Some(&after),
    )
    .await?;
    Ok(Json(json!(after)))
}

#[derive(Deserialize)]
struct RemoveBody {
    ids: Vec<String>,
}

async fn remove(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<RemoveBody>,
) -> ApiResult<Json<Value>> {
    editing(&st, &m, &pid).await?;
    if b.ids.is_empty() || b.ids.len() > MAX_BATCH {
        return Err(one("ids", "De 1 à 500 câbles par lot."));
    }
    let n = st.store.delete_cables(&pid, &b.ids).await?;
    st.store
        .audit_project(
            &pid,
            Some(&m.0.account.id),
            "delete",
            "cable",
            &b.ids[0],
            Some(&json!({ "cables": n })),
            None::<&()>,
        )
        .await?;
    st.hub.change(
        room(&pid),
        "cable",
        &b.ids[0],
        "delete",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(Json(json!({ "deleted": n })))
}

// ----- Vue générale -----

/// Un bloc par lieu et les seules liaisons entre lieux (câbles et ports externes), avec
/// les totaux du projet.
async fn overview(
    store: &Store,
    pid: &str,
    _nodes: &[Node],
    port_of: &HashMap<&str, (&Node, &Port)>,
    cables: &[strata_store::cables::Cable],
    _inv: &strata_store::Inventory,
) -> ApiResult<Value> {
    let (_, _, inv) = crate::projects::inventory_view(store, pid).await?;
    let locs = inv["locations"].as_array().cloned().unwrap_or_default();
    let loc_of = |r: &str| port_of.get(r).map(|(n, _)| n.location_id.clone());
    let mut links: HashMap<(String, String), (Vec<String>, usize)> = HashMap::new();
    let mut outside: HashMap<String, usize> = HashMap::new();
    for c in cables.iter().filter(|c| !c.fields.attach) {
        let Some(la) = loc_of(&c.a) else { continue };
        let lb = match &c.fields.external {
            Some(x) => match &x.location_id {
                Some(to) => Some(to.clone()),
                None => {
                    *outside.entry(la.clone()).or_default() += 1;
                    continue;
                }
            },
            None => loc_of(&c.b),
        };
        let Some(lb) = lb else { continue };
        if la == lb {
            continue;
        }
        let key = if la < lb { (la, lb) } else { (lb, la) };
        let e = links.entry(key).or_default();
        e.0.push(c.id.clone());
        if c.fields.external.is_some() {
            e.1 += 1;
        }
    }
    let f = |l: &Value, k: &str| l["totals"][k].as_f64().unwrap_or(0.0);
    Ok(json!({
        "locations": locs.iter().map(|l| json!({
            "id": l["id"], "name": l["name"], "code": l["code"], "elements": l["totals"]["containers"],
            "weight_kg": f(l, "weight_kg"), "power_w": f(l, "power_w"),
            "outside": outside.get(l["id"].as_str().unwrap_or("")).copied().unwrap_or(0),
        })).collect::<Vec<_>>(),
        "links": links.into_iter().map(|((a, b), (ids, ext))| json!({ "a": a, "b": b, "cables": ids, "external": ext })).collect::<Vec<_>>(),
        "totals": { "weight_kg": (locs.iter().map(|l| f(l, "weight_kg")).sum::<f64>() * 10.0).round() / 10.0, "power_w": locs.iter().map(|l| f(l, "power_w")).sum::<f64>().round() },
    }))
}

// ----- Ports externes et attaches d'adaptateurs -----

#[derive(Deserialize)]
struct ExternalBody {
    port: String,
    #[serde(flatten)]
    to: ExternalEnd,
    number: Option<String>,
}

/// Marque un port comme sortant de l'élément : vers un autre lieu du projet, ou hors projet.
async fn external(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<ExternalBody>,
) -> ApiResult<Response> {
    editing(&st, &m, &pid).await?;
    let nodes = project_nodes(&st.store, &pid).await?;
    let Some(node) = nodes.iter().find(|n| n.ports.iter().any(|p| p.r == b.port)) else {
        return Err(one("port", "Port introuvable dans le projet."));
    };
    if let Some(l) = &b.to.location_id {
        let inv = st.store.inventory(&pid).await?;
        if !inv.locations.iter().any(|x| &x.id == l) {
            return Err(one("location_id", "Emplacement inconnu dans ce projet."));
        }
        if *l == node.location_id {
            return Err(one(
                "location_id",
                "Un port externe sort vers un autre emplacement que le sien.",
            ));
        }
    }
    let mut f = CableFields {
        number: b.number,
        external: Some(b.to),
        ..Default::default()
    };
    fields(validate_cable(&mut f))?;
    let nc = NewCable {
        a: b.port,
        b: format!("x:{}", strata_store_id()),
        fields: f,
    };
    place_one(&st, &m, &origin, &pid, nc).await
}

#[derive(Deserialize)]
struct AttachBody {
    /// Entrée de l'adaptateur (`a:{article}:{unité}:in`).
    adapter: String,
    port: String,
    #[serde(default)]
    replace: bool,
}

/// Attache un adaptateur sur un port, sans câble : même connecteur, genres opposés.
async fn attach(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<AttachBody>,
) -> ApiResult<Response> {
    editing(&st, &m, &pid).await?;
    let nodes = project_nodes(&st.store, &pid).await?;
    let ports: HashMap<&str, (&Node, &Port)> = nodes
        .iter()
        .flat_map(|n| n.ports.iter().map(move |p| (p.r.as_str(), (n, p))))
        .collect();
    let (Some((_, ad)), Some((pn, port))) =
        (ports.get(b.adapter.as_str()), ports.get(b.port.as_str()))
    else {
        return Err(one("port", "Port introuvable dans le projet."));
    };
    if !b.adapter.starts_with("a:") || !b.adapter.ends_with(":in") {
        return Err(one(
            "adapter",
            "Seule l'entrée d'un adaptateur s'attache sur un port.",
        ));
    }
    if pn.kind == "adapter" {
        return Err(one(
            "port",
            "Un adaptateur s'attache sur un équipement ou un panel.",
        ));
    }
    attach_ok(
        &port.connector,
        port.gender.as_deref(),
        &ad.connector,
        ad.gender.as_deref().unwrap_or("N"),
    )
    .map_err(|e| one("port", e))?;
    let nc = NewCable {
        a: b.port.clone(),
        b: b.adapter.clone(),
        fields: CableFields {
            attach: true,
            ..Default::default()
        },
    };
    match st.store.insert_cables(&pid, std::slice::from_ref(&nc), b.replace, Some(&m.0.account.id)).await? {
        PlaceCables::Occupied(busy) => Ok((StatusCode::CONFLICT, Json(json!({ "code": "occupied", "message": "Port déjà câblé : le câble existant sera remplacé.", "ports": busy }))).into_response()),
        PlaceCables::Done { created, .. } => {
            for c in &created {
                record(&st, &m, &origin, &pid, "cable", &c.id, "create", Some(c.rev), None::<&()>, Some(c)).await?;
            }
            Ok((StatusCode::CREATED, Json(json!({ "created": created }))).into_response())
        }
    }
}

fn strata_store_id() -> String {
    uuid::Uuid::now_v7().simple().to_string()
}

async fn place_one(
    st: &AppState,
    m: &Member,
    origin: &Origin,
    pid: &str,
    nc: NewCable,
) -> ApiResult<Response> {
    match st.store.insert_cables(pid, std::slice::from_ref(&nc), false, Some(&m.0.account.id)).await? {
        PlaceCables::Occupied(busy) => Ok((StatusCode::CONFLICT, Json(json!({ "code": "occupied", "message": "Ce port porte déjà un câble : retirez-le d'abord.", "ports": busy }))).into_response()),
        PlaceCables::Done { created, .. } => {
            for c in &created {
                record(st, m, origin, pid, "cable", &c.id, "create", Some(c.rev), None::<&()>, Some(c)).await?;
            }
            Ok((StatusCode::CREATED, Json(json!({ "created": created }))).into_response())
        }
    }
}

// ----- Faisceaux -----

#[derive(Deserialize)]
struct HarnessBody {
    name: String,
    color: String,
    rev: Option<i64>,
}

fn check_harness(b: &mut HarnessBody) -> ApiResult<()> {
    b.name = b.name.trim().to_string();
    let mut f = CableFields {
        color: Some(b.color.clone()),
        ..Default::default()
    };
    if b.name.is_empty() || b.name.chars().count() > 120 {
        return Err(one("name", "Le nom compte de 1 à 120 caractères."));
    }
    if !validate_cable(&mut f).is_empty() {
        return Err(one("color", "Couleur au format #RRVVBB."));
    }
    Ok(())
}

async fn harness_create(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(mut b): Json<HarnessBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    editing(&st, &m, &pid).await?;
    check_harness(&mut b)?;
    let h = st.store.insert_harness(&pid, &b.name, &b.color).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "harness",
        &h.id,
        "create",
        Some(h.rev),
        None::<&()>,
        Some(&h),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(json!(h))))
}

async fn harness_update(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
    Json(mut b): Json<HarnessBody>,
) -> ApiResult<Json<Value>> {
    let h = st
        .store
        .harness(&id)
        .await?
        .ok_or(ApiError::NotFound("Faisceau"))?;
    editing(&st, &m, &h.project_id).await?;
    check_harness(&mut b)?;
    let after = st
        .store
        .update_harness(&id, b.rev.unwrap_or(h.rev), &b.name, &b.color)
        .await?;
    record(
        &st,
        &m,
        &origin,
        &h.project_id,
        "harness",
        &id,
        "update",
        Some(after.rev),
        Some(&h),
        Some(&after),
    )
    .await?;
    Ok(Json(json!(after)))
}

async fn harness_delete(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let h = st
        .store
        .harness(&id)
        .await?
        .ok_or(ApiError::NotFound("Faisceau"))?;
    editing(&st, &m, &h.project_id).await?;
    st.store.delete_harness(&id).await?;
    record(
        &st,
        &m,
        &origin,
        &h.project_id,
        "harness",
        &id,
        "delete",
        None,
        Some(&h),
        None::<&()>,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct AssignBody {
    ids: Vec<String>,
    harness_id: Option<String>,
}

/// Range des câbles dans un faisceau, ou les en sort ; un câble n'est que dans un faisceau.
async fn harness_assign(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<AssignBody>,
) -> ApiResult<Json<Value>> {
    editing(&st, &m, &pid).await?;
    if b.ids.is_empty() || b.ids.len() > MAX_BATCH {
        return Err(one("ids", "De 1 à 500 câbles par lot."));
    }
    if let Some(h) = &b.harness_id {
        st.store
            .harness(h)
            .await?
            .filter(|x| x.project_id == pid)
            .ok_or(ApiError::NotFound("Faisceau"))?;
    }
    let n = st
        .store
        .set_harness(&pid, &b.ids, b.harness_id.as_deref())
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
    Ok(Json(json!({ "changed": n })))
}

// ----- Annotations -----

#[derive(Deserialize)]
struct NewAnnotation {
    view: String,
    kind: String,
    data: Value,
}

fn ann_fields(e: Vec<strata_domain::catalog::FieldError>) -> ApiError {
    ApiError::Fields(e)
}

async fn annotation_create(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<NewAnnotation>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    editing(&st, &m, &pid).await?;
    let data =
        strata_domain::synoptic::validate_annotation(&b.kind, &b.data).map_err(ann_fields)?;
    let a = st
        .store
        .insert_annotation(&pid, &b.view, &b.kind, &data)
        .await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "annotation",
        &a.id,
        "create",
        Some(a.rev),
        None::<&()>,
        Some(&a),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(json!(a))))
}

#[derive(Deserialize)]
struct PatchAnnotation {
    rev: i64,
    data: Value,
}

async fn annotation_update(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
    Json(b): Json<PatchAnnotation>,
) -> ApiResult<Json<Value>> {
    let a = st
        .store
        .annotation(&id)
        .await?
        .ok_or(ApiError::NotFound("Annotation"))?;
    editing(&st, &m, &a.project_id).await?;
    let data =
        strata_domain::synoptic::validate_annotation(&a.kind, &b.data).map_err(ann_fields)?;
    let after = st.store.update_annotation(&id, b.rev, &data).await?;
    record(
        &st,
        &m,
        &origin,
        &a.project_id,
        "annotation",
        &id,
        "update",
        Some(after.rev),
        Some(&a),
        Some(&after),
    )
    .await?;
    Ok(Json(json!(after)))
}

async fn annotation_delete(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<RemoveBody>,
) -> ApiResult<Json<Value>> {
    editing(&st, &m, &pid).await?;
    let n = st.store.delete_annotations(&pid, &b.ids).await?;
    st.hub.change(
        room(&pid),
        "annotation",
        &pid,
        "delete",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(Json(json!({ "deleted": n })))
}
