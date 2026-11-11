//! Impression (sections 6.4, 11.6, 11.7) : cartouches, planches d'étiquettes, modèles de
//! dossier et de colonnes. Bibliothèques partagées par tout le site, sans donnée de projet.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub type FieldError = (&'static str, String);

pub const PAPERS: [(&str, f64, f64); 7] = [
    ("A4", 210.0, 297.0),
    ("A3", 297.0, 420.0),
    ("A2", 420.0, 594.0),
    ("A1", 594.0, 841.0),
    ("A0", 841.0, 1189.0),
    ("Letter", 215.9, 279.4),
    ("Legal", 215.9, 355.6),
];

pub fn paper(p: &str) -> Option<(f64, f64)> {
    PAPERS.iter().find(|x| x.0 == p).map(|x| (x.1, x.2))
}

/// Variables d'un cartouche.
pub const VARIABLES: [(&str, &str); 17] = [
    ("title", "Titre"),
    ("project", "Projet"),
    ("reference", "Référence"),
    ("client", "Client"),
    ("address", "Adresse"),
    ("date", "Date"),
    ("sheet", "N° de planche"),
    ("sheets", "Nombre de planches"),
    ("author", "Auteur"),
    ("checker", "Vérificateur"),
    ("version", "Version"),
    ("standards", "Normes"),
    ("notes", "Notes"),
    ("site", "Nom du site"),
    ("view", "Vue"),
    ("format", "Format"),
    ("logo", "Logo du site"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneKind {
    Text,
    Variable,
    Logo,
}

/// Une case du cartouche, en mm depuis le coin haut gauche du cartouche.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub kind: ZoneKind,
    /// Texte fixe, ou nom de variable.
    #[serde(default)]
    pub value: String,
    /// Petite étiquette au-dessus de la valeur.
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default = "nine")]
    pub size: f64,
    #[serde(default)]
    pub bold: bool,
}

fn nine() -> f64 {
    8.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TitleBlock {
    pub key: String,
    pub name: String,
    pub paper: String,
    pub landscape: bool,
    pub width: f64,
    pub height: f64,
    pub zones: Vec<Zone>,
    #[serde(default = "yes")]
    pub active: bool,
    #[serde(default)]
    pub builtin: bool,
}

fn yes() -> bool {
    true
}

#[allow(clippy::too_many_arguments)]
fn z(x: f64, y: f64, w: f64, h: f64, value: &str, label: &str, size: f64, bold: bool) -> Zone {
    Zone {
        x,
        y,
        w,
        h,
        kind: if value == "logo" {
            ZoneKind::Logo
        } else {
            ZoneKind::Variable
        },
        value: value.into(),
        label: (!label.is_empty()).then(|| label.into()),
        size,
        bold,
    }
}

/// Cartouches fournis : Standard, Compact, Détaillé, Vide.
pub fn default_title_blocks() -> Vec<TitleBlock> {
    let tb = |key: &str, name: &str, w: f64, h: f64, zones: Vec<Zone>| TitleBlock {
        key: key.into(),
        name: name.into(),
        paper: "A3".into(),
        landscape: true,
        width: w,
        height: h,
        zones,
        active: true,
        builtin: true,
    };
    vec![
        tb(
            "standard",
            "Standard",
            170.0,
            26.0,
            vec![
                z(0.0, 0.0, 64.6, 10.0, "site", "Site", 8.0, false),
                z(64.6, 0.0, 105.4, 10.0, "title", "Titre", 11.0, true),
                z(0.0, 10.0, 64.6, 8.0, "project", "Projet", 8.0, false),
                z(64.6, 10.0, 57.8, 8.0, "client", "Client", 8.0, false),
                z(122.4, 10.0, 47.6, 8.0, "view", "Vue", 8.0, false),
                z(0.0, 18.0, 64.6, 8.0, "date", "Date", 8.0, false),
                z(64.6, 18.0, 57.8, 8.0, "version", "Révision", 8.0, false),
                z(122.4, 18.0, 47.6, 8.0, "format", "Format", 8.0, false),
            ],
        ),
        tb(
            "compact",
            "Compact",
            120.0,
            14.0,
            vec![
                z(0.0, 0.0, 80.0, 8.0, "title", "", 10.0, true),
                z(80.0, 0.0, 40.0, 8.0, "date", "", 7.0, false),
                z(0.0, 8.0, 80.0, 6.0, "project", "", 7.0, false),
                z(80.0, 8.0, 40.0, 6.0, "sheet", "Planche", 7.0, false),
            ],
        ),
        tb(
            "detailed",
            "Détaillé",
            180.0,
            42.0,
            vec![
                z(0.0, 0.0, 40.0, 24.0, "logo", "", 8.0, false),
                z(40.0, 0.0, 140.0, 12.0, "title", "Titre", 12.0, true),
                z(40.0, 12.0, 70.0, 12.0, "project", "Projet", 8.0, false),
                z(110.0, 12.0, 70.0, 12.0, "client", "Client", 8.0, false),
                z(0.0, 24.0, 40.0, 9.0, "site", "Site", 7.0, false),
                z(40.0, 24.0, 70.0, 9.0, "address", "Adresse", 7.0, false),
                z(110.0, 24.0, 35.0, 9.0, "date", "Date", 7.0, false),
                z(145.0, 24.0, 35.0, 9.0, "sheet", "Planche", 7.0, false),
                z(0.0, 33.0, 40.0, 9.0, "author", "Auteur", 7.0, false),
                z(40.0, 33.0, 35.0, 9.0, "checker", "Vérificateur", 7.0, false),
                z(75.0, 33.0, 35.0, 9.0, "version", "Version", 7.0, false),
                z(110.0, 33.0, 35.0, 9.0, "standards", "Normes", 7.0, false),
                z(145.0, 33.0, 35.0, 9.0, "format", "Format", 7.0, false),
            ],
        ),
        tb("empty", "Vide", 0.0, 0.0, vec![]),
    ]
}

pub fn validate_title_blocks(list: &mut [TitleBlock]) -> Vec<FieldError> {
    let mut e = Vec::new();
    let mut keys = HashSet::new();
    let vars: HashSet<&str> = VARIABLES.iter().map(|v| v.0).collect();
    for t in list.iter_mut() {
        t.name = t.name.trim().to_string();
        t.key = t.key.trim().to_lowercase();
        if t.key.is_empty()
            || t.key.len() > 40
            || !t
                .key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            e.push(("key", "Clé : lettres, chiffres, tiret (40 au plus).".into()));
        } else if !keys.insert(t.key.clone()) {
            e.push(("key", format!("La clé « {} » est déjà prise.", t.key)));
        }
        if t.name.is_empty() || t.name.chars().count() > 60 {
            e.push(("name", "Nom de 1 à 60 caractères.".into()));
        }
        if paper(&t.paper).is_none() {
            e.push(("paper", "Papier de A4 à A0, Letter ou Legal.".into()));
        }
        if !(0.0..=400.0).contains(&t.width) || !(0.0..=200.0).contains(&t.height) {
            e.push(("width", "Cartouche de 400 × 200 mm au plus.".into()));
        }
        if t.zones.len() > 60 {
            e.push(("zones", "60 zones au plus.".into()));
        }
        for zn in &t.zones {
            if zn.x < 0.0
                || zn.y < 0.0
                || zn.w <= 0.0
                || zn.h <= 0.0
                || zn.x + zn.w > t.width + 0.01
                || zn.y + zn.h > t.height + 0.01
            {
                e.push((
                    "zones",
                    format!("Une zone sort du cartouche « {} ».", t.name),
                ));
                break;
            }
            if zn.kind == ZoneKind::Variable && !vars.contains(zn.value.as_str()) {
                e.push(("zones", format!("Variable inconnue : {}.", zn.value)));
                break;
            }
            if !(4.0..=30.0).contains(&zn.size) || zn.value.chars().count() > 200 {
                e.push((
                    "zones",
                    "Taille de texte de 4 à 30 points, 200 caractères au plus.".into(),
                ));
                break;
            }
        }
    }
    if !list.iter().any(|t| t.active) {
        e.push(("active", "Gardez au moins un cartouche actif.".into()));
    }
    e
}

/// Restaure les modèles fournis : seuls les manquants sont réinsérés, à leur place.
pub fn restore<T: Clone>(list: &mut Vec<T>, defaults: Vec<T>, key: impl Fn(&T) -> String) {
    let have: HashSet<String> = list.iter().map(&key).collect();
    for (i, d) in defaults.into_iter().enumerate() {
        if !have.contains(&key(&d)) {
            let at = i.min(list.len());
            list.insert(at, d);
        }
    }
}

// ----- Planches d'étiquettes -----

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelSheet {
    pub key: String,
    pub name: String,
    pub reference: String,
    #[serde(default)]
    pub brand: Option<String>,
    pub paper: String,
    pub margin_top: f64,
    pub margin_left: f64,
    pub cols: u32,
    pub rows: u32,
    pub gap_x: f64,
    pub gap_y: f64,
    pub label_w: f64,
    pub label_h: f64,
    /// Zone imprimable en haut de l'étiquette ; le reste est la zone transparente qui
    /// s'enroule autour du câble (auto-plastifiante). 0 : étiquette entièrement imprimable.
    pub print_h: f64,
    pub diam_min: Option<f64>,
    pub diam_max: Option<f64>,
    #[serde(default = "yes")]
    pub active: bool,
    #[serde(default)]
    pub builtin: bool,
}

pub fn default_label_sheets() -> Vec<LabelSheet> {
    let s = |key: &str,
             name: &str,
             reference: &str,
             cols,
             rows,
             w,
             h,
             print_h,
             mt,
             ml,
             gx,
             gy,
             dmin,
             dmax| LabelSheet {
        key: key.into(),
        name: name.into(),
        reference: reference.into(),
        brand: Some("Générique".into()),
        paper: "A4".into(),
        margin_top: mt,
        margin_left: ml,
        cols,
        rows,
        gap_x: gx,
        gap_y: gy,
        label_w: w,
        label_h: h,
        print_h,
        diam_min: dmin,
        diam_max: dmax,
        active: true,
        builtin: true,
    };
    vec![
        s(
            "self-lam-25x38",
            "Autoplastifiantes 25 × 38",
            "AP-2538",
            7,
            6,
            25.4,
            38.1,
            12.7,
            15.0,
            9.0,
            2.5,
            6.0,
            Some(2.0),
            Some(8.0),
        ),
        s(
            "self-lam-38x50",
            "Autoplastifiantes 38 × 50",
            "AP-3850",
            5,
            5,
            38.1,
            50.8,
            19.0,
            15.0,
            7.0,
            2.5,
            4.0,
            Some(5.0),
            Some(12.0),
        ),
        s(
            "flag-50x20",
            "Drapeaux 50 × 20",
            "DR-5020",
            3,
            12,
            50.0,
            20.0,
            0.0,
            15.0,
            22.5,
            7.5,
            2.5,
            None,
            None,
        ),
    ]
}

pub fn validate_label_sheets(list: &mut [LabelSheet]) -> Vec<FieldError> {
    let mut e = Vec::new();
    let mut keys = HashSet::new();
    let mut pairs = HashSet::new();
    for s in list.iter_mut() {
        s.name = s.name.trim().to_string();
        s.reference = s.reference.trim().to_string();
        s.key = s.key.trim().to_lowercase();
        if s.key.is_empty() || !keys.insert(s.key.clone()) {
            e.push(("key", "Chaque planche a une clé unique.".into()));
        }
        if s.name.is_empty()
            || s.name.chars().count() > 80
            || s.reference.is_empty()
            || s.reference.chars().count() > 40
        {
            e.push(("name", "Nom (80) et référence (40) obligatoires.".into()));
        }
        if !pairs.insert((s.name.to_lowercase(), s.reference.to_lowercase())) {
            e.push((
                "reference",
                format!(
                    "Le couple « {} » / « {} » existe déjà.",
                    s.name, s.reference
                ),
            ));
        }
        let Some((pw, ph)) = paper(&s.paper) else {
            e.push(("paper", "Papier de A4 à A0, Letter ou Legal.".into()));
            continue;
        };
        if !(1..=20).contains(&s.cols) || !(1..=60).contains(&s.rows) {
            e.push(("cols", "De 1 à 20 colonnes et de 1 à 60 lignes.".into()));
        }
        if s.label_w <= 0.0
            || s.label_h <= 0.0
            || s.gap_x < 0.0
            || s.gap_y < 0.0
            || s.margin_top < 0.0
            || s.margin_left < 0.0
        {
            e.push(("label_w", "Tailles, marges et espacements positifs.".into()));
        }
        let w =
            s.margin_left + s.cols as f64 * s.label_w + (s.cols.saturating_sub(1)) as f64 * s.gap_x;
        let h =
            s.margin_top + s.rows as f64 * s.label_h + (s.rows.saturating_sub(1)) as f64 * s.gap_y;
        if w > pw + 0.5 || h > ph + 0.5 {
            e.push((
                "cols",
                format!("« {} » déborde de la feuille {}.", s.name, s.paper),
            ));
        }
        if s.print_h < 0.0 || s.print_h > s.label_h {
            e.push((
                "print_h",
                "La zone imprimable tient dans l'étiquette.".into(),
            ));
        }
        if let (Some(a), Some(b)) = (s.diam_min, s.diam_max) {
            if a <= 0.0 || b < a {
                e.push((
                    "diam_min",
                    "Diamètres de câble : minimum positif, maximum au-dessus.".into(),
                ));
            }
        }
    }
    e
}

// ----- Dossier (6.4, 11.6) -----

/// Documents du dossier, dans l'ordre : (clé, groupe, titre, colonnes du tableau).
pub const DOCS: [(&str, &str, &str, &[&str]); 19] = [
    ("cover", "project", "Page de garde", &[]),
    ("sheet", "project", "Fiche projet", &[]),
    (
        "inventory",
        "project",
        "Inventaire",
        &["element", "nature", "label", "quantity", "tags"],
    ),
    (
        "prep_cables",
        "prep",
        "Câbles",
        &["label", "quantity", "reserve", "supply", "status"],
    ),
    (
        "prep_equipment",
        "prep",
        "Équipements",
        &["label", "quantity", "reserve", "supply", "status"],
    ),
    (
        "prep_accessories",
        "prep",
        "Accessoires",
        &["label", "quantity", "reserve", "supply", "status"],
    ),
    (
        "prep_panels",
        "prep",
        "Panels",
        &["label", "quantity", "reserve", "supply", "status"],
    ),
    (
        "prep_adapters",
        "prep",
        "Adaptateurs",
        &["label", "quantity", "reserve", "supply", "status"],
    ),
    (
        "prep_containers",
        "prep",
        "Contenants",
        &["label", "quantity", "supply", "status"],
    ),
    (
        "net_ip",
        "network",
        "Adressage IP",
        &[
            "element",
            "equipment",
            "tag",
            "port",
            "mode",
            "vlan",
            "ip",
            "hostname",
            "redundancy",
            "peer",
        ],
    ),
    (
        "net_2110",
        "network",
        "Flux 2110",
        &[
            "element",
            "equipment",
            "tag",
            "flow",
            "essence",
            "address",
            "udp",
        ],
    ),
    (
        "net_dmx",
        "network",
        "DMX",
        &[
            "fixture", "tag", "page", "console", "universe", "address", "channels", "mode",
        ],
    ),
    ("plan_racks", "plans", "Élévations", &[]),
    ("plan_synoptic", "plans", "Synoptique", &[]),
    ("plan_maps", "plans", "Cartographie", &[]),
    ("plan_schematic", "plans", "Schémas de principe", &[]),
    ("plan_av", "plans", "Implantations vidéo et audio", &[]),
    ("plan_lighting", "plans", "Plan de feu", &[]),
    ("recette", "recette", "Procès-verbal de recette", &[]),
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocOpt {
    pub key: String,
    pub include: bool,
    #[serde(default)]
    pub paper: Option<String>,
    #[serde(default)]
    pub landscape: Option<bool>,
    /// Colonnes affichées, dans l'ordre.
    #[serde(default)]
    pub columns: Vec<String>,
    /// Colonnes connues mais décochées : une colonne absente des deux listes est nouvelle.
    #[serde(default)]
    pub hidden: Vec<String>,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DossierTemplate {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub docs: Vec<DocOpt>,
}

pub fn default_docs() -> Vec<DocOpt> {
    DOCS.iter()
        .map(|(k, _, _, cols)| DocOpt {
            key: (*k).into(),
            include: true,
            paper: None,
            landscape: None,
            columns: cols.iter().map(|c| c.to_string()).collect(),
            hidden: Vec::new(),
            title: None,
        })
        .collect()
}

/// Répare une composition ancienne : documents inconnus écartés, nouveaux insérés à leur
/// place (cochés), nouvelles colonnes ajoutées cochées ; le procès-verbal reste dernier.
pub fn repair(docs: &[DocOpt]) -> Vec<DocOpt> {
    let known: Vec<&str> = DOCS.iter().map(|d| d.0).collect();
    let mut out: Vec<DocOpt> = docs
        .iter()
        .filter(|d| known.contains(&d.key.as_str()))
        .cloned()
        .collect();
    let mut seen = HashSet::new();
    out.retain(|d| seen.insert(d.key.clone()));
    for (i, (k, _, _, cols)) in DOCS.iter().enumerate() {
        match out.iter_mut().find(|d| d.key == *k) {
            Some(d) => {
                d.columns.retain(|c| cols.contains(&c.as_str()));
                let shown = d.columns.clone();
                d.hidden
                    .retain(|c| cols.contains(&c.as_str()) && !shown.contains(c));
                // Une colonne ni affichée ni décochée est nouvelle : ajoutée cochée.
                for c in cols.iter() {
                    if !d.columns.iter().any(|x| x == c) && !d.hidden.iter().any(|x| x == c) {
                        d.columns.push(c.to_string());
                    }
                }
            }
            None => {
                let at = DOCS[..i]
                    .iter()
                    .filter_map(|(pk, ..)| out.iter().position(|d| d.key == *pk))
                    .max()
                    .map(|p| p + 1)
                    .unwrap_or(0);
                out.insert(at, default_docs().into_iter().nth(i).unwrap());
            }
        }
    }
    if let Some(p) = out.iter().position(|d| d.key == "recette") {
        let r = out.remove(p);
        out.push(r);
    }
    out
}

pub fn validate_dossier_template(t: &mut DossierTemplate) -> Vec<FieldError> {
    let mut e = Vec::new();
    t.name = t.name.trim().to_string();
    if t.name.is_empty() || t.name.chars().count() > 100 {
        e.push(("name", "Nom de 1 à 100 caractères.".into()));
    }
    if t.description
        .as_deref()
        .is_some_and(|d| d.chars().count() > 500)
    {
        e.push(("description", "500 caractères au plus.".into()));
    }
    for d in &t.docs {
        if d.paper.as_deref().is_some_and(|p| p != "A4" && p != "A3") {
            e.push(("docs", "Papier A4 ou A3.".into()));
            break;
        }
        if d.title.as_deref().is_some_and(|x| x.chars().count() > 60) {
            e.push(("docs", "Titre court de 60 caractères au plus.".into()));
            break;
        }
    }
    t.docs = repair(&t.docs);
    e
}

/// Modèle de colonnes d'un tableau, partagé par le site.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnModel {
    pub id: String,
    pub table: String,
    pub name: String,
    pub columns: Vec<String>,
}

pub fn validate_column_model(m: &mut ColumnModel) -> Vec<FieldError> {
    let mut e = Vec::new();
    m.name = m.name.trim().to_string();
    if m.name.is_empty() || m.name.chars().count() > 60 {
        e.push(("name", "Nom de 1 à 60 caractères.".into()));
    }
    match DOCS.iter().find(|d| d.0 == m.table && !d.3.is_empty()) {
        None => e.push(("table", "Tableau inconnu.".into())),
        Some(d) => {
            let mut seen = HashSet::new();
            m.columns
                .retain(|c| d.3.contains(&c.as_str()) && seen.insert(c.clone()));
            if m.columns.is_empty() {
                e.push(("columns", "Gardez au moins une colonne.".into()));
            }
        }
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        let mut t = default_title_blocks();
        assert!(validate_title_blocks(&mut t).is_empty());
        let mut l = default_label_sheets();
        assert!(
            validate_label_sheets(&mut l).is_empty(),
            "{:?}",
            validate_label_sheets(&mut l)
        );
    }

    #[test]
    fn restore_inserts_only_missing() {
        let mut t = default_title_blocks();
        t.remove(1);
        t[0].name = "Maison".into();
        restore(&mut t, default_title_blocks(), |x| x.key.clone());
        assert_eq!(t.len(), 4);
        assert_eq!(t[0].name, "Maison");
        assert_eq!(t[1].key, "compact");
    }

    #[test]
    fn label_sheet_must_fit() {
        let mut l = default_label_sheets();
        l[0].cols = 9;
        l.push(l[1].clone());
        let e: Vec<_> = validate_label_sheets(&mut l).iter().map(|e| e.0).collect();
        assert!(
            e.contains(&"cols") && e.contains(&"key") && e.contains(&"reference"),
            "{e:?}"
        );
    }

    #[test]
    fn repair_old_composition() {
        let old = vec![
            DocOpt {
                key: "recette".into(),
                include: true,
                paper: None,
                landscape: None,
                columns: vec![],
                hidden: vec![],
                title: None,
            },
            DocOpt {
                key: "gone".into(),
                include: true,
                paper: None,
                landscape: None,
                columns: vec![],
                hidden: vec![],
                title: None,
            },
            DocOpt {
                key: "net_ip".into(),
                include: false,
                paper: Some("A3".into()),
                landscape: None,
                columns: vec!["ip".into(), "bogus".into()],
                hidden: vec!["mode".into()],
                title: Some("IP".into()),
            },
        ];
        let r = repair(&old);
        assert_eq!(r.len(), DOCS.len());
        assert_eq!(r.last().unwrap().key, "recette");
        let ip = r.iter().find(|d| d.key == "net_ip").unwrap();
        assert!(!ip.include);
        assert_eq!(ip.columns.first().map(String::as_str), Some("ip"));
        assert!(
            ip.columns.contains(&"vlan".into()),
            "nouvelles colonnes ajoutées cochées"
        );
        assert!(!ip.columns.contains(&"bogus".into()));
        assert!(
            !ip.columns.contains(&"mode".into()),
            "une colonne décochée le reste"
        );
        assert_eq!(r[0].key, "cover");
    }
}
