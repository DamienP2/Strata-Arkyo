//! Entrées et sorties du catalogue : import de tableurs (CSV, XLSX), export, échange
//! de catalogue entre sites et retour à un instantané.

use crate::auth::Member;
use crate::catalog::{
    CATALOG_ADMIN, CATALOG_READ, CATALOG_WRITE, REFS_WRITE, Snapshot, check_item, duplicates_of,
};
use crate::error::{ApiError, ApiResult};
use crate::{AppState, VERSION};
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Cursor;
use strata_domain::catalog::import::{
    CellIssue, ImportDraft, ImportField, match_columns, read_row,
};
use strata_domain::catalog::*;
use strata_domain::text::normalize;
use strata_domain::{Domain, Level};
use strata_store::{CatalogChanges, CatalogItem, RefItem, now};

/// Taille maximale d'un tableur déposé.
const MAX_SHEET: usize = 20 * 1024 * 1024;
/// Taille maximale d'un catalogue échangé.
const MAX_EXCHANGE: usize = 64 * 1024 * 1024;
const MAX_ROWS: usize = 5000;
const MAX_COLUMNS: usize = 100;
pub const EXCHANGE_FORMAT: &str = "strata-arkyo/catalogue";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/import/analyze",
            post(import_analyze).layer(DefaultBodyLimit::max(MAX_SHEET)),
        )
        .route("/import/{id}/values", post(import_values))
        .route("/import/{id}/run", post(import_run))
        .route("/import/{id}", delete(import_cancel))
        .route("/export", get(export))
        .route(
            "/exchange",
            get(exchange_export)
                .post(exchange_import)
                .layer(DefaultBodyLimit::max(MAX_EXCHANGE)),
        )
        .route("/snapshots", get(snapshots))
        .route("/snapshots/{id}/restore", post(snapshot_restore))
}

fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

pub(crate) fn file_response(bytes: Vec<u8>, content_type: &str, filename: &str) -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(Body::from(bytes))
        .unwrap_or_default()
}

pub(crate) fn today() -> String {
    chrono::Local::now().format("%Y%m%d").to_string()
}

// ----- Lecture des tableurs -----

/// Lit la première feuille non vide d'un classeur, ou un CSV (UTF-8 ou Windows-1252,
/// séparateur deviné). Renvoie les en-têtes et les lignes non vides.
pub fn read_table(filename: &str, bytes: &[u8]) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let ext = filename
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_lowercase();
    let mut rows = match ext.as_str() {
        "xlsx" | "xlsm" | "xls" | "ods" => read_workbook(bytes)?,
        "csv" | "txt" | "tsv" => read_csv(bytes)?,
        _ => {
            return Err(
                "Format non reconnu : déposez un fichier .csv, .xlsx, .xls ou .ods.".into(),
            );
        }
    };
    rows.retain(|r| r.iter().any(|c| !c.trim().is_empty()));
    if rows.is_empty() {
        return Err("Le fichier est vide.".into());
    }
    let headers: Vec<String> = rows
        .remove(0)
        .into_iter()
        .map(|h| h.trim().to_string())
        .collect();
    if headers.len() > MAX_COLUMNS {
        return Err(format!("Le fichier compte plus de {MAX_COLUMNS} colonnes."));
    }
    if rows.len() > MAX_ROWS {
        return Err(format!(
            "Le fichier compte plus de {MAX_ROWS} lignes : découpez-le."
        ));
    }
    let width = headers.len();
    for r in &mut rows {
        r.resize(width, String::new());
    }
    Ok((headers, rows))
}

fn read_workbook(bytes: &[u8]) -> Result<Vec<Vec<String>>, String> {
    use calamine::{Data, Reader, open_workbook_auto_from_rs};
    let mut wb = open_workbook_auto_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|e| format!("Classeur illisible : {e}."))?;
    for name in wb.sheet_names() {
        let range = wb
            .worksheet_range(&name)
            .map_err(|e| format!("Feuille « {name} » illisible : {e}."))?;
        if range.is_empty() {
            continue;
        }
        let cell = |c: &Data| match c {
            Data::Empty => String::new(),
            Data::Bool(b) => if *b { "oui" } else { "non" }.to_string(),
            Data::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", *f as i64),
            Data::Float(f) => f.to_string().replace('.', ","),
            other => other.to_string(),
        };
        return Ok(range.rows().map(|r| r.iter().map(cell).collect()).collect());
    }
    Ok(Vec::new())
}

fn read_csv(bytes: &[u8]) -> Result<Vec<Vec<String>>, String> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned(),
    };
    let first = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or_default();
    let delimiter = b";,\t|"
        .iter()
        .copied()
        .max_by_key(|d| first.bytes().filter(|b| b == d).count())
        .unwrap_or(b';');
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for r in reader.records() {
        let r = r.map_err(|e| format!("CSV illisible : {e}."))?;
        rows.push(r.iter().map(str::to_string).collect());
    }
    Ok(rows)
}

// ----- Valeurs inconnues -----

/// Référentiels que l'import rapproche par leur nom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Lookup {
    Brands,
    Categories,
    Types,
    Functions,
    Phases,
}

impl Lookup {
    fn kind(self) -> RefKind {
        match self {
            Lookup::Brands => RefKind::Brand,
            Lookup::Categories => RefKind::Category,
            Lookup::Types => RefKind::EquipmentType,
            Lookup::Functions => RefKind::Function,
            Lookup::Phases => RefKind::Phase,
        }
    }

    fn field(self) -> ImportField {
        match self {
            Lookup::Brands => ImportField::Brand,
            Lookup::Categories => ImportField::Category,
            Lookup::Types => ImportField::Type,
            Lookup::Functions => ImportField::Functions,
            Lookup::Phases => ImportField::Phases,
        }
    }

    fn can_create(self) -> bool {
        matches!(self, Lookup::Brands | Lookup::Categories)
    }

    fn values(self, d: &ImportDraft) -> Vec<String> {
        match self {
            Lookup::Brands => d.brand.iter().cloned().collect(),
            Lookup::Categories => d.category.iter().cloned().collect(),
            Lookup::Types => d.type_name.iter().cloned().collect(),
            Lookup::Functions => d.functions.clone(),
            Lookup::Phases => d.phases.clone(),
        }
    }

    const ALL: [Lookup; 5] = [
        Lookup::Brands,
        Lookup::Categories,
        Lookup::Types,
        Lookup::Functions,
        Lookup::Phases,
    ];
}

/// Trouve un référentiel par son nom (ou, pour une fonction, sa clé), sans accents ni casse.
fn find_ref<'a>(snap: &'a Snapshot, kind: RefKind, text: &str) -> Option<&'a RefItem> {
    let n = normalize(text);
    snap.refs
        .iter()
        .filter(|r| r.kind == kind)
        .find(|r| normalize(&r.name) == n || (kind == RefKind::Function && normalize(&r.id) == n))
}

fn suggest<'a>(snap: &'a Snapshot, kind: RefKind, text: &str) -> Option<&'a RefItem> {
    let n = normalize(text);
    snap.refs
        .iter()
        .filter(|r| r.kind == kind)
        .map(|r| (strsim::jaro_winkler(&normalize(&r.name), &n), r))
        .filter(|(s, _)| *s >= 0.85)
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, r)| r)
}

#[derive(Debug, Clone, Deserialize)]
struct ColumnChoice {
    column: usize,
    field: Option<ImportField>,
}

fn cells_of(row: &[String], mapping: &[ColumnChoice]) -> HashMap<ImportField, String> {
    let mut out = HashMap::new();
    for m in mapping {
        if let (Some(f), Some(v)) = (m.field, row.get(m.column)) {
            out.entry(f).or_insert_with(|| v.clone());
        }
    }
    out
}

fn unknown_values(snap: &Snapshot, rows: &[Vec<String>], mapping: &[ColumnChoice]) -> Value {
    let mut found: BTreeMap<Lookup, BTreeMap<String, (String, usize)>> = BTreeMap::new();
    for row in rows {
        let (draft, _) = read_row(&cells_of(row, mapping));
        for l in Lookup::ALL {
            for v in l.values(&draft) {
                if find_ref(snap, l.kind(), &v).is_none() {
                    let e = found
                        .entry(l)
                        .or_default()
                        .entry(normalize(&v))
                        .or_insert((v.clone(), 0));
                    e.1 += 1;
                }
            }
        }
    }
    let mut out = serde_json::Map::new();
    for l in Lookup::ALL {
        let list: Vec<Value> = found
            .get(&l)
            .map(|m| {
                m.values()
                    .map(|(v, count)| {
                        let s = suggest(snap, l.kind(), v).map(|r| json!({ "id": r.id, "name": r.name }));
                        json!({ "value": v, "rows": count, "suggestion": s, "can_create": l.can_create() })
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.insert(
            serde_json::to_value(l)
                .unwrap_or_default()
                .as_str()
                .unwrap_or_default()
                .to_string(),
            Value::Array(list),
        );
    }
    Value::Object(out)
}

// ----- Import : analyse -----

#[derive(Deserialize)]
struct AnalyzeQuery {
    filename: String,
}

async fn import_analyze(
    State(st): State<AppState>,
    Member(s): Member,
    Query(q): Query<AnalyzeQuery>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    s.require(Domain::Catalogue, Level::Ecriture, CATALOG_WRITE)?;
    let (headers, rows) = read_table(&q.filename, &body).map_err(ApiError::Invalid)?;
    let matches = match_columns(&headers, &rows);
    let session = st
        .store
        .insert_import_session(&s.account.id, &q.filename, &headers, &rows)
        .await?;
    let mapping: Vec<ColumnChoice> = matches
        .iter()
        .map(|m| ColumnChoice {
            column: m.column,
            field: m.field,
        })
        .collect();
    let snap = Snapshot::load(&st.store).await?;
    let fields: Vec<Value> = ImportField::ALL
        .iter()
        .map(|f| json!({ "field": f, "label": f.label() }))
        .collect();
    Ok(Json(json!({
        "session_id": session.id,
        "filename": session.filename,
        "expires_at": session.expires_at,
        "headers": headers,
        "row_count": rows.len(),
        "preview": rows.iter().take(20).collect::<Vec<_>>(),
        "matches": matches,
        "fields": fields,
        "unknown": unknown_values(&snap, &rows, &mapping),
    })))
}

#[derive(Deserialize)]
struct ValuesBody {
    mapping: Vec<ColumnChoice>,
}

/// Valeurs inconnues pour une correspondance de colonnes revue par la personne.
async fn import_values(
    State(st): State<AppState>,
    Member(s): Member,
    Path(id): Path<String>,
    Json(b): Json<ValuesBody>,
) -> ApiResult<Json<Value>> {
    s.require(Domain::Catalogue, Level::Ecriture, CATALOG_WRITE)?;
    let session = st
        .store
        .import_session(&id, &s.account.id)
        .await?
        .ok_or(ApiError::NotFound("Import"))?;
    let snap = Snapshot::load(&st.store).await?;
    Ok(Json(
        json!({ "unknown": unknown_values(&snap, &session.rows, &b.mapping) }),
    ))
}

async fn import_cancel(
    State(st): State<AppState>,
    Member(s): Member,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    s.require(Domain::Catalogue, Level::Ecriture, CATALOG_WRITE)?;
    st.store
        .import_session(&id, &s.account.id)
        .await?
        .ok_or(ApiError::NotFound("Import"))?;
    st.store.delete_import_session(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ----- Import : exécution -----

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum Resolution {
    Map { id: String },
    Create,
    Ignore,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DuplicateStrategy {
    /// Garde la fiche existante.
    #[default]
    Skip,
    /// Met à jour la fiche existante avec les colonnes importées.
    Update,
    /// Crée une nouvelle fiche malgré tout.
    Create,
}

#[derive(Deserialize)]
struct RunBody {
    mapping: Vec<ColumnChoice>,
    #[serde(default)]
    values: HashMap<Lookup, HashMap<String, Resolution>>,
    #[serde(default)]
    duplicates: DuplicateStrategy,
    /// Choix ligne par ligne (numéro de ligne de données, à partir de 1).
    #[serde(default)]
    rows: HashMap<String, DuplicateStrategy>,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Serialize)]
struct RowError {
    row: usize,
    name: String,
    messages: Vec<String>,
}

#[derive(Serialize)]
struct RowIssue {
    row: usize,
    #[serde(flatten)]
    issue: CellIssue,
}

/// Applique les colonnes importées à un équipement (nouveau ou existant).
fn apply_draft(x: &mut Equipment, d: &ImportDraft, ids: &ResolvedIds) {
    if let Some(n) = &d.name {
        x.name = n.clone();
    }
    if d.reference.is_some() {
        x.reference = d.reference.clone();
    }
    if ids.brand.is_some() {
        x.brand_id = ids.brand.clone();
    }
    if ids.category.is_some() {
        x.category_id = ids.category.clone();
    }
    if ids.type_id.is_some() {
        x.type_id = ids.type_id.clone();
    }
    for f in &ids.functions {
        if !x.functions.contains(f) {
            x.functions.push(f.clone());
        }
    }
    for p in &ids.phases {
        if !x.phases.contains(p) {
            x.phases.push(p.clone());
        }
    }
    for t in &d.tags {
        if !x.tags.contains(t) {
            x.tags.push(t.clone());
        }
    }
    if d.notes.is_some() {
        x.notes = d.notes.clone();
    }
    let rack = d.height_u.is_some()
        || d.width.is_some()
        || d.depth_mm.is_some()
        || d.rear_reserve_mm.is_some();
    let free = d.length_mm.is_some() || d.width_mm.is_some() || d.height_mm.is_some();
    match (&mut x.format, rack, free) {
        (
            Format::Rack {
                height_u,
                width,
                depth_mm,
                rear_reserve_mm,
            },
            _,
            false,
        )
        | (
            Format::Rack {
                height_u,
                width,
                depth_mm,
                rear_reserve_mm,
            },
            true,
            true,
        ) => {
            *height_u = d.height_u.unwrap_or(*height_u);
            *width = d.width.unwrap_or(*width);
            *depth_mm = d.depth_mm.unwrap_or(*depth_mm);
            *rear_reserve_mm = d.rear_reserve_mm.or(*rear_reserve_mm);
        }
        (
            Format::Free {
                length_mm,
                width_mm,
                height_mm,
            },
            false,
            _,
        ) => {
            *length_mm = d.length_mm.unwrap_or(*length_mm);
            *width_mm = d.width_mm.unwrap_or(*width_mm);
            *height_mm = d.height_mm.unwrap_or(*height_mm);
        }
        (Format::Rack { .. }, false, true) => {
            x.format = Format::Free {
                length_mm: d.length_mm.unwrap_or(0.0),
                width_mm: d.width_mm.unwrap_or(0.0),
                height_mm: d.height_mm.unwrap_or(0.0),
            };
        }
        (Format::Free { .. }, true, _) => {
            x.format = Format::Rack {
                height_u: d.height_u.unwrap_or(1.0),
                width: d.width.unwrap_or_default(),
                depth_mm: d.depth_mm.unwrap_or(0.0),
                rear_reserve_mm: d.rear_reserve_mm,
            };
        }
    }
    x.weight_kg = d.weight_kg.or(x.weight_kg);
    x.power_w = d.power_w.or(x.power_w);
    x.dual_psu = d.dual_psu.unwrap_or(x.dual_psu);
    x.btu_h = d.btu_h.or(x.btu_h);
    if let Some(n) = d.dmx_channels.filter(|n| *n > 0) {
        match &mut x.dmx {
            Some(dmx) => dmx.default_channels = n,
            None => {
                x.dmx = Some(Dmx {
                    default_channels: n,
                    modes: Vec::new(),
                })
            }
        }
    }
    if d.active.is_some() {
        x.active = d.active;
    }
}

#[derive(Default)]
struct ResolvedIds {
    brand: Option<String>,
    category: Option<String>,
    type_id: Option<String>,
    functions: Vec<String>,
    phases: Vec<String>,
}

async fn import_run(
    State(st): State<AppState>,
    Member(s): Member,
    Path(id): Path<String>,
    Json(b): Json<RunBody>,
) -> ApiResult<Json<Value>> {
    s.require(Domain::Catalogue, Level::Ecriture, CATALOG_WRITE)?;
    let session = st
        .store
        .import_session(&id, &s.account.id)
        .await?
        .ok_or(ApiError::NotFound("Import"))?;
    if !b.mapping.iter().any(|m| m.field == Some(ImportField::Name)) {
        return Err(ApiError::Invalid(
            "Indiquez la colonne qui porte le nom des équipements.".into(),
        ));
    }
    let mut taken = HashSet::new();
    if let Some(f) = b
        .mapping
        .iter()
        .filter_map(|m| m.field)
        .find(|f| !taken.insert(*f))
    {
        return Err(ApiError::Invalid(format!(
            "Deux colonnes visent le champ « {} ».",
            f.label()
        )));
    }
    let wants_create = b
        .values
        .iter()
        .any(|(l, m)| l.can_create() && m.values().any(|r| matches!(r, Resolution::Create)));
    if wants_create {
        s.require(Domain::Parametres, Level::Ecriture, "Créer des marques ou des catégories demande le droit « Paramètres et site : écriture ».")?;
    }
    let mut snap = Snapshot::load(&st.store).await?;
    let t = now();

    // Référentiels à créer, puis index à jour pour valider les fiches.
    let mut new_refs: Vec<RefItem> = Vec::new();
    let mut created_ids: HashMap<(Lookup, String), String> = HashMap::new();
    for (l, map) in &b.values {
        if !l.can_create() {
            continue;
        }
        let mut seen = HashSet::new();
        for (text, r) in map {
            if !matches!(r, Resolution::Create)
                || find_ref(&snap, l.kind(), text).is_some()
                || !seen.insert(normalize(text))
            {
                continue;
            }
            let name = text.trim().to_string();
            if name.is_empty() || name.chars().count() > 100 {
                continue;
            }
            let data = if *l == Lookup::Brands {
                json!({ "website": null })
            } else {
                json!({ "color": "", "icon": "" })
            };
            let r = RefItem {
                kind: l.kind(),
                id: new_id(),
                name,
                parent_id: None,
                data,
                active: true,
                position: snap
                    .refs
                    .iter()
                    .filter(|x| x.kind == l.kind())
                    .map(|x| x.position)
                    .max()
                    .unwrap_or(0)
                    + 1
                    + new_refs.len() as i64,
                builtin: false,
                protected: false,
                rev: 1,
                created_at: t.clone(),
                updated_at: t.clone(),
            };
            created_ids.insert((*l, normalize(text)), r.id.clone());
            new_refs.push(r);
        }
    }
    snap.refs.extend(new_refs.iter().cloned());
    snap.index = RefIndex::build(
        snap.refs
            .iter()
            .map(|r| (r.kind, r.id.as_str(), r.parent_id.as_deref(), &r.data)),
        snap.items.iter().map(|i| (i.kind, i.id.as_str(), &i.data)),
    );

    let resolve = |l: Lookup, text: &str, issues: &mut Vec<CellIssue>| -> Option<String> {
        if let Some(r) = find_ref(&snap, l.kind(), text) {
            return Some(r.id.clone());
        }
        let choice = b.values.get(&l).and_then(|m| {
            m.get(text).or_else(|| {
                m.iter()
                    .find(|(k, _)| normalize(k) == normalize(text))
                    .map(|(_, v)| v)
            })
        });
        match choice {
            Some(Resolution::Map { id })
                if snap.refs.iter().any(|r| r.kind == l.kind() && &r.id == id) =>
            {
                Some(id.clone())
            }
            Some(Resolution::Create) if l.can_create() => {
                created_ids.get(&(l, normalize(text))).cloned()
            }
            Some(Resolution::Ignore) => None,
            _ => {
                issues.push(CellIssue {
                    field: l.field(),
                    value: text.to_string(),
                    message: format!("{} : valeur inconnue, ignorée.", l.kind().label()),
                });
                None
            }
        }
    };

    let mut created: Vec<CatalogItem> = Vec::new();
    let mut updated: Vec<(CatalogItem, CatalogItem)> = Vec::new();
    let mut skipped = Vec::new();
    let mut errors = Vec::new();
    let mut issues = Vec::new();
    let existing: Vec<CatalogItem> = snap
        .items
        .iter()
        .filter(|i| i.kind == ItemKind::Equipment)
        .cloned()
        .collect();
    let mut touched: HashSet<String> = HashSet::new();

    for (i, row) in session.rows.iter().enumerate() {
        let n = i + 1;
        let (draft, mut row_issues) = read_row(&cells_of(row, &b.mapping));
        let Some(name) = draft.name.clone() else {
            errors.push(RowError {
                row: n,
                name: String::new(),
                messages: vec!["Nom manquant : ligne ignorée.".into()],
            });
            continue;
        };
        let mut ids = ResolvedIds {
            brand: draft
                .brand
                .as_deref()
                .and_then(|v| resolve(Lookup::Brands, v, &mut row_issues)),
            category: draft
                .category
                .as_deref()
                .and_then(|v| resolve(Lookup::Categories, v, &mut row_issues)),
            type_id: draft
                .type_name
                .as_deref()
                .and_then(|v| resolve(Lookup::Types, v, &mut row_issues)),
            functions: draft
                .functions
                .iter()
                .filter_map(|v| resolve(Lookup::Functions, v, &mut row_issues))
                .collect(),
            phases: draft
                .phases
                .iter()
                .filter_map(|v| resolve(Lookup::Phases, v, &mut row_issues))
                .collect(),
        };
        // Le type impose sa fonction.
        if let Some((f, _)) = ids.type_id.as_ref().and_then(|t| snap.index.types.get(t)) {
            if !ids.functions.contains(f) {
                ids.functions.push(f.clone());
            }
        }

        let probe = json!({ "name": name, "reference": draft.reference, "brand_id": ids.brand });
        let mut pool: Vec<CatalogItem> = existing.clone();
        pool.extend(created.iter().cloned());
        let dup = duplicates_of(ItemKind::Equipment, &probe, &pool, None)
            .into_iter()
            .next();
        let strategy = b.rows.get(&n.to_string()).copied().unwrap_or(b.duplicates);

        let target = match (&dup, strategy) {
            (Some(d), DuplicateStrategy::Skip) => {
                skipped.push(
                    json!({ "row": n, "name": name, "duplicate_of": d.id, "reason": d.reason }),
                );
                continue;
            }
            (Some(d), DuplicateStrategy::Update) => {
                if created.iter().any(|c| c.id == d.id) || !touched.insert(d.id.clone()) {
                    skipped.push(json!({ "row": n, "name": name, "duplicate_of": d.id, "reason": "Fiche déjà traitée par une ligne précédente" }));
                    continue;
                }
                existing.iter().find(|e| e.id == d.id).cloned()
            }
            _ => None,
        };

        let mut x: Equipment = match &target {
            Some(t) => serde_json::from_value(t.data.clone()).unwrap_or_default(),
            None => Equipment::default(),
        };
        if target.is_none()
            && draft.height_u.is_none()
            && draft.length_mm.is_none()
            && draft.width_mm.is_none()
            && draft.height_mm.is_none()
        {
            row_issues.push(CellIssue {
                field: ImportField::HeightU,
                value: String::new(),
                message: "Hauteur absente : 1 U par défaut.".into(),
            });
        }
        apply_draft(&mut x, &draft, &ids);
        let data = match serde_json::to_value(&x)
            .map_err(|e| ApiError::Internal(e.to_string()))
            .and_then(|v| {
                check_item(
                    ItemKind::Equipment,
                    &v,
                    &snap,
                    target.as_ref().map(|t| t.id.as_str()),
                )
            }) {
            Ok(v) => v,
            Err(ApiError::Fields(f)) => {
                errors.push(RowError {
                    row: n,
                    name,
                    messages: f.into_iter().map(|e| e.message).collect(),
                });
                issues.extend(
                    row_issues
                        .into_iter()
                        .map(|issue| RowIssue { row: n, issue }),
                );
                continue;
            }
            Err(e) => return Err(e),
        };
        issues.extend(
            row_issues
                .into_iter()
                .map(|issue| RowIssue { row: n, issue }),
        );
        match target {
            Some(before) => {
                let mut after = before.clone();
                after.data = data;
                after.rev += 1;
                after.updated_at = t.clone();
                after.updated_by = Some(s.account.id.clone());
                updated.push((before, after));
            }
            None => created.push(CatalogItem {
                id: new_id(),
                kind: ItemKind::Equipment,
                name: name.clone(),
                reference: draft.reference.clone(),
                brand_id: ids.brand.clone(),
                data,
                active: true,
                rev: 1,
                created_at: t.clone(),
                updated_at: t.clone(),
                created_by: Some(s.account.id.clone()),
                updated_by: Some(s.account.id.clone()),
            }),
        }
    }

    let mut snapshot_id = None;
    if !b.dry_run && (!created.is_empty() || !updated.is_empty() || !new_refs.is_empty()) {
        let mut ref_kinds: Vec<RefKind> = new_refs.iter().map(|r| r.kind).collect();
        ref_kinds.sort_by_key(|k| k.path());
        ref_kinds.dedup();
        let snap_info = st
            .store
            .snapshot_catalog(
                Some(&s.account.id),
                &format!("Avant l'import de {}", session.filename),
                &ref_kinds,
                &[ItemKind::Equipment],
            )
            .await?;
        snapshot_id = Some(snap_info.id);
        let mut put_items = created.clone();
        put_items.extend(updated.iter().map(|(_, a)| a.clone()));
        st.store
            .apply_catalog(&CatalogChanges {
                put_refs: new_refs.clone(),
                put_items,
                ..Default::default()
            })
            .await?;
        st.store.delete_import_session(&id).await?;
        st.store
            .audit(
                Some(&s.account.id),
                "import",
                "catalog",
                &id,
                None::<&()>,
                Some(&json!({ "file": session.filename, "created": created.len(), "updated": updated.len(), "skipped": skipped.len(), "errors": errors.len(), "refs_created": new_refs.len() })),
            )
            .await?;
    }
    Ok(Json(json!({
        "dry_run": b.dry_run,
        "created": created.len(),
        "updated": updated.len(),
        "skipped": skipped,
        "errors": errors,
        "issues": issues,
        "refs_created": new_refs.iter().map(|r| json!({ "kind": r.kind, "id": r.id, "name": r.name })).collect::<Vec<_>>(),
        "snapshot_id": snapshot_id,
    })))
}

// ----- Export -----

#[derive(Deserialize)]
struct ExportQuery {
    #[serde(default = "default_format")]
    format: String,
    /// Une ligne par groupe de ports, pour lire le détail (l'import relit la version simple).
    #[serde(default)]
    full: bool,
    #[serde(default)]
    active: Option<bool>,
}

fn default_format() -> String {
    "xlsx".into()
}

#[derive(Clone)]
pub(crate) enum Cell {
    Text(String),
    Number(Option<f64>),
}

impl Cell {
    fn csv(&self) -> String {
        match self {
            Cell::Text(s) => s.clone(),
            Cell::Number(None) => String::new(),
            Cell::Number(Some(v)) => {
                let s = format!("{v:.3}");
                s.trim_end_matches('0')
                    .trim_end_matches('.')
                    .replace('.', ",")
            }
        }
    }
}

const PORT_HEADERS: [&str; 8] = [
    "Face",
    "Connecteur",
    "Variante",
    "Signal",
    "Sens",
    "Genre",
    "Quantité",
    "Libellé du port",
];

fn equipment_rows(
    snap: &Snapshot,
    items: &[&CatalogItem],
    full: bool,
) -> (Vec<String>, Vec<Vec<Cell>>) {
    let mut headers: Vec<String> = ImportField::ALL
        .iter()
        .map(|f| f.label().to_string())
        .collect();
    if full {
        headers.extend(PORT_HEADERS.iter().map(|s| s.to_string()));
    }
    let name = |k: RefKind, id: &Option<String>| {
        Cell::Text(
            id.as_deref()
                .and_then(|i| snap.ref_name(k, i))
                .unwrap_or_default()
                .to_string(),
        )
    };
    let names = |k: RefKind, ids: &[String]| {
        Cell::Text(
            ids.iter()
                .map(|i| snap.ref_name(k, i).unwrap_or(i).to_string())
                .collect::<Vec<_>>()
                .join(", "),
        )
    };
    let yes = |b: bool| Cell::Text(if b { "oui" } else { "non" }.into());
    let mut rows = Vec::new();
    for item in items {
        let Ok(x) = serde_json::from_value::<Equipment>(item.data.clone()) else {
            continue;
        };
        let (rack, free) = match &x.format {
            Format::Rack {
                height_u,
                width,
                depth_mm,
                rear_reserve_mm,
            } => (
                [
                    Cell::Number(Some(*height_u)),
                    Cell::Text(width.label().into()),
                    Cell::Number(Some(*depth_mm)),
                    Cell::Number(*rear_reserve_mm),
                ],
                [Cell::Number(None), Cell::Number(None), Cell::Number(None)],
            ),
            Format::Free {
                length_mm,
                width_mm,
                height_mm,
            } => (
                [
                    Cell::Number(None),
                    Cell::Text(String::new()),
                    Cell::Number(None),
                    Cell::Number(None),
                ],
                [
                    Cell::Number(Some(*length_mm)),
                    Cell::Number(Some(*width_mm)),
                    Cell::Number(Some(*height_mm)),
                ],
            ),
        };
        let mut base = vec![
            Cell::Text(x.name.clone()),
            Cell::Text(x.reference.clone().unwrap_or_default()),
            name(RefKind::Brand, &x.brand_id),
            name(RefKind::Category, &x.category_id),
            name(RefKind::EquipmentType, &x.type_id),
            Cell::Text(x.functions.join(", ")),
            names(RefKind::Phase, &x.phases),
            Cell::Text(x.tags.join(", ")),
            Cell::Text(x.notes.clone().unwrap_or_default()),
        ];
        base.extend(rack);
        base.extend(free);
        base.extend([
            Cell::Number(x.weight_kg),
            Cell::Number(x.power_w),
            yes(x.dual_psu),
            Cell::Number(x.btu_h),
            Cell::Number(x.dmx.as_ref().map(|d| d.default_channels as f64)),
            yes(x.active.unwrap_or(true)),
        ]);
        if !full || x.ports.is_empty() {
            if full {
                base.extend(std::iter::repeat_n(
                    Cell::Text(String::new()),
                    PORT_HEADERS.len(),
                ));
            }
            rows.push(base);
            continue;
        }
        for p in &x.ports {
            let mut r = base.clone();
            let face = serde_json::to_value(p.face)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default();
            let dir = match p.direction {
                Direction::In => "entrée",
                Direction::Out => "sortie",
                Direction::Bidir => "bidirectionnel",
            };
            r.extend([
                Cell::Text(face),
                Cell::Text(
                    snap.ref_name(RefKind::Connector, &p.connector_id)
                        .unwrap_or(&p.connector_id)
                        .to_string(),
                ),
                Cell::Text(p.variant.clone().unwrap_or_default()),
                Cell::Text(
                    p.signal_id
                        .as_deref()
                        .map(|s| snap.ref_name(RefKind::Signal, s).unwrap_or(s).to_string())
                        .unwrap_or_default(),
                ),
                Cell::Text(dir.into()),
                Cell::Text(if p.gender == Gender::M { "M" } else { "F" }.into()),
                Cell::Number(Some(p.quantity as f64)),
                Cell::Text(p.label.clone().unwrap_or_default()),
            ]);
            rows.push(r);
        }
    }
    (headers, rows)
}

pub(crate) fn to_csv(headers: &[String], rows: &[Vec<Cell>]) -> Result<Vec<u8>, String> {
    let mut out = b"\xEF\xBB\xBF".to_vec();
    {
        let mut w = csv::WriterBuilder::new()
            .delimiter(b';')
            .from_writer(&mut out);
        w.write_record(headers).map_err(|e| e.to_string())?;
        for r in rows {
            w.write_record(r.iter().map(Cell::csv))
                .map_err(|e| e.to_string())?;
        }
        w.flush().map_err(|e| e.to_string())?;
    }
    Ok(out)
}

pub(crate) fn to_xlsx(
    headers: &[String],
    rows: &[Vec<Cell>],
) -> Result<Vec<u8>, rust_xlsxwriter::XlsxError> {
    to_xlsx_named(headers, rows, "Équipements")
}

pub(crate) fn to_xlsx_named(
    headers: &[String],
    rows: &[Vec<Cell>],
    sheet: &str,
) -> Result<Vec<u8>, rust_xlsxwriter::XlsxError> {
    use rust_xlsxwriter::{Format, Workbook};
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name(sheet)?;
    let bold = Format::new().set_bold();
    for (c, h) in headers.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, h, &bold)?;
    }
    for (r, row) in rows.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            let (r, c) = (r as u32 + 1, c as u16);
            match cell {
                Cell::Text(s) if !s.is_empty() => {
                    ws.write_string(r, c, s)?;
                }
                Cell::Number(Some(v)) => {
                    ws.write_number(r, c, *v)?;
                }
                _ => {}
            }
        }
    }
    ws.set_freeze_panes(1, 0)?;
    ws.autofit();
    wb.save_to_buffer()
}

async fn export(
    State(st): State<AppState>,
    Member(s): Member,
    Query(q): Query<ExportQuery>,
) -> ApiResult<Response> {
    s.require(Domain::Catalogue, Level::Lecture, CATALOG_READ)?;
    let snap = Snapshot::load(&st.store).await?;
    let items: Vec<&CatalogItem> = snap
        .items
        .iter()
        .filter(|i| i.kind == ItemKind::Equipment && q.active.is_none_or(|a| i.active == a))
        .collect();
    let (headers, rows) = equipment_rows(&snap, &items, q.full);
    let stem = if q.full {
        "catalogue-equipements-detail"
    } else {
        "catalogue-equipements"
    };
    match q.format.as_str() {
        "csv" => {
            let bytes = to_csv(&headers, &rows).map_err(ApiError::Internal)?;
            Ok(file_response(
                bytes,
                "text/csv; charset=utf-8",
                &format!("{stem}-{}.csv", today()),
            ))
        }
        "xlsx" => {
            let bytes = to_xlsx(&headers, &rows).map_err(|e| ApiError::Internal(e.to_string()))?;
            Ok(file_response(
                bytes,
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                &format!("{stem}-{}.xlsx", today()),
            ))
        }
        _ => Err(ApiError::Invalid("Format d'export : csv ou xlsx.".into())),
    }
}

// ----- Échange de catalogue entre sites -----

/// Section d'un échange : un référentiel (`refs/brands`) ou un genre de fiches (`equipment`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Section {
    Refs(RefKind),
    Items(ItemKind),
}

impl Section {
    fn all() -> Vec<Section> {
        RefKind::ALL
            .iter()
            .filter(|k| **k != RefKind::SystemCategory)
            .map(|k| Section::Refs(*k))
            .chain(ItemKind::ALL.iter().map(|k| Section::Items(*k)))
            .collect()
    }

    /// `refs` vaut tous les référentiels ; `items` toutes les fiches.
    fn parse_list(s: Option<&str>) -> ApiResult<Vec<Section>> {
        let Some(s) = s.filter(|s| !s.trim().is_empty()) else {
            return Ok(Section::all());
        };
        let mut out = Vec::new();
        for part in s.split(',').map(str::trim) {
            let found: Vec<Section> = match part {
                "refs" => Section::all()
                    .into_iter()
                    .filter(|x| matches!(x, Section::Refs(_)))
                    .collect(),
                "items" => Section::all()
                    .into_iter()
                    .filter(|x| matches!(x, Section::Items(_)))
                    .collect(),
                p => match p.strip_prefix("refs/") {
                    Some(k) => RefKind::from_path(k)
                        .filter(|k| *k != RefKind::SystemCategory)
                        .map(Section::Refs)
                        .into_iter()
                        .collect(),
                    None => ItemKind::from_path(p)
                        .map(Section::Items)
                        .into_iter()
                        .collect(),
                },
            };
            if found.is_empty() {
                return Err(ApiError::Invalid(format!("Section inconnue : {part}.")));
            }
            for f in found {
                if !out.contains(&f) {
                    out.push(f);
                }
            }
        }
        Ok(out)
    }

    fn name(self) -> String {
        match self {
            Section::Refs(k) => format!("refs/{}", k.path()),
            Section::Items(k) => k.path().to_string(),
        }
    }
}

fn split(sections: &[Section]) -> (Vec<RefKind>, Vec<ItemKind>) {
    let refs = sections
        .iter()
        .filter_map(|s| {
            if let Section::Refs(k) = s {
                Some(*k)
            } else {
                None
            }
        })
        .collect();
    let items = sections
        .iter()
        .filter_map(|s| {
            if let Section::Items(k) = s {
                Some(*k)
            } else {
                None
            }
        })
        .collect();
    (refs, items)
}

#[derive(Deserialize)]
struct ExchangeQuery {
    sections: Option<String>,
}

async fn exchange_export(
    State(st): State<AppState>,
    Member(s): Member,
    Query(q): Query<ExchangeQuery>,
) -> ApiResult<Response> {
    s.require(Domain::Catalogue, Level::Lecture, CATALOG_READ)?;
    let sections = Section::parse_list(q.sections.as_deref())?;
    let (ref_kinds, item_kinds) = split(&sections);
    let snap = Snapshot::load(&st.store).await?;
    let mut refs = serde_json::Map::new();
    for k in &ref_kinds {
        refs.insert(
            k.path().into(),
            json!(
                snap.refs
                    .iter()
                    .filter(|r| r.kind == *k)
                    .collect::<Vec<_>>()
            ),
        );
    }
    let mut items = serde_json::Map::new();
    for k in &item_kinds {
        let list: Vec<Value> = snap
            .items
            .iter()
            .filter(|i| i.kind == *k)
            .map(|i| {
                let mut v = serde_json::to_value(i).unwrap_or_default();
                // Les comptes d'un site ne veulent rien dire sur un autre.
                if let Some(o) = v.as_object_mut() {
                    o.remove("created_by");
                    o.remove("updated_by");
                }
                v
            })
            .collect();
        items.insert(k.path().into(), Value::Array(list));
    }
    let site = st.store.meta("site_name").await?;
    let doc = json!({
        "format": EXCHANGE_FORMAT,
        "version": 1,
        "engine": VERSION,
        "site": site,
        "exported_at": now(),
        "sections": sections.iter().map(|s| s.name()).collect::<Vec<_>>(),
        "refs": refs,
        "items": items,
    });
    let bytes = serde_json::to_vec_pretty(&doc).map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(file_response(
        bytes,
        "application/json",
        &format!("catalogue-strata-{}.json", today()),
    ))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum ExchangeMode {
    /// Ajoute ce qui manque et remplace ce qui est plus ancien localement.
    #[default]
    Merge,
    /// Vide les sections choisies puis reprend le contenu du fichier.
    Replace,
}

#[derive(Deserialize)]
struct ExchangeBody {
    document: Value,
    #[serde(default)]
    mode: ExchangeMode,
    sections: Option<String>,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Default, Serialize)]
struct Counts {
    added: usize,
    updated: usize,
    unchanged: usize,
    rejected: usize,
}

#[derive(Serialize)]
struct Problem {
    section: String,
    id: String,
    name: String,
    message: String,
}

async fn exchange_import(
    State(st): State<AppState>,
    Member(s): Member,
    Json(b): Json<ExchangeBody>,
) -> ApiResult<Json<Value>> {
    s.require(Domain::Catalogue, Level::Admin, CATALOG_ADMIN)?;
    let doc = &b.document;
    if doc.get("format").and_then(Value::as_str) != Some(EXCHANGE_FORMAT) {
        return Err(ApiError::Invalid(
            "Ce fichier n'est pas un catalogue Strata Arkyo.".into(),
        ));
    }
    if doc.get("version").and_then(Value::as_i64).unwrap_or(0) > 1 {
        return Err(ApiError::Invalid(
            "Ce catalogue vient d'une version plus récente : mettez le site à jour d'abord.".into(),
        ));
    }
    // Sections demandées et présentes dans le fichier.
    let wanted = Section::parse_list(b.sections.as_deref())?;
    let present: Vec<Section> = wanted
        .into_iter()
        .filter(|s| match s {
            Section::Refs(k) => doc.pointer(&format!("/refs/{}", k.path())).is_some(),
            Section::Items(k) => doc.pointer(&format!("/items/{}", k.path())).is_some(),
        })
        .collect();
    let (ref_kinds, item_kinds) = split(&present);
    if present.is_empty() {
        return Err(ApiError::Invalid(
            "Aucune des sections demandées n'est dans le fichier.".into(),
        ));
    }
    if !ref_kinds.is_empty() {
        s.require(Domain::Parametres, Level::Ecriture, REFS_WRITE)?;
    }

    let current = Snapshot::load(&st.store).await?;
    let mut problems = Vec::new();
    let mut ref_counts = Counts::default();
    let mut item_counts = Counts::default();

    // 1. Référentiels : résultat attendu après l'échange.
    let mut final_refs: Vec<RefItem> = current
        .refs
        .iter()
        .filter(|r| b.mode == ExchangeMode::Merge || !ref_kinds.contains(&r.kind) || r.protected)
        .cloned()
        .collect();
    let mut put_refs = Vec::new();
    for k in &ref_kinds {
        let incoming: Vec<RefItem> = match doc
            .pointer(&format!("/refs/{}", k.path()))
            .cloned()
            .map(serde_json::from_value::<Vec<Value>>)
        {
            Some(Ok(v)) => v
                .into_iter()
                .filter_map(|x| {
                    let parsed = serde_json::from_value::<RefItem>(x.clone());
                    if parsed.is_err() {
                        problems.push(Problem {
                            section: format!("refs/{}", k.path()),
                            id: x["id"].as_str().unwrap_or("?").into(),
                            name: x["name"].as_str().unwrap_or("").into(),
                            message: "Élément illisible.".into(),
                        });
                        ref_counts.rejected += 1;
                    }
                    parsed.ok()
                })
                .filter(|r| r.kind == *k)
                .collect(),
            _ => Vec::new(),
        };
        for r in incoming {
            let local = final_refs
                .iter()
                .position(|x| x.kind == r.kind && x.id == r.id);
            let clash = final_refs.iter().any(|x| {
                x.kind == r.kind
                    && x.id != r.id
                    && k.named()
                    && normalize(&x.name) == normalize(&r.name)
                    && (!k.unique_among_siblings() || x.parent_id == r.parent_id)
            });
            if clash {
                problems.push(Problem {
                    section: format!("refs/{}", k.path()),
                    id: r.id.clone(),
                    name: r.name.clone(),
                    message: "Un élément du site porte déjà ce nom : conservé tel quel.".into(),
                });
                ref_counts.rejected += 1;
                continue;
            }
            match local {
                Some(i) if final_refs[i].protected => ref_counts.unchanged += 1,
                Some(i) if final_refs[i].updated_at >= r.updated_at => ref_counts.unchanged += 1,
                Some(i) => {
                    let mut r = r;
                    r.rev = final_refs[i].rev + 1;
                    r.protected = false;
                    final_refs[i] = r.clone();
                    put_refs.push(r);
                    ref_counts.updated += 1;
                }
                None => {
                    let mut r = r;
                    r.protected = false;
                    final_refs.push(r.clone());
                    put_refs.push(r);
                    ref_counts.added += 1;
                }
            }
        }
    }
    // Référentiels vérifiés contre l'index final ; un refus les retire.
    let build = |refs: &[RefItem], items: &[CatalogItem]| {
        RefIndex::build(
            refs.iter()
                .map(|r| (r.kind, r.id.as_str(), r.parent_id.as_deref(), &r.data)),
            items.iter().map(|i| (i.kind, i.id.as_str(), &i.data)),
        )
    };
    let mut final_items: Vec<CatalogItem> = current
        .items
        .iter()
        .filter(|i| b.mode == ExchangeMode::Merge || !item_kinds.contains(&i.kind))
        .cloned()
        .collect();
    let index = build(&final_refs, &final_items);
    let mut rejected_refs = HashSet::new();
    for r in &put_refs {
        if r.kind == RefKind::SystemCategory {
            continue;
        }
        if let Err(e) = validate_ref(r.kind, &r.name, &r.data, r.parent_id.as_deref(), &index) {
            problems.push(Problem {
                section: format!("refs/{}", r.kind.path()),
                id: r.id.clone(),
                name: r.name.clone(),
                message: e
                    .into_iter()
                    .map(|e| e.message)
                    .collect::<Vec<_>>()
                    .join(" "),
            });
            rejected_refs.insert((r.kind, r.id.clone()));
        }
    }
    if !rejected_refs.is_empty() {
        ref_counts.rejected += rejected_refs.len();
        put_refs.retain(|r| !rejected_refs.contains(&(r.kind, r.id.clone())));
        let originals: HashMap<(RefKind, String), RefItem> = current
            .refs
            .iter()
            .map(|r| ((r.kind, r.id.clone()), r.clone()))
            .collect();
        final_refs.retain(|r| !rejected_refs.contains(&(r.kind, r.id.clone())));
        // Un élément refusé qui existait garde sa version locale.
        for key in &rejected_refs {
            if let Some(o) = originals.get(key).filter(|_| b.mode == ExchangeMode::Merge) {
                final_refs.push(o.clone());
            }
        }
    }

    // 2. Fiches : d'abord tout poser, puis valider contre l'index final.
    let mut put_items = Vec::new();
    for k in &item_kinds {
        let list: Vec<Value> = doc
            .pointer(&format!("/items/{}", k.path()))
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
        for x in list {
            let Ok(mut it) = serde_json::from_value::<CatalogItem>(x.clone()) else {
                problems.push(Problem {
                    section: k.path().into(),
                    id: x["id"].as_str().unwrap_or("?").into(),
                    name: x["name"].as_str().unwrap_or("").into(),
                    message: "Fiche illisible.".into(),
                });
                item_counts.rejected += 1;
                continue;
            };
            if it.kind != *k {
                continue;
            }
            it.created_by = None;
            it.updated_by = Some(s.account.id.clone());
            match final_items.iter().position(|i| i.id == it.id) {
                Some(i) if final_items[i].updated_at >= it.updated_at => item_counts.unchanged += 1,
                Some(i) => {
                    it.rev = final_items[i].rev + 1;
                    final_items[i] = it.clone();
                    put_items.push(it);
                    item_counts.updated += 1;
                }
                None => {
                    final_items.push(it.clone());
                    put_items.push(it);
                    item_counts.added += 1;
                }
            }
        }
    }
    // En remplacement, un élément local absent du fichier mais encore cité par une fiche reste.
    if b.mode == ExchangeMode::Replace {
        let needed: HashSet<Target> = final_items
            .iter()
            .flat_map(|i| item_dependencies(i.kind, &i.data))
            .collect();
        for r in &current.refs {
            let missing = ref_kinds.contains(&r.kind)
                && !final_refs.iter().any(|x| x.kind == r.kind && x.id == r.id);
            if missing && needed.contains(&Target::Ref(r.kind, r.id.clone())) {
                problems.push(Problem {
                    section: format!("refs/{}", r.kind.path()),
                    id: r.id.clone(),
                    name: r.name.clone(),
                    message: "Absent du fichier mais utilisé par une fiche : conservé.".into(),
                });
                final_refs.push(r.clone());
            }
        }
    }
    let check = Snapshot {
        index: build(&final_refs, &final_items),
        refs: final_refs.clone(),
        items: final_items.clone(),
    };
    let mut accepted = Vec::new();
    for mut it in put_items {
        match check_item(it.kind, &it.data, &check, Some(&it.id)) {
            Ok(data) => {
                it.data = data;
                accepted.push(it);
            }
            Err(ApiError::Fields(f)) => {
                problems.push(Problem {
                    section: it.kind.path().into(),
                    id: it.id.clone(),
                    name: it.name.clone(),
                    message: f
                        .into_iter()
                        .map(|e| e.message)
                        .collect::<Vec<_>>()
                        .join(" "),
                });
                item_counts.rejected += 1;
                if current.items.iter().any(|c| c.id == it.id) {
                    item_counts.updated -= 1;
                } else {
                    item_counts.added -= 1;
                }
            }
            Err(e) => return Err(e),
        }
    }
    // En remplacement, une fiche refusée qui existait reste perdue : on la garde plutôt.
    if b.mode == ExchangeMode::Replace {
        let accepted_ids: HashSet<&str> = accepted.iter().map(|i| i.id.as_str()).collect();
        let keep: Vec<CatalogItem> = current
            .items
            .iter()
            .filter(|i| {
                item_kinds.contains(&i.kind)
                    && !accepted_ids.contains(i.id.as_str())
                    && problems.iter().any(|p| p.id == i.id)
            })
            .cloned()
            .collect();
        accepted.extend(keep);
    }

    let mut snapshot_id = None;
    if !b.dry_run {
        let info = st
            .store
            .snapshot_catalog(
                Some(&s.account.id),
                "Avant l'échange de catalogue",
                &ref_kinds,
                &item_kinds,
            )
            .await?;
        snapshot_id = Some(info.id);
        let replace = b.mode == ExchangeMode::Replace;
        let mut put_refs = put_refs;
        if replace {
            // Tout le contenu retenu des sections remplacées est réécrit.
            put_refs = final_refs
                .iter()
                .filter(|r| ref_kinds.contains(&r.kind) && !r.protected)
                .cloned()
                .collect();
        }
        st.store
            .apply_catalog(&CatalogChanges {
                clear_refs: if replace {
                    ref_kinds.clone()
                } else {
                    Vec::new()
                },
                clear_items: if replace {
                    item_kinds.clone()
                } else {
                    Vec::new()
                },
                put_refs,
                put_items: accepted,
            })
            .await?;
        st.store
            .audit(
                Some(&s.account.id),
                "exchange",
                "catalog",
                snapshot_id.as_deref().unwrap_or_default(),
                None::<&()>,
                Some(&json!({ "mode": b.mode, "sections": present.iter().map(|s| s.name()).collect::<Vec<_>>(), "refs": ref_counts, "items": item_counts })),
            )
            .await?;
    }
    Ok(Json(json!({
        "dry_run": b.dry_run,
        "mode": b.mode,
        "sections": present.iter().map(|s| s.name()).collect::<Vec<_>>(),
        "refs": ref_counts,
        "items": item_counts,
        "problems": problems,
        "snapshot_id": snapshot_id,
    })))
}

// ----- Instantanés -----

async fn snapshots(State(st): State<AppState>, Member(s): Member) -> ApiResult<Json<Value>> {
    s.require(Domain::Catalogue, Level::Admin, CATALOG_ADMIN)?;
    Ok(Json(
        json!({ "snapshots": st.store.list_snapshots().await? }),
    ))
}

/// Revient à l'état photographié des sections concernées. L'état courant est
/// photographié d'abord : le retour arrière se défait lui aussi.
async fn snapshot_restore(
    State(st): State<AppState>,
    Member(s): Member,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    s.require(Domain::Catalogue, Level::Admin, CATALOG_ADMIN)?;
    let data = st
        .store
        .snapshot_data(&id)
        .await?
        .ok_or(ApiError::NotFound("Instantané"))?;
    if !data.ref_kinds.is_empty() {
        s.require(Domain::Parametres, Level::Ecriture, REFS_WRITE)?;
    }
    let before = st
        .store
        .snapshot_catalog(
            Some(&s.account.id),
            "Avant un retour arrière",
            &data.ref_kinds,
            &data.item_kinds,
        )
        .await?;
    let restored = (data.refs.len(), data.items.len());
    st.store
        .apply_catalog(&CatalogChanges {
            clear_refs: data.ref_kinds.clone(),
            clear_items: data.item_kinds.clone(),
            put_refs: data.refs.into_iter().filter(|r| !r.protected).collect(),
            put_items: data.items,
        })
        .await?;
    st.store
        .audit(
            Some(&s.account.id),
            "restore",
            "catalog_snapshot",
            &id,
            None::<&()>,
            Some(&json!({ "refs": restored.0, "items": restored.1 })),
        )
        .await?;
    Ok(Json(
        json!({ "restored": { "refs": restored.0, "items": restored.1 }, "undo_snapshot_id": before.id }),
    ))
}
