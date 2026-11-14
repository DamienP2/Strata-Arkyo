//! Réseau (section 11.3) : VLAN, ports réseau, plan IPv4, multicast SMPTE ST 2110, carte.

use crate::AppState;
use crate::auth::Member;
use crate::catalog_io::{Cell, file_response, to_csv, to_xlsx_named};
use crate::error::{ApiError, ApiResult};
use crate::live::Origin;
use crate::projects::{one, read, record};
use crate::synoptic::{Node, project_nodes};
use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use strata_domain::catalog::{FieldError, IpSpec, RefKind};
use strata_domain::network::{
    Cidr, Essence, NetPort, PortMode, VlanData, ip_string, next_group, parse_ip,
    validate_multicast, validate_port, validate_vlan,
};
use strata_domain::project::ArticleNature;
use strata_domain::{Domain, Level};
use strata_store::Store;
use strata_store::network::{Flow, Vlan};

const DESIGN: &str = "Vous n'avez pas le droit de modifier la conception.";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects/{id}/network", get(show))
        .route(
            "/projects/{id}/network/vlans",
            post(vlan_create).patch(vlan_update),
        )
        .route("/projects/{id}/network/vlans/delete", post(vlan_delete))
        .route("/projects/{id}/network/ports", post(ports_set))
        .route("/projects/{id}/network/ip/auto", post(ip_auto))
        .route("/projects/{id}/network/flows/assign", post(flows_assign))
        .route("/projects/{id}/network/flows/set", post(flow_set))
        .route("/projects/{id}/network/flows/clear", post(flows_clear))
        .route("/projects/{id}/network/export", get(export))
}

fn design(m: &Member) -> ApiResult<()> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)
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

// ----- Vue calculée -----

#[derive(Debug, Clone, Serialize)]
struct PortRow {
    #[serde(rename = "ref")]
    r: String,
    node: String,
    equipment: String,
    tag: String,
    element: String,
    port: String,
    connector: String,
    switching: bool,
    #[serde(flatten)]
    settings: NetPort,
    /// Bout d'en face, s'il est câblé.
    peer: Option<String>,
    peer_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct FlowRow {
    key: String,
    instance: String,
    equipment: String,
    tag: String,
    element: String,
    signal: String,
    essence: Essence,
    n: u32,
    address: Option<String>,
    udp: Option<i64>,
    manual: bool,
}

#[derive(Debug, Clone, Serialize)]
struct Issue {
    kind: &'static str,
    message: String,
    refs: Vec<String>,
}

struct View {
    vlans: Vec<Vlan>,
    ports: Vec<PortRow>,
    flows: Vec<FlowRow>,
}

fn split_sub(n: &Node) -> String {
    n.sub.rsplit(" · ").next().unwrap_or("").to_string()
}

async fn compute(store: &Store, pid: &str) -> ApiResult<View> {
    let inv = store.inventory(pid).await?;
    let names: HashMap<&str, &str> = inv
        .containers
        .iter()
        .map(|c| (c.id.as_str(), c.name.as_str()))
        .collect();
    let element = |n: &Node| {
        n.container_id
            .as_deref()
            .and_then(|c| names.get(c))
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Équipement libre".into())
    };
    let nodes: Vec<Node> = project_nodes(store, pid)
        .await?
        .into_iter()
        .filter(|n| n.kind == "equipment")
        .collect();
    let all_nodes = project_nodes(store, pid).await?;
    let label_of: HashMap<&str, String> = all_nodes
        .iter()
        .flat_map(|n| {
            n.ports.iter().map(move |p| {
                (
                    p.r.as_str(),
                    format!("{} {} › {}", n.label, split_sub(n), p.label),
                )
            })
        })
        .collect();
    let mut peer: HashMap<String, (String, Option<String>)> = HashMap::new();
    for c in store.cables(pid).await? {
        if c.fields.attach {
            continue;
        }
        let lb = match &c.fields.external {
            Some(x) => (
                format!(
                    "Port externe{}",
                    x.label
                        .as_ref()
                        .map(|l| format!(" · {l}"))
                        .unwrap_or_default()
                ),
                None,
            ),
            None => (
                label_of.get(c.b.as_str()).cloned().unwrap_or_default(),
                Some(c.b.clone()),
            ),
        };
        let la = (
            label_of.get(c.a.as_str()).cloned().unwrap_or_default(),
            Some(c.a.clone()),
        );
        peer.insert(c.a.clone(), lb);
        peer.insert(c.b.clone(), la);
    }
    let saved: HashMap<String, NetPort> = store.net_ports(pid).await?.into_iter().collect();
    let mut ports = Vec::new();
    for n in &nodes {
        for p in n.ports.iter().filter(|p| p.ip) {
            let (pl, pr) = peer.get(&p.r).cloned().unzip();
            ports.push(PortRow {
                r: p.r.clone(),
                node: n.key.clone(),
                equipment: n.label.clone(),
                tag: split_sub(n),
                element: element(n),
                port: p.label.clone(),
                connector: p.connector.clone(),
                switching: p.switching,
                settings: saved.get(&p.r).cloned().unwrap_or_default(),
                peer: pl,
                peer_ref: pr.flatten(),
            });
        }
    }

    // Flux IP : une ligne par sortie de flux déclarée au catalogue.
    let refs = store.list_refs(None).await?;
    let sig: HashMap<&str, (&str, Option<&str>)> = refs
        .iter()
        .filter(|r| r.kind == RefKind::Signal)
        .map(|r| {
            (
                r.id.as_str(),
                (
                    r.name.as_str(),
                    r.data.get("function").and_then(Value::as_str),
                ),
            )
        })
        .collect();
    let stored: HashMap<String, Flow> = store
        .net_flows(pid)
        .await?
        .into_iter()
        .map(|f| (f.key.clone(), f))
        .collect();
    let mut flows = Vec::new();
    let node_of: HashMap<&str, &Node> = nodes.iter().map(|n| (n.key.as_str(), n)).collect();
    for a in inv
        .articles
        .iter()
        .filter(|a| a.nature == ArticleNature::Equipment)
    {
        let Some(item_id) = &a.item_id else { continue };
        let Some(it) = store.item(item_id).await? else {
            continue;
        };
        let Some(spec) = it
            .data
            .get("ip")
            .filter(|v| !v.is_null())
            .cloned()
            .and_then(|v| serde_json::from_value::<IpSpec>(v).ok())
        else {
            continue;
        };
        for inst in inv.instances.iter().filter(|i| i.article_id == a.id) {
            let el = node_of
                .get(format!("i:{}", inst.id).as_str())
                .map(|n| element(n))
                .unwrap_or_else(|| "Équipement libre".into());
            for (fi, f) in spec.flows.iter().enumerate() {
                let (sname, fam) = sig
                    .get(f.signal_id.as_str())
                    .cloned()
                    .unwrap_or(("?", None));
                let essence = f.essence.unwrap_or_else(|| Essence::of_family(fam));
                for n in 1..=f.outputs.min(256) {
                    let key = format!("{}:{fi}:{n}", inst.id);
                    let s = stored.get(&key);
                    flows.push(FlowRow {
                        key,
                        instance: inst.id.clone(),
                        equipment: it.name.clone(),
                        tag: inst.tag.clone(),
                        element: el.clone(),
                        signal: if f.outputs > 1 {
                            format!("{sname} {n}")
                        } else {
                            sname.to_string()
                        },
                        essence,
                        n,
                        address: s.map(|s| s.address.clone()),
                        udp: s.map(|s| s.udp),
                        manual: s.is_some_and(|s| s.manual),
                    });
                }
            }
        }
    }
    Ok(View {
        vlans: store.vlans(pid).await?,
        ports,
        flows,
    })
}

/// VLAN effectif d'un port pour l'adressage : accès, ou natif en trunk.
fn addr_vlan(p: &NetPort) -> Option<u16> {
    p.vlan
}

fn issues(v: &View) -> Vec<Issue> {
    let mut out = Vec::new();
    let vl: HashMap<u16, &VlanData> = v.vlans.iter().map(|x| (x.data.vid, &x.data)).collect();
    let mut by_ip: BTreeMap<u32, Vec<&PortRow>> = BTreeMap::new();
    for p in &v.ports {
        let Some(ip) = p.settings.ip.as_deref().and_then(parse_ip) else {
            continue;
        };
        by_ip.entry(ip).or_default().push(p);
        let name = format!("{} {} › {}", p.equipment, p.tag, p.port);
        if let Some(x) = addr_vlan(&p.settings).and_then(|id| vl.get(&id)) {
            if let Some(n) = x.cidr.as_deref().and_then(Cidr::parse) {
                if !n.contains(ip) {
                    out.push(Issue {
                        kind: "subnet",
                        message: format!(
                            "{name} : {} est hors du sous-réseau {n} du VLAN {}.",
                            ip_string(ip),
                            x.vid
                        ),
                        refs: vec![p.r.clone()],
                    });
                } else if !n.host(ip) {
                    out.push(Issue {
                        kind: "subnet",
                        message: format!(
                            "{name} : {} est l'adresse de réseau ou de diffusion.",
                            ip_string(ip)
                        ),
                        refs: vec![p.r.clone()],
                    });
                }
            }
            if x.gateway.as_deref().and_then(parse_ip) == Some(ip) {
                out.push(Issue {
                    kind: "gateway",
                    message: format!("{name} : {} est la passerelle du VLAN.", ip_string(ip)),
                    refs: vec![p.r.clone()],
                });
            }
        }
    }
    for (ip, ps) in by_ip.iter().filter(|(_, ps)| ps.len() > 1) {
        out.push(Issue {
            kind: "ip_duplicate",
            message: format!(
                "{} est attribuée {} fois : {}.",
                ip_string(*ip),
                ps.len(),
                ps.iter()
                    .map(|p| format!("{} {} › {}", p.equipment, p.tag, p.port))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            refs: ps.iter().map(|p| p.r.clone()).collect(),
        });
    }
    // Mode : un port en accès porte exactement un VLAN ; un trunk au moins un, natif compris.
    if !v.vlans.is_empty() {
        for p in &v.ports {
            let s = &p.settings;
            let name = format!("{} {} › {}", p.equipment, p.tag, p.port);
            match s.mode {
                PortMode::Access
                    if s.vlan.is_none() && (s.ip.is_some() || p.peer_ref.is_some()) =>
                {
                    out.push(Issue {
                        kind: "mode",
                        message: format!("{name} : port en accès sans VLAN."),
                        refs: vec![p.r.clone()],
                    })
                }
                PortMode::Trunk if s.vlans.is_empty() => out.push(Issue {
                    kind: "mode",
                    message: format!("{name} : trunk sans VLAN transporté."),
                    refs: vec![p.r.clone()],
                }),
                PortMode::Trunk if s.vlan.is_some_and(|n| !s.vlans.contains(&n)) => {
                    out.push(Issue {
                        kind: "mode",
                        message: format!(
                            "{name} : le VLAN natif doit être transporté par le trunk."
                        ),
                        refs: vec![p.r.clone()],
                    })
                }
                _ => {}
            }
        }
    }
    // Liaisons : VLAN et réseau A/B cohérents des deux côtés.
    let by_ref: HashMap<&str, &PortRow> = v.ports.iter().map(|p| (p.r.as_str(), p)).collect();
    let carries = |p: &NetPort| -> BTreeSet<u16> {
        let mut s: BTreeSet<u16> = p.vlans.iter().copied().collect();
        s.extend(p.vlan);
        s
    };
    for p in &v.ports {
        let Some(q) = p.peer_ref.as_deref().and_then(|r| by_ref.get(r)) else {
            continue;
        };
        if p.r >= q.r {
            continue;
        }
        let (a, b) = (&p.settings, &q.settings);
        let lbl = format!(
            "{} {} › {} ↔ {} {} › {}",
            p.equipment, p.tag, p.port, q.equipment, q.tag, q.port
        );
        let mismatch = match (a.mode, b.mode) {
            (PortMode::Access, PortMode::Access) => {
                a.vlan.is_some() && b.vlan.is_some() && a.vlan != b.vlan
            }
            (PortMode::Access, PortMode::Trunk) => a.vlan.is_some_and(|x| !carries(b).contains(&x)),
            (PortMode::Trunk, PortMode::Access) => b.vlan.is_some_and(|x| !carries(a).contains(&x)),
            (PortMode::Trunk, PortMode::Trunk) => {
                !a.vlans.is_empty() && !b.vlans.is_empty() && carries(a) != carries(b)
            }
        };
        if mismatch {
            out.push(Issue {
                kind: "vlan_mismatch",
                message: format!("{lbl} : les VLAN ne correspondent pas aux deux bouts."),
                refs: vec![p.r.clone(), q.r.clone()],
            });
        }
        if let (Some(x), Some(y)) = (&a.redundancy, &b.redundancy) {
            if x != y {
                out.push(Issue {
                    kind: "redundancy",
                    message: format!("{lbl} : réseau {x} relié au réseau {y}."),
                    refs: vec![p.r.clone(), q.r.clone()],
                });
            }
        }
    }
    let mut by_group: BTreeMap<(String, i64), Vec<&FlowRow>> = BTreeMap::new();
    for f in &v.flows {
        if let (Some(a), Some(u)) = (&f.address, f.udp) {
            by_group.entry((a.clone(), u)).or_default().push(f);
        }
    }
    for ((a, u), fs) in by_group.iter().filter(|(_, fs)| fs.len() > 1) {
        out.push(Issue {
            kind: "multicast_duplicate",
            message: format!(
                "Groupe {a}:{u} partagé par {} flux : {}.",
                fs.len(),
                fs.iter()
                    .map(|f| format!("{} {} · {}", f.equipment, f.tag, f.signal))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            refs: fs.iter().map(|f| f.key.clone()).collect(),
        });
    }
    out
}

/// Carte : équipements à ports IP, liaisons directes entre eux.
fn map(v: &View) -> Value {
    let mut nodes: Vec<Value> = Vec::new();
    let mut seen = HashSet::new();
    for p in &v.ports {
        if seen.insert(p.node.clone()) {
            let mine: Vec<&PortRow> = v.ports.iter().filter(|q| q.node == p.node).collect();
            let mut vlans: BTreeSet<u16> = BTreeSet::new();
            for q in &mine {
                vlans.extend(q.settings.vlan);
                vlans.extend(q.settings.vlans.iter().copied());
            }
            let ips: Vec<&str> = mine
                .iter()
                .filter_map(|q| q.settings.ip.as_deref())
                .collect();
            nodes.push(json!({
                "key": p.node,
                "label": p.equipment,
                "tag": p.tag,
                "element": p.element,
                "switch": mine.iter().any(|q| q.switching),
                "ports": mine.len(),
                "vlans": vlans,
                "ips": ips,
            }));
        }
    }
    let by_ref: HashMap<&str, &PortRow> = v.ports.iter().map(|p| (p.r.as_str(), p)).collect();
    let mut links = Vec::new();
    for p in &v.ports {
        let Some(q) = p.peer_ref.as_deref().and_then(|r| by_ref.get(r)) else {
            continue;
        };
        if p.r < q.r {
            let mut vl: BTreeSet<u16> = BTreeSet::new();
            vl.extend(p.settings.vlan);
            vl.extend(p.settings.vlans.iter().copied());
            links.push(json!({
                "a": p.node, "b": q.node, "a_port": p.port, "b_port": q.port,
                "redundancy": p.settings.redundancy.clone().or(q.settings.redundancy.clone()),
                "trunk": p.settings.mode == PortMode::Trunk || q.settings.mode == PortMode::Trunk,
                "vlans": vl,
            }));
        }
    }
    json!({ "nodes": nodes, "links": links })
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
    let v = compute(&st.store, &pid).await?;
    let iss = issues(&v);
    let used: HashMap<u16, usize> = v
        .vlans
        .iter()
        .map(|x| {
            (
                x.data.vid,
                v.ports
                    .iter()
                    .filter(|p| {
                        p.settings.vlan == Some(x.data.vid)
                            || p.settings.vlans.contains(&x.data.vid)
                    })
                    .count(),
            )
        })
        .collect();
    let vlans: Vec<Value> = v
        .vlans
        .iter()
        .map(|x| {
            let n = x.data.cidr.as_deref().and_then(Cidr::parse);
            let in_net = n.map(|n| {
                v.ports
                    .iter()
                    .filter_map(|p| p.settings.ip.as_deref().and_then(parse_ip))
                    .filter(|ip| n.contains(*ip))
                    .count()
            });
            json!({
                "id": x.id, "vid": x.data.vid, "name": x.data.name, "color": x.data.color,
                "cidr": x.data.cidr, "gateway": x.data.gateway, "role": x.data.role,
                "ports": used.get(&x.data.vid).copied().unwrap_or(0),
                "hosts": n.map(|n| n.size()), "used": in_net,
            })
        })
        .collect();
    let stats = json!({
        "ports": v.ports.len(),
        "with_ip": v.ports.iter().filter(|p| p.settings.ip.is_some()).count(),
        "flows": v.flows.len(),
        "flows_assigned": v.flows.iter().filter(|f| f.address.is_some()).count(),
        "issues": iss.len(),
    });
    Ok(Json(json!({
        "vlans": vlans,
        "ports": v.ports,
        "flows": v.flows,
        "issues": iss,
        "map": map(&v),
        "stats": stats,
    })))
}

// ----- VLAN -----

#[derive(Deserialize)]
struct VlanBody {
    #[serde(default)]
    id: Option<String>,
    #[serde(flatten)]
    data: VlanData,
}

async fn vlan_create(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(mut b): Json<VlanBody>,
) -> ApiResult<Json<Value>> {
    design(&m)?;
    to_fields(validate_vlan(&mut b.data))?;
    if st
        .store
        .vlans(&pid)
        .await?
        .iter()
        .any(|v| v.data.vid == b.data.vid)
    {
        return Err(one("vid", "Ce numéro de VLAN existe déjà dans le projet."));
    }
    let id = st.store.insert_vlan(&pid, &b.data).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "network",
        &id,
        "vlan_create",
        None,
        None::<&Value>,
        Some(&b.data),
    )
    .await?;
    Ok(Json(json!({ "id": id })))
}

async fn vlan_update(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(mut b): Json<VlanBody>,
) -> ApiResult<Json<Value>> {
    design(&m)?;
    let id = b.id.clone().ok_or_else(|| one("id", "VLAN manquant."))?;
    to_fields(validate_vlan(&mut b.data))?;
    let list = st.store.vlans(&pid).await?;
    let before = list
        .iter()
        .find(|v| v.id == id)
        .ok_or(ApiError::NotFound("VLAN"))?;
    if list.iter().any(|v| v.id != id && v.data.vid == b.data.vid) {
        return Err(one("vid", "Ce numéro de VLAN existe déjà dans le projet."));
    }
    st.store.update_vlan(&pid, &id, &b.data).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "network",
        &id,
        "vlan_update",
        None,
        Some(&before.data),
        Some(&b.data),
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct IdBody {
    id: String,
}

async fn vlan_delete(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<IdBody>,
) -> ApiResult<Json<Value>> {
    design(&m)?;
    let before = st
        .store
        .vlans(&pid)
        .await?
        .into_iter()
        .find(|v| v.id == b.id)
        .ok_or(ApiError::NotFound("VLAN"))?;
    st.store.delete_vlan(&pid, &b.id).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "network",
        &b.id,
        "vlan_delete",
        None,
        Some(&before.data),
        None::<&Value>,
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

// ----- Ports -----

#[derive(Deserialize)]
struct PortsBody {
    refs: Vec<String>,
    /// Champs à poser sur tous les ports (les absents ne changent pas).
    set: Map<String, Value>,
}

/// IP déjà prises dans le projet, port par port.
fn ips_taken(ports: &[PortRow]) -> HashMap<u32, String> {
    ports
        .iter()
        .filter_map(|p| {
            p.settings
                .ip
                .as_deref()
                .and_then(parse_ip)
                .map(|ip| (ip, p.r.clone()))
        })
        .collect()
}

async fn ports_set(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<PortsBody>,
) -> ApiResult<Json<Value>> {
    design(&m)?;
    if b.refs.is_empty() {
        return Err(one("refs", "Choisissez au moins un port."));
    }
    const KEYS: [&str; 7] = [
        "mode",
        "vlan",
        "vlans",
        "ip",
        "hostname",
        "redundancy",
        "protocol",
    ];
    if let Some(k) = b.set.keys().find(|k| !KEYS.contains(&k.as_str())) {
        return Err(ApiError::Invalid(format!("Réglage inconnu : {k}.")));
    }
    let ip_given = b.set.get("ip").is_some_and(|v| !v.is_null());
    if ip_given && b.refs.len() > 1 {
        return Err(one(
            "ip",
            "Une adresse IP ne se pose que sur un port ; utilisez l'attribution automatique.",
        ));
    }
    let v = compute(&st.store, &pid).await?;
    let vids: HashSet<u16> = v.vlans.iter().map(|x| x.data.vid).collect();
    let by_ref: HashMap<&str, &PortRow> = v.ports.iter().map(|p| (p.r.as_str(), p)).collect();
    let mut taken = ips_taken(&v.ports);
    let mut out = Vec::new();
    for r in &b.refs {
        let p = by_ref
            .get(r.as_str())
            .ok_or_else(|| one("refs", "Ce port n'est pas un port réseau du projet."))?;
        let mut cur = serde_json::to_value(&p.settings).unwrap_or_default();
        if let Value::Object(o) = &mut cur {
            for (k, val) in &b.set {
                o.insert(k.clone(), val.clone());
            }
        }
        let mut np: NetPort = serde_json::from_value(cur)
            .map_err(|e| ApiError::Invalid(format!("Réglage illisible : {e}.")))?;
        to_fields(validate_port(&mut np, &vids))?;
        if let Some(ip) = np.ip.as_deref().and_then(parse_ip) {
            if let Some(other) = taken.get(&ip).filter(|o| *o != r) {
                let who = by_ref
                    .get(other.as_str())
                    .map(|o| format!("{} {} › {}", o.equipment, o.tag, o.port))
                    .unwrap_or_default();
                return Err(one(
                    "ip",
                    format!("{} est déjà utilisée par {who}.", ip_string(ip)),
                ));
            }
            taken.insert(ip, r.clone());
        }
        out.push((r.clone(), np));
    }
    st.store.set_net_ports(&pid, &out).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "network",
        "ports",
        "ports_set",
        None,
        None::<&Value>,
        Some(&json!({ "refs": b.refs, "set": b.set })),
    )
    .await?;
    Ok(Json(json!({ "ok": true, "count": out.len() })))
}

#[derive(Deserialize, Default)]
struct AutoBody {
    #[serde(default)]
    refs: Option<Vec<String>>,
}

/// Donne à chaque port sans adresse la première libre du sous-réseau de son VLAN.
async fn ip_auto(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<AutoBody>,
) -> ApiResult<Json<Value>> {
    design(&m)?;
    let v = compute(&st.store, &pid).await?;
    let nets: HashMap<u16, (Cidr, Option<u32>)> = v
        .vlans
        .iter()
        .filter_map(|x| {
            x.data.cidr.as_deref().and_then(Cidr::parse).map(|n| {
                (
                    x.data.vid,
                    (n, x.data.gateway.as_deref().and_then(parse_ip)),
                )
            })
        })
        .collect();
    let mut used: HashSet<u32> = ips_taken(&v.ports).into_keys().collect();
    let wanted: Option<HashSet<&str>> = b
        .refs
        .as_ref()
        .map(|r| r.iter().map(String::as_str).collect());
    let mut out = Vec::new();
    let (mut skipped, mut full) = (0, 0);
    for p in v
        .ports
        .iter()
        .filter(|p| p.settings.ip.is_none())
        .filter(|p| wanted.as_ref().is_none_or(|w| w.contains(p.r.as_str())))
    {
        let Some((n, gw)) = addr_vlan(&p.settings).and_then(|x| nets.get(&x)) else {
            skipped += 1;
            continue;
        };
        match n.first_free(&used, *gw) {
            Some(ip) => {
                used.insert(ip);
                let mut np = p.settings.clone();
                np.ip = Some(ip_string(ip));
                out.push((p.r.clone(), np));
            }
            None => full += 1,
        }
    }
    st.store.set_net_ports(&pid, &out).await?;
    if !out.is_empty() {
        record(
            &st,
            &m,
            &origin,
            &pid,
            "network",
            "ports",
            "ip_auto",
            None,
            None::<&Value>,
            Some(&json!({ "count": out.len() })),
        )
        .await?;
    }
    Ok(Json(
        json!({ "assigned": out.len(), "skipped": skipped, "full": full }),
    ))
}

// ----- Multicast -----

#[derive(Deserialize)]
struct AssignBody {
    /// « missing » : seulement les flux sans groupe ; « all » : tout redistribuer sauf les
    /// saisies manuelles.
    mode: String,
}

async fn flows_assign(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<AssignBody>,
) -> ApiResult<Json<Value>> {
    design(&m)?;
    let all = match b.mode.as_str() {
        "missing" => false,
        "all" => true,
        _ => return Err(one("mode", "Mode : missing ou all.")),
    };
    let v = compute(&st.store, &pid).await?;
    let mut used: HashSet<u32> = v
        .flows
        .iter()
        .filter(|f| !all || f.manual)
        .filter_map(|f| f.address.as_deref().and_then(parse_ip))
        .collect();
    let mut out = Vec::new();
    for f in v
        .flows
        .iter()
        .filter(|f| if all { !f.manual } else { f.address.is_none() })
    {
        let Some(g) = next_group(f.essence, &used) else {
            return Err(ApiError::Invalid(
                "Plus de groupe multicast libre pour cette essence.".into(),
            ));
        };
        used.insert(g);
        out.push(Flow {
            key: f.key.clone(),
            address: ip_string(g),
            udp: f.essence.udp() as i64,
            manual: false,
        });
    }
    st.store.set_net_flows(&pid, &out).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "network",
        "flows",
        "flows_assign",
        None,
        None::<&Value>,
        Some(&json!({ "mode": b.mode, "count": out.len() })),
    )
    .await?;
    Ok(Json(json!({ "assigned": out.len() })))
}

#[derive(Deserialize)]
struct FlowBody {
    key: String,
    address: String,
    #[serde(default)]
    udp: Option<i64>,
}

/// Saisie manuelle : elle prime et n'est pas redistribuée.
async fn flow_set(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<FlowBody>,
) -> ApiResult<Json<Value>> {
    design(&m)?;
    let v = compute(&st.store, &pid).await?;
    let f = v
        .flows
        .iter()
        .find(|f| f.key == b.key)
        .ok_or_else(|| one("key", "Flux inconnu."))?;
    let ip = validate_multicast(&b.address).map_err(|e| one("address", e))?;
    let udp = b.udp.unwrap_or(f.essence.udp() as i64);
    if !(1024..=65535).contains(&udp) {
        return Err(one("udp", "Port UDP entre 1024 et 65535."));
    }
    if let Some(o) = v.flows.iter().find(|o| {
        o.key != b.key && o.udp == Some(udp) && o.address.as_deref().and_then(parse_ip) == Some(ip)
    }) {
        return Err(one(
            "address",
            format!(
                "Ce groupe est déjà utilisé par {} {} · {}.",
                o.equipment, o.tag, o.signal
            ),
        ));
    }
    st.store
        .set_net_flows(
            &pid,
            &[Flow {
                key: b.key.clone(),
                address: ip_string(ip),
                udp,
                manual: true,
            }],
        )
        .await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "network",
        &b.key,
        "flow_set",
        None,
        None::<&Value>,
        Some(&json!({ "address": ip_string(ip), "udp": udp })),
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct ClearBody {
    #[serde(default)]
    keys: Option<Vec<String>>,
}

async fn flows_clear(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<ClearBody>,
) -> ApiResult<Json<Value>> {
    design(&m)?;
    let keys = match b.keys {
        Some(k) => k,
        None => st
            .store
            .net_flows(&pid)
            .await?
            .into_iter()
            .map(|f| f.key)
            .collect(),
    };
    st.store.clear_net_flows(&pid, &keys).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "network",
        "flows",
        "flows_clear",
        None,
        None::<&Value>,
        Some(&json!({ "count": keys.len() })),
    )
    .await?;
    Ok(Json(json!({ "cleared": keys.len() })))
}

// ----- Export du plan d'adressage -----

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
    let v = compute(&st.store, &pid).await?;
    let vname: HashMap<u16, &str> = v
        .vlans
        .iter()
        .map(|x| (x.data.vid, x.data.name.as_str()))
        .collect();
    let t = |s: &str| Cell::Text(s.to_string());
    let headers: Vec<String> = [
        "Type",
        "Élément",
        "Équipement",
        "ID",
        "Port ou flux",
        "Mode",
        "VLAN",
        "Adresse",
        "Port UDP",
        "Nom d'hôte",
        "Réseau",
        "Relié à",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    for r in &v.ports {
        let s = &r.settings;
        let vl = match s.mode {
            PortMode::Access => s
                .vlan
                .map(|x| format!("{x} {}", vname.get(&x).unwrap_or(&"")))
                .unwrap_or_default(),
            PortMode::Trunk => {
                let mut l: Vec<String> = s.vlans.iter().map(|x| x.to_string()).collect();
                if let Some(n) = s.vlan {
                    l.insert(0, format!("natif {n}"));
                }
                l.join(", ")
            }
        };
        rows.push(vec![
            t("Port"),
            t(&r.element),
            t(&r.equipment),
            t(&r.tag),
            t(&r.port),
            t(match s.mode {
                PortMode::Access => "Accès",
                PortMode::Trunk => "Trunk",
            }),
            t(vl.trim()),
            t(s.ip.as_deref().unwrap_or("")),
            Cell::Number(None),
            t(s.hostname.as_deref().unwrap_or("")),
            t(s.redundancy.as_deref().unwrap_or("")),
            t(r.peer.as_deref().unwrap_or("")),
        ]);
    }
    for f in &v.flows {
        rows.push(vec![
            t(&format!("Flux {}", f.essence.label().to_lowercase())),
            t(&f.element),
            t(&f.equipment),
            t(&f.tag),
            t(&f.signal),
            t("Multicast"),
            t(""),
            t(f.address.as_deref().unwrap_or("")),
            Cell::Number(f.udp.map(|u| u as f64)),
            t(""),
            t(""),
            t(""),
        ]);
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
    let stem = format!("Plan-IP_{safe}_{}", chrono::Local::now().format("%Y-%m-%d"));
    match q.format.as_str() {
        "csv" => Ok(file_response(
            to_csv(&headers, &rows).map_err(ApiError::Internal)?,
            "text/csv; charset=utf-8",
            &format!("{stem}.csv"),
        )),
        "xlsx" => Ok(file_response(
            to_xlsx_named(&headers, &rows, "Plan IP")
                .map_err(|e| ApiError::Internal(e.to_string()))?,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            &format!("{stem}.xlsx"),
        )),
        _ => Err(ApiError::Invalid("Format d'export : csv ou xlsx.".into())),
    }
}

/// Anomalies réseau du projet (messages), pour la vue d'ensemble.
pub(crate) async fn network_issues(store: &Store, pid: &str) -> ApiResult<Vec<String>> {
    let v = compute(store, pid).await?;
    Ok(issues(&v).into_iter().map(|i| i.message).collect())
}
