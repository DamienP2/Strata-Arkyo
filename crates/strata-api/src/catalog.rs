//! Catalogue (cahier des charges, section 5) : référentiels, fiches, doublons, compatibilité.
//!
//! Lire demande « Catalogue : lecture ». Écrire une fiche demande « Catalogue : écriture ».
//! Modifier un référentiel demande « Paramètres et site : écriture », car il change
//! les listes de tout le site.

use crate::auth::Member;
use crate::error::{ApiError, ApiResult};
use crate::{AppState, catalog_io};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use strata_domain::catalog::*;
use strata_domain::text::normalize;
use strata_domain::{Domain, Level};
use strata_store::{CatalogItem, ItemFilter, NewRef, RefItem, Store};

pub const CATALOG_READ: &str = "Vous n'avez pas accès au catalogue.";
pub const CATALOG_WRITE: &str = "Vous n'avez pas le droit de modifier le catalogue.";
pub const CATALOG_ADMIN: &str = "Seul un administrateur du catalogue peut faire cela.";
pub const REFS_WRITE: &str =
    "Modifier les référentiels demande le droit « Paramètres et site : écriture ».";
/// Nombre maximal de fiches supprimées d'un coup.
pub const BULK_MAX: usize = 500;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/refs", get(list_refs))
        .route("/refs/dimensions/restore", post(restore_dimensions))
        .route("/refs/{kind}", get(list_refs_of).post(create_ref))
        .route("/refs/{kind}/{id}", patch(update_ref).delete(delete_ref))
        .route("/items", get(list_items).post(create_item))
        .route("/items/delete", post(delete_items))
        .route("/items/{id}", get(get_item).put(update_item))
        .route("/items/{id}/duplicate", post(duplicate_item))
        .route("/duplicates", post(find_duplicates))
        .route("/compat", get(compat))
        .merge(catalog_io::routes())
}

// ----- Index et dépendances -----

/// Vue d'ensemble du catalogue chargée pour valider et compter les usages.
pub struct Snapshot {
    pub refs: Vec<RefItem>,
    pub items: Vec<CatalogItem>,
    pub index: RefIndex,
}

impl Snapshot {
    pub async fn load(store: &Store) -> ApiResult<Snapshot> {
        let refs = store.list_refs(None).await?;
        let items = store.all_items(None).await?;
        let index = RefIndex::build(
            refs.iter()
                .map(|r| (r.kind, r.id.as_str(), r.parent_id.as_deref(), &r.data)),
            items.iter().map(|i| (i.kind, i.id.as_str(), &i.data)),
        );
        Ok(Snapshot { refs, items, index })
    }

    /// Qui utilise quoi : cible → identifiants des fiches et référentiels qui en dépendent.
    pub fn users(&self) -> HashMap<Target, Vec<Target>> {
        let mut out: HashMap<Target, Vec<Target>> = HashMap::new();
        for i in &self.items {
            for t in item_dependencies(i.kind, &i.data) {
                out.entry(t).or_default().push(Target::Item(i.id.clone()));
            }
        }
        for r in &self.refs {
            for t in ref_dependencies(r.kind, r.parent_id.as_deref(), &r.data) {
                out.entry(t)
                    .or_default()
                    .push(Target::Ref(r.kind, r.id.clone()));
            }
        }
        out
    }

    pub fn ref_name(&self, kind: RefKind, id: &str) -> Option<&str> {
        self.refs
            .iter()
            .find(|r| r.kind == kind && r.id == id)
            .map(|r| r.name.as_str())
    }
}

fn fields(errors: Vec<FieldError>) -> ApiError {
    ApiError::Fields(errors)
}

fn one_field(field: &str, message: impl Into<String>) -> ApiError {
    fields(vec![FieldError {
        field: field.into(),
        message: message.into(),
    }])
}

fn read<T: serde::de::DeserializeOwned>(data: &Value) -> ApiResult<T> {
    serde_json::from_value(data.clone())
        .map_err(|e| one_field("data", format!("Données illisibles : {e}.")))
}

/// Vérifie une fiche et la remet en forme canonique.
pub fn check_item(
    kind: ItemKind,
    data: &Value,
    snap: &Snapshot,
    self_id: Option<&str>,
) -> ApiResult<Value> {
    let (errors, canonical) = match kind {
        ItemKind::Equipment => {
            let mut x: Equipment = read(data)?;
            x.name = x.name.trim().to_string();
            if let (
                Some(id),
                Some(Modular::Host {
                    default_composition,
                    ..
                }),
            ) = (self_id, &x.modular)
            {
                if default_composition.iter().any(|s| s.equipment_id == id) {
                    return Err(one_field(
                        "modular",
                        "Un équipement ne peut pas se contenir lui-même.",
                    ));
                }
            }
            (validate_equipment(&x, &snap.index), serde_json::to_value(x))
        }
        ItemKind::Cable => {
            let mut x: Cable = read(data)?;
            x.name = x.name.trim().to_string();
            if x.name.is_empty() {
                let n = |id: &str| {
                    snap.ref_name(RefKind::Connector, id)
                        .unwrap_or(id)
                        .to_string()
                };
                x.name = proposed_cable_name(
                    &n(&x.left.connector_id),
                    x.left.gender,
                    &n(&x.right.connector_id),
                    x.right.gender,
                );
            }
            (validate_cable(&x, &snap.index), serde_json::to_value(x))
        }
        ItemKind::CableModel => {
            let mut x: CableModel = read(data)?;
            x.name = x.name.trim().to_string();
            (
                validate_cable_model(&x, &snap.index),
                serde_json::to_value(x),
            )
        }
        ItemKind::Adapter => {
            let mut x: Adapter = read(data)?;
            x.name = x.name.trim().to_string();
            (validate_adapter(&x, &snap.index), serde_json::to_value(x))
        }
    };
    if !errors.is_empty() {
        return Err(fields(errors));
    }
    canonical.map_err(|e| ApiError::Internal(e.to_string()))
}

// ----- Doublons -----

#[derive(Debug, Clone, Serialize)]
pub struct Duplicate {
    pub id: String,
    pub name: String,
    pub reference: Option<String>,
    pub reason: &'static str,
}

/// Fiches qui ressemblent à `data` : même référence, même nom et même marque, ou nom très proche.
pub fn duplicates_of(
    kind: ItemKind,
    data: &Value,
    items: &[CatalogItem],
    except: Option<&str>,
) -> Vec<Duplicate> {
    let (name, reference, brand) = item_summary(data);
    let name = normalize(&name);
    let reference = reference.as_deref().map(normalize);
    let mut out = Vec::new();
    for i in items
        .iter()
        .filter(|i| i.kind == kind && Some(i.id.as_str()) != except)
    {
        let other = normalize(&i.name);
        let reason = if reference.is_some() && i.reference.as_deref().map(normalize) == reference {
            Some("Même référence")
        } else if !name.is_empty() && other == name && i.brand_id == brand {
            Some("Même nom et même marque")
        } else if name.chars().count() >= 6
            && strsim::jaro_winkler(&name, &other) >= 0.94
            && i.brand_id == brand
        {
            Some("Nom très proche")
        } else {
            None
        };
        if let Some(reason) = reason {
            out.push(Duplicate {
                id: i.id.clone(),
                name: i.name.clone(),
                reference: i.reference.clone(),
                reason,
            });
        }
    }
    out.truncate(10);
    out
}

// ----- Référentiels -----

#[derive(Serialize)]
struct RefView {
    #[serde(flatten)]
    item: RefItem,
    /// Nombre de fiches et de référentiels qui l'utilisent.
    uses: usize,
}

fn ref_views(snap: &Snapshot, kind: Option<RefKind>) -> Vec<RefView> {
    let users = snap.users();
    snap.refs
        .iter()
        .filter(|r| kind.is_none_or(|k| r.kind == k))
        .map(|r| RefView {
            uses: users
                .get(&Target::Ref(r.kind, r.id.clone()))
                .map_or(0, Vec::len),
            item: r.clone(),
        })
        .collect()
}

fn ref_kind(path: &str) -> ApiResult<RefKind> {
    RefKind::from_path(path).ok_or(ApiError::NotFound("Référentiel"))
}

async fn list_refs(State(st): State<AppState>, Member(s): Member) -> ApiResult<Json<Value>> {
    s.require(Domain::Catalogue, Level::Lecture, CATALOG_READ)?;
    let snap = Snapshot::load(&st.store).await?;
    let mut out = serde_json::Map::new();
    for k in RefKind::ALL {
        out.insert(k.path().into(), json!(ref_views(&snap, Some(k))));
    }
    Ok(Json(Value::Object(out)))
}

async fn list_refs_of(
    State(st): State<AppState>,
    Member(s): Member,
    Path(kind): Path<String>,
) -> ApiResult<Json<Value>> {
    s.require(Domain::Catalogue, Level::Lecture, CATALOG_READ)?;
    let kind = ref_kind(&kind)?;
    let snap = Snapshot::load(&st.store).await?;
    Ok(Json(json!({ "items": ref_views(&snap, Some(kind)) })))
}

#[derive(Deserialize)]
struct RefBody {
    #[serde(default)]
    name: String,
    #[serde(default)]
    parent_id: Option<String>,
    #[serde(default)]
    data: Value,
    #[serde(default)]
    active: Option<bool>,
}

/// Le nom (ou, pour un préréglage, la valeur) est libre parmi ceux du même genre.
fn check_unique(
    snap: &Snapshot,
    kind: RefKind,
    name: &str,
    parent: Option<&str>,
    data: &Value,
    except: Option<&str>,
) -> ApiResult<()> {
    let n = normalize(name);
    let same = snap
        .refs
        .iter()
        .filter(|r| r.kind == kind && Some(r.id.as_str()) != except)
        .any(|r| {
            if kind.named() {
                normalize(&r.name) == n
                    && (!kind.unique_among_siblings() || r.parent_id.as_deref() == parent)
            } else {
                r.data.get("kind") == data.get("kind")
                    && r.data.get("value").and_then(Value::as_f64)
                        == data.get("value").and_then(Value::as_f64)
            }
        });
    if same {
        let m = if kind.named() {
            format!("{} « {} » existe déjà.", kind.label(), name.trim())
        } else {
            "Ce préréglage existe déjà.".into()
        };
        return Err(ApiError::Conflict(m));
    }
    Ok(())
}

/// Une catégorie ne peut pas descendre d'elle-même.
fn check_no_cycle(snap: &Snapshot, kind: RefKind, id: &str, parent: Option<&str>) -> ApiResult<()> {
    let mut seen = HashSet::new();
    let mut cur = parent.map(str::to_string);
    while let Some(p) = cur {
        if p == id || !seen.insert(p.clone()) {
            return Err(one_field("parent_id", "Ce parent créerait une boucle."));
        }
        cur = snap
            .refs
            .iter()
            .find(|r| r.kind == kind && r.id == p)
            .and_then(|r| r.parent_id.clone());
    }
    Ok(())
}

/// Nom affiché d'un préréglage : la valeur et son unité.
fn dimension_label(data: &Value) -> String {
    let v = data
        .get("value")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let num = format!("{v:.3}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .replace('.', ",");
    match data.get("kind").and_then(Value::as_str).unwrap_or_default() {
        "height_u" => format!("{num} U"),
        "width" => match num.as_str() {
            "1" => "pleine".into(),
            "0,5" => "1/2".into(),
            "0,333" => "1/3".into(),
            "0,25" => "1/4".into(),
            _ => num,
        },
        "cable_length_m" => format!("{num} m"),
        _ => format!("{num} mm"),
    }
}

async fn create_ref(
    State(st): State<AppState>,
    Member(s): Member,
    Path(kind): Path<String>,
    Json(b): Json<RefBody>,
) -> ApiResult<(StatusCode, Json<RefItem>)> {
    s.require(Domain::Parametres, Level::Ecriture, REFS_WRITE)?;
    let kind = ref_kind(&kind)?;
    let snap = Snapshot::load(&st.store).await?;
    let data = validate_ref(kind, &b.name, &b.data, b.parent_id.as_deref(), &snap.index)
        .map_err(fields)?;
    let name = if kind.named() {
        b.name.trim().to_string()
    } else {
        dimension_label(&data)
    };
    check_unique(&snap, kind, &name, b.parent_id.as_deref(), &data, None)?;
    // Une fonction a pour identifiant sa clé : les fiches et les numéros de câble la citent.
    let id = match kind {
        RefKind::Function => {
            let key = data["key"].as_str().unwrap_or_default().to_string();
            if snap.index.functions.contains(&key) {
                return Err(ApiError::Conflict(format!("La clé {key} est déjà prise.")));
            }
            Some(key)
        }
        _ => None,
    };
    let parent_id = if matches!(kind, RefKind::Category | RefKind::EquipmentType) {
        b.parent_id
    } else {
        None
    };
    let r = st
        .store
        .insert_ref(NewRef {
            kind,
            id,
            name,
            parent_id,
            data,
            active: b.active.unwrap_or(true),
            position: None,
            builtin: false,
            protected: false,
        })
        .await?;
    st.store
        .audit(
            Some(&s.account.id),
            "create",
            &format!("ref:{}", kind.path()),
            &r.id,
            None::<&()>,
            Some(&r),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(r)))
}

#[derive(Deserialize)]
struct RefPatch {
    rev: i64,
    name: Option<String>,
    /// Absent : inchangé ; `null` : sans parent.
    #[serde(default)]
    parent_id: Option<Value>,
    data: Option<Value>,
    active: Option<bool>,
    position: Option<i64>,
}

async fn update_ref(
    State(st): State<AppState>,
    Member(s): Member,
    Path((kind, id)): Path<(String, String)>,
    Json(b): Json<RefPatch>,
) -> ApiResult<Json<RefItem>> {
    s.require(Domain::Parametres, Level::Ecriture, REFS_WRITE)?;
    let kind = ref_kind(&kind)?;
    let snap = Snapshot::load(&st.store).await?;
    let before = snap
        .refs
        .iter()
        .find(|r| r.kind == kind && r.id == id)
        .cloned()
        .ok_or(ApiError::NotFound("Élément du référentiel"))?;
    let only_order = b.name.is_none() && b.parent_id.is_none() && b.data.is_none();
    if before.protected && !only_order {
        return Err(ApiError::Forbidden(format!(
            "{} est protégée : seul son ordre change.",
            before.name
        )));
    }
    let parent = match &b.parent_id {
        None => before.parent_id.clone(),
        Some(Value::String(p)) => Some(p.clone()),
        Some(_) => None,
    };
    let mut data = b.data.unwrap_or_else(|| before.data.clone());
    if kind == RefKind::Function {
        // La clé ne change pas : elle sert d'identifiant.
        data["key"] = json!(before.id);
    }
    let name = b.name.unwrap_or_else(|| before.name.clone());
    let data = if before.protected {
        before.data.clone()
    } else {
        validate_ref(kind, &name, &data, parent.as_deref(), &snap.index).map_err(fields)?
    };
    let name = if kind.named() {
        name.trim().to_string()
    } else {
        dimension_label(&data)
    };
    if !before.protected {
        check_unique(&snap, kind, &name, parent.as_deref(), &data, Some(&id))?;
        check_no_cycle(&snap, kind, &id, parent.as_deref())?;
    }
    if kind == RefKind::EquipmentType
        && parent.is_some()
        && snap
            .refs
            .iter()
            .any(|r| r.kind == kind && r.parent_id.as_deref() == Some(&id))
    {
        return Err(one_field(
            "parent_id",
            "Ce type a des sous-types : il reste au premier niveau.",
        ));
    }
    let after = st
        .store
        .update_ref(
            kind,
            &id,
            b.rev,
            &name,
            parent.as_deref(),
            &data,
            b.active.unwrap_or(before.active),
            b.position.unwrap_or(before.position),
        )
        .await?;
    st.store
        .audit(
            Some(&s.account.id),
            "update",
            &format!("ref:{}", kind.path()),
            &id,
            Some(&before),
            Some(&after),
        )
        .await?;
    Ok(Json(after))
}

async fn delete_ref(
    State(st): State<AppState>,
    Member(s): Member,
    Path((kind, id)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    s.require(Domain::Parametres, Level::Ecriture, REFS_WRITE)?;
    let kind = ref_kind(&kind)?;
    let snap = Snapshot::load(&st.store).await?;
    let before = snap
        .refs
        .iter()
        .find(|r| r.kind == kind && r.id == id)
        .cloned()
        .ok_or(ApiError::NotFound("Élément du référentiel"))?;
    if before.protected {
        return Err(ApiError::Forbidden(format!(
            "{} est protégée.",
            before.name
        )));
    }
    let users = snap.users();
    if let Some(u) = users.get(&Target::Ref(kind, id.clone())) {
        return Err(ApiError::Conflict(format!(
            "{} est utilisé par {} élément(s) du catalogue : désactivez-le plutôt.",
            before.name,
            u.len()
        )));
    }
    st.store.delete_ref(kind, &id).await?;
    st.store
        .audit(
            Some(&s.account.id),
            "delete",
            &format!("ref:{}", kind.path()),
            &id,
            Some(&before),
            None::<&()>,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn restore_dimensions(
    State(st): State<AppState>,
    Member(s): Member,
) -> ApiResult<Json<Value>> {
    s.require(Domain::Parametres, Level::Ecriture, REFS_WRITE)?;
    let restored = st.store.restore_dimension_defaults().await?;
    st.store
        .audit(
            Some(&s.account.id),
            "restore",
            "ref:dimensions",
            "defaults",
            None::<&()>,
            Some(&json!({ "restored": restored })),
        )
        .await?;
    Ok(Json(json!({ "restored": restored })))
}

// ----- Fiches -----

fn item_kind(path: &str) -> ApiResult<ItemKind> {
    ItemKind::from_path(path).ok_or_else(|| {
        one_field(
            "kind",
            "Genre de fiche inconnu (equipment, cables, cable-models, adapters).",
        )
    })
}

#[derive(Deserialize)]
struct ItemsQuery {
    kind: Option<String>,
    q: Option<String>,
    active: Option<bool>,
    brand_id: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn list_items(
    State(st): State<AppState>,
    Member(s): Member,
    Query(q): Query<ItemsQuery>,
) -> ApiResult<Json<Value>> {
    s.require(Domain::Catalogue, Level::Lecture, CATALOG_READ)?;
    let kind = q.kind.as_deref().map(item_kind).transpose()?;
    let f = ItemFilter {
        kind,
        q: q.q,
        active: q.active,
        brand_id: q.brand_id,
        limit: q.limit.unwrap_or(200),
        offset: q.offset.unwrap_or(0),
    };
    let (items, total) = st.store.list_items(&f).await?;
    Ok(Json(json!({ "items": items, "total": total })))
}

async fn get_item(
    State(st): State<AppState>,
    Member(s): Member,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    s.require(Domain::Catalogue, Level::Lecture, CATALOG_READ)?;
    let snap = Snapshot::load(&st.store).await?;
    let item = snap
        .items
        .iter()
        .find(|i| i.id == id)
        .ok_or(ApiError::NotFound("Fiche"))?;
    let used_by = used_by(&snap, &id);
    Ok(Json(json!({ "item": item, "used_by": used_by })))
}

fn used_by(snap: &Snapshot, id: &str) -> Vec<Value> {
    snap.users()
        .remove(&Target::Item(id.to_string()))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|t| match t {
            Target::Item(i) => snap
                .items
                .iter()
                .find(|x| x.id == i)
                .map(|x| json!({ "id": x.id, "kind": x.kind, "name": x.name })),
            Target::Ref(..) => None,
        })
        .collect()
}

#[derive(Deserialize)]
struct NewItemBody {
    kind: String,
    data: Value,
}

async fn create_item(
    State(st): State<AppState>,
    Member(s): Member,
    Json(b): Json<NewItemBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    s.require(Domain::Catalogue, Level::Ecriture, CATALOG_WRITE)?;
    let kind = item_kind(&b.kind)?;
    let snap = Snapshot::load(&st.store).await?;
    let data = check_item(kind, &b.data, &snap, None)?;
    let duplicates = duplicates_of(kind, &data, &snap.items, None);
    let item = st
        .store
        .insert_item(kind, &data, Some(&s.account.id))
        .await?;
    st.store
        .audit(
            Some(&s.account.id),
            "create",
            "catalog_item",
            &item.id,
            None::<&()>,
            Some(&item),
        )
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "item": item, "duplicates": duplicates })),
    ))
}

#[derive(Deserialize)]
struct UpdateItemBody {
    rev: i64,
    data: Value,
}

async fn update_item(
    State(st): State<AppState>,
    Member(s): Member,
    Path(id): Path<String>,
    Json(b): Json<UpdateItemBody>,
) -> ApiResult<Json<Value>> {
    s.require(Domain::Catalogue, Level::Ecriture, CATALOG_WRITE)?;
    let snap = Snapshot::load(&st.store).await?;
    let before = snap
        .items
        .iter()
        .find(|i| i.id == id)
        .cloned()
        .ok_or(ApiError::NotFound("Fiche"))?;
    let data = check_item(before.kind, &b.data, &snap, Some(&id))?;
    if before.kind == ItemKind::Equipment {
        check_component_change(&snap, &before, &data)?;
    }
    let after = st
        .store
        .update_item(&id, b.rev, &data, Some(&s.account.id))
        .await?;
    st.store
        .audit(
            Some(&s.account.id),
            "update",
            "catalog_item",
            &id,
            Some(&before),
            Some(&after),
        )
        .await?;
    let duplicates = duplicates_of(after.kind, &after.data, &snap.items, Some(&id));
    Ok(Json(json!({ "item": after, "duplicates": duplicates })))
}

/// Un composant placé dans un châssis par défaut garde son rôle et sa taille.
fn check_component_change(snap: &Snapshot, before: &CatalogItem, data: &Value) -> ApiResult<()> {
    let hosts = used_by(snap, &before.id);
    if hosts.is_empty() {
        return Ok(());
    }
    if data.get("modular") != before.data.get("modular") {
        return Err(one_field(
            "modular",
            format!(
                "Cette carte est placée par défaut dans {} châssis : son rôle et sa taille ne changent pas.",
                hosts.len()
            ),
        ));
    }
    Ok(())
}

async fn duplicate_item(
    State(st): State<AppState>,
    Member(s): Member,
    Path(id): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    s.require(Domain::Catalogue, Level::Ecriture, CATALOG_WRITE)?;
    let snap = Snapshot::load(&st.store).await?;
    let source = snap
        .items
        .iter()
        .find(|i| i.id == id)
        .ok_or(ApiError::NotFound("Fiche"))?;
    let mut data = source.data.clone();
    let taken: HashSet<String> = snap
        .items
        .iter()
        .filter(|i| i.kind == source.kind)
        .map(|i| normalize(&i.name))
        .collect();
    let base = source.name.clone();
    let name = (1..)
        .map(|n| {
            if n == 1 {
                format!("{base} (copie)")
            } else {
                format!("{base} (copie {n})")
            }
        })
        .find(|n| !taken.contains(&normalize(n)))
        .unwrap_or_default();
    data["name"] = json!(name);
    let data = check_item(source.kind, &data, &snap, None)?;
    let item = st
        .store
        .insert_item(source.kind, &data, Some(&s.account.id))
        .await?;
    st.store
        .audit(
            Some(&s.account.id),
            "duplicate",
            "catalog_item",
            &item.id,
            Some(&json!({ "from": id })),
            Some(&item),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(json!({ "item": item }))))
}

#[derive(Deserialize)]
struct DeleteBody {
    ids: Vec<String>,
}

/// Suppression groupée (1 à 500 fiches). Une fiche utilisée par une autre est épargnée.
async fn delete_items(
    State(st): State<AppState>,
    Member(s): Member,
    Json(b): Json<DeleteBody>,
) -> ApiResult<Json<Value>> {
    s.require(Domain::Catalogue, Level::Ecriture, CATALOG_WRITE)?;
    let ids: Vec<String> = {
        let mut seen = HashSet::new();
        b.ids
            .into_iter()
            .filter(|i| seen.insert(i.clone()))
            .collect()
    };
    if ids.is_empty() || ids.len() > BULK_MAX {
        return Err(ApiError::Invalid(format!(
            "Choisissez de 1 à {BULK_MAX} fiches."
        )));
    }
    let snap = Snapshot::load(&st.store).await?;
    let chosen: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let mut deleted = Vec::new();
    let mut kept = Vec::new();
    let mut missing = 0;
    for id in &ids {
        let Some(item) = snap.items.iter().find(|i| &i.id == id) else {
            missing += 1;
            continue;
        };
        // Les fiches qui l'utilisent et qui partent aussi ne la retiennent pas.
        let blockers: Vec<Value> = used_by(&snap, id)
            .into_iter()
            .filter(|u| !chosen.contains(u["id"].as_str().unwrap_or_default()))
            .collect();
        if blockers.is_empty() {
            deleted.push(item.clone());
        } else {
            kept.push(json!({ "id": id, "name": item.name, "used_by": blockers }));
        }
    }
    let delete_ids: Vec<String> = deleted.iter().map(|i| i.id.clone()).collect();
    st.store.delete_items(&delete_ids).await?;
    for item in &deleted {
        st.store
            .audit(
                Some(&s.account.id),
                "delete",
                "catalog_item",
                &item.id,
                Some(item),
                None::<&()>,
            )
            .await?;
    }
    Ok(Json(
        json!({ "deleted": delete_ids.len(), "kept": kept, "missing": missing }),
    ))
}

#[derive(Deserialize)]
struct DuplicatesBody {
    kind: String,
    data: Value,
    except_id: Option<String>,
}

async fn find_duplicates(
    State(st): State<AppState>,
    Member(s): Member,
    Json(b): Json<DuplicatesBody>,
) -> ApiResult<Json<Value>> {
    s.require(Domain::Catalogue, Level::Lecture, CATALOG_READ)?;
    let kind = item_kind(&b.kind)?;
    let items = st.store.all_items(Some(kind)).await?;
    Ok(Json(
        json!({ "duplicates": duplicates_of(kind, &b.data, &items, b.except_id.as_deref()) }),
    ))
}

#[derive(Deserialize)]
struct CompatQuery {
    model_id: String,
    signal_id: String,
}

/// Le modèle de câble convient-il au signal ? (impédance, puis bande passante)
async fn compat(
    State(st): State<AppState>,
    Member(s): Member,
    Query(q): Query<CompatQuery>,
) -> ApiResult<Json<Value>> {
    s.require(Domain::Catalogue, Level::Lecture, CATALOG_READ)?;
    let model = st
        .store
        .item(&q.model_id)
        .await?
        .filter(|i| i.kind == ItemKind::CableModel)
        .ok_or(ApiError::NotFound("Modèle de câble"))?;
    let signal = st
        .store
        .ref_item(RefKind::Signal, &q.signal_id)
        .await?
        .ok_or(ApiError::NotFound("Signal"))?;
    let m: CableModel = read(&model.data)?;
    let sig: SignalData = read(&signal.data)?;
    let c = cable_compatibility(&m, &sig);
    let message = match c {
        Compatibility::Compatible => "Compatible.",
        Compatibility::Warning => {
            "Impédance correcte, mais la bande passante est inférieure à celle qu'exige le signal."
        }
        Compatibility::Incompatible => "Impédance différente de celle du signal : câble à éviter.",
        Compatibility::Unknown => "Il manque l'impédance ou la bande passante pour conclure.",
    };
    Ok(Json(json!({ "result": c, "message": message })))
}
