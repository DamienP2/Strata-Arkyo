//! Caractéristiques propres à chaque type d'élément d'un projet.
//!
//! Un élément est ce qu'on pose dans un emplacement : contenant (baie, flight-case…), mur
//! d'écrans, pupitre, position caméra, emplacement libre, point de raccordement… Tous
//! partagent nom, code câble, matériel, poids et consommation ; chaque type ajoute ici ses
//! propres champs. Seuls ceux du type choisi sont gardés à l'enregistrement.

use crate::catalog::validate::FieldError;
use crate::project::ContainerKind;
use serde::{Deserialize, Serialize};

fn err(field: impl Into<String>, message: impl Into<String>) -> FieldError {
    FieldError {
        field: field.into(),
        message: message.into(),
    }
}

/// Tuile d'un mur d'écrans : un écran (exemplaire de l'inventaire de l'élément) qui occupe
/// une ou plusieurs cases de la grille, et la source qu'il affiche.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallTile {
    /// Case en haut à gauche, à partir de 1.
    pub row: u32,
    pub col: u32,
    pub w: u32,
    pub h: u32,
    pub instance_id: Option<String>,
    /// Source affichée : « PGM », « Multiviewer 1 », « CAM 3 »…
    pub source: String,
    pub label: String,
}

impl Default for WallTile {
    fn default() -> Self {
        WallTile {
            row: 1,
            col: 1,
            w: 1,
            h: 1,
            instance_id: None,
            source: String::new(),
            label: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Wall {
    pub rows: u32,
    pub cols: u32,
    pub tiles: Vec<WallTile>,
    /// Structure, pieds, bras, accroche…
    pub support: Option<String>,
}

impl Default for Wall {
    fn default() -> Self {
        Wall {
            rows: 2,
            cols: 3,
            tiles: Vec::new(),
            support: None,
        }
    }
}

/// Poste d'un pupitre (ou poste opérateur seul) : un rôle et le matériel qui lui revient.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Position {
    pub id: String,
    /// Réalisateur, scripte, truquiste, ingé son, vision, ralentis…
    pub role: String,
    /// Personne prévue, si on la connaît.
    pub name: String,
    pub instance_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ElementDetails {
    // Emplacement libre
    pub surface: Option<String>,
    pub area_m2: Option<f64>,
    // Mur d'écrans
    pub wall: Option<Wall>,
    // Pupitre et poste opérateur
    pub positions: Vec<Position>,
    // Position caméra
    pub camera_number: Option<u32>,
    pub mount: Option<String>,
    pub link: Option<String>,
    // Mât HF
    pub mast_height_m: Option<f64>,
    pub frequencies: Option<String>,
    // Énergie
    pub power_kva: Option<f64>,
    pub phases: Option<u32>,
    pub outlet: Option<String>,
    // Arrivée externe
    pub provider: Option<String>,
    pub feed: Option<String>,
    pub circuit_ref: Option<String>,
    // Point de raccordement et boîtier de terrain
    pub point_kind: Option<String>,
    // Diffusion
    pub display_kind: Option<String>,
    pub diagonal_in: Option<f64>,
    pub resolution: Option<String>,
    // Touret
    pub cable_length_m: Option<f64>,
    pub drum_count: Option<u32>,
}

impl ElementDetails {
    /// Remplace chaque exemplaire cité (tuiles, postes) ; `None` retire la référence.
    pub fn map_instances(&mut self, f: &dyn Fn(&str) -> Option<String>) {
        if let Some(w) = &mut self.wall {
            for t in &mut w.tiles {
                t.instance_id = t.instance_id.as_deref().and_then(f);
            }
        }
        for p in &mut self.positions {
            p.instance_ids = p.instance_ids.iter().filter_map(|i| f(i)).collect();
        }
    }

    /// Exemplaires cités par l'élément.
    pub fn instances(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self
            .wall
            .iter()
            .flat_map(|w| w.tiles.iter().filter_map(|t| t.instance_id.as_deref()))
            .collect();
        v.extend(
            self.positions
                .iter()
                .flat_map(|p| p.instance_ids.iter().map(String::as_str)),
        );
        v
    }
}

fn text(e: &mut Vec<FieldError>, f: &str, v: Option<String>, max: usize) -> Option<String> {
    let v = v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    if v.as_ref().is_some_and(|s| s.chars().count() > max) {
        e.push(err(
            format!("details.{f}"),
            format!("{max} caractères au plus."),
        ));
    }
    v
}

fn num(
    e: &mut Vec<FieldError>,
    f: &str,
    v: Option<f64>,
    min: f64,
    max: f64,
    unit: &str,
) -> Option<f64> {
    if let Some(x) = v {
        if !x.is_finite() || x < min || x > max {
            e.push(err(
                format!("details.{f}"),
                format!("De {min} à {max} {unit}."),
            ));
        }
    }
    v
}

/// Valide les caractéristiques et ne garde que celles du type.
pub fn validate_details(kind: ContainerKind, d: &mut ElementDetails) -> Vec<FieldError> {
    use ContainerKind as K;
    let mut e = Vec::new();
    let o = std::mem::take(d);
    let mut n = ElementDetails::default();
    match kind {
        K::Zone => {
            n.surface = text(&mut e, "surface", o.surface, 80);
            n.area_m2 = num(&mut e, "area_m2", o.area_m2, 0.1, 100_000.0, "m²");
        }
        K::MonitorWall => {
            let mut w = o.wall.unwrap_or_default();
            if !(1..=12).contains(&w.rows) {
                e.push(err("details.wall.rows", "De 1 à 12 rangées."));
            }
            if !(1..=16).contains(&w.cols) {
                e.push(err("details.wall.cols", "De 1 à 16 colonnes."));
            }
            w.support = text(&mut e, "wall.support", w.support, 80);
            let mut taken: Vec<(u32, u32)> = Vec::new();
            for (k, t) in w.tiles.iter_mut().enumerate() {
                t.w = t.w.max(1);
                t.h = t.h.max(1);
                t.source = t.source.trim().chars().take(60).collect();
                t.label = t.label.trim().chars().take(60).collect();
                if t.row == 0 || t.col == 0 || t.row + t.h - 1 > w.rows || t.col + t.w - 1 > w.cols
                {
                    e.push(err(format!("details.wall.tiles.{k}"), "Tuile hors du mur."));
                    continue;
                }
                for r in t.row..t.row + t.h {
                    for c in t.col..t.col + t.w {
                        if taken.contains(&(r, c)) {
                            e.push(err(
                                format!("details.wall.tiles.{k}"),
                                "Deux tuiles se chevauchent.",
                            ));
                        }
                        taken.push((r, c));
                    }
                }
            }
            let mut seen = std::collections::HashSet::new();
            for t in &mut w.tiles {
                if let Some(i) = &t.instance_id {
                    if !seen.insert(i.clone()) {
                        t.instance_id = None;
                    }
                }
            }
            w.tiles.sort_by_key(|t| (t.row, t.col));
            n.wall = Some(w);
        }
        K::Desk | K::OperatorPosition => {
            let max = if kind == K::Desk { 24 } else { 1 };
            let mut ps = o.positions;
            if ps.len() > max {
                e.push(err("details.positions", format!("{max} poste(s) au plus.")));
            }
            let mut seen = std::collections::HashSet::new();
            for (k, p) in ps.iter_mut().enumerate() {
                p.role = p.role.trim().chars().take(60).collect();
                p.name = p.name.trim().chars().take(80).collect();
                if p.role.is_empty() {
                    e.push(err(
                        format!("details.positions.{k}.role"),
                        "Indiquez le rôle du poste.",
                    ));
                }
                if p.id.trim().is_empty() {
                    p.id = format!("p{}", k + 1);
                }
                // Un exemplaire n'appartient qu'à un poste.
                p.instance_ids.retain(|i| seen.insert(i.clone()));
            }
            n.positions = ps;
        }
        K::CameraPosition => {
            if o.camera_number.is_some_and(|c| c == 0 || c > 999) {
                e.push(err("details.camera_number", "De 1 à 999."));
            }
            n.camera_number = o.camera_number;
            n.mount = text(&mut e, "mount", o.mount, 60);
            n.link = text(&mut e, "link", o.link, 60);
        }
        K::RfMast => {
            n.mast_height_m = num(&mut e, "mast_height_m", o.mast_height_m, 0.5, 100.0, "m");
            n.frequencies = text(&mut e, "frequencies", o.frequencies, 400);
        }
        K::Power => {
            n.power_kva = num(&mut e, "power_kva", o.power_kva, 0.1, 5000.0, "kVA");
            if o.phases.is_some_and(|p| p != 1 && p != 3) {
                e.push(err("details.phases", "Monophasé (1) ou triphasé (3)."));
            }
            n.phases = o.phases;
            n.outlet = text(&mut e, "outlet", o.outlet, 60);
        }
        K::ExternalFeed => {
            n.provider = text(&mut e, "provider", o.provider, 80);
            n.feed = text(&mut e, "feed", o.feed, 60);
            n.circuit_ref = text(&mut e, "circuit_ref", o.circuit_ref, 80);
        }
        K::SitePoint | K::FieldBox => {
            n.point_kind = text(&mut e, "point_kind", o.point_kind, 60);
        }
        K::Display => {
            n.display_kind = text(&mut e, "display_kind", o.display_kind, 60);
            n.diagonal_in = num(&mut e, "diagonal_in", o.diagonal_in, 1.0, 2000.0, "pouces");
            n.resolution = text(&mut e, "resolution", o.resolution, 40);
        }
        K::CableDrum => {
            n.cable_length_m = num(
                &mut e,
                "cable_length_m",
                o.cable_length_m,
                1.0,
                10_000.0,
                "m",
            );
            if o.drum_count.is_some_and(|c| c == 0 || c > 999) {
                e.push(err("details.drum_count", "De 1 à 999."));
            }
            n.drum_count = o.drum_count;
        }
        K::Rack | K::FlightCase | K::HardCase | K::WaterproofCase | K::Furniture | K::Casquette => {
        }
    }
    *d = n;
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_fields_of_the_kind_are_kept() {
        let mut d = ElementDetails {
            camera_number: Some(3),
            power_kva: Some(60.0),
            mount: Some("  Grue ".into()),
            ..Default::default()
        };
        assert!(validate_details(ContainerKind::CameraPosition, &mut d).is_empty());
        assert_eq!(
            (d.camera_number, d.mount.as_deref(), d.power_kva),
            (Some(3), Some("Grue"), None)
        );
        let mut r = ElementDetails {
            camera_number: Some(3),
            ..Default::default()
        };
        assert!(validate_details(ContainerKind::Rack, &mut r).is_empty());
        assert_eq!(r, ElementDetails::default());
    }

    #[test]
    fn wall_tiles_stay_inside_and_apart() {
        let tile = |row, col, w, h, i: Option<&str>| WallTile {
            row,
            col,
            w,
            h,
            instance_id: i.map(str::to_string),
            ..Default::default()
        };
        let mut d = ElementDetails {
            wall: Some(Wall {
                rows: 2,
                cols: 3,
                tiles: vec![
                    tile(1, 1, 2, 2, Some("a")),
                    tile(1, 3, 1, 1, Some("a")),
                    tile(2, 3, 1, 1, None),
                ],
                support: None,
            }),
            ..Default::default()
        };
        assert!(validate_details(ContainerKind::MonitorWall, &mut d).is_empty());
        let w = d.wall.as_ref().unwrap();
        assert_eq!(
            w.tiles[1].instance_id, None,
            "un écran n'est que sur une tuile"
        );
        let mut bad = ElementDetails {
            wall: Some(Wall {
                rows: 2,
                cols: 3,
                tiles: vec![
                    tile(1, 1, 2, 2, None),
                    tile(2, 2, 1, 1, None),
                    tile(2, 3, 2, 1, None),
                ],
                support: None,
            }),
            ..Default::default()
        };
        assert_eq!(
            validate_details(ContainerKind::MonitorWall, &mut bad).len(),
            2
        );
    }

    #[test]
    fn positions_need_a_role_and_own_their_gear() {
        let mut d = ElementDetails {
            positions: vec![
                Position {
                    id: "".into(),
                    role: "Réalisateur".into(),
                    instance_ids: vec!["x".into()],
                    ..Default::default()
                },
                Position {
                    id: "b".into(),
                    role: " ".into(),
                    instance_ids: vec!["x".into(), "y".into()],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let e = validate_details(ContainerKind::Desk, &mut d);
        assert_eq!(e.len(), 1);
        assert_eq!(d.positions[0].id, "p1");
        assert_eq!(d.positions[1].instance_ids, vec!["y".to_string()]);
        d.map_instances(&|i| (i != "y").then(|| i.to_uppercase()));
        assert_eq!(d.instances(), vec!["X"]);
    }
}
