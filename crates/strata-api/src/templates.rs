//! Bibliothèque de modèles de contenants (section 6.1).
//!
//! Un modèle garde le contenant et tout son contenu : articles, accessoires, circuits et
//! placements dans l'élévation. « Enregistrer en tant que modèle » le photographie depuis
//! un projet ; « Ajouter depuis un modèle » le recopie dans un projet. L'original ne change
//! jamais. Gérer la bibliothèque demande « Catalogue : écriture ».

use crate::AppState;
use crate::auth::Member;
use crate::error::{ApiError, ApiResult};
use crate::live::Origin;
use crate::projects::{ItemView, fields, item_views, one, room};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use strata_domain::catalog::{FieldError, Width};
use strata_domain::project::{
    Accessory, ArticleNature, ContainerData, unique_name, validate_container,
};
use strata_domain::rack::{Phase, RackFace};
use strata_domain::{Domain, Level};
use strata_store::{Container, ContainerTemplate, PlaceSpec, Store, StoreError, TemplateStatus};

const LIB_WRITE: &str = "Gérer la bibliothèque de modèles demande « Catalogue : écriture ».";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/templates", get(list))
        .route("/templates/{id}", get(show).patch(update).delete(remove))
        .route("/containers/{id}/save-as-template", post(save_as))
        .route(
            "/projects/{id}/containers/from-template",
            post(from_template),
        )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TArticle {
    key: usize,
    nature: ArticleNature,
    item_id: Option<String>,
    quantity: i64,
    accessory: Option<Accessory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TCircuit {
    key: usize,
    name: String,
    phase: Phase,
    rating_a: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TPlacement {
    /// Article (clé) et rang de l'exemplaire dans l'article.
    article: usize,
    nth: Option<usize>,
    face: RackFace,
    u_start: i64,
    width: Width,
    col: i64,
    circuit: Option<usize>,
    /// Rang, dans cette liste, de l'étagère qui porte l'élément.
    shelf: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Content {
    container: ContainerData,
    articles: Vec<TArticle>,
    circuits: Vec<TCircuit>,
    placements: Vec<TPlacement>,
}

fn content(t: &ContainerTemplate) -> ApiResult<Content> {
    serde_json::from_value(t.data.clone())
        .map_err(|e| ApiError::Internal(format!("modèle illisible : {e}")))
}

/// Occupation, poids et consommation d'un modèle, à partir des fiches du catalogue.
fn summary(c: &Content, items: &HashMap<String, ItemView>) -> Value {
    let mut used = HashMap::from([
        (RackFace::Front, 0.0),
        (RackFace::Rear, 0.0),
        (RackFace::RearInner, 0.0),
    ]);
    let height = |a: &TArticle| match (&a.accessory, a.item_id.as_ref().and_then(|i| items.get(i)))
    {
        (Some(acc), _) => Some(acc.height_u() as f64),
        (None, Some(it)) if it.rackable => it.height_u,
        _ => None,
    };
    for p in c.placements.iter().filter(|p| p.shelf.is_none()) {
        if let Some(h) = c
            .articles
            .iter()
            .find(|a| a.key == p.article)
            .and_then(height)
        {
            *used.entry(p.face).or_default() += h;
        }
    }
    let weight = c.container.empty_weight_kg.unwrap_or(0.0)
        + c.articles
            .iter()
            .map(|a| {
                a.item_id
                    .as_ref()
                    .and_then(|i| items.get(i))
                    .and_then(|i| i.weight_kg)
                    .unwrap_or(0.0)
                    * a.quantity as f64
            })
            .sum::<f64>();
    let power: f64 = c
        .articles
        .iter()
        .map(|a| {
            a.item_id
                .as_ref()
                .and_then(|i| items.get(i))
                .and_then(|i| i.power_w)
                .unwrap_or(0.0)
                * a.quantity as f64
        })
        .sum();
    let total = c.container.height_u.unwrap_or(0.0);
    let pct = |f: RackFace| {
        if total > 0.0 {
            (used[&f] / total * 100.0).round()
        } else {
            0.0
        }
    };
    let missing = c
        .articles
        .iter()
        .filter(|a| a.item_id.as_ref().is_some_and(|i| !items.contains_key(i)))
        .count();
    json!({
        "used_front_u": used[&RackFace::Front], "used_rear_u": used[&RackFace::Rear], "front_pct": pct(RackFace::Front), "rear_pct": pct(RackFace::Rear),
        "weight_kg": (weight * 10.0).round() / 10.0 + 0.0, "power_w": power.round() + 0.0,
        "over_weight": c.container.max_weight_kg.is_some_and(|m| weight > m), "over_power": c.container.max_power_w.is_some_and(|m| power > m),
        "articles": c.articles.iter().map(|a| a.quantity).sum::<i64>(), "missing_items": missing,
    })
}

async fn items_of(store: &Store, cs: &[&Content]) -> ApiResult<HashMap<String, ItemView>> {
    let ids: HashSet<String> = cs
        .iter()
        .flat_map(|c| c.articles.iter().filter_map(|a| a.item_id.clone()))
        .collect();
    item_views(store, &ids).await
}

fn view(t: &ContainerTemplate, c: &Content, items: &HashMap<String, ItemView>) -> Value {
    let mut v = serde_json::to_value(t).unwrap_or_default();
    v["container"] = json!(c.container);
    v["summary"] = summary(c, items);
    v
}

#[derive(Deserialize)]
struct ListQuery {
    status: Option<TemplateStatus>,
}

/// Tous les modèles de contenants, avec leurs résumés.
pub(crate) async fn views(store: &Store) -> ApiResult<Vec<Value>> {
    let all: Vec<(ContainerTemplate, Content)> = store
        .templates()
        .await?
        .into_iter()
        .filter_map(|t| content(&t).ok().map(|c| (t, c)))
        .collect();
    let items = items_of(store, &all.iter().map(|(_, c)| c).collect::<Vec<_>>()).await?;
    Ok(all.iter().map(|(t, c)| view(t, c, &items)).collect())
}

async fn list(
    State(st): State<AppState>,
    m: Member,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Value>> {
    if !m.0.actor.allows(Domain::Catalogue, Level::Lecture)
        && !m.0.actor.allows(Domain::Projets, Level::Lecture)
    {
        return Err(ApiError::Forbidden(
            "Vous n'avez pas accès à la bibliothèque.".into(),
        ));
    }
    let all = st.store.templates().await?;
    let all: Vec<(ContainerTemplate, Content)> = all
        .into_iter()
        .filter(|t| q.status.is_none_or(|s| t.status == s))
        .filter_map(|t| content(&t).ok().map(|c| (t, c)))
        .collect();
    let items = items_of(&st.store, &all.iter().map(|(_, c)| c).collect::<Vec<_>>()).await?;
    Ok(Json(
        json!({ "templates": all.iter().map(|(t, c)| view(t, c, &items)).collect::<Vec<_>>() }),
    ))
}

async fn show(
    State(st): State<AppState>,
    m: Member,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    m.0.require(
        Domain::Catalogue,
        Level::Lecture,
        "Vous n'avez pas accès à la bibliothèque.",
    )?;
    let t = st
        .store
        .template(&id)
        .await?
        .ok_or(ApiError::NotFound("Modèle"))?;
    let c = content(&t)?;
    let items = items_of(&st.store, &[&c]).await?;
    let mut v = view(&t, &c, &items);
    v["articles"] = json!(c.articles.iter().map(|a| json!({
        "nature": a.nature, "quantity": a.quantity,
        "label": match (&a.accessory, a.item_id.as_ref().and_then(|i| items.get(i))) { (Some(acc), _) => acc.label(), (None, Some(it)) => it.name.clone(), _ => "Fiche supprimée du catalogue".into() },
    })).collect::<Vec<_>>());
    Ok(Json(v))
}

#[derive(Deserialize)]
struct SaveAs {
    name: Option<String>,
}

/// « Enregistrer en tant que modèle » : photographie du contenant et de son contenu.
/// Photographie d'un contenant de projet et de son contenu, au format d'un modèle.
pub(crate) async fn snapshot(store: &Store, c: &Container) -> ApiResult<Content> {
    let inv = store.inventory(&c.project_id).await?;
    let arts: Vec<_> = inv
        .articles
        .iter()
        .filter(|a| a.container_id.as_deref() == Some(&c.id))
        .collect();
    let key_of: HashMap<&str, usize> = arts
        .iter()
        .enumerate()
        .map(|(i, a)| (a.id.as_str(), i))
        .collect();
    let circuits = store.circuits(&c.id).await?;
    let circ_key: HashMap<&str, usize> = circuits
        .iter()
        .enumerate()
        .map(|(i, x)| (x.id.as_str(), i))
        .collect();
    let mut placements = store.placements(&c.id).await?;
    // Les étagères avant ce qu'elles portent.
    placements.sort_by_key(|p| p.shelf_id.is_some());
    let index: HashMap<String, usize> = placements
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id.clone(), i))
        .collect();
    let nth = |p: &strata_store::Placement| {
        p.instance_id.as_ref().and_then(|iid| {
            inv.instances
                .iter()
                .filter(|i| i.article_id == p.article_id)
                .position(|i| &i.id == iid)
        })
    };
    let data = Content {
        container: {
            let mut d = c.data.clone();
            d.cable_code = String::new();
            d
        },
        articles: arts
            .iter()
            .enumerate()
            .map(|(i, a)| TArticle {
                key: i,
                nature: a.nature,
                item_id: a.item_id.clone(),
                quantity: a.quantity,
                accessory: a.accessory.clone(),
            })
            .collect(),
        circuits: circuits
            .iter()
            .enumerate()
            .map(|(i, x)| TCircuit {
                key: i,
                name: x.name.clone(),
                phase: x.phase,
                rating_a: x.rating_a,
            })
            .collect(),
        placements: placements
            .iter()
            .filter_map(|p| {
                Some(TPlacement {
                    article: *key_of.get(p.article_id.as_str())?,
                    nth: nth(p),
                    face: p.face,
                    u_start: p.u_start,
                    width: p.width,
                    col: p.col,
                    circuit: p
                        .circuit_id
                        .as_deref()
                        .and_then(|x| circ_key.get(x).copied()),
                    shelf: p.shelf_id.as_ref().and_then(|s| index.get(s).copied()),
                })
            })
            .collect(),
    };
    Ok(data)
}

async fn save_as(
    State(st): State<AppState>,
    m: Member,
    Path(cid): Path<String>,
    Json(b): Json<SaveAs>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    m.0.require(Domain::Catalogue, Level::Ecriture, LIB_WRITE)?;
    m.0.require(
        Domain::Projets,
        Level::Lecture,
        "Vous n'avez pas accès aux projets.",
    )?;
    let c = st
        .store
        .container(&cid)
        .await?
        .ok_or(ApiError::NotFound("Contenant"))?;
    let data = snapshot(&st.store, &c).await?;
    let taken: Vec<String> = st
        .store
        .templates()
        .await?
        .into_iter()
        .map(|t| t.name)
        .collect();
    let base = b
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(&c.name)
        .chars()
        .take(120)
        .collect::<String>();
    let name = unique_name(&base, &taken);
    let t = st
        .store
        .insert_template(
            &name,
            &serde_json::to_value(&data).unwrap_or_default(),
            Some(&m.0.account.id),
        )
        .await?;
    st.store
        .audit(
            Some(&m.0.account.id),
            "create",
            "container_template",
            &t.id,
            None::<&()>,
            Some(&json!({ "name": t.name, "from": cid })),
        )
        .await?;
    let items = items_of(&st.store, &[&data]).await?;
    Ok((StatusCode::CREATED, Json(view(&t, &data, &items))))
}

#[derive(Deserialize)]
struct Patch {
    rev: i64,
    name: Option<String>,
    status: Option<TemplateStatus>,
    /// Caractéristiques du contenant (hauteur, poids maximum…), sans toucher au contenu.
    container: Option<ContainerData>,
}

async fn update(
    State(st): State<AppState>,
    m: Member,
    Path(id): Path<String>,
    Json(b): Json<Patch>,
) -> ApiResult<Json<Value>> {
    m.0.require(Domain::Catalogue, Level::Ecriture, LIB_WRITE)?;
    let t = st
        .store
        .template(&id)
        .await?
        .ok_or(ApiError::NotFound("Modèle"))?;
    let mut c = content(&t)?;
    let mut name = b.name.unwrap_or(t.name.clone());
    if let Some(mut d) = b.container {
        fields(validate_container(&mut name, &mut d))?;
        c.container = d;
    } else {
        name = name.trim().to_string();
        if name.is_empty() || name.chars().count() > 120 {
            return Err(one("name", "Le nom compte de 1 à 120 caractères."));
        }
    }
    let after = st
        .store
        .update_template(
            &id,
            b.rev,
            &name,
            b.status.unwrap_or(t.status),
            &serde_json::to_value(&c).unwrap_or_default(),
        )
        .await
        .map_err(|e| match e {
            StoreError::Conflict(_) => one("name", "Un modèle porte déjà ce nom."),
            e => e.into(),
        })?;
    st.store
        .audit(
            Some(&m.0.account.id),
            "update",
            "container_template",
            &id,
            Some(&json!({ "name": t.name, "status": t.status })),
            Some(&json!({ "name": after.name, "status": after.status })),
        )
        .await?;
    let items = items_of(&st.store, &[&c]).await?;
    Ok(Json(view(&after, &c, &items)))
}

async fn remove(
    State(st): State<AppState>,
    m: Member,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    m.0.require(Domain::Catalogue, Level::Ecriture, LIB_WRITE)?;
    let t = st
        .store
        .template(&id)
        .await?
        .ok_or(ApiError::NotFound("Modèle"))?;
    st.store.delete_template(&id).await?;
    st.store
        .audit(
            Some(&m.0.account.id),
            "delete",
            "container_template",
            &id,
            Some(&json!({ "name": t.name })),
            None::<&()>,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct FromTemplate {
    template_id: String,
    location_id: String,
    name: Option<String>,
}

/// Recopie un modèle actif dans un projet : contenant, articles et leurs exemplaires,
/// circuits, placements. Les fiches disparues du catalogue sont ignorées et signalées.
/// Recopie un modèle de contenant dans un projet : contenant, articles, circuits et
/// placements. Renvoie le contenant et le nombre de fiches disparues du catalogue.
pub(crate) async fn instantiate(
    store: &Store,
    pid: &str,
    lid: &str,
    t: &ContainerTemplate,
    name: Option<&str>,
) -> ApiResult<(Container, usize)> {
    let c = content(t)?;
    let mut name = name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(&t.name)
        .to_string();
    let mut data = c.container.clone();
    let errs: Vec<FieldError> = validate_container(&mut name, &mut data);
    fields(errs)?;
    let taken: Vec<String> = store
        .inventory(pid)
        .await?
        .containers
        .into_iter()
        .map(|x| x.name)
        .collect();
    let name = unique_name(&name, &taken);
    let container: Container = store.insert_container(pid, lid, &name, &data).await?;
    // Articles : une fiche disparue du catalogue est ignorée.
    let mut skipped = 0;
    let mut art_ids: HashMap<usize, String> = HashMap::new();
    for a in &c.articles {
        if let Some(item) = &a.item_id {
            if store.item(item).await?.is_none() {
                skipped += 1;
                continue;
            }
        }
        let (made, _) = store
            .add_article(
                pid,
                lid,
                Some(&container.id),
                a.nature,
                a.item_id.as_deref(),
                a.quantity,
                a.accessory.as_ref(),
                false,
            )
            .await?;
        art_ids.insert(a.key, made.id);
    }
    let mut circ_ids: HashMap<usize, String> = HashMap::new();
    for x in &c.circuits {
        circ_ids.insert(
            x.key,
            store
                .insert_circuit(pid, &container.id, &x.name, x.phase, x.rating_a)
                .await?
                .id,
        );
    }
    let mut placed: Vec<Option<String>> = Vec::new();
    for p in &c.placements {
        let Some(aid) = art_ids.get(&p.article) else {
            placed.push(None);
            continue;
        };
        let instance = match p.nth {
            Some(n) => store
                .article_instances(aid)
                .await?
                .get(n)
                .map(|i| i.id.clone()),
            None => None,
        };
        let spec = PlaceSpec {
            face: p.face,
            u_start: p.u_start,
            width: p.width,
            col: p.col,
            circuit_id: p.circuit.and_then(|k| circ_ids.get(&k).cloned()),
            shelf_id: p.shelf.and_then(|s| placed.get(s).cloned().flatten()),
        };
        placed.push(
            store
                .insert_placement(pid, &container.id, aid, instance.as_deref(), &spec)
                .await
                .ok()
                .map(|x| x.id),
        );
    }
    Ok((container, skipped))
}

async fn from_template(
    State(st): State<AppState>,
    m: Member,
    origin: Origin,
    Path(pid): Path<String>,
    Json(b): Json<FromTemplate>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    m.0.require(
        Domain::Projets,
        Level::Ecriture,
        "Vous n'avez pas le droit de modifier les projets.",
    )?;
    st.store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    let l = st
        .store
        .location(&b.location_id)
        .await?
        .filter(|l| l.project_id == pid)
        .ok_or_else(|| one("location_id", "Choisissez un emplacement du projet."))?;
    let t = st
        .store
        .template(&b.template_id)
        .await?
        .ok_or(ApiError::NotFound("Modèle"))?;
    if t.status != TemplateStatus::Active {
        return Err(ApiError::Conflict(
            "Seuls les modèles actifs se copient dans un projet.".into(),
        ));
    }
    let (container, skipped) = instantiate(&st.store, &pid, &l.id, &t, b.name.as_deref()).await?;
    st.store
        .audit_project(
            &pid,
            Some(&m.0.account.id),
            "create",
            "container",
            &container.id,
            None::<&()>,
            Some(&json!({ "name": container.name, "template": t.name })),
        )
        .await?;
    st.hub.change(
        room(&pid),
        "container",
        &container.id,
        "create",
        Some(container.rev),
        &m.0.account.id,
        origin.0.as_deref(),
    );
    Ok((
        StatusCode::CREATED,
        Json(json!({ "container": container, "skipped_items": skipped })),
    ))
}
