//! Import d'équipements depuis un tableur (section 5.6) : rapprochement des colonnes
//! et conversion tolérante des cellules. Le fichier lui-même est lu par l'API.

use super::model::Width;
use crate::text::normalize;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportField {
    Name,
    Reference,
    Brand,
    Category,
    Type,
    Functions,
    Phases,
    Tags,
    Notes,
    HeightU,
    Width,
    DepthMm,
    RearReserveMm,
    LengthMm,
    WidthMm,
    HeightMm,
    WeightKg,
    PowerW,
    DualPsu,
    BtuH,
    DmxChannels,
    Active,
}

impl ImportField {
    pub const ALL: [ImportField; 22] = [
        ImportField::Name,
        ImportField::Reference,
        ImportField::Brand,
        ImportField::Category,
        ImportField::Type,
        ImportField::Functions,
        ImportField::Phases,
        ImportField::Tags,
        ImportField::Notes,
        ImportField::HeightU,
        ImportField::Width,
        ImportField::DepthMm,
        ImportField::RearReserveMm,
        ImportField::LengthMm,
        ImportField::WidthMm,
        ImportField::HeightMm,
        ImportField::WeightKg,
        ImportField::PowerW,
        ImportField::DualPsu,
        ImportField::BtuH,
        ImportField::DmxChannels,
        ImportField::Active,
    ];

    /// En-tête écrit à l'export ; il se relit à l'import.
    pub fn label(self) -> &'static str {
        match self {
            ImportField::Name => "Nom",
            ImportField::Reference => "Référence",
            ImportField::Brand => "Marque",
            ImportField::Category => "Catégorie",
            ImportField::Type => "Type",
            ImportField::Functions => "Fonctions",
            ImportField::Phases => "Phases",
            ImportField::Tags => "Étiquettes",
            ImportField::Notes => "Notes",
            ImportField::HeightU => "Hauteur (U)",
            ImportField::Width => "Largeur",
            ImportField::DepthMm => "Profondeur (mm)",
            ImportField::RearReserveMm => "Réserve arrière (mm)",
            ImportField::LengthMm => "Longueur hors rack (mm)",
            ImportField::WidthMm => "Largeur hors rack (mm)",
            ImportField::HeightMm => "Hauteur hors rack (mm)",
            ImportField::WeightKg => "Poids (kg)",
            ImportField::PowerW => "Puissance (W)",
            ImportField::DualPsu => "Double alimentation",
            ImportField::BtuH => "Dissipation (BTU/h)",
            ImportField::DmxChannels => "Canaux DMX",
            ImportField::Active => "Actif",
        }
    }

    /// Synonymes français et anglais, déjà normalisés à la comparaison.
    fn synonyms(self) -> &'static [&'static str] {
        match self {
            ImportField::Name => &[
                "nom",
                "name",
                "designation",
                "modele",
                "model",
                "nom reference",
                "produit",
                "product",
                "equipement",
                "article",
                "libelle",
            ],
            ImportField::Reference => &[
                "reference",
                "ref",
                "part number",
                "pn",
                "sku",
                "code article",
                "ref fabricant",
                "mpn",
            ],
            ImportField::Brand => &[
                "marque",
                "brand",
                "fabricant",
                "manufacturer",
                "constructeur",
                "make",
                "vendor",
            ],
            ImportField::Category => &["categorie", "category", "famille", "family", "groupe"],
            ImportField::Type => &["type", "sous type", "subtype", "kind"],
            ImportField::Functions => &["fonction", "fonctions", "function", "functions", "metier"],
            ImportField::Phases => &["phase", "phases", "etape"],
            ImportField::Tags => &["etiquettes", "tags", "mots cles", "keywords", "labels"],
            ImportField::Notes => &[
                "notes",
                "note",
                "commentaire",
                "commentaires",
                "comment",
                "remarques",
                "description",
            ],
            ImportField::HeightU => &[
                "hauteur u",
                "hauteur (u)",
                "u",
                "rack units",
                "unites",
                "ru",
                "height u",
                "hauteur rack",
            ],
            ImportField::Width => &["largeur", "width", "largeur rack", "rack width"],
            ImportField::DepthMm => &["profondeur", "profondeur (mm)", "depth", "depth mm", "prof"],
            ImportField::RearReserveMm => &[
                "reserve arriere",
                "reserve de cablage",
                "rear clearance",
                "cable clearance",
            ],
            ImportField::LengthMm => &["longueur", "longueur hors rack", "length"],
            ImportField::WidthMm => &["largeur hors rack", "largeur mm", "width mm"],
            ImportField::HeightMm => &["hauteur hors rack", "hauteur mm", "height mm", "hauteur"],
            ImportField::WeightKg => &["poids", "poids (kg)", "poids kg", "weight", "masse", "kg"],
            ImportField::PowerW => &[
                "puissance",
                "puissance (w)",
                "conso",
                "consommation",
                "power",
                "watts",
                "w",
                "power consumption",
            ],
            ImportField::DualPsu => &[
                "double alimentation",
                "double alim",
                "dual psu",
                "redundant psu",
                "alim redondante",
                "psu redondante",
            ],
            ImportField::BtuH => &[
                "dissipation",
                "btu",
                "btu h",
                "btu/h",
                "heat",
                "dissipation thermique",
            ],
            ImportField::DmxChannels => &["canaux dmx", "dmx", "dmx channels", "canaux"],
            ImportField::Active => &["actif", "active", "statut", "status", "enabled"],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColumnMatch {
    pub column: usize,
    pub header: String,
    pub field: Option<ImportField>,
    /// Confiance du rapprochement, de 0 à 1.
    pub score: f64,
}

fn header_score(header: &str, field: ImportField) -> f64 {
    let h = normalize(&header.replace(['_', '-', '.'], " "));
    if h.is_empty() {
        return 0.0;
    }
    let mut best: f64 = 0.0;
    for s in field.synonyms() {
        let s = normalize(s);
        let score = if h == s {
            1.0
        } else if h.len() >= 3 && s.len() >= 3 && (h.contains(&s) || s.contains(&h)) {
            0.8
        } else {
            let j = strsim::jaro_winkler(&h, &s);
            if j >= 0.88 { j * 0.85 } else { 0.0 }
        };
        best = best.max(score);
    }
    best
}

/// Indice tiré du contenu : « 2U », « 12 kg », « 350 W ».
fn content_score(samples: &[&str], field: ImportField) -> f64 {
    let filled: Vec<String> = samples
        .iter()
        .map(|s| normalize(s))
        .filter(|s| !s.is_empty())
        .collect();
    if filled.is_empty() {
        return 0.0;
    }
    let share = |pred: &dyn Fn(&str) -> bool| {
        filled.iter().filter(|s| pred(s)).count() as f64 / filled.len() as f64
    };
    let numeric_with = |suffix: &'static str| {
        move |s: &str| {
            s.ends_with(suffix)
                && s.trim_end_matches(suffix)
                    .trim()
                    .replace(',', ".")
                    .parse::<f64>()
                    .is_ok()
        }
    };
    match field {
        ImportField::HeightU => share(&numeric_with("u")) * 0.7,
        ImportField::WeightKg => share(&numeric_with("kg")) * 0.7,
        ImportField::PowerW => share(&numeric_with("w")) * 0.6,
        ImportField::DualPsu | ImportField::Active => {
            share(&|s| parse_bool(s).is_some() && !s.is_empty()) * 0.2
        }
        _ => 0.0,
    }
}

/// Propose un champ par colonne ; un champ n'est jamais proposé deux fois.
pub fn match_columns(headers: &[String], rows: &[Vec<String>]) -> Vec<ColumnMatch> {
    let mut candidates = Vec::new();
    for (c, h) in headers.iter().enumerate() {
        let samples: Vec<&str> = rows
            .iter()
            .take(50)
            .filter_map(|r| r.get(c).map(String::as_str))
            .collect();
        for f in ImportField::ALL {
            let score = (header_score(h, f) + content_score(&samples, f)).min(1.0);
            if score >= 0.6 {
                candidates.push((score, c, f));
            }
        }
    }
    candidates.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut taken_cols = HashMap::new();
    let mut taken_fields = HashSet::new();
    for (score, c, f) in candidates {
        if !taken_cols.contains_key(&c) && taken_fields.insert(f) {
            taken_cols.insert(c, (f, score));
        }
    }
    headers
        .iter()
        .enumerate()
        .map(|(c, h)| {
            let found = taken_cols.get(&c);
            ColumnMatch {
                column: c,
                header: h.clone(),
                field: found.map(|x| x.0),
                score: found.map(|x| (x.1 * 100.0).round() / 100.0).unwrap_or(0.0),
            }
        })
        .collect()
}

/// Nombre tolérant : virgule décimale, espaces, unité collée (« 2U », « 1,5 kg », « 350 W »).
pub fn parse_number(s: &str) -> Option<f64> {
    let t: String = s
        .trim()
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{a0}' && *c != '\u{202f}')
        .collect();
    let t = t.replace(',', ".");
    let end = t
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .unwrap_or(t.len());
    let (num, unit) = t.split_at(end);
    if num.is_empty() || !num.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let unit = unit.to_lowercase();
    if !unit.is_empty()
        && ![
            "u", "ru", "he", "kg", "w", "mm", "m", "btu", "btu/h", "btuh", "ch", "canaux",
        ]
        .contains(&unit.as_str())
    {
        return None;
    }
    num.parse::<f64>().ok().filter(|v| v.is_finite())
}

pub fn parse_bool(s: &str) -> Option<bool> {
    match normalize(s).as_str() {
        "oui" | "o" | "yes" | "y" | "1" | "x" | "vrai" | "true" | "actif" | "active" => Some(true),
        "non" | "n" | "no" | "0" | "faux" | "false" | "inactif" | "inactive" | "" => Some(false),
        _ => None,
    }
}

/// Largeur de rack : 1/2, demi, half, tiers, 1/4…
pub fn parse_width(s: &str) -> Option<Width> {
    match normalize(s).replace(' ', "").as_str() {
        "" | "1" | "pleine" | "full" | "19" | "19\"" | "19pouces" | "entiere" | "1/1" => {
            Some(Width::Full)
        }
        "1/2" | "demi" | "half" | "0.5" | "0,5" | "1/2rack" | "demilargeur" => Some(Width::Half),
        "1/3" | "tiers" | "third" | "0.33" | "0,33" => Some(Width::Third),
        "1/4" | "quart" | "quarter" | "0.25" | "0,25" => Some(Width::Quarter),
        _ => None,
    }
}

pub fn split_list(s: &str) -> Vec<String> {
    s.split([',', ';', '|', '\n'])
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

/// Ligne lue, cellules converties ; les marques, catégories, types, fonctions et phases
/// restent en texte et sont rapprochés des référentiels par l'API.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ImportDraft {
    pub name: Option<String>,
    pub reference: Option<String>,
    pub brand: Option<String>,
    pub category: Option<String>,
    pub type_name: Option<String>,
    pub functions: Vec<String>,
    pub phases: Vec<String>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
    pub height_u: Option<f64>,
    pub width: Option<Width>,
    pub depth_mm: Option<f64>,
    pub rear_reserve_mm: Option<f64>,
    pub length_mm: Option<f64>,
    pub width_mm: Option<f64>,
    pub height_mm: Option<f64>,
    pub weight_kg: Option<f64>,
    pub power_w: Option<f64>,
    pub dual_psu: Option<bool>,
    pub btu_h: Option<f64>,
    pub dmx_channels: Option<u32>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CellIssue {
    pub field: ImportField,
    pub value: String,
    pub message: String,
}

/// Convertit une ligne. Une cellule illisible est signalée et vidée ; la ligne reste.
pub fn read_row(cells: &HashMap<ImportField, String>) -> (ImportDraft, Vec<CellIssue>) {
    let mut d = ImportDraft::default();
    let mut issues = Vec::new();
    let text = |f: ImportField| {
        cells
            .get(&f)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let num = |f: ImportField, issues: &mut Vec<CellIssue>| -> Option<f64> {
        let raw = text(f)?;
        let v = parse_number(&raw);
        if v.is_none() {
            issues.push(CellIssue {
                field: f,
                value: raw,
                message: "Nombre illisible : cellule vidée.".into(),
            });
        }
        v
    };
    d.name = text(ImportField::Name);
    d.reference = text(ImportField::Reference);
    d.brand = text(ImportField::Brand);
    d.category = text(ImportField::Category);
    d.type_name = text(ImportField::Type);
    d.functions = text(ImportField::Functions)
        .map(|s| split_list(&s))
        .unwrap_or_default();
    d.phases = text(ImportField::Phases)
        .map(|s| split_list(&s))
        .unwrap_or_default();
    d.tags = text(ImportField::Tags)
        .map(|s| split_list(&s))
        .unwrap_or_default();
    d.notes = text(ImportField::Notes);
    d.height_u = num(ImportField::HeightU, &mut issues);
    d.depth_mm = num(ImportField::DepthMm, &mut issues);
    d.rear_reserve_mm = num(ImportField::RearReserveMm, &mut issues);
    d.length_mm = num(ImportField::LengthMm, &mut issues);
    d.width_mm = num(ImportField::WidthMm, &mut issues);
    d.height_mm = num(ImportField::HeightMm, &mut issues);
    d.weight_kg = num(ImportField::WeightKg, &mut issues);
    d.power_w = num(ImportField::PowerW, &mut issues);
    d.btu_h = num(ImportField::BtuH, &mut issues);
    d.dmx_channels = num(ImportField::DmxChannels, &mut issues).map(|v| v.round() as u32);
    if let Some(raw) = text(ImportField::Width) {
        d.width = parse_width(&raw);
        if d.width.is_none() {
            issues.push(CellIssue {
                field: ImportField::Width,
                value: raw,
                message: "Largeur inconnue (pleine, 1/2, 1/3, 1/4) : cellule vidée.".into(),
            });
        }
    }
    for (f, slot) in [
        (ImportField::DualPsu, &mut d.dual_psu),
        (ImportField::Active, &mut d.active),
    ] {
        if let Some(raw) = text(f) {
            *slot = parse_bool(&raw);
            if slot.is_none() {
                issues.push(CellIssue {
                    field: f,
                    value: raw,
                    message: "Oui ou non attendu : cellule vidée.".into(),
                });
            }
        }
    }
    (d, issues)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tolerant_numbers() {
        assert_eq!(parse_number("2U"), Some(2.0));
        assert_eq!(parse_number("1,5 U"), Some(1.5));
        assert_eq!(parse_number("12,4 kg"), Some(12.4));
        assert_eq!(parse_number("1 200"), Some(1200.0));
        assert_eq!(parse_number("350W"), Some(350.0));
        assert_eq!(parse_number("environ 3"), None);
        assert_eq!(parse_number("3 pieds"), None);
    }

    #[test]
    fn widths_and_booleans() {
        assert_eq!(parse_width("demi"), Some(Width::Half));
        assert_eq!(parse_width("1/3"), Some(Width::Third));
        assert_eq!(parse_width("19\""), Some(Width::Full));
        assert_eq!(parse_bool("Oui"), Some(true));
        assert_eq!(parse_bool("peut-être"), None);
    }

    #[test]
    fn columns_are_matched_once() {
        let headers: Vec<String> = [
            "Désignation",
            "Fabricant",
            "Ref.",
            "Hauteur",
            "Poids",
            "Conso (W)",
            "Prof.",
            "Notes",
            "Colonne X",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let rows = vec![vec![
            "Mélangeur".into(),
            "Blackmagic".into(),
            "SWATEM".into(),
            "2U".into(),
            "12,4".into(),
            "330".into(),
            "390".into(),
            "".into(),
            "?".into(),
        ]];
        let m = match_columns(&headers, &rows);
        let f: Vec<_> = m.iter().map(|c| c.field).collect();
        assert_eq!(
            f,
            [
                Some(ImportField::Name),
                Some(ImportField::Brand),
                Some(ImportField::Reference),
                Some(ImportField::HeightU),
                Some(ImportField::WeightKg),
                Some(ImportField::PowerW),
                Some(ImportField::DepthMm),
                Some(ImportField::Notes),
                None
            ]
        );
    }

    #[test]
    fn exported_headers_read_back() {
        let headers: Vec<String> = ImportField::ALL
            .iter()
            .map(|f| f.label().to_string())
            .collect();
        let m = match_columns(&headers, &[]);
        for (c, f) in m.iter().zip(ImportField::ALL) {
            assert_eq!(c.field, Some(f), "{}", c.header);
        }
    }

    #[test]
    fn invalid_cells_are_flagged_and_emptied() {
        let cells: HashMap<_, _> = [
            (ImportField::Name, "Grille".to_string()),
            (ImportField::WeightKg, "lourd".to_string()),
            (ImportField::HeightU, "2U".to_string()),
        ]
        .into();
        let (d, issues) = read_row(&cells);
        assert_eq!(d.height_u, Some(2.0));
        assert_eq!(d.weight_kg, None);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].field, ImportField::WeightKg);
    }
}
