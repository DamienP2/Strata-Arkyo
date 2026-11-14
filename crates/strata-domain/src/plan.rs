//! Cartographie 2D et 3D (section 11.1). Toutes les positions sont en mm réels ; le repère
//! commun d'une carte est donné par les deux points de calibration : le premier est l'origine,
//! le second fixe l'axe des X. Le même couple de points physiques sert à tous les étages.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type FieldError = (&'static str, String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapKind {
    #[default]
    MultiFloor,
    Flat,
    Outdoor,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MapData {
    pub name: String,
    pub kind: MapKind,
}

pub fn validate_map(m: &mut MapData) -> Vec<FieldError> {
    m.name = m.name.trim().to_string();
    let mut e = Vec::new();
    if m.name.is_empty() || m.name.chars().count() > 80 {
        e.push(("name", "Nom de 1 à 80 caractères.".into()));
    }
    e
}

/// Deux points cliqués sur le fond (en pixels de l'image) et leur distance réelle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    pub a: [f64; 2],
    pub b: [f64; 2],
    pub dist_mm: f64,
}

impl Calibration {
    /// Millimètres par pixel.
    pub fn scale(&self) -> f64 {
        let d = ((self.b[0] - self.a[0]).powi(2) + (self.b[1] - self.a[1]).powi(2)).sqrt();
        if d > 0.0 { self.dist_mm / d } else { 0.0 }
    }

    /// Pixel de l'image vers le repère commun (mm).
    pub fn to_world(&self, p: [f64; 2]) -> [f64; 2] {
        let (dx, dy) = (self.b[0] - self.a[0], self.b[1] - self.a[1]);
        let ang = dy.atan2(dx);
        let s = self.scale();
        let (px, py) = (p[0] - self.a[0], p[1] - self.a[1]);
        let (c, si) = (ang.cos(), ang.sin());
        [(px * c + py * si) * s, (-px * si + py * c) * s]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FloorData {
    pub level: i32,
    pub name: String,
    /// Hauteur entre sols.
    pub height_mm: f64,
    /// Hauteur du faux-plafond ou du chemin de câbles.
    pub ceiling_mm: f64,
    pub calibration: Option<Calibration>,
    /// Fond de plan : identifiant du fichier, type et taille en pixels.
    pub background: Option<String>,
    pub bg_type: Option<String>,
    pub bg_w: Option<f64>,
    pub bg_h: Option<f64>,
    /// Opacité du fond (plan estompé).
    pub bg_opacity: f64,
}

impl Default for FloorData {
    fn default() -> Self {
        FloorData {
            level: 0,
            name: String::new(),
            height_mm: 3000.0,
            ceiling_mm: 2500.0,
            calibration: None,
            background: None,
            bg_type: None,
            bg_w: None,
            bg_h: None,
            bg_opacity: 0.6,
        }
    }
}

pub fn level_name(level: i32) -> String {
    match level {
        0 => "Rez-de-chaussée".into(),
        l if l < 0 => format!("Sous-sol {}", -l),
        1 => "1er étage".into(),
        l => format!("{l}e étage"),
    }
}

pub fn validate_floor(f: &mut FloorData) -> Vec<FieldError> {
    let mut e = Vec::new();
    f.name = f.name.trim().to_string();
    if f.name.is_empty() {
        f.name = level_name(f.level);
    }
    if !(-2..=10).contains(&f.level) {
        e.push(("level", "Niveau entier de −2 à +10.".into()));
    }
    if f.name.chars().count() > 80 {
        e.push(("name", "80 caractères au plus.".into()));
    }
    if !(f.height_mm > 0.0 && f.height_mm <= 20_000.0) {
        e.push((
            "height_mm",
            "Hauteur entre sols : plus de 0 et 20 m au plus.".into(),
        ));
    }
    if !(f.ceiling_mm >= 0.0 && f.ceiling_mm <= f.height_mm) {
        e.push((
            "ceiling_mm",
            "Le faux-plafond est entre le sol et la hauteur de l'étage.".into(),
        ));
    }
    if let Some(c) = &f.calibration {
        if c.dist_mm <= 0.0 || c.dist_mm.is_nan() || c.scale() <= 0.0 {
            e.push((
                "calibration",
                "Deux points distincts et une distance réelle positive.".into(),
            ));
        }
    }
    f.bg_opacity = f.bg_opacity.clamp(0.05, 1.0);
    e
}

/// Altitude du sol de chaque niveau (mm), le rez-de-chaussée à 0. Un niveau manquant entre
/// 0 et un étage rend son altitude inconnue.
pub fn floor_bases(floors: &[FloorData]) -> HashMap<i32, f64> {
    let by: HashMap<i32, &FloorData> = floors.iter().map(|f| (f.level, f)).collect();
    let mut out = HashMap::new();
    let lo = floors.iter().map(|f| f.level).min().unwrap_or(0).min(0);
    let hi = floors.iter().map(|f| f.level).max().unwrap_or(0).max(0);
    // Une carte sans rez-de-chaussée (un seul étage, par exemple) prend son plus bas niveau
    // comme référence.
    let origin = if by.contains_key(&0) {
        0
    } else {
        floors.iter().map(|f| f.level).min().unwrap_or(0)
    };
    out.insert(origin, 0.0);
    let mut z = 0.0;
    for l in origin..hi {
        match by.get(&l) {
            Some(f) => {
                z += f.height_mm;
                if by.contains_key(&(l + 1)) {
                    out.insert(l + 1, z);
                } else {
                    break;
                }
            }
            None => break,
        }
    }
    let mut z = 0.0;
    for l in (lo..origin).rev() {
        match by.get(&l) {
            Some(f) => {
                z -= f.height_mm;
                out.insert(l, z);
            }
            None => break,
        }
    }
    out.retain(|l, _| by.contains_key(l));
    out
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObjectData {
    /// Contenant posé : un seul placement par contenant sur l'ensemble des cartes.
    Container {
        container_id: String,
        x: f64,
        y: f64,
        #[serde(default)]
        z: f64,
        #[serde(default)]
        rot: u16,
    },
    /// Point de connexion : prise murale, boîtier de sol…
    Point {
        x: f64,
        y: f64,
        #[serde(default)]
        z: f64,
        label: String,
        #[serde(default)]
        signal_id: Option<String>,
        #[serde(default)]
        connector_id: Option<String>,
        ports: u32,
    },
    Text {
        x: f64,
        y: f64,
        text: String,
        #[serde(default = "text_size")]
        size: f64,
    },
    /// Mesure : chaîne de segments.
    Measure { points: Vec<[f64; 2]> },
}

fn text_size() -> f64 {
    300.0
}

const LIMIT: f64 = 10_000_000.0;

fn coord_ok(v: f64) -> bool {
    v.is_finite() && v.abs() <= LIMIT
}

pub fn validate_object(o: &mut ObjectData) -> Vec<FieldError> {
    let mut e = Vec::new();
    match o {
        ObjectData::Container { x, y, z, rot, .. } => {
            if !coord_ok(*x) || !coord_ok(*y) {
                e.push(("x", "Position hors du plan.".into()));
            }
            if !(0.0..=100_000.0).contains(z) {
                e.push(("z", "Hauteur au sol de 0 à 100 m.".into()));
            }
            if ![0, 90, 180, 270].contains(rot) {
                e.push(("rot", "Rotation : 0, 90, 180 ou 270°.".into()));
            }
        }
        ObjectData::Point {
            x,
            y,
            z,
            label,
            ports,
            signal_id,
            connector_id,
        } => {
            *label = label.trim().to_string();
            for s in [signal_id, connector_id] {
                if s.as_deref().is_some_and(|v| v.trim().is_empty()) {
                    *s = None;
                }
            }
            if !coord_ok(*x) || !coord_ok(*y) {
                e.push(("x", "Position hors du plan.".into()));
            }
            if !(0.0..=100_000.0).contains(z) {
                e.push(("z", "Hauteur au sol de 0 à 100 m.".into()));
            }
            if label.is_empty() || label.chars().count() > 60 {
                e.push(("label", "Libellé de 1 à 60 caractères.".into()));
            }
            if !(1..=256).contains(ports) {
                e.push(("ports", "De 1 à 256 ports.".into()));
            }
        }
        ObjectData::Text { x, y, text, size } => {
            *text = text.trim().to_string();
            if !coord_ok(*x) || !coord_ok(*y) {
                e.push(("x", "Position hors du plan.".into()));
            }
            if text.is_empty() || text.chars().count() > 500 {
                e.push(("text", "Texte de 1 à 500 caractères.".into()));
            }
            if !(20.0..=20_000.0).contains(size) {
                e.push(("size", "Taille de 2 cm à 20 m.".into()));
            }
        }
        ObjectData::Measure { points } => {
            if points.len() < 2 || points.len() > 200 {
                e.push(("points", "Une mesure a de 2 à 200 points.".into()));
            }
            if points.iter().any(|p| !coord_ok(p[0]) || !coord_ok(p[1])) {
                e.push(("points", "Point hors du plan.".into()));
            }
        }
    }
    e
}

pub fn measure_length(points: &[[f64; 2]]) -> f64 {
    points
        .windows(2)
        .map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt())
        .sum()
}

/// Point de passage d'un tracé : position, niveau et hauteur au-dessus du sol (par défaut
/// celle du faux-plafond de l'étage).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Waypoint {
    pub x: f64,
    pub y: f64,
    pub level: i32,
    #[serde(default)]
    pub h: Option<f64>,
}

/// Extrémité d'un tracé : un contenant posé (niveau, position, hauteur au sol).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct End {
    pub x: f64,
    pub y: f64,
    pub level: i32,
    pub z: f64,
}

pub fn validate_route(w: &[Waypoint], floors: &[FloorData]) -> Vec<FieldError> {
    let mut e = Vec::new();
    if w.len() > 500 {
        e.push(("waypoints", "500 points de passage au plus.".into()));
    }
    for p in w {
        if !coord_ok(p.x) || !coord_ok(p.y) {
            e.push(("waypoints", "Point de passage hors de la carte.".into()));
            break;
        }
        if !floors.iter().any(|f| f.level == p.level) {
            e.push((
                "waypoints",
                format!("Le niveau {} n'existe pas sur cette carte.", p.level),
            ));
            break;
        }
        if p.h.is_some_and(|h| !(0.0..=20_000.0).contains(&h)) {
            e.push(("waypoints", "Hauteur de passage de 0 à 20 m.".into()));
            break;
        }
    }
    e
}

/// Points 3D d'un tracé (mm), montées verticales comprises : entre deux points, on monte ou
/// descend d'abord à l'aplomb du premier, puis on file à l'horizontale.
pub fn route_points(
    a: End,
    waypoints: &[Waypoint],
    b: End,
    floors: &[FloorData],
) -> Option<Vec<[f64; 3]>> {
    let bases = floor_bases(floors);
    let ceil: HashMap<i32, f64> = floors.iter().map(|f| (f.level, f.ceiling_mm)).collect();
    let mut pts: Vec<[f64; 3]> = Vec::new();
    pts.push([a.x, a.y, bases.get(&a.level)? + a.z]);
    for w in waypoints {
        let z = bases.get(&w.level)? + w.h.unwrap_or(*ceil.get(&w.level)?);
        pts.push([w.x, w.y, z]);
    }
    pts.push([b.x, b.y, bases.get(&b.level)? + b.z]);
    let mut out = vec![pts[0]];
    for w in pts.windows(2) {
        let (p, q) = (w[0], w[1]);
        if (q[2] - p[2]).abs() > 0.5 {
            out.push([p[0], p[1], q[2]]);
        }
        out.push(q);
    }
    Some(out)
}

/// Longueur d'un tracé en mètres : somme des segments 3D, arrondie aux 10 cm supérieurs ;
/// rien s'il manque un étage ou un point.
pub fn route_length_m(a: End, waypoints: &[Waypoint], b: End, floors: &[FloorData]) -> Option<f64> {
    let pts = route_points(a, waypoints, b, floors)?;
    let mm: f64 = pts
        .windows(2)
        .map(|w| {
            ((w[1][0] - w[0][0]).powi(2)
                + (w[1][1] - w[0][1]).powi(2)
                + (w[1][2] - w[0][2]).powi(2))
            .sqrt()
        })
        .sum();
    Some((mm / 100.0 - 1e-9).ceil().max(0.0) / 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floor(level: i32) -> FloorData {
        FloorData {
            level,
            ..Default::default()
        }
    }

    #[test]
    fn calibration_maps_pixels_to_mm() {
        let c = Calibration {
            a: [100.0, 100.0],
            b: [100.0, 300.0],
            dist_mm: 10_000.0,
        };
        assert_eq!(c.scale(), 50.0);
        let p = c.to_world([100.0, 300.0]);
        assert!((p[0] - 10_000.0).abs() < 1e-6 && p[1].abs() < 1e-6);
        let p = c.to_world([0.0, 100.0]);
        assert!(p[0].abs() < 1e-6 && (p[1] - 5_000.0).abs() < 1e-6, "{p:?}");
    }

    #[test]
    fn bases_and_missing_levels() {
        let mut f1 = floor(1);
        f1.height_mm = 4000.0;
        let b = floor_bases(&[floor(-1), floor(0), f1.clone(), floor(2)]);
        assert_eq!(b[&-1], -3000.0);
        assert_eq!(b[&1], 3000.0);
        assert_eq!(b[&2], 7000.0);
        // Étage 1 manquant : l'étage 2 n'a pas d'altitude.
        let b = floor_bases(&[floor(0), floor(2)]);
        assert!(!b.contains_key(&2));
        // Carte plate au niveau 3 seul : référence locale.
        assert_eq!(floor_bases(&[floor(3)])[&3], 0.0);
    }

    #[test]
    fn route_length_with_risers() {
        let floors = [floor(0), floor(1)];
        let a = End {
            x: 0.0,
            y: 0.0,
            level: 0,
            z: 0.0,
        };
        let b = End {
            x: 4000.0,
            y: 3000.0,
            level: 1,
            z: 1000.0,
        };
        // Montée au faux-plafond (2,5 m), 5 m en diagonale au niveau 1 (montée de 3 m au
        // faux-plafond de l'étage : 3 + 2,5 = 5,5 m), descente à 4 m (1,5 m).
        let w = [
            Waypoint {
                x: 0.0,
                y: 0.0,
                level: 0,
                h: None,
            },
            Waypoint {
                x: 4000.0,
                y: 3000.0,
                level: 1,
                h: None,
            },
        ];
        let l = route_length_m(a, &w, b, &floors).unwrap();
        // 2,5 + 3 (0 → plafond étage 1 : 5,5 − 2,5) + 5 + 1,5 = 12 m
        assert_eq!(l, 12.0);
        let l = route_length_m(
            a,
            &[],
            End {
                x: 1234.0,
                y: 0.0,
                level: 0,
                z: 0.0,
            },
            &floors,
        )
        .unwrap();
        assert_eq!(l, 1.3);
        assert!(
            route_length_m(
                a,
                &[Waypoint {
                    x: 0.0,
                    y: 0.0,
                    level: 5,
                    h: None
                }],
                b,
                &floors
            )
            .is_none()
        );
    }

    #[test]
    fn floor_and_object_rules() {
        let mut f = FloorData {
            level: 11,
            height_mm: 25_000.0,
            ..Default::default()
        };
        let e: Vec<_> = validate_floor(&mut f).iter().map(|e| e.0).collect();
        assert_eq!(e, vec!["level", "height_mm"]);
        assert_eq!(f.name, "11e étage");
        let mut o = ObjectData::Container {
            container_id: "c".into(),
            x: 0.0,
            y: 0.0,
            z: 0.0,
            rot: 45,
        };
        assert_eq!(validate_object(&mut o)[0].0, "rot");
        let mut o = ObjectData::Point {
            x: 0.0,
            y: 0.0,
            z: 300.0,
            label: " ".into(),
            signal_id: Some("".into()),
            connector_id: None,
            ports: 0,
        };
        let e: Vec<_> = validate_object(&mut o).iter().map(|e| e.0).collect();
        assert_eq!(e, vec!["label", "ports"]);
        assert_eq!(
            measure_length(&[[0.0, 0.0], [3000.0, 4000.0], [3000.0, 5000.0]]),
            6000.0
        );
    }
}
