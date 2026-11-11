//! Dessins et plan de feu (section 11.2) : pages de quatre natures, blocs équipement tirés de
//! la nomenclature (un exemplaire posé une fois par nature), synchronisation, adressage DMX.
//! Écran de conception : une main par page ; enregistrement automatique avec révision.

use crate::AppState;
use crate::auth::Member;
use crate::error::{ApiError, ApiResult};
use crate::live::Origin;
use crate::projects::{one, read, record, room};
use crate::racks::LOCK_TTL_SECS;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{StatusCode, header};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use strata_domain::catalog::{FieldError, RefKind};
use strata_domain::drawing::{
    DmxPatch, Element, Fixture, Nature, PageData, Snapshot, channels, conflicts, suggest,
    validate_page,
};
use strata_domain::project::ArticleNature;
use strata_domain::{Domain, Level};
use strata_store::drawings::Page;
use strata_store::{LockOutcome, Store};

const DESIGN: &str = "Vous n'avez pas le droit de modifier la conception.";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects/{id}/drawings", get(index).post(create))
        .route("/projects/{id}/drawings/order", post(order))
        .route(
            "/projects/{id}/drawings/files",
            post(upload).layer(DefaultBodyLimit::max(10 * 1024 * 1024 + 1024)),
        )
        .route("/projects/{id}/drawings/dmx/suggest", post(dmx_suggest))
        .route("/projects/{id}/drawings/dmx/auto", post(dmx_auto))
        .route("/projects/{id}/drawings/{page}", get(show).put(save))
        .route("/projects/{id}/drawings/{page}/delete", post(delete))
        .route("/projects/{id}/drawings/{page}/sync", post(sync))
        .route(
            "/projects/{id}/drawings/{page}/lock",
            post(take).delete(release),
        )
        .route("/projects/{id}/drawings/{page}/lock/request", post(request))
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

fn lock_key(page: &str) -> String {
    format!("draw:{page}")
}

async fn page_of(store: &Store, pid: &str, page: &str) -> ApiResult<Page> {
    store
        .drawing(pid, page)
        .await?
        .ok_or(ApiError::NotFound("Page"))
}

async fn editing(st: &AppState, m: &Member, pid: &str, page: &str) -> ApiResult<Page> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    let p = page_of(&st.store, pid, page).await?;
    if !st
        .store
        .holds_lock(&lock_key(page), &m.0.token_hash, LOCK_TTL_SECS)
        .await?
    {
        let who = st
            .store
            .lock(&lock_key(page))
            .await?
            .map(|l| l.account_name);
        return Err(ApiError::Conflict(match who {
            Some(n) => format!("{n} a la main sur cette page : vous la voyez en lecture seule."),
            None => "Prenez la main sur la page pour la modifier.".into(),
        }));
    }
    Ok(p)
}

// ----- Nomenclature -----

struct BomLine {
    instance_id: String,
    functions: Vec<String>,
    snapshot: Snapshot,
}

/// Exemplaires d'équipements du projet, avec ce qu'un bloc en garde.
async fn bom(store: &Store, pid: &str) -> ApiResult<Vec<BomLine>> {
    let inv = store.inventory(pid).await?;
    let cats: HashMap<String, String> = store
        .list_refs(Some(RefKind::Category))
        .await?
        .into_iter()
        .map(|r| (r.id, r.name))
        .collect();
    let mut items = HashMap::new();
    let mut out = Vec::new();
    for a in inv
        .articles
        .iter()
        .filter(|a| a.nature == ArticleNature::Equipment)
    {
        let Some(item_id) = &a.item_id else { continue };
        if !items.contains_key(item_id) {
            items.insert(item_id.clone(), store.item(item_id).await?);
        }
        let Some(Some(it)) = items.get(item_id) else {
            continue;
        };
        let d = &it.data;
        let functions: Vec<String> = d
            .get("functions")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let images: Vec<String> = d
            .get("documents")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.get("url").and_then(Value::as_str))
                    .filter(|u| {
                        let l = u.to_lowercase();
                        [".png", ".jpg", ".jpeg", ".svg", ".webp"]
                            .iter()
                            .any(|e| l.split('?').next().unwrap_or("").ends_with(e))
                    })
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let dmx = d.get("dmx").filter(|v| !v.is_null());
        for inst in inv.instances.iter().filter(|i| i.article_id == a.id) {
            out.push(BomLine {
                instance_id: inst.id.clone(),
                functions: functions.clone(),
                snapshot: Snapshot {
                    item_id: Some(it.id.clone()),
                    name: it.name.clone(),
                    reference: d
                        .get("reference")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    category: d
                        .get("category_id")
                        .and_then(Value::as_str)
                        .and_then(|c| cats.get(c).cloned()),
                    tag: inst.tag.clone(),
                    function: functions.first().cloned(),
                    images: images.clone(),
                    dmx_channels: dmx
                        .and_then(|x| x.get("default_channels"))
                        .and_then(Value::as_u64)
                        .map(|v| v as u32),
                    dmx_modes: dmx
                        .and_then(|x| x.get("modes"))
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(|m| {
                                    Some((
                                        m.get("name")?.as_str()?.to_string(),
                                        m.get("channels")?.as_u64()? as u32,
                                    ))
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                },
            });
        }
    }
    Ok(out)
}

/// Projecteurs posés sur tous les plans de feu du projet.
fn fixtures(pages: &[Page]) -> Vec<Fixture> {
    let mut out = Vec::new();
    for p in pages.iter().filter(|p| p.nature == Nature::Lighting) {
        for e in &p.data.elements {
            if let Element::Equipment {
                id, snapshot, dmx, ..
            } = e
            {
                out.push(Fixture {
                    key: format!("{}:{id}", p.id),
                    item_id: snapshot.item_id.clone(),
                    patch: dmx.clone(),
                    channels: channels(snapshot, dmx.as_ref().and_then(|d| d.mode.as_deref()))
                        .unwrap_or(1),
                });
            }
        }
    }
    out
}

fn fresh(beat_at: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(beat_at)
        .map(|t| (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_seconds() < LOCK_TTL_SECS)
        .unwrap_or(false)
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
    let pages = st.store.drawings(&pid).await?;
    let lines = bom(&st.store, &pid).await?;
    let mut placed: HashMap<(&str, Nature), (&str, &str)> = HashMap::new();
    for p in &pages {
        for e in &p.data.elements {
            if let Element::Equipment {
                instance_id, id, ..
            } = e
            {
                placed.insert(
                    (instance_id.as_str(), p.nature),
                    (p.id.as_str(), id.as_str()),
                );
            }
        }
    }
    let placed_of = |inst: &str| -> Vec<Value> {
        [
            Nature::Schematic,
            Nature::Video,
            Nature::Audio,
            Nature::Lighting,
        ]
        .iter()
        .filter_map(|n| {
            placed
                .get(&(inst, *n))
                .map(|(pg, el)| json!({ "nature": n, "page": pg, "element": el }))
        })
        .collect()
    };
    let fx = fixtures(&pages);
    let conf = conflicts(&fx);
    Ok(Json(json!({
        "pages": pages.iter().map(|p| json!({
            "id": p.id, "nature": p.nature, "name": p.data.name, "position": p.position, "rev": p.rev,
            "count": p.data.elements.len(), "updated_at": p.updated_at,
        })).collect::<Vec<_>>(),
        "bom": lines.iter().map(|l| json!({
            "instance_id": l.instance_id, "functions": l.functions, "snapshot": l.snapshot,
            "placed": placed_of(&l.instance_id),
        })).collect::<Vec<_>>(),
        "dmx": {
            "fixtures": fx.iter().map(|f| json!({ "key": f.key, "item_id": f.item_id, "patch": f.patch, "channels": f.channels })).collect::<Vec<_>>(),
            "conflicts": conf,
        },
    })))
}

async fn show(
    State(st): State<AppState>,
    m: Member,
    Path((pid, page)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    let p = page_of(&st.store, &pid, &page).await?;
    let lock = st.store.lock(&lock_key(&page)).await?;
    let mut v = serde_json::to_value(&p).unwrap_or_default();
    v["lock"] = json!(lock.filter(|l| fresh(&l.beat_at)).map(|l| json!({ "account_id": l.account_id, "account_name": l.account_name, "taken_at": l.taken_at, "mine": l.token_hash == m.0.token_hash })));
    Ok(Json(v))
}

// ----- Écriture -----

#[derive(Deserialize)]
struct CreateBody {
    nature: Nature,
    #[serde(default)]
    name: String,
}

async fn create(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<CreateBody>,
) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    st.store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    let n = st
        .store
        .drawings(&pid)
        .await?
        .iter()
        .filter(|p| p.nature == b.nature)
        .count();
    let mut data = PageData {
        name: if b.name.trim().is_empty() {
            format!("{} {}", b.nature.label(), n + 1)
        } else {
            b.name.clone()
        },
        ..Default::default()
    };
    to_fields(validate_page(&mut data))?;
    let id = st.store.insert_drawing(&pid, b.nature, &data).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "drawing",
        &id,
        "create",
        None,
        None::<&Value>,
        Some(&json!({ "nature": b.nature, "name": data.name })),
    )
    .await?;
    Ok(Json(json!({ "id": id })))
}

#[derive(Deserialize)]
struct SaveBody {
    rev: i64,
    #[serde(flatten)]
    data: PageData,
}

/// Enregistrement automatique : la page entière, si la révision est la bonne.
async fn save(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, page)): Path<(String, String)>,
    Json(mut b): Json<SaveBody>,
) -> ApiResult<Json<Value>> {
    let p = editing(&st, &m, &pid, &page).await?;
    // L'empreinte d'un bloc vient du serveur : la nomenclature pour un bloc posé maintenant,
    // la version enregistrée sinon (la synchronisation la rafraîchit).
    let lines = bom(&st.store, &pid).await?;
    let known: HashMap<&str, &BomLine> =
        lines.iter().map(|l| (l.instance_id.as_str(), l)).collect();
    let kept: HashMap<&str, &Snapshot> = p
        .data
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Equipment {
                instance_id,
                snapshot,
                ..
            } => Some((instance_id.as_str(), snapshot)),
            _ => None,
        })
        .collect();
    for e in b.data.elements.iter_mut() {
        if let Element::Equipment {
            instance_id,
            snapshot,
            ..
        } = e
        {
            if let Some(k) = kept.get(instance_id.as_str()) {
                *snapshot = (*k).clone();
            } else if let Some(l) = known.get(instance_id.as_str()) {
                *snapshot = l.snapshot.clone();
            }
        }
    }
    to_fields(validate_page(&mut b.data))?;
    // Un exemplaire ne se place qu'une fois par nature, toutes pages confondues ; il doit
    // exister dans la nomenclature et convenir à la nature de la page.
    let others: HashSet<String> = st
        .store
        .drawings(&pid)
        .await?
        .iter()
        .filter(|x| x.nature == p.nature && x.id != p.id)
        .flat_map(|x| {
            x.data.elements.iter().filter_map(|e| match e {
                Element::Equipment { instance_id, .. } => Some(instance_id.clone()),
                _ => None,
            })
        })
        .collect();
    let before: HashSet<&str> = p
        .data
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Equipment { instance_id, .. } => Some(instance_id.as_str()),
            _ => None,
        })
        .collect();
    for e in &b.data.elements {
        if let Element::Equipment { instance_id, .. } = e {
            if others.contains(instance_id) {
                return Err(one(
                    "elements",
                    "Cet exemplaire est déjà posé sur une autre page de cette nature.",
                ));
            }
            match known.get(instance_id.as_str()) {
                Some(l)
                    if !p.nature.accepts(&l.functions)
                        && !before.contains(instance_id.as_str()) =>
                {
                    return Err(one(
                        "elements",
                        format!("{} ne relève pas de cette nature de page.", l.snapshot.name),
                    ));
                }
                None if !before.contains(instance_id.as_str()) => {
                    return Err(one("elements", "Exemplaire absent de la nomenclature."));
                }
                _ => {}
            }
        }
        if let Element::Image { file, .. } = e {
            if st.store.plan_file(&pid, file).await?.is_none() {
                return Err(one("elements", "Image introuvable."));
            }
        }
    }
    let rev = st.store.save_drawing(&pid, &page, b.rev, &b.data).await?;
    st.hub.change(
        room(&pid),
        "drawing",
        &page,
        "save",
        Some(rev),
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(Json(json!({ "rev": rev })))
}

async fn delete(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, page)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    let p = editing(&st, &m, &pid, &page).await?;
    st.store.delete_drawing(&pid, &page).await?;
    st.store
        .release_lock(&lock_key(&page), &m.0.token_hash)
        .await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "drawing",
        &page,
        "delete",
        None,
        Some(&json!({ "nature": p.nature, "name": p.data.name })),
        None::<&Value>,
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct OrderBody {
    ids: Vec<String>,
}

async fn order(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<OrderBody>,
) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    let pages = st.store.drawings(&pid).await?;
    let natures: HashSet<Nature> = b
        .ids
        .iter()
        .filter_map(|id| pages.iter().find(|p| &p.id == id).map(|p| p.nature))
        .collect();
    if natures.len() != 1 || b.ids.iter().any(|id| !pages.iter().any(|p| &p.id == id)) {
        return Err(one("ids", "Ordonnez les pages d'une même nature."));
    }
    st.store.order_drawings(&pid, &b.ids).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "drawing",
        "order",
        "order",
        None,
        None::<&Value>,
        Some(&b.ids),
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

/// Synchronisation avec la nomenclature : met à jour nom, référence, images, catégorie et
/// empreinte DMX ; retire les orphelins ; ne crée rien ; garde positions et réglages.
async fn sync(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, page)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    let mut p = editing(&st, &m, &pid, &page).await?;
    let lines = bom(&st.store, &pid).await?;
    let known: HashMap<&str, &BomLine> =
        lines.iter().map(|l| (l.instance_id.as_str(), l)).collect();
    let (mut updated, mut removed_ids) = (0, Vec::new());
    for e in p.data.elements.iter_mut() {
        if let Element::Equipment {
            id,
            instance_id,
            snapshot,
            dmx,
            ..
        } = e
        {
            match known.get(instance_id.as_str()) {
                Some(l) => {
                    if *snapshot != l.snapshot {
                        *snapshot = l.snapshot.clone();
                        updated += 1;
                    }
                    if let Some(d) = dmx {
                        if d.mode
                            .as_deref()
                            .is_some_and(|mo| !snapshot.dmx_modes.iter().any(|(n, _)| n == mo))
                        {
                            d.mode = None;
                        }
                    }
                }
                None => removed_ids.push(id.clone()),
            }
        }
    }
    let gone: HashSet<&String> = removed_ids.iter().collect();
    let centers: HashMap<String, (f64, f64)> = p
        .data
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Equipment { id, x, y, w, h, .. } => {
                Some((id.clone(), (x + w / 2.0, y + h / 2.0)))
            }
            _ => None,
        })
        .collect();
    p.data
        .elements
        .retain(|e| !gone.contains(&e.id().to_string()));
    // Les liens accrochés à un orphelin gardent leur tracé, détachés.
    for e in p.data.elements.iter_mut() {
        if let Element::Link { from, to, .. } = e {
            for a in [from, to] {
                if let Some(x) = a.element.clone().filter(|x| gone.contains(x)) {
                    if let Some((cx, cy)) = centers.get(&x) {
                        a.x = *cx;
                        a.y = *cy;
                    }
                    a.element = None;
                }
            }
        }
    }
    let rev = st.store.force_drawing(&pid, &page, &p.data).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "drawing",
        &page,
        "sync",
        Some(rev),
        None::<&Value>,
        Some(&json!({ "updated": updated, "removed": removed_ids.len() })),
    )
    .await?;
    Ok(Json(
        json!({ "updated": updated, "removed": removed_ids.len(), "rev": rev }),
    ))
}

async fn upload(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    st.store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
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
        "image/svg+xml" => String::from_utf8_lossy(&body[..body.len().min(4096)])
            .to_lowercase()
            .contains("<svg"),
        _ => false,
    };
    if !ok || body.len() > 10 * 1024 * 1024 {
        return Err(ApiError::Invalid(
            "Image PNG, JPG ou SVG de 10 Mo au plus.".into(),
        ));
    }
    let id = st.store.insert_plan_file(&pid, &ct, &body).await?;
    Ok(Json(json!({ "file": id })))
}

// ----- DMX -----

#[derive(Deserialize)]
struct SuggestBody {
    instance_id: String,
    #[serde(default)]
    mode: Option<String>,
}

async fn dmx_suggest(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
    Json(b): Json<SuggestBody>,
) -> ApiResult<Json<DmxPatch>> {
    read(&m)?;
    let lines = bom(&st.store, &pid).await?;
    let l = lines
        .iter()
        .find(|l| l.instance_id == b.instance_id)
        .ok_or_else(|| one("instance_id", "Exemplaire absent de la nomenclature."))?;
    let pages = st.store.drawings(&pid).await?;
    let n = channels(&l.snapshot, b.mode.as_deref()).unwrap_or(1);
    let mut p = suggest(&fixtures(&pages), l.snapshot.item_id.as_deref(), n);
    p.mode = b
        .mode
        .filter(|mo| l.snapshot.dmx_modes.iter().any(|(x, _)| x == mo));
    Ok(Json(p))
}

#[derive(Deserialize)]
struct AutoBody {
    mode: String,
}

/// Adressage automatique des projecteurs sans adresse, ou de tous, sur tous les plans de feu.
async fn dmx_auto(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<AutoBody>,
) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    let all = match b.mode.as_str() {
        "missing" => false,
        "all" => true,
        _ => return Err(one("mode", "Mode : missing ou all.")),
    };
    let mut pages: Vec<Page> = st
        .store
        .drawings(&pid)
        .await?
        .into_iter()
        .filter(|p| p.nature == Nature::Lighting)
        .collect();
    for p in &pages {
        if let Some(l) = st.store.lock(&lock_key(&p.id)).await? {
            if fresh(&l.beat_at) && l.token_hash != m.0.token_hash {
                return Err(ApiError::Conflict(format!(
                    "{} a la main sur « {} » : attendez qu'elle soit rendue.",
                    l.account_name, p.data.name
                )));
            }
        }
    }
    if all {
        for p in pages.iter_mut() {
            for e in p.data.elements.iter_mut() {
                if let Element::Equipment { dmx, .. } = e {
                    if let Some(d) = dmx {
                        // On garde le mode choisi : il fixe la taille de la plage.
                        *dmx = Some(DmxPatch {
                            console: 0,
                            universe: 0,
                            address: 0,
                            mode: d.mode.clone(),
                        });
                    }
                }
            }
        }
    }
    // Ordre de pose : pages dans leur ordre, puis de haut en bas et de gauche à droite.
    let mut order: Vec<(usize, usize, f64, f64)> = Vec::new();
    for (pi, p) in pages.iter().enumerate() {
        for (ei, e) in p.data.elements.iter().enumerate() {
            if let Element::Equipment { x, y, dmx, .. } = e {
                let pending = match dmx {
                    None => true,
                    Some(d) => d.address == 0,
                };
                if pending {
                    order.push((pi, ei, *y, *x));
                }
            }
        }
    }
    order.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(a.2.total_cmp(&b.2))
            .then(a.3.total_cmp(&b.3))
    });
    let mut count = 0;
    for (pi, ei, _, _) in order {
        let existing: Vec<Fixture> = fixtures(&pages)
            .into_iter()
            .filter(|f| f.patch.as_ref().is_some_and(|p| p.address > 0))
            .collect();
        let Element::Equipment { snapshot, dmx, .. } = &pages[pi].data.elements[ei] else {
            continue;
        };
        let mode = dmx.as_ref().and_then(|d| d.mode.clone());
        let n = channels(snapshot, mode.as_deref()).unwrap_or(1);
        let mut patch = suggest(&existing, snapshot.item_id.as_deref(), n);
        patch.mode = mode;
        if let Element::Equipment { dmx, .. } = &mut pages[pi].data.elements[ei] {
            *dmx = Some(patch);
            count += 1;
        }
    }
    for p in &pages {
        st.store.force_drawing(&pid, &p.id, &p.data).await?;
    }
    record(
        &st,
        &m,
        &origin,
        &pid,
        "drawing",
        "dmx",
        "dmx_auto",
        None,
        None::<&Value>,
        Some(&json!({ "mode": b.mode, "count": count })),
    )
    .await?;
    Ok(Json(json!({ "assigned": count })))
}

// ----- Main sur la page -----

async fn take(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path((pid, page)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    page_of(&st.store, &pid, &page).await?;
    match st
        .store
        .take_lock(
            &lock_key(&page),
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
                    &lock_key(&page),
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
    Path((pid, page)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    if st
        .store
        .release_lock(&lock_key(&page), &m.0.token_hash)
        .await?
    {
        st.hub.change(
            room(&pid),
            "lock",
            &lock_key(&page),
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
    Path((pid, page)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    st.hub.change(
        room(&pid),
        "lock",
        &lock_key(&page),
        "request",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(StatusCode::NO_CONTENT)
}
