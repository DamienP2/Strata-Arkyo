//! Cartographie 2D et 3D (section 11.1) : cartes, étages, fonds de plan calibrés, contenants
//! posés, points de connexion, liens libres, textes, mesures et tracés des liaisons entre
//! contenants. Écran de conception : une main par carte.

use crate::AppState;
use crate::auth::Member;
use crate::error::{ApiError, ApiResult};
use crate::live::Origin;
use crate::projects::{one, read, record, room};
use crate::racks::LOCK_TTL_SECS;
use crate::synoptic::project_nodes;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::Response;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use strata_domain::catalog::{FieldError, RefKind};
use strata_domain::plan::{
    End, FloorData, MapData, MapKind, ObjectData, Waypoint, floor_bases, level_name,
    measure_length, route_length_m, route_points, validate_floor, validate_map, validate_object,
    validate_route,
};
use strata_domain::{Domain, Level};
use strata_store::plans::{Floor, PlanMap, PlanObject};
use strata_store::{Container, LockOutcome, Store};

const DESIGN: &str = "Vous n'avez pas le droit de modifier la conception.";
const MAX_BG: usize = 15 * 1024 * 1024;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects/{id}/plans", get(index).post(create))
        .route("/projects/{id}/plans/files/{file}", get(file))
        .route("/projects/{id}/plans/{map}", get(show).patch(rename))
        .route("/projects/{id}/plans/{map}/delete", post(delete_map))
        .route(
            "/projects/{id}/plans/{map}/floors",
            post(floor_add).patch(floor_update),
        )
        .route(
            "/projects/{id}/plans/{map}/floors/delete",
            post(floor_delete),
        )
        .route(
            "/projects/{id}/plans/{map}/floors/{floor}/background",
            put(bg_put)
                .delete(bg_delete)
                .layer(DefaultBodyLimit::max(MAX_BG + 1024)),
        )
        .route(
            "/projects/{id}/plans/{map}/objects",
            post(obj_create).patch(obj_update),
        )
        .route(
            "/projects/{id}/plans/{map}/objects/delete",
            post(obj_delete),
        )
        .route("/projects/{id}/plans/{map}/links", post(link_create))
        .route("/projects/{id}/plans/{map}/links/delete", post(link_delete))
        .route("/projects/{id}/plans/{map}/routes", put(route_set))
        .route(
            "/projects/{id}/plans/{map}/routes/delete",
            post(route_delete),
        )
        .route(
            "/projects/{id}/plans/{map}/lock",
            post(take).delete(release),
        )
        .route("/projects/{id}/plans/{map}/lock/request", post(request))
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

fn lock_key(map: &str) -> String {
    format!("map:{map}")
}

async fn map_of(store: &Store, pid: &str, map: &str) -> ApiResult<PlanMap> {
    store
        .plan_maps(pid)
        .await?
        .into_iter()
        .find(|m| m.id == map)
        .ok_or(ApiError::NotFound("Plan"))
}

/// Écriture : droit « Conception : écriture » et main tenue sur la carte.
async fn editing(st: &AppState, m: &Member, pid: &str, map: &str) -> ApiResult<PlanMap> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    let pm = map_of(&st.store, pid, map).await?;
    if !st
        .store
        .holds_lock(&lock_key(map), &m.0.token_hash, LOCK_TTL_SECS)
        .await?
    {
        let who = st.store.lock(&lock_key(map)).await?.map(|l| l.account_name);
        return Err(ApiError::Conflict(match who {
            Some(n) => format!("{n} a la main sur ce plan : vous le voyez en lecture seule."),
            None => "Prenez la main sur le plan pour le modifier.".into(),
        }));
    }
    Ok(pm)
}

// ----- Liaisons entre contenants -----

struct Liaison {
    key: String,
    a: String,
    b: String,
    count: usize,
    numbers: Vec<String>,
    family: Option<String>,
}

/// Une liaison par paire de contenants reliés par au moins un câble.
async fn liaisons(store: &Store, pid: &str) -> ApiResult<Vec<Liaison>> {
    let nodes = project_nodes(store, pid).await?;
    let port: HashMap<&str, (Option<&str>, Option<&str>)> = nodes
        .iter()
        .flat_map(|n| {
            n.ports.iter().map(move |p| {
                (
                    p.r.as_str(),
                    (n.container_id.as_deref(), p.family.as_deref()),
                )
            })
        })
        .collect();
    let mut by: BTreeMap<String, Liaison> = BTreeMap::new();
    let mut fams: HashMap<String, HashMap<String, usize>> = HashMap::new();
    for c in store.cables(pid).await? {
        if c.fields.attach || c.fields.external.is_some() {
            continue;
        }
        let (Some((Some(ca), fa)), Some((Some(cb), fb))) =
            (port.get(c.a.as_str()), port.get(c.b.as_str()))
        else {
            continue;
        };
        if ca == cb {
            continue;
        }
        let (x, y) = if ca < cb { (*ca, *cb) } else { (*cb, *ca) };
        let key = format!("{x}|{y}");
        let l = by.entry(key.clone()).or_insert_with(|| Liaison {
            key: key.clone(),
            a: x.to_string(),
            b: y.to_string(),
            count: 0,
            numbers: Vec::new(),
            family: None,
        });
        l.count += 1;
        if let Some(n) = &c.fields.number {
            l.numbers.push(n.clone());
        }
        if let Some(f) = fa.or(*fb) {
            *fams
                .entry(key)
                .or_default()
                .entry(f.to_string())
                .or_default() += 1;
        }
    }
    Ok(by
        .into_values()
        .map(|mut l| {
            l.family = fams.get(&l.key).and_then(|m| {
                m.iter()
                    .max_by_key(|(k, n)| (**n, std::cmp::Reverse((*k).clone())))
                    .map(|(k, _)| k.clone())
            });
            l
        })
        .collect())
}

/// Encombrement d'un contenant (largeur, profondeur, hauteur en mm).
fn dims(c: &Container) -> [f64; 3] {
    let d = &c.data;
    let w = d.width_mm.unwrap_or(600.0);
    let p = d.length_mm.or(d.depth_mm).unwrap_or(600.0);
    let h = d
        .outer_height_mm
        .or(d.height_u.map(|u| u * 44.45 + 150.0))
        .unwrap_or(1000.0);
    [w, p, h]
}

fn end_of(o: &PlanObject, floors: &[Floor]) -> Option<End> {
    let ObjectData::Container { x, y, z, .. } = &o.data else {
        return None;
    };
    let f = floors.iter().find(|f| f.id == o.floor_id)?;
    Some(End {
        x: *x,
        y: *y,
        level: f.data.level,
        z: *z,
    })
}

/// Longueurs des tracés du projet, par paire de contenants (clé « a|b »).
pub(crate) async fn route_lengths(store: &Store, pid: &str) -> ApiResult<HashMap<String, f64>> {
    let routes = store.plan_routes(pid).await?;
    if routes.is_empty() {
        return Ok(HashMap::new());
    }
    let objects = store.plan_objects(pid, None).await?;
    let placed: HashMap<&str, &PlanObject> = objects
        .iter()
        .filter_map(|o| match &o.data {
            ObjectData::Container { container_id, .. } => Some((container_id.as_str(), o)),
            _ => None,
        })
        .collect();
    let mut floors: HashMap<String, Vec<Floor>> = HashMap::new();
    let mut out = HashMap::new();
    for r in routes {
        let Some((a, b)) = r.key.split_once('|') else {
            continue;
        };
        let (Some(oa), Some(ob)) = (placed.get(a), placed.get(b)) else {
            continue;
        };
        if oa.map_id != r.map_id || ob.map_id != r.map_id {
            continue;
        }
        if !floors.contains_key(&r.map_id) {
            floors.insert(r.map_id.clone(), store.plan_floors(&r.map_id).await?);
        }
        let fl = &floors[&r.map_id];
        let fd: Vec<FloorData> = fl.iter().map(|f| f.data.clone()).collect();
        if let (Some(ea), Some(eb)) = (end_of(oa, fl), end_of(ob, fl)) {
            if let Some(l) = route_length_m(ea, &r.waypoints, eb, &fd) {
                out.insert(r.key.clone(), l);
            }
        }
    }
    Ok(out)
}

// ----- Lecture -----

async fn index(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    st.store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    let maps = st.store.plan_maps(&pid).await?;
    let objects = st.store.plan_objects(&pid, None).await?;
    let mut out = Vec::new();
    for mp in &maps {
        let floors = st.store.plan_floors(&mp.id).await?;
        out.push(json!({
            "id": mp.id, "name": mp.data.name, "kind": mp.data.kind,
            "floors": floors.iter().map(|f| json!({ "id": f.id, "level": f.data.level, "name": f.data.name })).collect::<Vec<_>>(),
            "containers": objects.iter().filter(|o| o.map_id == mp.id && matches!(o.data, ObjectData::Container { .. })).count(),
        }));
    }
    let inv = st.store.inventory(&pid).await?;
    let placed = objects
        .iter()
        .filter(|o| matches!(o.data, ObjectData::Container { .. }))
        .count();
    let li = liaisons(&st.store, &pid).await?;
    let lens = route_lengths(&st.store, &pid).await?;
    Ok(Json(json!({
        "maps": out,
        "stats": {
            "containers": inv.containers.len(),
            "placed": placed,
            "liaisons": li.len(),
            "traced": li.iter().filter(|l| lens.contains_key(&l.key)).count(),
        },
    })))
}

fn fresh(beat_at: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(beat_at)
        .map(|t| (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_seconds() < LOCK_TTL_SECS)
        .unwrap_or(false)
}

async fn show(
    State(st): State<AppState>,
    m: Member,
    Path((pid, map)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    let pm = map_of(&st.store, &pid, &map).await?;
    let floors = st.store.plan_floors(&map).await?;
    let fd: Vec<FloorData> = floors.iter().map(|f| f.data.clone()).collect();
    let bases = floor_bases(&fd);
    let all = st.store.plan_objects(&pid, None).await?;
    let inv = st.store.inventory(&pid).await?;
    let cont: HashMap<&str, &Container> =
        inv.containers.iter().map(|c| (c.id.as_str(), c)).collect();
    let links = st.store.plan_links(&map).await?;
    let refs = st.store.list_refs(None).await?;
    let color = |kind: RefKind, id: &str| {
        refs.iter()
            .find(|r| r.kind == kind && r.id == id)
            .and_then(|r| r.data.get("color").and_then(Value::as_str))
            .map(str::to_string)
    };
    let sig_family = |id: &str| {
        refs.iter()
            .find(|r| r.kind == RefKind::Signal && r.id == id)
            .and_then(|r| r.data.get("function").and_then(Value::as_str))
            .map(str::to_string)
    };
    let mut objects = Vec::new();
    for o in all.iter().filter(|o| o.map_id == map) {
        let mut v = serde_json::to_value(o).unwrap_or_default();
        match &o.data {
            ObjectData::Container { container_id, .. } => {
                if let Some(c) = cont.get(container_id.as_str()) {
                    v["name"] = json!(c.name);
                    v["element_kind"] = json!(c.data.kind);
                    v["dims"] = json!(dims(c));
                    v["location_id"] = json!(c.location_id);
                }
            }
            ObjectData::Point {
                ports, signal_id, ..
            } => {
                let used = links.iter().filter(|l| l.point_id == o.id).count();
                v["used"] = json!(used);
                v["free"] = json!((*ports as usize).saturating_sub(used));
                v["color"] = json!(
                    signal_id
                        .as_deref()
                        .and_then(|s| color(RefKind::Signal, s))
                        .or_else(|| signal_id
                            .as_deref()
                            .and_then(sig_family)
                            .and_then(|f| color(RefKind::Function, &f)))
                );
            }
            ObjectData::Measure { points } => {
                v["length_mm"] = json!(measure_length(points));
            }
            ObjectData::Text { .. } => {}
        }
        objects.push(v);
    }
    // Liaisons : à tracer si les deux contenants sont posés sur cette carte.
    let placed: HashMap<&str, &PlanObject> = all
        .iter()
        .filter_map(|o| match &o.data {
            ObjectData::Container { container_id, .. } => Some((container_id.as_str(), o)),
            _ => None,
        })
        .collect();
    let routes: HashMap<String, Vec<Waypoint>> = st
        .store
        .plan_routes(&pid)
        .await?
        .into_iter()
        .filter(|r| r.map_id == map)
        .map(|r| (r.key, r.waypoints))
        .collect();
    let mut lis = Vec::new();
    for l in liaisons(&st.store, &pid).await? {
        let (oa, ob) = (placed.get(l.a.as_str()), placed.get(l.b.as_str()));
        let here = |o: Option<&&PlanObject>| o.is_some_and(|o| o.map_id == map);
        if !here(oa) && !here(ob) {
            continue;
        }
        let state = match (oa, ob) {
            (Some(a), Some(b)) if a.map_id == map && b.map_id == map => "ready",
            (Some(_), Some(_)) => "other_map",
            _ => "unplaced",
        };
        let route = routes.get(&l.key);
        let (length, pts) = match (
            state,
            route,
            oa.and_then(|o| end_of(o, &floors)),
            ob.and_then(|o| end_of(o, &floors)),
        ) {
            ("ready", Some(w), Some(a), Some(b)) => {
                (route_length_m(a, w, b, &fd), route_points(a, w, b, &fd))
            }
            _ => (None, None),
        };
        lis.push(json!({
            "key": l.key,
            "a": { "id": l.a, "name": cont.get(l.a.as_str()).map(|c| c.name.clone()) },
            "b": { "id": l.b, "name": cont.get(l.b.as_str()).map(|c| c.name.clone()) },
            "count": l.count,
            "numbers": l.numbers,
            "family": l.family,
            "color": l.family.as_deref().and_then(|f| color(RefKind::Function, f)),
            "state": state,
            "waypoints": route,
            "points3d": pts,
            "length_m": length,
        }));
    }
    let unplaced: Vec<Value> = inv
        .containers
        .iter()
        .filter(|c| !placed.contains_key(c.id.as_str()))
        .map(|c| json!({ "id": c.id, "name": c.name, "kind": c.data.kind, "dims": dims(c) }))
        .collect();
    let lock = st.store.lock(&lock_key(&map)).await?;
    Ok(Json(json!({
        "map": pm,
        "floors": floors.iter().map(|f| {
            let mut v = serde_json::to_value(f).unwrap_or_default();
            v["base_mm"] = json!(bases.get(&f.data.level));
            v
        }).collect::<Vec<_>>(),
        "objects": objects,
        "links": links,
        "liaisons": lis,
        "unplaced": unplaced,
        "lock": lock.filter(|l| fresh(&l.beat_at)).map(|l| json!({ "account_id": l.account_id, "account_name": l.account_name, "taken_at": l.taken_at, "mine": l.token_hash == m.0.token_hash })),
    })))
}

// ----- Cartes et étages -----

#[derive(Deserialize)]
struct CreateBody {
    #[serde(flatten)]
    map: MapData,
    /// Niveaux à créer (assistant) ; par défaut le rez-de-chaussée.
    #[serde(default)]
    levels: Vec<i32>,
}

async fn create(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(mut b): Json<CreateBody>,
) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    st.store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    to_fields(validate_map(&mut b.map))?;
    let mut levels = b.levels.clone();
    levels.sort_unstable();
    levels.dedup();
    if levels.is_empty() {
        levels.push(0);
    }
    if b.map.kind != MapKind::MultiFloor && levels.len() > 1 {
        return Err(one(
            "levels",
            "Un plan à plat ou extérieur n'a qu'un niveau.",
        ));
    }
    let mut floors = Vec::new();
    for l in levels {
        let mut f = FloorData {
            level: l,
            name: if b.map.kind == MapKind::Outdoor {
                "Extérieur".into()
            } else {
                level_name(l)
            },
            ..Default::default()
        };
        to_fields(validate_floor(&mut f))?;
        floors.push(f);
    }
    let id = st.store.insert_plan_map(&pid, &b.map, &floors).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "plan",
        &id,
        "create",
        None,
        None::<&Value>,
        Some(&b.map),
    )
    .await?;
    Ok(Json(json!({ "id": id })))
}

async fn rename(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, map)): Path<(String, String)>,
    Json(mut b): Json<MapData>,
) -> ApiResult<Json<Value>> {
    let before = editing(&st, &m, &pid, &map).await?;
    to_fields(validate_map(&mut b))?;
    let floors = st.store.plan_floors(&map).await?;
    if b.kind != MapKind::MultiFloor && floors.len() > 1 {
        return Err(one(
            "kind",
            "Ce plan a plusieurs étages : il reste multi-étages.",
        ));
    }
    st.store.update_plan_map(&pid, &map, &b).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "plan",
        &map,
        "update",
        None,
        Some(&before.data),
        Some(&b),
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn delete_map(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, map)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    let before = editing(&st, &m, &pid, &map).await?;
    st.store.delete_plan_map(&pid, &map).await?;
    st.store
        .release_lock(&lock_key(&map), &m.0.token_hash)
        .await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "plan",
        &map,
        "delete",
        None,
        Some(&before.data),
        None::<&Value>,
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn floor_add(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, map)): Path<(String, String)>,
    Json(mut f): Json<FloorData>,
) -> ApiResult<Json<Value>> {
    let pm = editing(&st, &m, &pid, &map).await?;
    if pm.data.kind != MapKind::MultiFloor {
        return Err(one(
            "level",
            "Seul un plan multi-étages reçoit d'autres étages.",
        ));
    }
    f.background = None;
    f.calibration = None;
    to_fields(validate_floor(&mut f))?;
    if st
        .store
        .plan_floors(&map)
        .await?
        .iter()
        .any(|x| x.data.level == f.level)
    {
        return Err(one("level", "Ce niveau existe déjà sur le plan."));
    }
    let id = st.store.insert_floor(&map, &f).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "plan",
        &map,
        "floor_add",
        None,
        None::<&Value>,
        Some(&f),
    )
    .await?;
    Ok(Json(json!({ "id": id })))
}

#[derive(Deserialize)]
struct FloorBody {
    id: String,
    #[serde(flatten)]
    data: FloorData,
}

async fn floor_update(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, map)): Path<(String, String)>,
    Json(mut b): Json<FloorBody>,
) -> ApiResult<Json<Value>> {
    editing(&st, &m, &pid, &map).await?;
    let before = st
        .store
        .plan_floors(&map)
        .await?
        .into_iter()
        .find(|f| f.id == b.id)
        .ok_or(ApiError::NotFound("Étage"))?;
    // Le niveau est figé ; le fond se change par sa propre route.
    b.data.level = before.data.level;
    b.data.background = before.data.background.clone();
    b.data.bg_type = before.data.bg_type.clone();
    b.data.bg_w = before.data.bg_w;
    b.data.bg_h = before.data.bg_h;
    to_fields(validate_floor(&mut b.data))?;
    st.store.update_floor(&map, &b.id, &b.data).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "plan",
        &map,
        "floor_update",
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

async fn floor_delete(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, map)): Path<(String, String)>,
    Json(b): Json<IdBody>,
) -> ApiResult<Json<Value>> {
    editing(&st, &m, &pid, &map).await?;
    let floors = st.store.plan_floors(&map).await?;
    let before = floors
        .iter()
        .find(|f| f.id == b.id)
        .ok_or(ApiError::NotFound("Étage"))?;
    if floors.len() == 1 {
        return Err(ApiError::Invalid("Un plan garde au moins un étage.".into()));
    }
    st.store.delete_floor(&map, &b.id).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "plan",
        &map,
        "floor_delete",
        None,
        Some(&before.data),
        None::<&Value>,
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct BgQuery {
    w: f64,
    h: f64,
}

/// Fond de plan PNG, JPG ou SVG (15 Mo au plus), remplaçable.
async fn bg_put(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, map, floor)): Path<(String, String, String)>,
    Query(q): Query<BgQuery>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    editing(&st, &m, &pid, &map).await?;
    let mut f = st
        .store
        .plan_floors(&map)
        .await?
        .into_iter()
        .find(|f| f.id == floor)
        .ok_or(ApiError::NotFound("Étage"))?;
    if body.len() > MAX_BG {
        return Err(ApiError::Invalid("Fond de plan de 15 Mo au plus.".into()));
    }
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
        "image/svg+xml" => {
            let head = String::from_utf8_lossy(&body[..body.len().min(4096)]).to_lowercase();
            head.contains("<svg")
        }
        _ => false,
    };
    if !ok {
        return Err(ApiError::Invalid(
            "Fond de plan : une image PNG, JPG ou SVG.".into(),
        ));
    }
    if !(q.w > 0.0 && q.h > 0.0 && q.w <= 100_000.0 && q.h <= 100_000.0) {
        return Err(ApiError::Invalid(
            "Dimensions de l'image illisibles.".into(),
        ));
    }
    let id = st.store.insert_plan_file(&pid, &ct, &body).await?;
    if let Some(old) = f.data.background.replace(id.clone()) {
        st.store.delete_plan_file(&old).await?;
    }
    // Un nouveau fond garde la calibration seulement s'il a les mêmes dimensions.
    if f.data.bg_w != Some(q.w) || f.data.bg_h != Some(q.h) {
        f.data.calibration = None;
    }
    f.data.bg_type = Some(ct);
    f.data.bg_w = Some(q.w);
    f.data.bg_h = Some(q.h);
    st.store.update_floor(&map, &floor, &f.data).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "plan",
        &map,
        "background",
        None,
        None::<&Value>,
        Some(&json!({ "floor": floor })),
    )
    .await?;
    Ok(Json(json!({ "file": id })))
}

async fn bg_delete(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, map, floor)): Path<(String, String, String)>,
) -> ApiResult<Json<Value>> {
    editing(&st, &m, &pid, &map).await?;
    let mut f = st
        .store
        .plan_floors(&map)
        .await?
        .into_iter()
        .find(|f| f.id == floor)
        .ok_or(ApiError::NotFound("Étage"))?;
    if let Some(old) = f.data.background.take() {
        st.store.delete_plan_file(&old).await?;
    }
    f.data.bg_type = None;
    f.data.bg_w = None;
    f.data.bg_h = None;
    f.data.calibration = None;
    st.store.update_floor(&map, &floor, &f.data).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "plan",
        &map,
        "background",
        None,
        None::<&Value>,
        Some(&json!({ "floor": floor, "removed": true })),
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn file(
    State(st): State<AppState>,
    m: Member,
    Path((pid, id)): Path<(String, String)>,
) -> ApiResult<Response> {
    read(&m)?;
    let (ct, bytes) = st
        .store
        .plan_file(&pid, &id)
        .await?
        .ok_or(ApiError::NotFound("Fichier"))?;
    Response::builder()
        .header(header::CONTENT_TYPE, ct)
        .header(
            header::CACHE_CONTROL,
            "private, max-age=31536000, immutable",
        )
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; style-src 'unsafe-inline'; img-src data:",
        )
        .body(axum::body::Body::from(bytes))
        .map_err(|e| ApiError::Internal(e.to_string()))
}

// ----- Objets -----

#[derive(Deserialize)]
struct ObjBody {
    #[serde(default)]
    id: Option<String>,
    floor_id: String,
    #[serde(flatten)]
    data: ObjectData,
}

async fn check_object(store: &Store, pid: &str, map: &str, b: &mut ObjBody) -> ApiResult<()> {
    to_fields(validate_object(&mut b.data))?;
    if !store
        .plan_floors(map)
        .await?
        .iter()
        .any(|f| f.id == b.floor_id)
    {
        return Err(one("floor_id", "Cet étage n'est pas sur le plan."));
    }
    if let ObjectData::Container { container_id, .. } = &b.data {
        if !store
            .inventory(pid)
            .await?
            .containers
            .iter()
            .any(|c| &c.id == container_id)
        {
            return Err(one("container_id", "Cet élément n'est pas dans le projet."));
        }
    }
    if let ObjectData::Point {
        signal_id: Some(s), ..
    } = &b.data
    {
        if !store
            .list_refs(Some(RefKind::Signal))
            .await?
            .iter()
            .any(|r| &r.id == s)
        {
            return Err(one("signal_id", "Signal inconnu."));
        }
    }
    Ok(())
}

async fn obj_create(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, map)): Path<(String, String)>,
    Json(mut b): Json<ObjBody>,
) -> ApiResult<Json<Value>> {
    editing(&st, &m, &pid, &map).await?;
    check_object(&st.store, &pid, &map, &mut b).await?;
    let id = st
        .store
        .insert_plan_object(&pid, &map, &b.floor_id, &b.data)
        .await
        .map_err(|e| match e {
            strata_store::StoreError::Conflict(_) => {
                ApiError::Conflict("Cet élément est déjà posé sur un plan du projet.".into())
            }
            e => e.into(),
        })?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "plan",
        &map,
        "object_create",
        None,
        None::<&Value>,
        Some(&b.data),
    )
    .await?;
    Ok(Json(json!({ "id": id })))
}

async fn obj_update(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, map)): Path<(String, String)>,
    Json(mut b): Json<ObjBody>,
) -> ApiResult<Json<Value>> {
    editing(&st, &m, &pid, &map).await?;
    let id = b.id.clone().ok_or_else(|| one("id", "Objet manquant."))?;
    let before = st
        .store
        .plan_objects(&pid, Some(&map))
        .await?
        .into_iter()
        .find(|o| o.id == id)
        .ok_or(ApiError::NotFound("Objet"))?;
    check_object(&st.store, &pid, &map, &mut b).await?;
    match (&before.data, &b.data) {
        (
            ObjectData::Container {
                container_id: a, ..
            },
            ObjectData::Container {
                container_id: c, ..
            },
        ) if a == c => {}
        (ObjectData::Container { .. }, _) | (_, ObjectData::Container { .. }) => {
            return Err(ApiError::Invalid(
                "La nature d'un objet ne change pas.".into(),
            ));
        }
        (x, y) if std::mem::discriminant(x) != std::mem::discriminant(y) => {
            return Err(ApiError::Invalid(
                "La nature d'un objet ne change pas.".into(),
            ));
        }
        _ => {}
    }
    if let ObjectData::Point { ports, .. } = &b.data {
        let used = st
            .store
            .plan_links(&map)
            .await?
            .iter()
            .filter(|l| l.point_id == id)
            .count();
        if (*ports as usize) < used {
            return Err(one(
                "ports",
                format!("{used} liens partent de ce point : gardez au moins {used} ports."),
            ));
        }
    }
    st.store
        .update_plan_object(&map, &id, &b.floor_id, &b.data)
        .await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "plan",
        &map,
        "object_update",
        None,
        Some(&before.data),
        Some(&b.data),
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct IdsBody {
    ids: Vec<String>,
}

async fn obj_delete(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, map)): Path<(String, String)>,
    Json(b): Json<IdsBody>,
) -> ApiResult<Json<Value>> {
    editing(&st, &m, &pid, &map).await?;
    let n = st.store.delete_plan_objects(&map, &b.ids).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "plan",
        &map,
        "object_delete",
        None,
        None::<&Value>,
        Some(&json!({ "ids": b.ids })),
    )
    .await?;
    Ok(Json(json!({ "deleted": n })))
}

// ----- Liens libres -----

#[derive(Deserialize)]
struct LinkBody {
    point_id: String,
    target_id: String,
    #[serde(default)]
    label: Option<String>,
}

async fn link_create(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, map)): Path<(String, String)>,
    Json(b): Json<LinkBody>,
) -> ApiResult<Json<Value>> {
    editing(&st, &m, &pid, &map).await?;
    let objs = st.store.plan_objects(&pid, Some(&map)).await?;
    let p = objs
        .iter()
        .find(|o| o.id == b.point_id)
        .ok_or_else(|| one("point_id", "Point de connexion introuvable sur ce plan."))?;
    let ObjectData::Point { ports, .. } = &p.data else {
        return Err(one("point_id", "Un lien part d'un point de connexion."));
    };
    let t = objs
        .iter()
        .find(|o| o.id == b.target_id)
        .ok_or_else(|| one("target_id", "Cible introuvable sur ce plan."))?;
    if !matches!(
        t.data,
        ObjectData::Container { .. } | ObjectData::Point { .. }
    ) || t.id == p.id
    {
        return Err(one(
            "target_id",
            "Un lien va vers un élément ou un autre point.",
        ));
    }
    let used = st
        .store
        .plan_links(&map)
        .await?
        .iter()
        .filter(|l| l.point_id == p.id)
        .count();
    if used >= *ports as usize {
        return Err(one(
            "point_id",
            "Plus de port libre sur ce point de connexion.",
        ));
    }
    let label = b.label.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if label.is_some_and(|l| l.chars().count() > 60) {
        return Err(one("label", "60 caractères au plus."));
    }
    let id = st.store.insert_plan_link(&map, &p.id, &t.id, label).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "plan",
        &map,
        "link_create",
        None,
        None::<&Value>,
        Some(&json!({ "point": p.id, "target": t.id })),
    )
    .await?;
    Ok(Json(json!({ "id": id })))
}

async fn link_delete(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, map)): Path<(String, String)>,
    Json(b): Json<IdBody>,
) -> ApiResult<Json<Value>> {
    editing(&st, &m, &pid, &map).await?;
    st.store.delete_plan_link(&map, &b.id).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "plan",
        &map,
        "link_delete",
        None,
        None::<&Value>,
        Some(&json!({ "id": b.id })),
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

// ----- Tracés -----

#[derive(Deserialize)]
struct RouteBody {
    key: String,
    waypoints: Vec<Waypoint>,
}

async fn route_set(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, map)): Path<(String, String)>,
    Json(b): Json<RouteBody>,
) -> ApiResult<Json<Value>> {
    editing(&st, &m, &pid, &map).await?;
    if !liaisons(&st.store, &pid)
        .await?
        .iter()
        .any(|l| l.key == b.key)
    {
        return Err(one("key", "Aucune liaison entre ces deux éléments."));
    }
    let floors = st.store.plan_floors(&map).await?;
    let fd: Vec<FloorData> = floors.iter().map(|f| f.data.clone()).collect();
    to_fields(validate_route(&b.waypoints, &fd))?;
    let (a, c) = b.key.split_once('|').unwrap_or_default();
    let objs = st.store.plan_objects(&pid, Some(&map)).await?;
    let on_map = |cid: &str| {
        objs.iter().any(|o| matches!(&o.data, ObjectData::Container { container_id, .. } if container_id == cid))
    };
    if !on_map(a) || !on_map(c) {
        return Err(one("key", "Posez d'abord les deux éléments sur ce plan."));
    }
    st.store
        .set_plan_route(&pid, &b.key, &map, &b.waypoints)
        .await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "plan",
        &map,
        "route_set",
        None,
        None::<&Value>,
        Some(&json!({ "key": b.key, "points": b.waypoints.len() })),
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct KeyBody {
    key: String,
}

async fn route_delete(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, map)): Path<(String, String)>,
    Json(b): Json<KeyBody>,
) -> ApiResult<Json<Value>> {
    editing(&st, &m, &pid, &map).await?;
    st.store.delete_plan_route(&pid, &b.key).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "plan",
        &map,
        "route_delete",
        None,
        None::<&Value>,
        Some(&json!({ "key": b.key })),
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

// ----- Main sur la carte -----

async fn take(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, map)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    map_of(&st.store, &pid, &map).await?;
    match st
        .store
        .take_lock(
            &lock_key(&map),
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
                    &lock_key(&map),
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
    Path((pid, map)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    if st
        .store
        .release_lock(&lock_key(&map), &m.0.token_hash)
        .await?
    {
        st.hub.change(
            room(&pid),
            "lock",
            &lock_key(&map),
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
    Path((pid, map)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    st.hub.change(
        room(&pid),
        "lock",
        &lock_key(&map),
        "request",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(StatusCode::NO_CONTENT)
}
