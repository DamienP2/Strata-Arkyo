//! Racks (section 9) : élévations, placement, bilan électrique et verrou d'édition.
//!
//! L'élévation est un écran de conception : une seule personne la modifie à la fois.
//! Toute écriture exige de tenir la main (`rack:<contenant>`), avec un battement récent.
//! Les autres membres ne reçoivent que l'annonce « nouvelle version enregistrée ».

use crate::AppState;
use crate::auth::Member;
use crate::error::{ApiError, ApiResult};
use crate::live::Origin;
use crate::projects::{ItemView, article_label, fields, item_views, load_of, one, read, room};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{patch, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use strata_domain::catalog::{FieldError, Width};
use strata_domain::project::{ArticleNature, container_totals};
use strata_domain::rack::*;
use strata_domain::{Domain, Level};
use strata_store::{Article, Container, LockOutcome, PlaceSpec, Placement, Store, StoreError};

/// Sans battement pendant ce délai, la main se libère.
pub const LOCK_TTL_SECS: i64 = 45;
const DESIGN: &str = "Vous n'avez pas le droit de modifier la conception.";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/containers/{id}/rack", axum::routing::get(show))
        .route("/containers/{id}/rack/lock", post(take).delete(release))
        .route("/containers/{id}/rack/lock/request", post(request))
        .route("/containers/{id}/placements", post(place))
        .route("/containers/{id}/placements/clear", post(clear))
        .route(
            "/placements/{id}",
            patch(move_placement).delete(remove_placement),
        )
        .route("/containers/{id}/circuits", post(create_circuit))
        .route(
            "/circuits/{id}",
            patch(update_circuit).delete(delete_circuit),
        )
}

fn view_key(cid: &str) -> String {
    format!("rack:{cid}")
}

async fn container_of(store: &Store, id: &str) -> ApiResult<Container> {
    store
        .container(id)
        .await?
        .ok_or(ApiError::NotFound("Élément"))
}

/// Écriture sur une élévation : droit « Conception : écriture » et main tenue.
async fn editing(st: &AppState, m: &Member, cid: &str) -> ApiResult<Container> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    let c = container_of(&st.store, cid).await?;
    if !c.data.rackable {
        return Err(ApiError::Invalid(
            "Cet élément n'est pas rackable : il n'a pas d'élévation.".into(),
        ));
    }
    if !st
        .store
        .holds_lock(&view_key(cid), &m.0.token_hash, LOCK_TTL_SECS)
        .await?
    {
        let who = st.store.lock(&view_key(cid)).await?.map(|l| l.account_name);
        return Err(ApiError::Conflict(match who {
            Some(n) => {
                format!("{n} a la main sur cette élévation : vous la voyez en lecture seule.")
            }
            None => "Prenez la main sur l'élévation avant de la modifier.".into(),
        }));
    }
    Ok(c)
}

/// Annonce aux autres : la version enregistrée a changé.
fn saved(st: &AppState, m: &Member, origin: &Origin, c: &Container) {
    st.hub.change(
        room(&c.project_id),
        "rack",
        &c.id,
        "saved",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
}

// ----- Vue de l'élévation -----

struct Ctx {
    items: HashMap<String, ItemView>,
    articles: Vec<Article>,
}

impl Ctx {
    async fn load(store: &Store, c: &Container) -> ApiResult<Ctx> {
        let inv = store.inventory(&c.project_id).await?;
        let articles: Vec<Article> = inv
            .articles
            .into_iter()
            .filter(|a| a.container_id.as_deref() == Some(&c.id))
            .collect();
        let ids: HashSet<String> = articles.iter().filter_map(|a| a.item_id.clone()).collect();
        Ok(Ctx {
            items: item_views(store, &ids).await?,
            articles,
        })
    }
    fn article(&self, id: &str) -> Option<&Article> {
        self.articles.iter().find(|a| a.id == id)
    }
    fn item(&self, a: &Article) -> Option<&ItemView> {
        a.item_id.as_ref().and_then(|i| self.items.get(i))
    }
    /// Hauteur et largeur d'un exemplaire de cet article ; `None` s'il ne se pose pas en rack.
    fn geometry(&self, a: &Article) -> Option<(f64, Width)> {
        match (&a.accessory, self.item(a)) {
            (Some(acc), _) => Some((acc.height_u() as f64, Width::Full)),
            (None, Some(it)) if it.rackable => Some((it.height_u.unwrap_or(1.0), it.width)),
            _ => None,
        }
    }
    fn slot(&self, p: &Placement) -> Option<Slot> {
        let a = self.article(&p.article_id)?;
        let (h, _) = self.geometry(a)?;
        Some(Slot {
            id: p.id.clone(),
            label: article_label(a, &self.items),
            face: p.face,
            u_start: p.u_start,
            height_u: h,
            width: p.width,
            col: p.col as u32,
        })
    }
}

async fn rack_view(st: &AppState, c: &Container, token: &str) -> ApiResult<Value> {
    let ctx = Ctx::load(&st.store, c).await?;
    let placements = st.store.placements(&c.id).await?;
    let inv = st.store.inventory(&c.project_id).await?;
    let tag_of = |iid: &Option<String>| {
        iid.as_ref()
            .and_then(|i| inv.instances.iter().find(|x| &x.id == i))
            .map(|x| x.tag.clone())
    };
    let mut elements = Vec::new();
    for p in &placements {
        let Some(a) = ctx.article(&p.article_id) else {
            continue;
        };
        let it = ctx.item(a);
        let (h, _) = ctx.geometry(a).unwrap_or((1.0, Width::Full));
        elements.push(json!({
            "placement": p, "label": article_label(a, &ctx.items), "tag": tag_of(&p.instance_id), "nature": a.nature,
            "height_u": h, "depth_mm": it.and_then(|i| i.depth_mm).or(a.accessory.as_ref().and_then(|x| x.depth_mm())), "accessory": a.accessory, "article_rev": a.rev,
            "rear_reserve_mm": it.and_then(|i| i.rear_reserve_mm), "weight_kg": it.and_then(|i| i.weight_kg), "power_w": it.and_then(|i| i.power_w),
            "function": it.and_then(|i| i.function.clone()), "category": it.and_then(|i| i.category.clone()), "brand": it.and_then(|i| i.brand.clone()), "item_id": a.item_id,
            "type_id": it.and_then(|i| i.type_id.clone()), "visual": it.and_then(|i| i.visual.clone()),
        }));
    }
    // À placer : exemplaires non posés, et accessoires non posés.
    let placed_inst: HashSet<&str> = placements
        .iter()
        .filter_map(|p| p.instance_id.as_deref())
        .collect();
    let placed_acc: HashSet<&str> = placements
        .iter()
        .filter(|p| p.instance_id.is_none())
        .map(|p| p.article_id.as_str())
        .collect();
    let mut pending = Vec::new();
    for a in ctx.articles.iter().filter(|a| a.host.is_none()) {
        // Un module est dans son châssis : il ne se pose pas seul.
        let geo = ctx.geometry(a);
        let base = json!({ "article_id": a.id, "label": article_label(a, &ctx.items), "nature": a.nature, "height_u": geo.map(|g| g.0), "width": geo.map(|g| g.1), "rackable": geo.is_some(), "function": ctx.item(a).and_then(|i| i.function.clone()), "accessory": a.accessory, "type_id": ctx.item(a).and_then(|i| i.type_id.clone()), "visual": ctx.item(a).and_then(|i| i.visual.clone()) });
        match a.nature {
            ArticleNature::Equipment => {
                for i in inv
                    .instances
                    .iter()
                    .filter(|i| i.article_id == a.id && !placed_inst.contains(i.id.as_str()))
                {
                    let mut v = base.clone();
                    v["instance_id"] = json!(i.id);
                    v["tag"] = json!(i.tag);
                    pending.push(v);
                }
            }
            ArticleNature::Accessory if !placed_acc.contains(a.id.as_str()) => pending.push(base),
            _ => {}
        }
    }
    let circuits = st.store.circuits(&c.id).await?;
    let specs: Vec<CircuitSpec> = circuits
        .iter()
        .map(|x| CircuitSpec {
            id: x.id.clone(),
            name: x.name.clone(),
            phase: x.phase,
            rating_a: x.rating_a,
        })
        .collect();
    let loads: Vec<(Option<String>, f64)> = placements
        .iter()
        .filter_map(|p| {
            ctx.article(&p.article_id)
                .and_then(|a| ctx.item(a))
                .and_then(|i| i.power_w)
                .map(|w| (p.circuit_id.clone(), w))
        })
        .collect();
    let all_loads: Vec<_> = ctx
        .articles
        .iter()
        .map(|a| load_of(a, &ctx.items))
        .collect();
    let warnings: Vec<Value> = placements
        .iter()
        .filter_map(|p| {
            let a = ctx.article(&p.article_id)?;
            let it = ctx.item(a)?;
            depth_warning(
                c.data.depth_mm,
                it.depth_mm?,
                it.rear_reserve_mm.unwrap_or(80.0),
            )
            .map(|m| json!({ "placement_id": p.id, "message": format!("{} : {m}", it.name) }))
        })
        .collect();
    let lock = st.store.lock(&view_key(&c.id)).await?;
    let limit =
        strata_store::timestamp(chrono::Utc::now() - chrono::Duration::seconds(LOCK_TTL_SECS));
    let lock = lock.filter(|l| l.beat_at >= limit);
    let mine = lock.as_ref().is_some_and(|l| l.token_hash == token);
    Ok(json!({
        "container": c,
        "elements": elements,
        "pending": pending,
        "circuits": circuits,
        "electrical": electrical(&specs, &loads),
        "totals": container_totals(&c.data, &all_loads),
        "warnings": warnings,
        "lock": lock.map(|l| json!({ "account_id": l.account_id, "account_name": l.account_name, "taken_at": l.taken_at, "mine": mine })),
        "faces": if c.data.rear_rails_mm.is_some() { json!(["front", "rear", "rear_inner"]) } else { json!(["front", "rear"]) },
    }))
}

async fn show(
    State(st): State<AppState>,
    m: Member,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    let c = container_of(&st.store, &id).await?;
    Ok(Json(rack_view(&st, &c, &m.0.token_hash).await?))
}

// ----- Verrou -----

/// Prendre la main (ou renouveler son battement).
async fn take(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    let c = container_of(&st.store, &id).await?;
    match st
        .store
        .take_lock(
            &view_key(&id),
            &c.project_id,
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
                    room(&c.project_id),
                    "lock",
                    &id,
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
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let c = container_of(&st.store, &id).await?;
    if st
        .store
        .release_lock(&view_key(&id), &m.0.token_hash)
        .await?
    {
        st.hub.change(
            room(&c.project_id),
            "lock",
            &id,
            "released",
            None,
            &m.0.account.id,
            origin.0.as_deref(),
        );
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Demander la main : l'éditeur est prévenu.
async fn request(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    m.0.require(Domain::Conception, Level::Ecriture, DESIGN)?;
    let c = container_of(&st.store, &id).await?;
    st.hub.change(
        room(&c.project_id),
        "lock",
        &id,
        "request",
        None,
        &m.0.account.name,
        origin.0.as_deref(),
    );
    Ok(StatusCode::ACCEPTED)
}

// ----- Placement -----

#[derive(Deserialize)]
struct PlaceBody {
    article_id: String,
    instance_id: Option<String>,
    face: RackFace,
    u_start: i64,
    width: Option<Width>,
    #[serde(default)]
    col: i64,
    circuit_id: Option<String>,
    /// Étagère qui recevra un équipement hors format rack.
    shelf_id: Option<String>,
}

fn place_error(e: PlaceError) -> ApiError {
    ApiError::Fields(vec![FieldError {
        field: "u_start".into(),
        message: e.message().to_string(),
    }])
}

fn check_face(c: &Container, f: RackFace) -> ApiResult<()> {
    if f == RackFace::RearInner && c.data.rear_rails_mm.is_none() {
        return Err(one(
            "face",
            "Cet élément n'a pas de rails arrière internes.",
        ));
    }
    Ok(())
}

async fn check_circuit(store: &Store, c: &Container, id: &Option<String>) -> ApiResult<()> {
    if let Some(id) = id {
        if !store
            .circuit(id)
            .await?
            .is_some_and(|x| x.container_id == c.id)
        {
            return Err(one("circuit_id", "Circuit inconnu dans cet élément."));
        }
    }
    Ok(())
}

async fn place(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(cid): Path<String>,
    Json(b): Json<PlaceBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let c = editing(&st, &m, &cid).await?;
    check_face(&c, b.face)?;
    check_circuit(&st.store, &c, &b.circuit_id).await?;
    let a = st
        .store
        .article(&b.article_id)
        .await?
        .ok_or(ApiError::NotFound("Article"))?;
    if a.container_id.as_deref() != Some(&cid) {
        let name = match &a.container_id {
            Some(o) => st
                .store
                .container(o)
                .await?
                .map(|x| x.name)
                .unwrap_or_default(),
            None => "les équipements libres".into(),
        };
        return Err(ApiError::Conflict(format!(
            "Cet élément appartient à « {name} »."
        )));
    }
    let ctx = Ctx::load(&st.store, &c).await?;
    let geo = ctx.geometry(&a);
    if a.nature == ArticleNature::Adapter {
        return Err(ApiError::Invalid(
            "Un adaptateur ne se pose pas dans le rack.".into(),
        ));
    }
    // Exemplaire : celui demandé, sinon le premier non posé.
    let placements = st.store.placements(&cid).await?;
    let instance_id = if a.nature == ArticleNature::Equipment {
        let placed: HashSet<&str> = placements
            .iter()
            .filter_map(|p| p.instance_id.as_deref())
            .collect();
        let inst = st.store.article_instances(&a.id).await?;
        let pick = match &b.instance_id {
            Some(i) => inst
                .iter()
                .find(|x| &x.id == i)
                .ok_or(ApiError::NotFound("Exemplaire"))?,
            None => inst
                .iter()
                .find(|x| !placed.contains(x.id.as_str()))
                .ok_or_else(|| {
                    ApiError::Conflict(
                        "Tous les exemplaires de cet article sont déjà posés.".into(),
                    )
                })?,
        };
        if placed.contains(pick.id.as_str()) {
            return Err(ApiError::Conflict(format!(
                "L'exemplaire {} est déjà posé.",
                pick.tag
            )));
        }
        Some(pick.id.clone())
    } else {
        None
    };
    let (spec, adjusted) = match &b.shelf_id {
        // Sur une étagère : l'équipement prend la position de l'étagère.
        Some(sid) => {
            let shelf = st
                .store
                .placement(sid)
                .await?
                .filter(|p| p.container_id == cid)
                .ok_or_else(|| one("shelf_id", "Étagère inconnue dans cet élément."))?;
            let is_shelf = ctx
                .article(&shelf.article_id)
                .and_then(|x| x.accessory.as_ref())
                .is_some_and(|acc| matches!(acc, strata_domain::project::Accessory::Shelf { .. }));
            if !is_shelf {
                return Err(one("shelf_id", "Choisissez une étagère."));
            }
            if geo.is_some() {
                return Err(ApiError::Invalid(
                    "Un équipement au format rack se pose dans la baie, pas sur une étagère."
                        .into(),
                ));
            }
            if st.store.shelf_load(sid).await? >= 10 {
                return Err(ApiError::Conflict(
                    "Une étagère porte 10 équipements au plus.".into(),
                ));
            }
            (
                PlaceSpec {
                    face: shelf.face,
                    u_start: shelf.u_start,
                    width: Width::Full,
                    col: 0,
                    circuit_id: b.circuit_id.clone(),
                    shelf_id: Some(sid.clone()),
                },
                false,
            )
        }
        None => {
            let Some((h, w)) = geo else {
                return Err(ApiError::Invalid(
                    "Cet équipement se pose sur une étagère.".into(),
                ));
            };
            let width = b.width.unwrap_or(w);
            let others: Vec<Slot> = placements.iter().filter_map(|p| ctx.slot(p)).collect();
            let slot = Slot {
                id: String::new(),
                label: article_label(&a, &ctx.items),
                face: b.face,
                u_start: b.u_start,
                height_u: h,
                width,
                col: b.col.max(0) as u32,
            };
            let u = adjust(c.data.height_u.unwrap_or(0.0), &slot, &others).map_err(place_error)?;
            (
                PlaceSpec {
                    face: b.face,
                    u_start: u,
                    width,
                    col: b.col.max(0),
                    circuit_id: b.circuit_id.clone(),
                    shelf_id: None,
                },
                u != b.u_start,
            )
        }
    };
    let p = st
        .store
        .insert_placement(&c.project_id, &cid, &a.id, instance_id.as_deref(), &spec)
        .await
        .map_err(|e| match e {
            StoreError::Conflict(_) => ApiError::Conflict("Cet élément est déjà posé.".into()),
            e => e.into(),
        })?;
    st.store
        .audit_project(
            &c.project_id,
            Some(&m.0.account.id),
            "place",
            "placement",
            &p.id,
            None::<&()>,
            Some(&p),
        )
        .await?;
    saved(&st, &m, &origin, &c);
    let warning = ctx.item(&a).and_then(|it| {
        depth_warning(
            c.data.depth_mm,
            it.depth_mm?,
            it.rear_reserve_mm.unwrap_or(80.0),
        )
    });
    Ok((
        StatusCode::CREATED,
        Json(json!({ "placement": p, "adjusted": adjusted, "warning": warning })),
    ))
}

#[derive(Deserialize)]
struct MoveBody {
    rev: i64,
    face: RackFace,
    u_start: i64,
    width: Width,
    #[serde(default)]
    col: i64,
    circuit_id: Option<String>,
}

async fn move_placement(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
    Json(b): Json<MoveBody>,
) -> ApiResult<Json<Value>> {
    let before = st
        .store
        .placement(&id)
        .await?
        .ok_or(ApiError::NotFound("Élément posé"))?;
    let c = editing(&st, &m, &before.container_id).await?;
    check_face(&c, b.face)?;
    check_circuit(&st.store, &c, &b.circuit_id).await?;
    let ctx = Ctx::load(&st.store, &c).await?;
    let placements = st.store.placements(&c.id).await?;
    let others: Vec<Slot> = placements
        .iter()
        .filter(|p| p.id != id)
        .filter_map(|p| ctx.slot(p))
        .collect();
    if before.shelf_id.is_some() {
        let after = st
            .store
            .update_placement(
                &id,
                b.rev,
                &PlaceSpec {
                    face: before.face,
                    u_start: before.u_start,
                    width: before.width,
                    col: before.col,
                    circuit_id: b.circuit_id,
                    shelf_id: before.shelf_id.clone(),
                },
            )
            .await?;
        saved(&st, &m, &origin, &c);
        return Ok(Json(json!({ "placement": after })));
    }
    let mut slot = ctx
        .slot(&before)
        .ok_or(ApiError::NotFound("Élément posé"))?;
    slot.face = b.face;
    slot.u_start = b.u_start;
    slot.width = b.width;
    slot.col = b.col.max(0) as u32;
    check_place(c.data.height_u.unwrap_or(0.0), &slot, &others).map_err(place_error)?;
    let after = st
        .store
        .update_placement(
            &id,
            b.rev,
            &PlaceSpec {
                face: b.face,
                u_start: b.u_start,
                width: b.width,
                col: b.col.max(0),
                circuit_id: b.circuit_id,
                shelf_id: None,
            },
        )
        .await?;
    st.store
        .audit_project(
            &c.project_id,
            Some(&m.0.account.id),
            "move",
            "placement",
            &id,
            Some(&before),
            Some(&after),
        )
        .await?;
    saved(&st, &m, &origin, &c);
    Ok(Json(json!({ "placement": after })))
}

async fn remove_placement(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let before = st
        .store
        .placement(&id)
        .await?
        .ok_or(ApiError::NotFound("Élément posé"))?;
    let c = editing(&st, &m, &before.container_id).await?;
    st.store
        .delete_placements(std::slice::from_ref(&id))
        .await?;
    st.store
        .audit_project(
            &c.project_id,
            Some(&m.0.account.id),
            "unplace",
            "placement",
            &id,
            Some(&before),
            None::<&()>,
        )
        .await?;
    saved(&st, &m, &origin, &c);
    Ok(StatusCode::NO_CONTENT)
}

/// Vider le rack : tous les éléments redeviennent « à placer ».
async fn clear(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(cid): Path<String>,
) -> ApiResult<Json<Value>> {
    let c = editing(&st, &m, &cid).await?;
    let n = st.store.clear_placements(&cid).await?;
    st.store
        .audit_project(
            &c.project_id,
            Some(&m.0.account.id),
            "clear",
            "rack",
            &cid,
            None::<&()>,
            Some(&json!({ "removed": n })),
        )
        .await?;
    saved(&st, &m, &origin, &c);
    Ok(Json(json!({ "removed": n })))
}

// ----- Circuits -----

#[derive(Deserialize)]
struct CircuitBody {
    #[serde(default)]
    rev: i64,
    name: String,
    phase: Phase,
    rating_a: f64,
}

fn check_circuit_body(b: &mut CircuitBody) -> ApiResult<()> {
    b.name = b.name.trim().to_string();
    let mut e = Vec::new();
    if b.name.is_empty() || b.name.chars().count() > 60 {
        e.push(FieldError {
            field: "name".into(),
            message: "Le nom compte de 1 à 60 caractères.".into(),
        });
    }
    if !(1.0..=125.0).contains(&b.rating_a) {
        e.push(FieldError {
            field: "rating_a".into(),
            message: "Calibre de 1 à 125 A (16 ou 32 A en général).".into(),
        });
    }
    fields(e)
}

async fn create_circuit(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(cid): Path<String>,
    Json(mut b): Json<CircuitBody>,
) -> ApiResult<(StatusCode, Json<strata_store::Circuit>)> {
    let c = editing(&st, &m, &cid).await?;
    check_circuit_body(&mut b)?;
    let x = st
        .store
        .insert_circuit(&c.project_id, &cid, &b.name, b.phase, b.rating_a)
        .await?;
    st.store
        .audit_project(
            &c.project_id,
            Some(&m.0.account.id),
            "create",
            "circuit",
            &x.id,
            None::<&()>,
            Some(&x),
        )
        .await?;
    saved(&st, &m, &origin, &c);
    Ok((StatusCode::CREATED, Json(x)))
}

async fn update_circuit(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
    Json(mut b): Json<CircuitBody>,
) -> ApiResult<Json<strata_store::Circuit>> {
    let before = st
        .store
        .circuit(&id)
        .await?
        .ok_or(ApiError::NotFound("Circuit"))?;
    let c = editing(&st, &m, &before.container_id).await?;
    check_circuit_body(&mut b)?;
    let after = st
        .store
        .update_circuit(&id, b.rev, &b.name, b.phase, b.rating_a)
        .await?;
    st.store
        .audit_project(
            &c.project_id,
            Some(&m.0.account.id),
            "update",
            "circuit",
            &id,
            Some(&before),
            Some(&after),
        )
        .await?;
    saved(&st, &m, &origin, &c);
    Ok(Json(after))
}

async fn delete_circuit(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let before = st
        .store
        .circuit(&id)
        .await?
        .ok_or(ApiError::NotFound("Circuit"))?;
    let c = editing(&st, &m, &before.container_id).await?;
    st.store.delete_circuit(&id).await?;
    st.store
        .audit_project(
            &c.project_id,
            Some(&m.0.account.id),
            "delete",
            "circuit",
            &id,
            Some(&before),
            None::<&()>,
        )
        .await?;
    saved(&st, &m, &origin, &c);
    Ok(StatusCode::NO_CONTENT)
}

/// Éléments posés qui dépasseraient une nouvelle hauteur de rack.
pub(crate) async fn overflow(
    store: &Store,
    c: &Container,
    new_height: f64,
) -> ApiResult<Vec<(String, String)>> {
    let ctx = Ctx::load(store, c).await?;
    Ok(store
        .placements(&c.id)
        .await?
        .iter()
        .filter_map(|p| ctx.slot(p))
        .filter(|s| (s.u_start - 1) as f64 + s.height_u > new_height + 1e-9)
        .map(|s| (s.id, s.label))
        .collect())
}
