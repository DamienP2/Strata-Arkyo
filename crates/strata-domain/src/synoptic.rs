//! Synoptique (section 10) : adresses des ports, compatibilité, champs d'un câble, longueur.
//!
//! Un port s'adresse par une chaîne stable :
//! - `i:{exemplaire}:{groupe}:{n}` : n-ième port (à partir de 1) du groupe de ports
//!   (rang dans la fiche du catalogue) d'un exemplaire d'équipement ;
//! - `p:{article}:{emplacement}:{f|r}` : face avant ou arrière d'un emplacement de panel ;
//! - `a:{article}:{unité}:{in|o<k>.<n>}` : entrée, ou n-ième sortie du groupe k, d'une unité
//!   d'adaptateur.

use crate::catalog::validate::FieldError;
use serde::{Deserialize, Serialize};
use std::fmt;

fn err(field: impl Into<String>, message: impl Into<String>) -> FieldError {
    FieldError {
        field: field.into(),
        message: message.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PortRef {
    Instance {
        instance: String,
        group: u32,
        n: u32,
    },
    Panel {
        article: String,
        slot: u32,
        rear: bool,
    },
    Adapter {
        article: String,
        unit: u32,
        out: Option<(u32, u32)>,
    },
}

impl fmt::Display for PortRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PortRef::Instance { instance, group, n } => write!(f, "i:{instance}:{group}:{n}"),
            PortRef::Panel {
                article,
                slot,
                rear,
            } => {
                write!(f, "p:{article}:{slot}:{}", if *rear { "r" } else { "f" })
            }
            PortRef::Adapter { article, unit, out } => match out {
                None => write!(f, "a:{article}:{unit}:in"),
                Some((k, n)) => write!(f, "a:{article}:{unit}:o{k}.{n}"),
            },
        }
    }
}

impl std::str::FromStr for PortRef {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        let p: Vec<&str> = s.split(':').collect();
        let num = |x: &str| x.parse::<u32>().map_err(|_| ());
        match p.as_slice() {
            ["i", id, g, n] if !id.is_empty() => Ok(PortRef::Instance {
                instance: id.to_string(),
                group: num(g)?,
                n: num(n)?.max(1),
            }),
            ["p", id, slot, side] if !id.is_empty() && (*side == "f" || *side == "r") => {
                Ok(PortRef::Panel {
                    article: id.to_string(),
                    slot: num(slot)?,
                    rear: *side == "r",
                })
            }
            ["a", id, unit, port] if !id.is_empty() => {
                let out = if *port == "in" {
                    None
                } else {
                    let (k, n) = port
                        .strip_prefix('o')
                        .ok_or(())?
                        .split_once('.')
                        .ok_or(())?;
                    Some((num(k)?, num(n)?))
                };
                Ok(PortRef::Adapter {
                    article: id.to_string(),
                    unit: num(unit)?,
                    out,
                })
            }
            _ => Err(()),
        }
    }
}

impl PortRef {
    /// Nœud du synoptique qui porte ce port : exemplaire, rangée de panel ou unité d'adaptateur.
    pub fn node(&self, per_u: Option<u32>) -> String {
        match self {
            PortRef::Instance { instance, .. } => format!("i:{instance}"),
            PortRef::Panel { article, slot, .. } => {
                let row = (slot.saturating_sub(1)) / per_u.unwrap_or(u32::MAX).max(1) + 1;
                format!("p:{article}:{row}")
            }
            PortRef::Adapter { article, unit, .. } => format!("a:{article}:{unit}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dir {
    In,
    Out,
    Bidir,
    /// Port de panel ou d'adaptateur : il ne fait que passer le signal.
    Through,
}

/// Ce que la compatibilité doit savoir d'un port.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PortInfo {
    pub connector: String,
    pub signal: Option<String>,
    /// Famille de signal (fonction) : V, A, N…
    pub family: Option<String>,
    pub ip: bool,
    pub dir: Dir,
}

/// Avertissements de compatibilité (section 10.2) : ils n'empêchent pas la pose.
pub fn compat(a: &PortInfo, b: &PortInfo) -> Vec<String> {
    let mut w = Vec::new();
    if matches!((a.dir, b.dir), (Dir::In, Dir::In) | (Dir::Out, Dir::Out)) {
        w.push(if a.dir == Dir::In {
            "Deux entrées reliées.".to_string()
        } else {
            "Deux sorties reliées.".to_string()
        });
    }
    // Pas de contrôle entre ports IP, ni sur un port sans signal (transit).
    let checked = !(a.ip || b.ip) && a.signal.is_some() && b.signal.is_some();
    if checked {
        if a.family.is_some() && b.family.is_some() && a.family != b.family {
            w.push("Familles de signal différentes.".into());
        } else if a.signal != b.signal {
            w.push("Signaux différents.".into());
        }
    }
    if checked && a.connector != b.connector {
        w.push("Connecteurs différents : câble mixte ou adaptateur.".into());
    }
    w
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    #[default]
    Orthogonal,
    Rounded,
    Arrow,
}

/// Champs modifiables d'un câble.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CableFields {
    pub number: Option<String>,
    pub color: Option<String>,
    pub style: Style,
    pub waypoints: Vec<(f64, f64)>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub cable_model_id: Option<String>,
    pub harness_id: Option<String>,
    /// Longueur réelle, saisie ; la longueur estimée est calculée.
    pub length_m: Option<f64>,
}

pub fn validate_cable(c: &mut CableFields) -> Vec<FieldError> {
    let mut e = Vec::new();
    let clean = |s: &Option<String>| {
        s.as_ref()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
    };
    c.number = clean(&c.number);
    c.notes = clean(&c.notes);
    c.color = clean(&c.color);
    if c.number.as_ref().is_some_and(|n| n.chars().count() > 40) {
        e.push(err("number", "40 caractères au plus."));
    }
    if c.notes.as_ref().is_some_and(|n| n.chars().count() > 2000) {
        e.push(err("notes", "2 000 caractères au plus."));
    }
    if let Some(col) = &c.color {
        let ok = col.len() == 7
            && col.starts_with('#')
            && col[1..].chars().all(|x| x.is_ascii_hexdigit());
        if !ok {
            e.push(err("color", "Couleur au format #RRVVBB."));
        }
    }
    c.tags = c
        .tags
        .iter()
        .map(|t| t.trim().chars().take(30).collect::<String>())
        .filter(|t| !t.is_empty())
        .collect();
    if c.tags.len() > 10 {
        e.push(err("tags", "10 étiquettes au plus."));
    }
    if c.waypoints.len() > 50
        || c.waypoints
            .iter()
            .any(|(x, y)| !x.is_finite() || !y.is_finite())
    {
        e.push(err("waypoints", "50 points de passage au plus."));
    }
    if c.length_m
        .is_some_and(|l| !l.is_finite() || !(0.0..=10_000.0).contains(&l))
    {
        e.push(err("length_m", "De 0 à 10 000 m."));
    }
    e
}

/// Profondeur d'un connecteur pour l'estimation (mm).
pub fn connector_depth(connector: &str) -> f64 {
    match connector {
        "bnc" | "hd-bnc" => 30.0,
        c if c.starts_with("xlr") => 45.0,
        "rj45" | "ethercon" => 35.0,
        "lc" => 25.0,
        _ => 35.0,
    }
}

/// Position d'une extrémité dans un rack : centre vertical (mm depuis le bas), face avant ou non.
#[derive(Debug, Clone, Copy)]
pub struct RackSpot {
    pub center_mm: f64,
    pub front: bool,
}

/// Longueur estimée d'un câble dans un même rack (section 10.2), en mètres :
/// V écart vertical, H trajets horizontaux (+ profondeur si faces opposées), D profondeur des
/// connecteurs, B coudes. Minimum 0,5 m, arrondi au demi-mètre supérieur.
pub fn estimate_in_rack(
    a: RackSpot,
    b: RackSpot,
    depth_mm: f64,
    conn_a: &str,
    conn_b: &str,
) -> f64 {
    let v = (a.center_mm - b.center_mm).abs();
    let same = a.front == b.front;
    let h = 2.0 * 150.0 + if same { 0.0 } else { depth_mm };
    let d = connector_depth(conn_a) + connector_depth(conn_b);
    let bends = if same { 2.0 } else { 4.0 };
    let b_ = bends * 50.0 * std::f64::consts::FRAC_PI_2;
    let m = (v + h + d + b_) / 1000.0;
    ((m.max(0.5)) * 2.0).ceil() / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_refs_round_trip() {
        for s in ["i:abc:2:7", "p:pan:17:r", "a:ad:3:in", "a:ad:1:o2.4"] {
            assert_eq!(s.parse::<PortRef>().unwrap().to_string(), s);
        }
        assert!("i:abc:x:1".parse::<PortRef>().is_err());
        assert!("p:pan:3:z".parse::<PortRef>().is_err());
        assert_eq!(
            "p:pan:17:f".parse::<PortRef>().unwrap().node(Some(16)),
            "p:pan:2"
        );
    }

    #[test]
    fn compat_warns_without_blocking() {
        let p = |dir, conn: &str, sig: Option<&str>, fam: Option<&str>| PortInfo {
            connector: conn.into(),
            signal: sig.map(str::to_string),
            family: fam.map(str::to_string),
            ip: false,
            dir,
        };
        assert!(
            compat(
                &p(Dir::Out, "bnc", Some("sdi"), Some("V")),
                &p(Dir::In, "bnc", Some("sdi"), Some("V"))
            )
            .is_empty()
        );
        let w = compat(
            &p(Dir::Out, "bnc", Some("sdi"), Some("V")),
            &p(Dir::Out, "xlr3", Some("aes"), Some("A")),
        );
        assert_eq!(w.len(), 3, "{w:?}");
        // Transit : pas de contrôle de signal ni de connecteur.
        assert!(
            compat(
                &p(Dir::Out, "bnc", Some("sdi"), Some("V")),
                &p(Dir::Through, "hd-bnc", None, None)
            )
            .is_empty()
        );
        let mut ip = p(Dir::Bidir, "rj45", Some("st2110"), Some("F"));
        ip.ip = true;
        assert!(compat(&ip, &p(Dir::Bidir, "lc", Some("eth"), Some("N"))).is_empty());
    }

    #[test]
    fn estimate_rounds_up_to_half_metre() {
        let a = RackSpot {
            center_mm: 100.0,
            front: true,
        };
        let b = RackSpot {
            center_mm: 100.0 + 10.0 * 44.45,
            front: true,
        };
        // 444,5 + 300 + 60 + 157 ≈ 961 mm → 1 m
        assert_eq!(estimate_in_rack(a, b, 600.0, "bnc", "bnc"), 1.0);
        let c = RackSpot {
            center_mm: 100.0,
            front: false,
        };
        assert_eq!(estimate_in_rack(a, c, 600.0, "bnc", "bnc"), 1.5);
        let mut f = CableFields {
            number: Some(" V-001 ".into()),
            color: Some("#12ab".into()),
            ..Default::default()
        };
        assert_eq!(validate_cable(&mut f).len(), 1);
        assert_eq!(f.number.as_deref(), Some("V-001"));
    }
}
