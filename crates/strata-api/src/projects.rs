//! Projets et inventaire (cahier des charges, sections 7 et 8).
//!
//! Lire demande « Projets : lecture » ; écrire demande « Projets : écriture ».
//! Chaque écriture est publiée dans la salle temps réel du projet.

use crate::AppState;
use crate::auth::Member;
use crate::catalog_io::{Cell, file_response, to_csv, to_xlsx_named};
use crate::error::{ApiError, ApiResult};
use crate::live::Origin;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use strata_domain::catalog::{FieldError, Format, Modular};
use strata_domain::project::*;
use strata_domain::{Domain, Level};
use strata_store::{
    Article, CatalogItem, Container, Inventory, Location, ModuleHost, Project, Store, StoreError,
};

const READ: &str = "Vous n'avez pas accès aux projets.";
const WRITE: &str = "Vous n'avez pas le droit de modifier les projets.";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/dashboard", get(dashboard))
        .route("/projects", get(list).post(create))
        .route("/projects/check-name", get(check_name))
        .route("/projects/{id}", get(show).patch(update).delete(remove))
        .route("/projects/{id}/history", get(history))
        .route("/projects/{id}/inventory/export", get(export))
        .route("/projects/{id}/locations", post(create_location))
        .route("/projects/{id}/locations/order", post(order_locations))
        .route(
            "/locations/{id}",
            patch(update_location).delete(delete_location),
        )
        .route("/projects/{id}/containers", post(create_container))
        .route("/projects/{id}/containers/order", post(order_containers))
        .route(
            "/containers/{id}",
            patch(update_container).delete(delete_container),
        )
        .route("/projects/{id}/articles", post(create_article))
        .route(
            "/articles/{id}",
            patch(update_article).delete(delete_article),
        )
        .route("/articles/{id}/move", post(move_article))
        .route("/articles/{id}/modules", post(add_module))
        .route("/instances/{id}", patch(update_instance))
        .route("/projects/{id}/tasks", get(list_tasks).post(create_task))
        .route("/tasks/{id}", patch(update_task).delete(delete_task))
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

pub(crate) fn fields(e: Vec<FieldError>) -> ApiResult<()> {
    if e.is_empty() {
        Ok(())
    } else {
        Err(ApiError::Fields(e))
    }
}

pub(crate) fn one(field: &str, message: impl Into<String>) -> ApiError {
    ApiError::Fields(vec![FieldError {
        field: field.into(),
        message: message.into(),
    }])
}

pub(crate) fn read(m: &Member) -> ApiResult<()> {
    m.0.require(Domain::Projets, Level::Lecture, READ)
}

pub(crate) fn write(m: &Member) -> ApiResult<()> {
    m.0.require(Domain::Projets, Level::Ecriture, WRITE)
}

async fn project_of(store: &Store, id: &str) -> ApiResult<Project> {
    store.project(id).await?.ok_or(ApiError::NotFound("Projet"))
}

pub(crate) fn room(pid: &str) -> String {
    format!("project:{pid}")
}

/// Publie un changement dans la salle du projet et l'écrit dans son historique.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn record<B: Serialize, A: Serialize>(
    st: &AppState,
    m: &Member,
    origin: &Origin,
    pid: &str,
    kind: &str,
    id: &str,
    action: &str,
    rev: Option<i64>,
    before: Option<&B>,
    after: Option<&A>,
) -> ApiResult<()> {
    st.store
        .audit_project(pid, Some(&m.0.account.id), action, kind, id, before, after)
        .await?;
    st.hub.change(
        room(pid),
        kind,
        id,
        action,
        rev,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(())
}

// ----- Catalogue vu par l'inventaire -----

/// Ce que l'inventaire doit savoir d'une fiche du catalogue.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ItemView {
    pub(crate) id: String,
    pub(crate) kind: strata_domain::catalog::ItemKind,
    pub(crate) name: String,
    pub(crate) reference: Option<String>,
    pub(crate) brand: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) rackable: bool,
    pub(crate) height_u: Option<f64>,
    pub(crate) depth_mm: Option<f64>,
    pub(crate) weight_kg: Option<f64>,
    pub(crate) power_w: Option<f64>,
    pub(crate) modular: bool,
    /// Châssis : nombre d'emplacements ; module : emplacements occupés.
    pub(crate) host_slots: Option<u32>,
    pub(crate) module_slots: Option<u32>,
    pub(crate) active: bool,
    /// Largeur en rack (pleine, 1/2…), réserve de câbles à l'arrière, première fonction.
    pub(crate) width: strata_domain::catalog::Width,
    pub(crate) rear_reserve_mm: Option<f64>,
    pub(crate) function: Option<String>,
    /// Type du catalogue et visuel choisi : la face en baie en est déduite.
    pub(crate) type_id: Option<String>,
    pub(crate) visual: Option<String>,
}

pub(crate) async fn item_views(
    store: &Store,
    ids: &HashSet<String>,
) -> ApiResult<HashMap<String, ItemView>> {
    let refs = store.list_refs(None).await?;
    let name = |kind: strata_domain::catalog::RefKind, id: Option<&str>| {
        id.and_then(|id| refs.iter().find(|r| r.kind == kind && r.id == id))
            .map(|r| r.name.clone())
    };
    let mut out = HashMap::new();
    for id in ids {
        let Some(i) = store.item(id).await? else {
            continue;
        };
        out.insert(id.clone(), view_of(&i, &name));
    }
    Ok(out)
}

pub(crate) fn view_of(
    i: &CatalogItem,
    name: &dyn Fn(strata_domain::catalog::RefKind, Option<&str>) -> Option<String>,
) -> ItemView {
    use strata_domain::catalog::RefKind;
    let d = &i.data;
    let format: Option<Format> = d
        .get("format")
        .and_then(|f| serde_json::from_value(f.clone()).ok());
    let (rackable, height_u, depth_mm, width, rear_reserve_mm) = match format {
        Some(Format::Rack {
            height_u,
            depth_mm,
            width,
            rear_reserve_mm,
        }) => (true, Some(height_u), Some(depth_mm), width, rear_reserve_mm),
        Some(Format::Free { length_mm, .. }) => {
            (false, None, Some(length_mm), Default::default(), None)
        }
        None => (false, None, None, Default::default(), None),
    };
    let modular: Option<Modular> = d
        .get("modular")
        .and_then(|m| serde_json::from_value(m.clone()).ok());
    ItemView {
        id: i.id.clone(),
        kind: i.kind,
        name: i.name.clone(),
        reference: i.reference.clone(),
        brand: name(RefKind::Brand, i.brand_id.as_deref()),
        category: name(
            RefKind::Category,
            d.get("category_id").and_then(Value::as_str),
        ),
        rackable,
        height_u,
        depth_mm,
        weight_kg: d.get("weight_kg").and_then(Value::as_f64),
        power_w: d.get("power_w").and_then(Value::as_f64),
        modular: matches!(modular, Some(Modular::Host { .. })),
        host_slots: match &modular {
            Some(Modular::Host { slots, .. }) => Some(*slots),
            _ => None,
        },
        module_slots: match &modular {
            Some(Modular::Component { slots }) => Some(*slots),
            _ => None,
        },
        active: i.active,
        width,
        rear_reserve_mm,
        function: d
            .get("functions")
            .and_then(|f| f.get(0))
            .and_then(Value::as_str)
            .map(str::to_string),
        type_id: d.get("type_id").and_then(Value::as_str).map(str::to_string),
        visual: d
            .get("visual")
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .map(str::to_string),
    }
}

pub(crate) fn load_of(a: &Article, items: &HashMap<String, ItemView>) -> ArticleLoad {
    let q = a.quantity.max(0) as u32;
    match (&a.accessory, a.item_id.as_ref().and_then(|i| items.get(i))) {
        (Some(acc), _) => ArticleLoad {
            quantity: q,
            height_u: Some(acc.height_u() as f64),
            weight_kg: 0.0,
            power_w: 0.0,
        },
        (None, Some(it)) => ArticleLoad {
            quantity: q,
            // Un module est dans son châssis : il ne prend pas de place en baie.
            height_u: it.height_u.filter(|_| it.rackable && a.host.is_none()),
            weight_kg: it.weight_kg.unwrap_or(0.0),
            power_w: it.power_w.unwrap_or(0.0),
        },
        _ => ArticleLoad {
            quantity: q,
            ..Default::default()
        },
    }
}

pub(crate) fn article_label(a: &Article, items: &HashMap<String, ItemView>) -> String {
    match (&a.accessory, a.item_id.as_ref().and_then(|i| items.get(i))) {
        (Some(acc), _) => acc.label(),
        (None, Some(it)) => it.name.clone(),
        _ => "Fiche supprimée du catalogue".into(),
    }
}

/// Inventaire enrichi : totaux par contenant et par emplacement, libellés des articles.
/// Exemplaires du matériel posé dans un élément.
pub(crate) async fn own_instances(
    store: &Store,
    pid: &str,
    cid: &str,
) -> ApiResult<HashSet<String>> {
    let inv = store.inventory(pid).await?;
    let arts: HashSet<&str> = inv
        .articles
        .iter()
        .filter(|a| a.container_id.as_deref() == Some(cid))
        .map(|a| a.id.as_str())
        .collect();
    Ok(inv
        .instances
        .iter()
        .filter(|i| arts.contains(i.article_id.as_str()))
        .map(|i| i.id.clone())
        .collect())
}

pub(crate) async fn inventory_view(
    store: &Store,
    pid: &str,
) -> ApiResult<(Inventory, HashMap<String, ItemView>, Value)> {
    let inv = store.inventory(pid).await?;
    let ids: HashSet<String> = inv
        .articles
        .iter()
        .filter_map(|a| a.item_id.clone())
        .collect();
    let items = item_views(store, &ids).await?;
    let mut containers = Vec::new();
    for c in &inv.containers {
        let loads: Vec<ArticleLoad> = inv
            .articles
            .iter()
            .filter(|a| a.container_id.as_deref() == Some(&c.id))
            .map(|a| load_of(a, &items))
            .collect();
        let mut v = serde_json::to_value(c).unwrap_or_default();
        v["totals"] = json!(container_totals(&c.data, &loads));
        containers.push(v);
    }
    let mut locations = Vec::new();
    for l in &inv.locations {
        let free: Vec<&Article> = inv
            .articles
            .iter()
            .filter(|a| a.location_id == l.id && a.container_id.is_none())
            .collect();
        let all: Vec<ArticleLoad> = inv
            .articles
            .iter()
            .filter(|a| a.location_id == l.id)
            .map(|a| load_of(a, &items))
            .collect();
        let empty: f64 = inv
            .containers
            .iter()
            .filter(|c| c.location_id == l.id)
            .filter_map(|c| c.data.empty_weight_kg)
            .sum();
        let mut v = serde_json::to_value(l).unwrap_or_default();
        v["totals"] = json!({
            "containers": inv.containers.iter().filter(|c| c.location_id == l.id).count(),
            "free_items": free.iter().map(|a| a.quantity).sum::<i64>(),
            "power_w": all.iter().map(|a| a.power_w * a.quantity as f64).sum::<f64>().round(),
            "weight_kg": ((empty + all.iter().map(|a| a.weight_kg * a.quantity as f64).sum::<f64>()) * 10.0).round() / 10.0,
        });
        locations.push(v);
    }
    let articles: Vec<Value> = inv
        .articles
        .iter()
        .map(|a| {
            let mut v = serde_json::to_value(a).unwrap_or_default();
            v["label"] = json!(article_label(a, &items));
            v
        })
        .collect();
    let view = json!({ "locations": locations, "containers": containers, "articles": articles, "instances": inv.instances, "items": items });
    Ok((inv, items, view))
}

// ----- Projets -----

#[derive(Serialize)]
struct Summary {
    #[serde(flatten)]
    project: Project,
    completion: u32,
    locations: i64,
    containers: i64,
    articles: i64,
}

async fn summaries(store: &Store) -> ApiResult<Vec<Summary>> {
    let counts: HashMap<String, (i64, i64, i64)> = store
        .project_counts()
        .await?
        .into_iter()
        .map(|(id, l, c, a)| (id, (l, c, a)))
        .collect();
    Ok(store
        .list_projects()
        .await?
        .into_iter()
        .map(|p| {
            let (l, c, a) = counts.get(&p.id).copied().unwrap_or_default();
            Summary {
                completion: completion(&p.data, l as usize, c as usize, a as usize),
                locations: l,
                containers: c,
                articles: a,
                project: p,
            }
        })
        .collect())
}

async fn list(State(st): State<AppState>, m: Member) -> ApiResult<Json<Value>> {
    read(&m)?;
    Ok(Json(
        json!({ "projects": summaries(&st.store).await?, "today": today() }),
    ))
}

#[derive(Deserialize)]
struct NameQuery {
    name: String,
    except: Option<String>,
}

async fn check_name(
    State(st): State<AppState>,
    m: Member,
    Query(q): Query<NameQuery>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    Ok(Json(
        json!({ "taken": st.store.project_name_taken(q.name.trim(), q.except.as_deref()).await? }),
    ))
}

#[derive(Deserialize)]
struct NewLocation {
    name: String,
    #[serde(default)]
    code: String,
}

#[derive(Deserialize)]
struct NewContainer {
    #[serde(default)]
    location_id: Option<String>,
    name: String,
    #[serde(flatten)]
    data: ContainerData,
}

#[derive(Deserialize)]
struct NewProject {
    #[serde(flatten)]
    project: ProjectData,
    location: Option<NewLocation>,
    container: Option<NewContainer>,
}

/// Assistant de création : le projet, puis le premier emplacement, puis le premier contenant.
/// Un contenant refusé n'empêche pas le projet d'exister ; l'échec est rendu.
async fn create(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Json(mut b): Json<NewProject>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    write(&m)?;
    b.project.status = ProjectStatus::Preparation;
    let mut errors = validate_project(&mut b.project, true, &today());
    let mut loc = b.location;
    if let Some(l) = loc.as_mut() {
        errors.extend(
            validate_location(&mut l.name, &mut l.code)
                .into_iter()
                .map(|e| FieldError {
                    field: format!("location.{}", e.field),
                    ..e
                }),
        );
    }
    fields(errors)?;
    if st.store.project_name_taken(&b.project.name, None).await? {
        return Err(one("name", "Un projet porte déjà ce nom."));
    }
    let p = st
        .store
        .insert_project(&b.project, Some(&m.0.account.id))
        .await?;
    record(
        &st,
        &m,
        &origin,
        &p.id,
        "project",
        &p.id,
        "create",
        Some(p.rev),
        None::<&()>,
        Some(&p),
    )
    .await?;
    let mut location = None;
    let mut container = None;
    let mut container_error: Option<Value> = None;
    if let Some(l) = loc {
        let created = st.store.insert_location(&p.id, &l.name, &l.code).await?;
        record(
            &st,
            &m,
            &origin,
            &p.id,
            "location",
            &created.id,
            "create",
            Some(created.rev),
            None::<&()>,
            Some(&created),
        )
        .await?;
        if let Some(mut c) = b.container {
            let errs = validate_container(&mut c.name, &mut c.data);
            if errs.is_empty() {
                let made = st
                    .store
                    .insert_container(&p.id, &created.id, &c.name, &c.data)
                    .await?;
                record(
                    &st,
                    &m,
                    &origin,
                    &p.id,
                    "container",
                    &made.id,
                    "create",
                    Some(made.rev),
                    None::<&()>,
                    Some(&made),
                )
                .await?;
                container = Some(made);
            } else {
                container_error = Some(
                    json!({ "message": "Le projet est créé, mais l'élément a été refusé.", "fields": errs }),
                );
            }
        }
        location = Some(created);
    }
    Ok((
        StatusCode::CREATED,
        Json(
            json!({ "project": p, "location": location, "container": container, "container_error": container_error }),
        ),
    ))
}

async fn show(
    State(st): State<AppState>,
    m: Member,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    let p = project_of(&st.store, &id).await?;
    let (inv, _, view) = inventory_view(&st.store, &id).await?;
    let tasks = st.store.tasks(Some(&id)).await?;
    let mut out = view;
    out["project"] = json!(p);
    out["completion"] = json!(completion(
        &p.data,
        inv.locations.len(),
        inv.containers.len(),
        inv.articles.len()
    ));
    out["tasks"] = json!(tasks);
    out["presence"] = json!(st.hub.present(&room(&id)));
    out["today"] = json!(today());
    Ok(Json(out))
}

#[derive(Deserialize)]
struct ProjectPatch {
    rev: i64,
    #[serde(flatten)]
    data: ProjectData,
}

async fn update(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
    Json(mut b): Json<ProjectPatch>,
) -> ApiResult<Json<Project>> {
    write(&m)?;
    let before = project_of(&st.store, &id).await?;
    fields(validate_project(&mut b.data, false, &today()))?;
    if st.store.project_name_taken(&b.data.name, Some(&id)).await? {
        return Err(one("name", "Un projet porte déjà ce nom."));
    }
    let after = st.store.update_project(&id, b.rev, &b.data).await?;
    record(
        &st,
        &m,
        &origin,
        &id,
        "project",
        &id,
        "update",
        Some(after.rev),
        Some(&before),
        Some(&after),
    )
    .await?;
    Ok(Json(after))
}

async fn remove(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    write(&m)?;
    let before = project_of(&st.store, &id).await?;
    st.store.delete_project(&id).await?;
    st.store
        .audit(
            Some(&m.0.account.id),
            "delete",
            "project",
            &id,
            Some(&before),
            None::<&()>,
        )
        .await?;
    st.hub.change(
        room(&id),
        "project",
        &id,
        "delete",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct HistoryQuery {
    limit: Option<i64>,
}

async fn history(
    State(st): State<AppState>,
    m: Member,
    Path(id): Path<String>,
    Query(q): Query<HistoryQuery>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    project_of(&st.store, &id).await?;
    Ok(Json(
        json!({ "entries": crate::journal::describe(&st.store, st.store.project_history(&id, q.limit.unwrap_or(150)).await?).await? }),
    ))
}

// ----- Tableau de bord -----

async fn dashboard(State(st): State<AppState>, m: Member) -> ApiResult<Json<Value>> {
    read(&m)?;
    let all = summaries(&st.store).await?;
    let tasks = st.store.tasks(None).await?;
    let t = today();
    let today_n = day_number(&t).unwrap_or(0);
    let active: Vec<&Summary> = all
        .iter()
        .filter(|s| s.project.data.status.active())
        .collect();
    let done = tasks
        .iter()
        .filter(|t| matches!(t.data.status, TaskStatus::Done | TaskStatus::Validated))
        .count();
    let key_date = |s: &Summary| s.project.data.key_date().cloned();
    let next = active
        .iter()
        .filter_map(|s| {
            key_date(s)
                .filter(|d| d.as_str() > t.as_str())
                .map(|d| (d, *s))
        })
        .min_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, s)| s);
    let mut upcoming: Vec<(String, &Summary)> = active
        .iter()
        .filter_map(|s| key_date(s).map(|d| (d, *s)))
        .filter(|(d, _)| day_number(d).is_some_and(|n| (0..=30).contains(&(n - today_n))))
        .collect();
    upcoming.sort_by(|a, b| a.0.cmp(&b.0));
    let mut by_status: HashMap<String, usize> = HashMap::new();
    for s in &all {
        *by_status
            .entry(
                serde_json::to_value(s.project.data.status)
                    .unwrap_or_default()
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            )
            .or_default() += 1;
    }
    let up = |(d, s): &(String, &Summary)| json!({ "project": s.project, "completion": s.completion, "date": d, "days": day_number(d).unwrap_or(today_n) - today_n });
    Ok(Json(json!({
        "today": t,
        "projects_total": all.len(),
        "projects_active": active.len(),
        "tasks_total": tasks.len(),
        "tasks_done": done,
        "tasks_pct": if tasks.is_empty() { 0 } else { (done as f64 * 100.0 / tasks.len() as f64).round() as i64 },
        "completion_avg": if all.is_empty() { 0 } else { (all.iter().map(|s| s.completion as f64).sum::<f64>() / all.len() as f64).round() as i64 },
        "next": next.map(|s| json!({ "project": s.project, "completion": s.completion, "days": s.project.data.shoot_start.as_deref().and_then(day_number).unwrap_or(today_n) - today_n })),
        "upcoming": upcoming.iter().take(5).map(up).collect::<Vec<_>>(),
        "recent": all.iter().take(5).map(|s| json!({ "project": s.project, "completion": s.completion })).collect::<Vec<_>>(),
        "by_status": by_status,
    })))
}

// ----- Emplacements -----

async fn create_location(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(mut b): Json<NewLocation>,
) -> ApiResult<(StatusCode, Json<Location>)> {
    write(&m)?;
    project_of(&st.store, &pid).await?;
    fields(validate_location(&mut b.name, &mut b.code))?;
    let l = st.store.insert_location(&pid, &b.name, &b.code).await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "location",
        &l.id,
        "create",
        Some(l.rev),
        None::<&()>,
        Some(&l),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(l)))
}

#[derive(Deserialize)]
struct LocationPatch {
    rev: i64,
    name: String,
    #[serde(default)]
    code: String,
}

async fn update_location(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
    Json(mut b): Json<LocationPatch>,
) -> ApiResult<Json<Location>> {
    write(&m)?;
    let before = st
        .store
        .location(&id)
        .await?
        .ok_or(ApiError::NotFound("Emplacement"))?;
    fields(validate_location(&mut b.name, &mut b.code))?;
    let after = st
        .store
        .update_location(&id, b.rev, &b.name, &b.code)
        .await?;
    record(
        &st,
        &m,
        &origin,
        &after.project_id,
        "location",
        &id,
        "update",
        Some(after.rev),
        Some(&before),
        Some(&after),
    )
    .await?;
    Ok(Json(after))
}

async fn delete_location(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    write(&m)?;
    let l = st
        .store
        .location(&id)
        .await?
        .ok_or(ApiError::NotFound("Emplacement"))?;
    let inv = st.store.inventory(&l.project_id).await?;
    if inv.containers.iter().any(|c| c.location_id == id) {
        return Err(ApiError::Conflict(
            "Cet emplacement contient encore des éléments : déplacez-les avant.".into(),
        ));
    }
    if inv.articles.iter().any(|a| a.location_id == id) {
        return Err(ApiError::Conflict(
            "Cet emplacement porte encore des équipements libres : retirez-les avant.".into(),
        ));
    }
    st.store.delete_location(&id).await?;
    record(
        &st,
        &m,
        &origin,
        &l.project_id,
        "location",
        &id,
        "delete",
        None,
        Some(&l),
        None::<&()>,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct Order {
    ids: Vec<String>,
}

async fn order_locations(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<Order>,
) -> ApiResult<StatusCode> {
    write(&m)?;
    project_of(&st.store, &pid).await?;
    st.store.reorder("locations", &pid, &b.ids).await?;
    st.hub.change(
        room(&pid),
        "location",
        "*",
        "reorder",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(StatusCode::NO_CONTENT)
}

// ----- Contenants -----

async fn location_in(store: &Store, pid: &str, lid: &str) -> ApiResult<Location> {
    store
        .location(lid)
        .await?
        .filter(|l| l.project_id == pid)
        .ok_or_else(|| one("location_id", "Choisissez un emplacement du projet."))
}

async fn create_container(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(mut b): Json<NewContainer>,
) -> ApiResult<(StatusCode, Json<Container>)> {
    write(&m)?;
    project_of(&st.store, &pid).await?;
    let lid = b
        .location_id
        .clone()
        .ok_or_else(|| one("location_id", "Choisissez un emplacement du projet."))?;
    location_in(&st.store, &pid, &lid).await?;
    fields(validate_container(&mut b.name, &mut b.data))?;
    // Une casquette prend l'emplacement de son mobilier.
    let lid = match casquette_parent(&st.store, &pid, &b.data).await? {
        Some(p) => p.location_id,
        None => lid,
    };
    // Un élément neuf n'a pas encore de matériel : aucun écran ni poste ne peut en citer.
    b.data.details.map_instances(&|_| None);
    // Nom unique dans le projet : suffixe automatique en cas de collision.
    let taken: Vec<String> = st
        .store
        .inventory(&pid)
        .await?
        .containers
        .into_iter()
        .map(|c| c.name)
        .collect();
    let name = unique_name(&b.name, &taken);
    let c = st
        .store
        .insert_container(&pid, &lid, &name, &b.data)
        .await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "container",
        &c.id,
        "create",
        Some(c.rev),
        None::<&()>,
        Some(&c),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(c)))
}

#[derive(Deserialize)]
struct ContainerPatch {
    rev: i64,
    location_id: String,
    name: String,
    /// Retirer de l'élévation ce qui dépasserait une hauteur réduite (après confirmation).
    #[serde(default)]
    confirm_remove: bool,
    #[serde(flatten)]
    data: ContainerData,
}

async fn update_container(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
    Json(mut b): Json<ContainerPatch>,
) -> ApiResult<Json<Container>> {
    write(&m)?;
    let before = st
        .store
        .container(&id)
        .await?
        .ok_or(ApiError::NotFound("Élément"))?;
    location_in(&st.store, &before.project_id, &b.location_id).await?;
    fields(validate_container(&mut b.name, &mut b.data))?;
    if let Some(p) = casquette_parent(&st.store, &before.project_id, &b.data).await? {
        if p.id == id {
            return Err(one("parent_id", "Choisissez un autre mobilier."));
        }
        b.location_id = p.location_id;
    }
    // Tuiles et postes ne citent que des exemplaires du matériel de l'élément.
    let own = own_instances(&st.store, &before.project_id, &id).await?;
    b.data
        .details
        .map_instances(&|i| own.contains(i).then(|| i.to_string()));
    // Hauteur réduite ou contenant devenu non rackable : la liste de ce qui dépasse
    // est rendue ; après confirmation, ces éléments redeviennent « à placer ».
    let limit = if b.data.rackable {
        b.data.height_u.unwrap_or(0.0)
    } else {
        0.0
    };
    let over = crate::racks::overflow(&st.store, &before, limit).await?;
    if !over.is_empty() {
        if !b.confirm_remove {
            let names: Vec<&str> = over.iter().map(|o| o.1.as_str()).collect();
            return Err(ApiError::Conflict(format!(
                "{} {} : {}. Confirmez pour {} retirer de l'élévation.",
                over.len(),
                if over.len() > 1 {
                    "éléments dépasseraient"
                } else {
                    "élément dépasserait"
                },
                names.join(", "),
                if over.len() > 1 { "les" } else { "le" }
            )));
        }
        let ids: Vec<String> = over.into_iter().map(|o| o.0).collect();
        st.store.delete_placements(&ids).await?;
    }
    let after = st
        .store
        .update_container(&id, b.rev, &b.location_id, &b.name, &b.data)
        .await
        .map_err(|e| match e {
            StoreError::Conflict(_) => one("name", "Un élément porte déjà ce nom dans le projet."),
            e => e.into(),
        })?;
    record(
        &st,
        &m,
        &origin,
        &after.project_id,
        "container",
        &id,
        "update",
        Some(after.rev),
        Some(&before),
        Some(&after),
    )
    .await?;
    Ok(Json(after))
}

/// Mobilier qui porte une casquette : il doit exister dans le projet et accepter des casquettes.
async fn casquette_parent(
    store: &Store,
    pid: &str,
    data: &ContainerData,
) -> ApiResult<Option<Container>> {
    let Some(parent) = data.parent_id.as_deref() else {
        return Ok(None);
    };
    let p = store
        .container(parent)
        .await?
        .filter(|c| c.project_id == pid && c.data.kind.takes_casquettes())
        .ok_or_else(|| one("parent_id", "Choisissez un mobilier de ce projet."))?;
    Ok(Some(p))
}

async fn delete_container(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    write(&m)?;
    let before = st
        .store
        .container(&id)
        .await?
        .ok_or(ApiError::NotFound("Élément"))?;
    st.store.delete_container(&id).await?;
    record(
        &st,
        &m,
        &origin,
        &before.project_id,
        "container",
        &id,
        "delete",
        None,
        Some(&before),
        None::<&()>,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn order_containers(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<Order>,
) -> ApiResult<StatusCode> {
    write(&m)?;
    project_of(&st.store, &pid).await?;
    st.store.reorder("containers", &pid, &b.ids).await?;
    st.hub.change(
        room(&pid),
        "container",
        "*",
        "reorder",
        None,
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok(StatusCode::NO_CONTENT)
}

// ----- Articles -----

#[derive(Deserialize)]
struct NewArticle {
    location_id: Option<String>,
    container_id: Option<String>,
    nature: ArticleNature,
    item_id: Option<String>,
    #[serde(default = "one_qty")]
    quantity: i64,
    accessory: Option<Accessory>,
}

fn one_qty() -> i64 {
    1
}

/// Avertissements non bloquants d'un ajout dans un contenant rackable.
fn fit_warnings(
    c: &Container,
    used_u: f64,
    item: Option<&ItemView>,
    acc: Option<&Accessory>,
    quantity: i64,
) -> Vec<String> {
    let mut w = Vec::new();
    if !c.data.rackable {
        return w;
    }
    let h = match (item, acc) {
        (Some(i), _) if i.rackable => i.height_u,
        (_, Some(a)) => Some(a.height_u() as f64),
        _ => None,
    };
    if let (Some(h), Some(total)) = (h, c.data.height_u) {
        let need = h * quantity as f64;
        if used_u + need > total {
            w.push(format!(
                "Place insuffisante : {} U demandés, {} U libres dans {}.",
                fmt(need),
                fmt((total - used_u).max(0.0)),
                c.name
            ));
        }
    }
    let depth = match (item, acc) {
        (Some(i), _) if i.rackable => i.depth_mm,
        (_, Some(acc)) => acc.depth_mm(),
        _ => None,
    };
    if let (Some(d), Some(cd)) = (depth, c.data.depth_mm) {
        if d > cd - 80.0 {
            w.push(format!(
                "Profondeur : {} mm pour {} mm d'élément, moins 80 mm de marge de câblage.",
                fmt(d),
                fmt(cd)
            ));
        }
    }
    w
}

fn fmt(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.1}").replace('.', ",")
    }
}

async fn create_article(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(mut b): Json<NewArticle>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    write(&m)?;
    project_of(&st.store, &pid).await?;
    let container = match &b.container_id {
        Some(cid) => Some(
            st.store
                .container(cid)
                .await?
                .filter(|c| c.project_id == pid)
                .ok_or_else(|| one("container_id", "Élément inconnu dans ce projet."))?,
        ),
        None => None,
    };
    let lid = match (&container, &b.location_id) {
        (Some(c), _) => c.location_id.clone(),
        (None, Some(l)) => location_in(&st.store, &pid, l).await?.id,
        (None, None) => {
            return Err(one(
                "location_id",
                "Choisissez un élément ou un emplacement.",
            ));
        }
    };
    if !(1..=999).contains(&b.quantity) {
        return Err(one("quantity", "De 1 à 999."));
    }
    let mut quantity = b.quantity;
    let mut merge = true;
    let mut item_view = None;
    match b.nature {
        ArticleNature::Equipment | ArticleNature::Adapter => {
            let want = if b.nature == ArticleNature::Equipment {
                strata_domain::catalog::ItemKind::Equipment
            } else {
                strata_domain::catalog::ItemKind::Adapter
            };
            let id = b
                .item_id
                .as_deref()
                .ok_or_else(|| one("item_id", "Choisissez une fiche du catalogue."))?;
            let item = st
                .store
                .item(id)
                .await?
                .filter(|i| i.kind == want)
                .ok_or_else(|| one("item_id", "Fiche du catalogue introuvable."))?;
            let refs = st.store.list_refs(None).await?;
            let name = |kind: strata_domain::catalog::RefKind, id: Option<&str>| {
                id.and_then(|id| refs.iter().find(|r| r.kind == kind && r.id == id))
                    .map(|r| r.name.clone())
            };
            let v = view_of(&item, &name);
            if v.modular {
                // Un équipement modulaire est toujours en quantité 1.
                quantity = 1;
                merge = false;
            }
            item_view = Some(v);
        }
        ArticleNature::Accessory => {
            let acc = b
                .accessory
                .as_mut()
                .ok_or_else(|| one("accessory", "Décrivez l'accessoire."))?;
            fields(acc.validate())?;
            if let Accessory::Panel { panel } = acc {
                crate::panels::check_panel(&st.store, panel).await?;
            }
            if !container.as_ref().is_some_and(|c| c.data.rackable) {
                return Err(one(
                    "container_id",
                    "Un accessoire se pose dans un élément rackable.",
                ));
            }
            merge = false;
        }
    }
    let warnings = match &container {
        Some(c) => {
            let (_, items, _) = inventory_view(&st.store, &pid).await?;
            let inv = st.store.inventory(&pid).await?;
            let used: f64 = inv
                .articles
                .iter()
                .filter(|a| a.container_id.as_deref() == Some(&c.id))
                .map(|a| load_of(a, &items))
                .filter_map(|l| l.height_u.map(|h| h * l.quantity as f64))
                .sum();
            fit_warnings(c, used, item_view.as_ref(), b.accessory.as_ref(), quantity)
        }
        None => Vec::new(),
    };
    let (a, created) = st
        .store
        .add_article(
            &pid,
            &lid,
            b.container_id.as_deref(),
            b.nature,
            b.item_id.as_deref(),
            quantity,
            if b.nature == ArticleNature::Accessory {
                b.accessory.as_ref()
            } else {
                None
            },
            merge,
        )
        .await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "article",
        &a.id,
        if created { "create" } else { "update" },
        Some(a.rev),
        None::<&()>,
        Some(&a),
    )
    .await?;
    // Châssis : ses modules par défaut (fiche du catalogue) sont glissés dans ses emplacements.
    let mut modules = Vec::new();
    if created && item_view.as_ref().is_some_and(|v| v.modular) {
        if let Some(it) = st
            .store
            .item(a.item_id.as_deref().unwrap_or_default())
            .await?
        {
            if let Some(Modular::Host {
                default_composition,
                ..
            }) = it
                .data
                .get("modular")
                .and_then(|v| serde_json::from_value::<Modular>(v.clone()).ok())
            {
                for p in default_composition {
                    if st.store.item(&p.equipment_id).await?.is_none() {
                        continue;
                    }
                    let (mo, _) = st
                        .store
                        .add_article(
                            &pid,
                            &lid,
                            b.container_id.as_deref(),
                            ArticleNature::Equipment,
                            Some(&p.equipment_id),
                            1,
                            None,
                            false,
                        )
                        .await?;
                    let mo = st
                        .store
                        .set_article_host(
                            &mo.id,
                            Some(&ModuleHost {
                                article_id: a.id.clone(),
                                slot: p.slot,
                            }),
                        )
                        .await?;
                    modules.push(mo);
                }
            }
        }
    }
    Ok((
        StatusCode::CREATED,
        Json(json!({ "article": a, "merged": !created, "warnings": warnings, "modules": modules })),
    ))
}

#[derive(Deserialize)]
struct ArticlePatch {
    rev: i64,
    quantity: i64,
    /// PDU vertical : longueur, prises, couloir et circuit d'alimentation modifiables.
    #[serde(default)]
    accessory: Option<Accessory>,
}

async fn update_article(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
    Json(b): Json<ArticlePatch>,
) -> ApiResult<Json<Article>> {
    write(&m)?;
    let before = st
        .store
        .article(&id)
        .await?
        .ok_or(ApiError::NotFound("Matériel"))?;
    if let Some(acc) = b.accessory {
        let (Some(Accessory::VerticalPdu { .. }), Accessory::VerticalPdu { circuit_id, .. }) =
            (&before.accessory, &acc)
        else {
            return Err(one("accessory", "Seul un PDU vertical se modifie ainsi."));
        };
        fields(acc.validate())?;
        if let Some(cid) = circuit_id {
            let ok = match before.container_id.as_deref() {
                Some(c) => st.store.circuits(c).await?.iter().any(|x| &x.id == cid),
                None => false,
            };
            if !ok {
                return Err(one("accessory.circuit_id", "Circuit inconnu dans ce rack."));
            }
        }
        let after = st.store.set_article_accessory(&id, b.rev, &acc).await?;
        record(
            &st,
            &m,
            &origin,
            &after.project_id,
            "article",
            &id,
            "update",
            Some(after.rev),
            Some(&before),
            Some(&after),
        )
        .await?;
        return Ok(Json(after));
    }
    if !(1..=999).contains(&b.quantity) {
        return Err(one(
            "quantity",
            "De 1 à 999. Pour retirer ce matériel, supprimez-le.",
        ));
    }
    if before.nature == ArticleNature::Accessory && b.quantity != before.quantity {
        return Err(one("quantity", "Un accessoire se pose à l'unité."));
    }
    if before.host.is_some() && b.quantity != 1 {
        return Err(one(
            "quantity",
            "Un module glissé dans un châssis est à l'unité.",
        ));
    }
    if let Some(item) = before.item_id.as_deref() {
        let ids = HashSet::from([item.to_string()]);
        if item_views(&st.store, &ids)
            .await?
            .get(item)
            .is_some_and(|v| v.modular)
            && b.quantity != 1
        {
            return Err(one(
                "quantity",
                "Un équipement modulaire est toujours en quantité 1.",
            ));
        }
    }
    let after = st
        .store
        .set_article_quantity(&id, b.rev, b.quantity)
        .await?;
    record(
        &st,
        &m,
        &origin,
        &after.project_id,
        "article",
        &id,
        "update",
        Some(after.rev),
        Some(&before),
        Some(&after),
    )
    .await?;
    Ok(Json(after))
}

#[derive(Deserialize)]
struct MoveBody {
    rev: i64,
    /// Contenant d'arrivée ; absent : l'article devient libre dans `location_id`.
    container_id: Option<String>,
    location_id: Option<String>,
    /// Exemplaire à sortir d'un article en quantité N (le dernier par défaut).
    instance_id: Option<String>,
}

/// Déplacement : un article en quantité 1 (ou un adaptateur, un accessoire) part entier ;
/// sinon un seul exemplaire part, avec son repère.
async fn move_article(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
    Json(b): Json<MoveBody>,
) -> ApiResult<Json<Article>> {
    write(&m)?;
    let a = st
        .store
        .article(&id)
        .await?
        .ok_or(ApiError::NotFound("Matériel"))?;
    let pid = a.project_id.clone();
    if a.host.is_some() {
        return Err(one(
            "container_id",
            "Un module suit son châssis : déplacez le châssis, ou retirez le module.",
        ));
    }
    let (lid, cid) = match &b.container_id {
        Some(cid) => {
            let c = st
                .store
                .container(cid)
                .await?
                .filter(|c| c.project_id == pid)
                .ok_or_else(|| one("container_id", "Élément inconnu dans ce projet."))?;
            if a.nature == ArticleNature::Accessory && !c.data.rackable {
                return Err(one(
                    "container_id",
                    "Un accessoire se pose dans un élément rackable.",
                ));
            }
            (c.location_id, Some(c.id))
        }
        None => {
            if a.nature == ArticleNature::Accessory {
                return Err(one("container_id", "Un accessoire reste dans un élément."));
            }
            let l = b
                .location_id
                .as_deref()
                .ok_or_else(|| one("location_id", "Choisissez un élément ou un emplacement."))?;
            (location_in(&st.store, &pid, l).await?.id, None)
        }
    };
    let moved = if a.nature == ArticleNature::Equipment && a.quantity > 1 {
        let inst = st.store.article_instances(&id).await?;
        let pick = match &b.instance_id {
            Some(iid) => inst
                .iter()
                .find(|i| &i.id == iid)
                .ok_or(ApiError::NotFound("Exemplaire"))?,
            None => inst.last().ok_or(ApiError::NotFound("Exemplaire"))?,
        };
        st.store
            .split_instance(&id, b.rev, &pick.id, &lid, cid.as_deref())
            .await?
    } else {
        st.store
            .move_article(&id, b.rev, &lid, cid.as_deref())
            .await?
    };
    record(
        &st,
        &m,
        &origin,
        &pid,
        "article",
        &moved.id,
        "move",
        Some(moved.rev),
        Some(&a),
        Some(&moved),
    )
    .await?;
    Ok(Json(moved))
}

async fn delete_article(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    write(&m)?;
    let before = st
        .store
        .article(&id)
        .await?
        .ok_or(ApiError::NotFound("Matériel"))?;
    st.store.delete_article(&id).await?;
    record(
        &st,
        &m,
        &origin,
        &before.project_id,
        "article",
        &id,
        "delete",
        None,
        Some(&before),
        None::<&()>,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct NewModule {
    item_id: String,
    slot: u32,
}

/// Glisse un module du catalogue dans un emplacement libre d'un châssis du projet.
async fn add_module(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
    Json(b): Json<NewModule>,
) -> ApiResult<(StatusCode, Json<Article>)> {
    write(&m)?;
    let host = st
        .store
        .article(&id)
        .await?
        .ok_or(ApiError::NotFound("Matériel"))?;
    let mut ids = HashSet::from([b.item_id.clone()]);
    if let Some(i) = &host.item_id {
        ids.insert(i.clone());
    }
    let views = item_views(&st.store, &ids).await?;
    let slots = host
        .item_id
        .as_deref()
        .and_then(|i| views.get(i))
        .and_then(|v| v.host_slots)
        .ok_or_else(|| one("item_id", "Cet équipement ne reçoit pas de modules."))?;
    let width = views
        .get(&b.item_id)
        .and_then(|v| v.module_slots)
        .ok_or_else(|| one("item_id", "Cette fiche n'est pas un module."))?;
    if b.slot == 0 || b.slot + width - 1 > slots {
        return Err(one("slot", "Le module dépasse du châssis."));
    }
    // Emplacements déjà pris par les autres modules du châssis.
    let inv = st.store.inventory(&host.project_id).await?;
    let others: Vec<&Article> = inv
        .articles
        .iter()
        .filter(|a| a.host.as_ref().is_some_and(|h| h.article_id == host.id))
        .collect();
    let oids: HashSet<String> = others.iter().filter_map(|a| a.item_id.clone()).collect();
    let ov = item_views(&st.store, &oids).await?;
    for o in &others {
        let h = o.host.as_ref().unwrap();
        let w = o
            .item_id
            .as_deref()
            .and_then(|i| ov.get(i))
            .and_then(|v| v.module_slots)
            .unwrap_or(1);
        if b.slot < h.slot + w && h.slot < b.slot + width {
            return Err(one("slot", "Emplacement déjà occupé."));
        }
    }
    let (mo, _) = st
        .store
        .add_article(
            &host.project_id,
            &host.location_id,
            host.container_id.as_deref(),
            ArticleNature::Equipment,
            Some(&b.item_id),
            1,
            None,
            false,
        )
        .await?;
    let mo = st
        .store
        .set_article_host(
            &mo.id,
            Some(&ModuleHost {
                article_id: host.id.clone(),
                slot: b.slot,
            }),
        )
        .await?;
    record(
        &st,
        &m,
        &origin,
        &host.project_id,
        "article",
        &mo.id,
        "create",
        Some(mo.rev),
        None::<&()>,
        Some(&mo),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(mo)))
}

#[derive(Deserialize)]
struct InstancePatch {
    rev: i64,
    #[serde(default)]
    label: Option<String>,
    serial: Option<String>,
    mac: Option<String>,
    inventory_tag: Option<String>,
}

async fn update_instance(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
    Json(b): Json<InstancePatch>,
) -> ApiResult<Json<strata_store::Instance>> {
    write(&m)?;
    let before = st
        .store
        .instance(&id)
        .await?
        .ok_or(ApiError::NotFound("Exemplaire"))?;
    let clean = |s: Option<String>, max: usize| {
        s.map(|v| v.trim().chars().take(max).collect::<String>())
            .filter(|v| !v.is_empty())
    };
    let mac = match clean(b.mac, 40) {
        Some(v) => Some(normalize_mac(&v).ok_or_else(|| {
            one(
                "mac",
                "Adresse MAC attendue, par exemple 00:1A:2B:3C:4D:5E.",
            )
        })?),
        None => None,
    };
    let after = st
        .store
        .update_instance(
            &id,
            b.rev,
            clean(b.label, 80).as_deref(),
            clean(b.serial, 80).as_deref(),
            mac.as_deref(),
            clean(b.inventory_tag, 80).as_deref(),
        )
        .await?;
    record(
        &st,
        &m,
        &origin,
        &after.project_id,
        "instance",
        &id,
        "update",
        Some(after.rev),
        Some(&before),
        Some(&after),
    )
    .await?;
    Ok(Json(after))
}

// ----- Tâches -----

async fn list_tasks(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    project_of(&st.store, &pid).await?;
    Ok(Json(json!({ "tasks": st.store.tasks(Some(&pid)).await? })))
}

async fn check_assignee(store: &Store, t: &TaskData) -> ApiResult<()> {
    if let Some(a) = &t.assignee_id {
        if !store
            .account(a)
            .await?
            .is_some_and(|a| a.active && !a.removed)
        {
            return Err(one("assignee_id", "Membre inconnu ou désactivé."));
        }
    }
    Ok(())
}

async fn create_task(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(mut b): Json<TaskData>,
) -> ApiResult<(StatusCode, Json<strata_store::Task>)> {
    write(&m)?;
    project_of(&st.store, &pid).await?;
    fields(validate_task(&mut b))?;
    check_assignee(&st.store, &b).await?;
    let t = st
        .store
        .insert_task(&pid, &b, Some(&m.0.account.id))
        .await?;
    record(
        &st,
        &m,
        &origin,
        &pid,
        "task",
        &t.id,
        "create",
        Some(t.rev),
        None::<&()>,
        Some(&t),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(t)))
}

#[derive(Deserialize)]
struct TaskPatch {
    rev: i64,
    #[serde(flatten)]
    data: TaskData,
}

async fn update_task(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
    Json(mut b): Json<TaskPatch>,
) -> ApiResult<Json<strata_store::Task>> {
    write(&m)?;
    let before = st
        .store
        .task(&id)
        .await?
        .ok_or(ApiError::NotFound("Tâche"))?;
    fields(validate_task(&mut b.data))?;
    check_assignee(&st.store, &b.data).await?;
    // Une tâche qui passe à Terminé ou Validé prend sa date de fin.
    if matches!(b.data.status, TaskStatus::Done | TaskStatus::Validated) && b.data.done_on.is_none()
    {
        b.data.done_on = Some(today());
    }
    if matches!(b.data.status, TaskStatus::Todo | TaskStatus::InProgress) {
        b.data.done_on = None;
    }
    let after = st.store.update_task(&id, b.rev, &b.data).await?;
    record(
        &st,
        &m,
        &origin,
        &after.project_id,
        "task",
        &id,
        "update",
        Some(after.rev),
        Some(&before),
        Some(&after),
    )
    .await?;
    Ok(Json(after))
}

async fn delete_task(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    write(&m)?;
    let before = st
        .store
        .task(&id)
        .await?
        .ok_or(ApiError::NotFound("Tâche"))?;
    st.store.delete_task(&id).await?;
    record(
        &st,
        &m,
        &origin,
        &before.project_id,
        "task",
        &id,
        "delete",
        None,
        Some(&before),
        None::<&()>,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ----- Nomenclature -----

#[derive(Deserialize)]
struct ExportQuery {
    #[serde(default = "xlsx")]
    format: String,
    /// none, location ou container.
    #[serde(default)]
    group: Option<String>,
    #[serde(default = "yes")]
    free: bool,
}

fn xlsx() -> String {
    "xlsx".into()
}
fn yes() -> bool {
    true
}

/// Ligne de nomenclature : rang de l'emplacement, du contenant, cellules, totaux (quantité, poids, conso).
type Line = (usize, usize, Vec<Cell>, (f64, f64, f64));

/// Nomenclature consolidée (section 8.5), avec sous-totaux selon le regroupement.
async fn export(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
    Query(q): Query<ExportQuery>,
) -> ApiResult<Response> {
    read(&m)?;
    let p = project_of(&st.store, &pid).await?;
    let (inv, items, _) = inventory_view(&st.store, &pid).await?;
    let headers: Vec<String> = [
        "Emplacement",
        "Élément",
        "Type",
        "Désignation",
        "Marque",
        "Catégorie",
        "Quantité",
        "Hauteur (U)",
        "Poids (kg)",
        "Consommation (W)",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let loc_name = |id: &str| {
        inv.locations
            .iter()
            .find(|l| l.id == id)
            .map(|l| l.name.clone())
            .unwrap_or_default()
    };
    let mut lines: Vec<Line> = Vec::new();
    for a in &inv.articles {
        if a.container_id.is_none() && !q.free {
            continue;
        }
        let it = a.item_id.as_ref().and_then(|i| items.get(i));
        let load = load_of(a, &items);
        let kind = match a.nature {
            ArticleNature::Equipment => "Équipement",
            ArticleNature::Adapter => "Adaptateur",
            ArticleNature::Accessory => "Accessoire",
        };
        let li = inv
            .locations
            .iter()
            .position(|l| l.id == a.location_id)
            .unwrap_or(usize::MAX);
        let ci = a
            .container_id
            .as_ref()
            .and_then(|c| inv.containers.iter().position(|x| &x.id == c))
            .unwrap_or(usize::MAX);
        let cname = a
            .container_id
            .as_ref()
            .and_then(|c| inv.containers.iter().find(|x| &x.id == c))
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "Équipements libres".into());
        let w = load.weight_kg * a.quantity as f64;
        let pw = load.power_w * a.quantity as f64;
        lines.push((
            li,
            ci,
            vec![
                Cell::Text(loc_name(&a.location_id)),
                Cell::Text(cname),
                Cell::Text(kind.into()),
                Cell::Text(article_label(a, &items)),
                Cell::Text(it.and_then(|i| i.brand.clone()).unwrap_or_default()),
                Cell::Text(it.and_then(|i| i.category.clone()).unwrap_or_default()),
                Cell::Number(Some(a.quantity as f64)),
                Cell::Number(load.height_u),
                Cell::Number(if w > 0.0 { Some(w) } else { None }),
                Cell::Number(if pw > 0.0 { Some(pw) } else { None }),
            ],
            (a.quantity as f64, w, pw),
        ));
    }
    lines.sort_by_key(|l| (l.0, l.1));
    let total_row = |label: String, t: (f64, f64, f64)| {
        let mut r = vec![
            Cell::Text(label),
            Cell::Text(String::new()),
            Cell::Text(String::new()),
            Cell::Text(String::new()),
            Cell::Text(String::new()),
            Cell::Text(String::new()),
        ];
        r.extend([
            Cell::Number(Some(t.0)),
            Cell::Number(None),
            Cell::Number(Some((t.1 * 10.0).round() / 10.0)),
            Cell::Number(Some(t.2.round())),
        ]);
        r
    };
    let group = q.group.as_deref().unwrap_or("none");
    let mut rows = Vec::new();
    let mut sub = (0.0, 0.0, 0.0);
    let mut grand = (0.0, 0.0, 0.0);
    for (i, (li, ci, cells, t)) in lines.iter().enumerate() {
        rows.push(cells.clone());
        sub = (sub.0 + t.0, sub.1 + t.1, sub.2 + t.2);
        grand = (grand.0 + t.0, grand.1 + t.1, grand.2 + t.2);
        let next = lines.get(i + 1);
        let boundary = match group {
            "location" => next.is_none_or(|n| n.0 != *li),
            "container" => next.is_none_or(|n| (n.0, n.1) != (*li, *ci)),
            _ => false,
        };
        if boundary {
            let label = match (group, &cells[0], &cells[1]) {
                ("location", Cell::Text(l), _) => format!("Sous-total · {l}"),
                (_, Cell::Text(l), Cell::Text(c)) => format!("Sous-total · {l} · {c}"),
                _ => "Sous-total".into(),
            };
            rows.push(total_row(label, sub));
            sub = (0.0, 0.0, 0.0);
        }
    }
    rows.push(total_row("Total".into(), grand));
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
    let stem = format!("Inventaire_{safe}_{}", today());
    match q.format.as_str() {
        "csv" => Ok(file_response(
            to_csv(&headers, &rows).map_err(ApiError::Internal)?,
            "text/csv; charset=utf-8",
            &format!("{stem}.csv"),
        )),
        "xlsx" => Ok(file_response(
            to_xlsx_named(&headers, &rows, "Inventaire")
                .map_err(|e| ApiError::Internal(e.to_string()))?,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            &format!("{stem}.xlsx"),
        )),
        _ => Err(ApiError::Invalid("Format d'export : csv ou xlsx.".into())),
    }
}
