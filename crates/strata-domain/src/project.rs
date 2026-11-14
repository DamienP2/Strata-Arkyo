//! Projets et inventaire (cahier des charges, sections 7 et 8) : fiche, emplacements,
//! contenants, articles et exemplaires. Les dates sont au format AAAA-MM-JJ ; la date
//! du jour est fournie par l'appelant.

use crate::catalog::FieldError;
use crate::text::normalize;
use serde::{Deserialize, Serialize};

fn err(field: impl Into<String>, message: impl Into<String>) -> FieldError {
    FieldError {
        field: field.into(),
        message: message.into(),
    }
}

// ----- Projet -----

/// Ancienne distinction prestation / intégration : gardée pour relire les anciennes
/// fiches, elle ne change plus rien. Un projet a une période de préparation et une
/// période d'exploitation (tournage, mise en service…).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intervention {
    #[default]
    Prestation,
    Integration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Regie {
    #[default]
    FlightCase,
    Car,
    Fixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    #[default]
    Preparation,
    Ready,
    InProgress,
    Done,
    Cancelled,
}

impl ProjectStatus {
    /// Statut autre que Terminé ou Annulé.
    pub fn active(self) -> bool {
        !matches!(self, ProjectStatus::Done | ProjectStatus::Cancelled)
    }
}

/// Fiche du projet.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectData {
    pub name: String,
    /// Vide : projet interne.
    pub client: Option<String>,
    pub venue: Option<String>,
    pub description: Option<String>,
    /// Texte libre : sport, concert, TV…
    pub service_type: Option<String>,
    pub intervention: Intervention,
    pub regie: Regie,
    pub status: ProjectStatus,
    pub planning_start: Option<String>,
    pub planning_end: Option<String>,
    /// Période d'exploitation : tournage, événement, mise en service.
    pub shoot_start: Option<String>,
    pub shoot_end: Option<String>,
    /// Ancienne date de mise en service : reprise comme période d'exploitation.
    pub commissioning: Option<String>,
}

impl ProjectData {
    /// Date clé du projet : début de l'exploitation.
    pub fn key_date(&self) -> Option<&String> {
        self.shoot_start.as_ref().or(self.commissioning.as_ref())
    }
}

pub fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let num = |r: std::ops::Range<usize>| s[r].parse::<u32>().ok();
    match (num(0..4), num(5..7), num(8..10)) {
        (Some(y), Some(m), Some(d)) => {
            let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
            let max = match m {
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                4 | 6 | 9 | 11 => 30,
                2 if leap => 29,
                2 => 28,
                _ => return false,
            };
            (1900..=2200).contains(&y) && (1..=max).contains(&d)
        }
        _ => false,
    }
}

fn trim_opt(v: &mut Option<String>, max: usize, field: &str, e: &mut Vec<FieldError>) {
    *v = v
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if v.as_ref().is_some_and(|s| s.chars().count() > max) {
        e.push(err(field, format!("{max} caractères au plus.")));
    }
}

/// Vérifie et normalise une fiche. `creating` : à la création, aucune date ne peut être
/// passée. Une ancienne date de mise en service devient la période d'exploitation.
pub fn validate_project(p: &mut ProjectData, creating: bool, today: &str) -> Vec<FieldError> {
    let mut e = Vec::new();
    p.name = p.name.trim().to_string();
    if p.name.is_empty() {
        e.push(err("name", "Le nom est obligatoire."));
    } else if p.name.chars().count() > 120 {
        e.push(err("name", "120 caractères au plus."));
    }
    trim_opt(&mut p.client, 120, "client", &mut e);
    trim_opt(&mut p.venue, 200, "venue", &mut e);
    trim_opt(&mut p.description, 4000, "description", &mut e);
    trim_opt(&mut p.service_type, 60, "service_type", &mut e);
    for (f, v) in [
        ("planning_start", &mut p.planning_start),
        ("planning_end", &mut p.planning_end),
        ("shoot_start", &mut p.shoot_start),
        ("shoot_end", &mut p.shoot_end),
        ("commissioning", &mut p.commissioning),
    ] {
        *v = v
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        if let Some(d) = v {
            if !is_date(d) {
                e.push(err(f, "Date attendue au format AAAA-MM-JJ."));
            } else if creating && d.as_str() < today {
                e.push(err(f, "La date est passée."));
            }
        }
    }
    p.intervention = Intervention::Prestation;
    if let Some(c) = p.commissioning.take() {
        if p.shoot_start.is_none() {
            p.shoot_start = Some(c.clone());
            p.shoot_end.get_or_insert(c);
        }
    }
    let before =
        |a: &Option<String>, b: &Option<String>| matches!((a, b), (Some(a), Some(b)) if a > b);
    if before(&p.planning_start, &p.planning_end) {
        e.push(err(
            "planning_end",
            "La fin de planification précède son début.",
        ));
    }
    if before(&p.shoot_start, &p.shoot_end) {
        e.push(err(
            "shoot_end",
            "La fin de l'exploitation précède son début.",
        ));
    }
    e
}

/// Taux de complétion de la fiche, en pourcentage arrondi.
pub fn completion(p: &ProjectData, locations: usize, containers: usize, articles: usize) -> u32 {
    let dates = (p.shoot_start.is_some() && p.shoot_end.is_some()) || p.commissioning.is_some();
    let checks = [
        p.client.is_some(),
        p.venue.is_some(),
        p.description.is_some(),
        p.service_type.is_some(),
        p.planning_start.is_some() && p.planning_end.is_some(),
        dates,
        locations > 0,
        containers > 0,
        articles > 0,
    ];
    (checks.iter().filter(|c| **c).count() as f64 * 100.0 / checks.len() as f64).round() as u32
}

/// Jours restants jusqu'à une date (arrondi supérieur), à partir des numéros de jour.
pub fn day_number(d: &str) -> Option<i64> {
    if !is_date(d) {
        return None;
    }
    let y: i64 = d[0..4].parse().ok()?;
    let m: i64 = d[5..7].parse().ok()?;
    let day: i64 = d[8..10].parse().ok()?;
    // Jours depuis une origine fixe (algorithme civil de H. Hinnant).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146097 + doe)
}

// ----- Codes de câble -----

/// Code câble tiré d'un nom : majuscules, lettres et chiffres, 5 caractères au plus.
pub fn cable_code(s: &str) -> String {
    normalize(s)
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .take(5)
        .collect()
}

// ----- Emplacements -----

pub fn validate_location(name: &mut String, code: &mut String) -> Vec<FieldError> {
    let mut e = Vec::new();
    *name = name.trim().to_string();
    if name.is_empty() {
        e.push(err("name", "Le nom est obligatoire."));
    } else if name.chars().count() > 120 {
        e.push(err("name", "120 caractères au plus."));
    }
    *code = if code.trim().is_empty() {
        cable_code(name)
    } else {
        cable_code(code)
    };
    e
}

// ----- Contenants -----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerKind {
    #[default]
    Rack,
    FlightCase,
    HardCase,
    WaterproofCase,
    Furniture,
    // Postes de régie
    MonitorWall,
    Desk,
    OperatorPosition,
    /// Bloc rackable posé sur un mobilier (pupitre, console) : il en suit l'emplacement.
    Casquette,
    // Terrain et plateau
    Zone,
    CameraPosition,
    FieldBox,
    RfMast,
    // Infrastructure du site
    SitePoint,
    Power,
    ExternalFeed,
    // Diffusion et logistique
    Display,
    CableDrum,
}

impl ContainerKind {
    pub fn default_rackable(self) -> bool {
        matches!(
            self,
            ContainerKind::Rack
                | ContainerKind::FlightCase
                | ContainerKind::HardCase
                | ContainerKind::Casquette
        )
    }
    /// Mobilier qui peut recevoir des casquettes.
    pub fn takes_casquettes(self) -> bool {
        matches!(self, ContainerKind::Desk | ContainerKind::Furniture)
    }
    pub fn mobile(self) -> bool {
        matches!(
            self,
            ContainerKind::FlightCase | ContainerKind::HardCase | ContainerKind::WaterproofCase
        )
    }
    pub fn label(self) -> &'static str {
        match self {
            ContainerKind::Rack => "Baie 19\"",
            ContainerKind::FlightCase => "Flight-case",
            ContainerKind::HardCase => "Valise rigide",
            ContainerKind::WaterproofCase => "Valise étanche",
            ContainerKind::Furniture => "Meuble / armoire",
            ContainerKind::MonitorWall => "Mur d'écrans",
            ContainerKind::Desk => "Mobilier",
            ContainerKind::Casquette => "Casquette",
            ContainerKind::OperatorPosition => "Poste opérateur",
            ContainerKind::Zone => "Emplacement libre",
            ContainerKind::CameraPosition => "Position caméra",
            ContainerKind::FieldBox => "Boîtier de terrain",
            ContainerKind::RfMast => "Mât HF",
            ContainerKind::SitePoint => "Point de raccordement",
            ContainerKind::Power => "Énergie",
            ContainerKind::ExternalFeed => "Arrivée externe",
            ContainerKind::Display => "Diffusion",
            ContainerKind::CableDrum => "Touret",
        }
    }
    /// Seuls les contenants 19″ peuvent être rackables.
    pub fn can_rack(self) -> bool {
        matches!(
            self,
            ContainerKind::Rack
                | ContainerKind::FlightCase
                | ContainerKind::HardCase
                | ContainerKind::Casquette
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContainerData {
    pub kind: ContainerKind,
    pub rackable: bool,
    pub height_u: Option<f64>,
    pub depth_mm: Option<f64>,
    /// Distance fond-rails des rails arrière internes, s'il y en a.
    pub rear_rails_mm: Option<f64>,
    pub length_mm: Option<f64>,
    pub width_mm: Option<f64>,
    pub outer_height_mm: Option<f64>,
    pub wheels: bool,
    pub empty_weight_kg: Option<f64>,
    pub max_weight_kg: Option<f64>,
    pub max_power_w: Option<f64>,
    pub cable_code: String,
    pub notes: Option<String>,
    /// Caractéristiques propres au type d'élément (mur d'écrans, pupitre, caméra…).
    pub details: crate::element::ElementDetails,
    /// Règles de numérotation des câbles propres à l'élément ; sinon celles du site.
    pub numbering: Option<crate::numbering::NumberingRules>,
    /// Casquette : mobilier qui la porte, position depuis son bord gauche, inclinaison.
    pub parent_id: Option<String>,
    pub offset_mm: Option<f64>,
    pub tilt_deg: Option<f64>,
}

impl Default for ContainerData {
    fn default() -> Self {
        ContainerData {
            kind: ContainerKind::Rack,
            rackable: true,
            height_u: Some(42.0),
            depth_mm: Some(600.0),
            rear_rails_mm: None,
            length_mm: None,
            width_mm: None,
            outer_height_mm: None,
            wheels: false,
            empty_weight_kg: None,
            max_weight_kg: None,
            max_power_w: None,
            cable_code: String::new(),
            notes: None,
            details: Default::default(),
            numbering: None,
            parent_id: None,
            offset_mm: None,
            tilt_deg: None,
        }
    }
}

fn range(e: &mut Vec<FieldError>, f: &str, v: Option<f64>, min: f64, max: f64, unit: &str) {
    if let Some(v) = v {
        if !v.is_finite() || v < min || v > max {
            e.push(err(f, format!("De {min} à {max} {unit}.")));
        }
    }
}

pub fn validate_container(name: &mut String, c: &mut ContainerData) -> Vec<FieldError> {
    let mut e = Vec::new();
    *name = name.trim().to_string();
    if name.is_empty() {
        e.push(err("name", "Le nom est obligatoire."));
    } else if name.chars().count() > 120 {
        e.push(err("name", "120 caractères au plus."));
    }
    if !c.kind.mobile() {
        c.wheels = false;
    }
    if !c.kind.can_rack() {
        c.rackable = false;
    }
    if c.kind == ContainerKind::Casquette {
        c.rackable = true;
        c.wheels = false;
        if c.parent_id.as_deref().is_none_or(|p| p.trim().is_empty()) {
            e.push(err("parent_id", "Une casquette se pose sur un mobilier."));
        }
        if c.height_u.is_some_and(|h| h > 24.0) {
            e.push(err("height_u", "Une casquette fait de 1 à 24 U."));
        }
        if c.depth_mm.is_none() {
            c.depth_mm = Some(350.0);
        }
        range(&mut e, "offset_mm", c.offset_mm, 0.0, 10_000.0, "mm");
        range(&mut e, "tilt_deg", c.tilt_deg, 0.0, 60.0, "°");
    } else {
        c.parent_id = None;
        c.offset_mm = None;
        c.tilt_deg = None;
    }
    e.extend(crate::element::validate_details(c.kind, &mut c.details));
    if let Some(r) = &mut c.numbering {
        e.extend(
            crate::numbering::validate_rules(r)
                .into_iter()
                .map(|mut f| {
                    f.field = format!("numbering.{}", f.field);
                    f
                }),
        );
    }
    if c.rackable {
        if c.height_u.is_none() {
            e.push(err("height_u", "Indiquez la hauteur en U."));
        }
        if let Some(h) = c.height_u {
            if !(1.0..=60.0).contains(&h) || h.fract() != 0.0 {
                e.push(err("height_u", "De 1 à 60 U."));
            }
        }
        range(&mut e, "depth_mm", c.depth_mm, 50.0, 2000.0, "mm");
        range(&mut e, "rear_rails_mm", c.rear_rails_mm, 0.0, 2000.0, "mm");
    } else {
        c.height_u = None;
        c.rear_rails_mm = None;
        range(&mut e, "depth_mm", c.depth_mm, 50.0, 2000.0, "mm");
        range(&mut e, "length_mm", c.length_mm, 50.0, 3000.0, "mm");
        range(&mut e, "width_mm", c.width_mm, 50.0, 3000.0, "mm");
        range(
            &mut e,
            "outer_height_mm",
            c.outer_height_mm,
            50.0,
            3000.0,
            "mm",
        );
    }
    range(
        &mut e,
        "empty_weight_kg",
        c.empty_weight_kg,
        0.0,
        500.0,
        "kg",
    );
    range(&mut e, "max_weight_kg", c.max_weight_kg, 0.0, 5000.0, "kg");
    range(&mut e, "max_power_w", c.max_power_w, 0.0, 100_000.0, "W");
    c.cable_code = cable_code(if c.cable_code.trim().is_empty() {
        name
    } else {
        &c.cable_code
    });
    c.notes = c
        .notes
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    e
}

/// Nom libre parmi `taken` (sans casse) : « Baie », « Baie 2 », « Baie 3 »…
pub fn unique_name(base: &str, taken: &[String]) -> String {
    let set: std::collections::HashSet<String> = taken.iter().map(|t| normalize(t)).collect();
    if !set.contains(&normalize(base)) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|c| !set.contains(&normalize(c)))
        .unwrap_or_default()
}

// ----- Articles -----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlateVariant {
    Blank,
    Brush,
    Vented,
}

/// Couloir arrière d'un PDU vertical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PduSide {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Accessory {
    Plate {
        variant: PlateVariant,
        height_u: u32,
    },
    Shelf {
        depth_mm: u32,
        height_u: u32,
    },
    Drawer {
        depth_mm: u32,
        height_u: u32,
    },
    /// Barre de laçage : se pose à l'arrière, à une distance du rail (déport), pour attacher
    /// les câbles qui sortent des équipements.
    LacingBar {
        height_u: u32,
        /// Déport depuis le rail, en mm.
        #[serde(default)]
        depth_mm: u32,
    },
    /// PDU vertical (0U) : fixé dans le couloir arrière gauche ou droit de la baie, hors
    /// des U ; il ne se pose pas dans l'élévation.
    VerticalPdu {
        length_mm: u32,
        outlets: u32,
        side: PduSide,
    },
    /// Panel de brassage : instance propre au projet, copiée depuis la bibliothèque.
    Panel {
        panel: crate::panel::PanelData,
    },
}

impl Accessory {
    pub fn height_u(&self) -> u32 {
        match self {
            Accessory::Plate { height_u, .. }
            | Accessory::Shelf { height_u, .. }
            | Accessory::Drawer { height_u, .. }
            | Accessory::LacingBar { height_u, .. } => *height_u,
            Accessory::Panel { panel } => panel.height_u,
            Accessory::VerticalPdu { .. } => 0,
        }
    }

    /// Un PDU vertical ne se pose pas dans les U.
    pub fn is_vertical(&self) -> bool {
        matches!(self, Accessory::VerticalPdu { .. })
    }

    /// Profondeur occupée dans la baie (panel : profondeur de câblage arrière).
    pub fn depth_mm(&self) -> Option<f64> {
        match self {
            Accessory::Shelf { depth_mm, .. } | Accessory::Drawer { depth_mm, .. } => {
                Some(*depth_mm as f64)
            }
            Accessory::Panel { panel } => Some(panel.rear_depth_mm as f64),
            Accessory::LacingBar { depth_mm, .. } => Some(*depth_mm as f64),
            Accessory::Plate { .. } | Accessory::VerticalPdu { .. } => None,
        }
    }

    /// Libellé généré, par exemple « Étagère 1U · 450mm ».
    pub fn label(&self) -> String {
        match self {
            Accessory::Plate { variant, height_u } => {
                let v = match variant {
                    PlateVariant::Blank => "pleine",
                    PlateVariant::Brush => "passe-câble",
                    PlateVariant::Vented => "aérée",
                };
                format!("Plaque {v} {height_u}U")
            }
            Accessory::Shelf { depth_mm, height_u } => {
                format!("Étagère {height_u}U · {depth_mm}mm")
            }
            Accessory::Drawer { depth_mm, height_u } => {
                format!("Tiroir {height_u}U · {depth_mm}mm")
            }
            Accessory::LacingBar { height_u, depth_mm } => {
                if *depth_mm > 0 {
                    format!("Barre de laçage {height_u}U · déport {depth_mm}mm")
                } else {
                    format!("Barre de laçage {height_u}U")
                }
            }
            Accessory::Panel { panel } => panel.name.clone(),
            Accessory::VerticalPdu {
                length_mm,
                outlets,
                side,
            } => format!(
                "PDU vertical {length_mm}mm · {outlets} prises · {}",
                if *side == PduSide::Left {
                    "gauche"
                } else {
                    "droite"
                }
            ),
        }
    }

    pub fn validate(&self) -> Vec<FieldError> {
        let mut e = Vec::new();
        if let Accessory::Panel { panel } = self {
            if !(1..=4).contains(&panel.height_u) {
                e.push(err("accessory.panel.height_u", "De 1 à 4 U."));
            }
            return e;
        }
        if let Accessory::VerticalPdu {
            length_mm, outlets, ..
        } = self
        {
            if !(300..=2200).contains(length_mm) {
                e.push(err(
                    "accessory.length_mm",
                    "PDU vertical : de 300 à 2 200 mm.",
                ));
            }
            if !(1..=60).contains(outlets) {
                e.push(err("accessory.outlets", "PDU vertical : de 1 à 60 prises."));
            }
            return e;
        }
        if !(1..=8).contains(&self.height_u()) {
            e.push(err("accessory.height_u", "De 1 à 8 U."));
        }
        match self {
            Accessory::Shelf { depth_mm, .. } if !(200..=1200).contains(depth_mm) => {
                e.push(err("accessory.depth_mm", "Étagère : de 200 à 1 200 mm."))
            }
            Accessory::Drawer { depth_mm, .. }
                if !(200..=500).contains(depth_mm) || depth_mm % 50 != 0 =>
            {
                e.push(err(
                    "accessory.depth_mm",
                    "Tiroir : de 200 à 500 mm, par pas de 50.",
                ))
            }
            Accessory::LacingBar { depth_mm, .. } if *depth_mm > 300 => e.push(err(
                "accessory.depth_mm",
                "Barre de laçage : déport de 0 à 300 mm.",
            )),
            _ => {}
        }
        e
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArticleNature {
    Equipment,
    Adapter,
    Accessory,
}

/// Repère d'exemplaire « #XXXX » : sans les caractères ambigus O, 0, I, 1.
pub const TAG_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

pub fn tag_from(n: u64) -> String {
    let mut s = String::from("#");
    let mut v = n;
    for _ in 0..4 {
        s.push(TAG_ALPHABET[(v % TAG_ALPHABET.len() as u64) as usize] as char);
        v /= TAG_ALPHABET.len() as u64;
    }
    s
}

pub fn is_mac(s: &str) -> bool {
    let parts: Vec<&str> = s.split([':', '-']).collect();
    parts.len() == 6
        && parts
            .iter()
            .all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Normalise une adresse MAC en AA:BB:CC:DD:EE:FF.
pub fn normalize_mac(s: &str) -> Option<String> {
    let t = s.trim();
    if is_mac(t) {
        return Some(t.replace('-', ":").to_uppercase());
    }
    let hex: String = t.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() == 12
        && t.chars()
            .all(|c| c.is_ascii_hexdigit() || ".:- ".contains(c))
    {
        let up = hex.to_uppercase();
        return Some(
            (0..6)
                .map(|i| &up[i * 2..i * 2 + 2])
                .collect::<Vec<_>>()
                .join(":"),
        );
    }
    None
}

// ----- Totaux -----

/// Ce qu'il faut savoir d'un article pour les totaux.
#[derive(Debug, Clone, Default)]
pub struct ArticleLoad {
    pub quantity: u32,
    /// Hauteur en U d'un exemplaire rackable, sinon `None`.
    pub height_u: Option<f64>,
    pub weight_kg: f64,
    pub power_w: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ContainerTotals {
    pub used_u: f64,
    pub total_u: Option<f64>,
    pub weight_kg: f64,
    pub power_w: f64,
    pub volume_m3: Option<f64>,
    pub over_weight: bool,
    pub over_power: bool,
    pub over_u: bool,
}

pub fn container_totals(c: &ContainerData, articles: &[ArticleLoad]) -> ContainerTotals {
    let used_u: f64 = articles
        .iter()
        .filter_map(|a| a.height_u.map(|h| h * a.quantity as f64))
        .sum();
    let weight = c.empty_weight_kg.unwrap_or(0.0)
        + articles
            .iter()
            .map(|a| a.weight_kg * a.quantity as f64)
            .sum::<f64>();
    let power: f64 = articles.iter().map(|a| a.power_w * a.quantity as f64).sum();
    let volume = if c.rackable {
        c.height_u
            .zip(c.depth_mm)
            .map(|(u, d)| 0.6 * (u * 44.45 / 1000.0) * (d / 1000.0))
    } else {
        match (c.length_mm, c.width_mm, c.depth_mm.or(c.outer_height_mm)) {
            (Some(l), Some(w), Some(d)) => Some(l * w * d / 1e9),
            _ => None,
        }
    };
    ContainerTotals {
        used_u: used_u + 0.0,
        total_u: if c.rackable { c.height_u } else { None },
        weight_kg: (weight * 10.0).round() / 10.0 + 0.0,
        power_w: power.round() + 0.0,
        volume_m3: volume.map(|v| (v * 1000.0).round() / 1000.0),
        over_weight: c.max_weight_kg.is_some_and(|m| weight > m),
        over_power: c.max_power_w.is_some_and(|m| power > m),
        over_u: c.rackable && c.height_u.is_some_and(|h| used_u > h),
    }
}

// ----- Tâches -----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Todo,
    InProgress,
    Done,
    Validated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    #[default]
    Normal,
    High,
    Urgent,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskData {
    pub title: String,
    pub description: Option<String>,
    pub due: Option<String>,
    pub done_on: Option<String>,
    pub assignee_id: Option<String>,
    pub status: TaskStatus,
    pub priority: Priority,
}

pub fn validate_task(t: &mut TaskData) -> Vec<FieldError> {
    let mut e = Vec::new();
    t.title = t.title.trim().to_string();
    if t.title.is_empty() || t.title.chars().count() > 200 {
        e.push(err("title", "Le titre compte de 1 à 200 caractères."));
    }
    t.description = t
        .description
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    for (f, v) in [("due", &t.due), ("done_on", &t.done_on)] {
        if v.as_deref().is_some_and(|d| !is_date(d)) {
            e.push(err(f, "Date attendue au format AAAA-MM-JJ."));
        }
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> ProjectData {
        ProjectData {
            name: " Finale Coupe ".into(),
            planning_start: Some("2026-11-12".into()),
            planning_end: Some("2026-11-14".into()),
            shoot_start: Some("2026-11-15".into()),
            shoot_end: Some("2026-11-16".into()),
            commissioning: Some("2026-12-01".into()),
            ..Default::default()
        }
    }

    #[test]
    fn project_rules() {
        let mut p = base();
        assert!(validate_project(&mut p, true, "2026-11-11").is_empty());
        assert_eq!(p.name, "Finale Coupe");
        assert_eq!(
            p.commissioning, None,
            "ancienne date de mise en service reprise"
        );
        assert_eq!(p.shoot_start.as_deref(), Some("2026-11-15"));
        let mut p = base();
        assert!(
            validate_project(&mut p, true, "2026-11-13")
                .iter()
                .any(|e| e.field == "planning_start")
        );
        // Toutes les régies sont permises ; une ancienne intégration devient globale.
        let mut p = base();
        p.regie = Regie::Fixed;
        p.intervention = Intervention::Integration;
        p.shoot_start = None;
        p.shoot_end = None;
        assert!(validate_project(&mut p, false, "2026-11-11").is_empty());
        assert_eq!(p.shoot_start.as_deref(), Some("2026-12-01"));
        assert_eq!(p.shoot_end.as_deref(), Some("2026-12-01"));
        assert_eq!(p.intervention, Intervention::Prestation);
        assert!(!is_date("2026-02-30"));
        assert!(is_date("2028-02-29"));
    }

    #[test]
    fn days_and_codes() {
        assert_eq!(
            day_number("2026-11-15").unwrap() - day_number("2026-11-11").unwrap(),
            4
        );
        assert_eq!(
            day_number("2027-01-01").unwrap() - day_number("2026-12-31").unwrap(),
            1
        );
        assert_eq!(cable_code("Car régie CR-2"), "CARRE");
        assert_eq!(cable_code("Bord terrain"), "BORDT");
        assert_eq!(
            unique_name("Baie vidéo", &["baie VIDÉO".into(), "Baie vidéo 2".into()]),
            "Baie vidéo 3"
        );
    }

    #[test]
    fn containers_and_totals() {
        let mut name = "Flight 1".to_string();
        let mut c = ContainerData {
            kind: ContainerKind::WaterproofCase,
            rackable: false,
            wheels: true,
            height_u: Some(42.0),
            length_mm: Some(500.0),
            width_mm: Some(400.0),
            depth_mm: Some(200.0),
            ..Default::default()
        };
        assert!(validate_container(&mut name, &mut c).is_empty());
        assert_eq!(c.height_u, None);
        assert_eq!(c.cable_code, "FLIGH");
        let mut c = ContainerData {
            height_u: Some(42.0),
            empty_weight_kg: Some(40.0),
            max_weight_kg: Some(60.0),
            ..Default::default()
        };
        let t = container_totals(
            &c,
            &[ArticleLoad {
                quantity: 2,
                height_u: Some(2.0),
                weight_kg: 12.0,
                power_w: 150.0,
            }],
        );
        assert_eq!(
            (t.used_u, t.weight_kg, t.power_w, t.over_weight),
            (4.0, 64.0, 300.0, true)
        );
        c.kind = ContainerKind::Rack;
        c.wheels = true;
        let mut n = "Baie".to_string();
        validate_container(&mut n, &mut c);
        assert!(!c.wheels, "pas de roulettes sur une baie");
    }

    #[test]
    fn tags_and_macs() {
        let tags: std::collections::HashSet<String> = (0..2000).map(tag_from).collect();
        assert_eq!(tags.len(), 2000);
        assert!(tags.iter().all(|t| !t.contains(['O', '0', 'I', '1'])));
        assert_eq!(
            normalize_mac("aa-bb-cc-dd-ee-0f").as_deref(),
            Some("AA:BB:CC:DD:EE:0F")
        );
        assert_eq!(
            normalize_mac("aabb.ccdd.ee0f").as_deref(),
            Some("AA:BB:CC:DD:EE:0F")
        );
        assert_eq!(normalize_mac("pas une mac"), None);
        let a = Accessory::Shelf {
            depth_mm: 450,
            height_u: 1,
        };
        assert_eq!(a.label(), "Étagère 1U · 450mm");
        assert!(
            Accessory::Drawer {
                depth_mm: 275,
                height_u: 2
            }
            .validate()
            .len()
                == 1
        );
    }
}

// ----- Régies (section 6.2) -----

/// Champs d'une régie : nom, type, type de prestation, notes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RegieFields {
    pub name: String,
    pub kind: Regie,
    pub service_type: Option<String>,
    pub notes: Option<String>,
}

pub fn validate_regie(f: &mut RegieFields) -> Vec<FieldError> {
    let mut e = Vec::new();
    f.name = f.name.trim().to_string();
    if f.name.is_empty() || f.name.chars().count() > 120 {
        e.push(err("name", "Le nom compte de 1 à 120 caractères."));
    }
    let clean = |s: &Option<String>| {
        s.as_ref()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
    };
    f.service_type = clean(&f.service_type);
    f.notes = clean(&f.notes);
    if f.service_type
        .as_ref()
        .is_some_and(|s| s.chars().count() > 80)
    {
        e.push(err("service_type", "80 caractères au plus."));
    }
    if f.notes.as_ref().is_some_and(|s| s.chars().count() > 2000) {
        e.push(err("notes", "2 000 caractères au plus."));
    }
    e
}

/// Nom de la régie principale d'un projet.
pub fn main_regie_name(project: &str) -> String {
    format!("{project} — Régie principale")
}
