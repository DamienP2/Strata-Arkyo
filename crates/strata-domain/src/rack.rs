//! Racks (cahier des charges, section 9) : placement au demi-U, collisions, ajustement
//! de la position, profondeur utile, bilan électrique par circuit et par phase.

use crate::catalog::Width;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RackFace {
    Front,
    Rear,
    /// Rails arrière internes : seulement si le contenant en a.
    RearInner,
}

pub fn columns(w: Width) -> u32 {
    match w {
        Width::Full => 1,
        Width::Half => 2,
        Width::Third => 3,
        Width::Quarter => 4,
    }
}

/// Un élément posé (ou à poser) dans une élévation.
#[derive(Debug, Clone, PartialEq)]
pub struct Slot {
    pub id: String,
    pub label: String,
    pub face: RackFace,
    /// Position de départ, 1 = bas du rack.
    pub u_start: i64,
    pub height_u: f64,
    pub width: Width,
    pub col: u32,
}

impl Slot {
    fn u_range(&self) -> (f64, f64) {
        let a = (self.u_start - 1) as f64;
        (a, a + self.height_u)
    }
    fn h_range(&self) -> (f64, f64) {
        let n = columns(self.width) as f64;
        (self.col as f64 / n, (self.col + 1) as f64 / n)
    }
    /// Même face, U qui se chevauchent et largeur qui se chevauche strictement.
    pub fn collides(&self, o: &Slot) -> bool {
        if self.face != o.face {
            return false;
        }
        let (a0, a1) = self.u_range();
        let (b0, b1) = o.u_range();
        let (h0, h1) = self.h_range();
        let (g0, g1) = o.h_range();
        a0 < b1 - 1e-9 && b0 < a1 - 1e-9 && h0 < g1 - 1e-9 && g0 < h1 - 1e-9
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum PlaceError {
    TooBig { message: String },
    Occupied { message: String, by: String },
    BadColumn { message: String },
}

impl PlaceError {
    pub fn message(&self) -> &str {
        match self {
            PlaceError::TooBig { message }
            | PlaceError::Occupied { message, .. }
            | PlaceError::BadColumn { message } => message,
        }
    }
}

/// Vérifie une position : dans le rack, colonne valide, sans chevauchement.
pub fn check_place(rack_u: f64, s: &Slot, others: &[Slot]) -> Result<(), PlaceError> {
    if s.col >= columns(s.width) {
        return Err(PlaceError::BadColumn {
            message: format!("Colonne de 1 à {} pour cette largeur.", columns(s.width)),
        });
    }
    if s.u_start < 1 || (s.u_start - 1) as f64 + s.height_u > rack_u + 1e-9 {
        return Err(PlaceError::TooBig {
            message: "L'élément dépasse la taille du rack.".into(),
        });
    }
    if let Some(o) = others.iter().find(|o| o.id != s.id && s.collides(o)) {
        return Err(PlaceError::Occupied {
            message: format!("Position occupée par « {} ».", o.label),
            by: o.id.clone(),
        });
    }
    Ok(())
}

/// Ajuste une position demandée : un dépôt trop haut est ramené sous le haut du rack ;
/// un élément de plusieurs U cherche la place libre la plus proche (± hauteur − 1,
/// en alternant bas et haut). Renvoie la position retenue, ou l'erreur de la demande.
pub fn adjust(rack_u: f64, s: &Slot, others: &[Slot]) -> Result<i64, PlaceError> {
    let span = s.height_u.ceil() as i64;
    let top = (rack_u.floor() as i64 - span + 1).max(1);
    let wanted = s.u_start.clamp(1, top);
    let mut t = s.clone();
    t.u_start = wanted;
    if check_place(rack_u, &t, others).is_ok() {
        return Ok(wanted);
    }
    for d in 1..span.max(1) {
        for cand in [wanted - d, wanted + d] {
            if cand >= 1 && cand <= top {
                t.u_start = cand;
                if check_place(rack_u, &t, others).is_ok() {
                    return Ok(cand);
                }
            }
        }
    }
    t.u_start = wanted;
    check_place(rack_u, &t, others).map(|_| wanted)
}

/// Profondeur (avertissement non bloquant) : profondeur de l'élément plus sa réserve de
/// câbles, comparée à la profondeur utile du rack.
pub fn depth_warning(
    rack_depth_mm: Option<f64>,
    item_depth_mm: f64,
    cable_reserve_mm: f64,
) -> Option<String> {
    let rack = rack_depth_mm?;
    let total = item_depth_mm + cable_reserve_mm;
    if item_depth_mm > 0.0 && total > rack {
        let f = |v: f64| format!("{v:.0}");
        return Some(format!(
            "{} mm + {} mm câbles = {} mm dépasse la profondeur utile ({} mm).",
            f(item_depth_mm),
            f(cable_reserve_mm),
            f(total),
            f(rack)
        ));
    }
    None
}

// ----- Bilan électrique -----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum Phase {
    L1,
    L2,
    L3,
}

pub const VOLTAGE: f64 = 230.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CircuitSpec {
    pub id: String,
    pub name: String,
    pub phase: Phase,
    /// Calibre : 16 ou 32 A.
    pub rating_a: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CircuitLoad {
    pub id: String,
    pub name: String,
    pub phase: Phase,
    pub rating_a: f64,
    pub watts: f64,
    pub amps: f64,
    pub over: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhaseLoad {
    pub phase: Phase,
    pub watts: f64,
    pub amps: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Electrical {
    pub circuits: Vec<CircuitLoad>,
    pub phases: Vec<PhaseLoad>,
    /// Écart entre la phase la plus chargée et la moins chargée, en % de la plus chargée.
    pub imbalance_pct: f64,
    pub unbalanced: bool,
    /// Puissance des éléments posés sans circuit.
    pub unassigned_watts: f64,
}

/// Courant par circuit et par phase. `loads` : (circuit, watts) de chaque élément branché.
pub fn electrical(circuits: &[CircuitSpec], loads: &[(Option<String>, f64)]) -> Electrical {
    let round = |v: f64| (v * 10.0).round() / 10.0;
    let mut out: Vec<CircuitLoad> = circuits
        .iter()
        .map(|c| {
            let w: f64 = loads
                .iter()
                .filter(|(id, _)| id.as_deref() == Some(c.id.as_str()))
                .map(|(_, w)| w)
                .sum();
            let a = w / VOLTAGE;
            CircuitLoad {
                id: c.id.clone(),
                name: c.name.clone(),
                phase: c.phase,
                rating_a: c.rating_a,
                watts: w.round(),
                amps: round(a),
                over: a > c.rating_a,
            }
        })
        .collect();
    out.sort_by(|a, b| a.phase.cmp(&b.phase).then(a.name.cmp(&b.name)));
    let phases: Vec<PhaseLoad> = [Phase::L1, Phase::L2, Phase::L3]
        .into_iter()
        .map(|p| {
            let w: f64 = out.iter().filter(|c| c.phase == p).map(|c| c.watts).sum();
            PhaseLoad {
                phase: p,
                watts: w,
                amps: round(w / VOLTAGE),
            }
        })
        .collect();
    let used: Vec<f64> = phases
        .iter()
        .filter(|p| circuits.iter().any(|c| c.phase == p.phase))
        .map(|p| p.amps)
        .collect();
    let (max, min) = used
        .iter()
        .fold((0.0f64, f64::MAX), |(mx, mn), &v| (mx.max(v), mn.min(v)));
    let imbalance = if used.len() > 1 && max > 0.0 {
        ((max - min) / max * 100.0).round()
    } else {
        0.0
    };
    let unassigned: f64 = loads
        .iter()
        .filter(|(id, _)| {
            id.as_ref()
                .is_none_or(|i| !circuits.iter().any(|c| &c.id == i))
        })
        .map(|(_, w)| w)
        .sum();
    Electrical {
        circuits: out,
        phases,
        imbalance_pct: imbalance,
        // Signalé au-delà de 30 % d'écart, quand la phase la plus chargée dépasse 2 A.
        unbalanced: imbalance > 30.0 && max > 2.0,
        unassigned_watts: unassigned.round(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(id: &str, u: i64, h: f64, w: Width, col: u32) -> Slot {
        Slot {
            id: id.into(),
            label: id.into(),
            face: RackFace::Front,
            u_start: u,
            height_u: h,
            width: w,
            col,
        }
    }

    #[test]
    fn collisions_and_bounds() {
        let others = vec![
            s("mix", 10, 2.0, Width::Full, 0),
            s("half", 20, 1.0, Width::Half, 0),
        ];
        assert!(
            check_place(42.0, &s("a", 12, 1.0, Width::Full, 0), &others).is_ok(),
            "contigu au-dessus"
        );
        assert!(
            check_place(42.0, &s("a", 9, 1.0, Width::Full, 0), &others).is_ok(),
            "contigu au-dessous"
        );
        assert!(matches!(
            check_place(42.0, &s("a", 11, 1.0, Width::Full, 0), &others),
            Err(PlaceError::Occupied { .. })
        ));
        assert!(
            check_place(42.0, &s("b", 20, 1.0, Width::Half, 1), &others).is_ok(),
            "l'autre moitié est libre"
        );
        assert!(matches!(
            check_place(42.0, &s("c", 20, 1.0, Width::Third, 1), &others),
            Err(PlaceError::Occupied { .. })
        ));
        assert!(matches!(
            check_place(42.0, &s("d", 42, 2.0, Width::Full, 0), &others),
            Err(PlaceError::TooBig { .. })
        ));
        assert!(
            check_place(42.0, &s("e", 41, 1.5, Width::Full, 0), &others).is_ok(),
            "demi-U"
        );
        let mut rear = s("f", 10, 2.0, Width::Full, 0);
        rear.face = RackFace::Rear;
        assert!(check_place(42.0, &rear, &others).is_ok(), "autre face");
    }

    #[test]
    fn adjustment() {
        let others = vec![s("mix", 10, 2.0, Width::Full, 0)];
        assert_eq!(
            adjust(42.0, &s("a", 50, 3.0, Width::Full, 0), &others).unwrap(),
            40,
            "ramené sous le haut"
        );
        assert_eq!(
            adjust(42.0, &s("a", 11, 2.0, Width::Full, 0), &others).unwrap(),
            12,
            "place libre la plus proche"
        );
        assert_eq!(
            adjust(42.0, &s("a", 10, 3.0, Width::Full, 0), &others).unwrap(),
            12,
            "en alternant bas et haut"
        );
        assert!(
            adjust(42.0, &s("a", 10, 1.0, Width::Full, 0), &others).is_err(),
            "1 U : pas de recherche"
        );
    }

    #[test]
    fn electrical_balance() {
        let c = |id: &str, p: Phase| CircuitSpec {
            id: id.into(),
            name: id.into(),
            phase: p,
            rating_a: 16.0,
        };
        let e = electrical(
            &[c("a", Phase::L1), c("b", Phase::L2)],
            &[
                (Some("a".into()), 2300.0),
                (Some("a".into()), 1840.0),
                (Some("b".into()), 460.0),
                (None, 100.0),
            ],
        );
        assert_eq!(e.circuits[0].amps, 18.0);
        assert!(e.circuits[0].over);
        assert_eq!(e.phases[1].amps, 2.0);
        assert!(e.unbalanced);
        assert_eq!(e.unassigned_watts, 100.0);
        assert!(depth_warning(Some(600.0), 450.0, 150.0).is_none());
        assert_eq!(
            depth_warning(Some(600.0), 560.0, 150.0).unwrap(),
            "560 mm + 150 mm câbles = 710 mm dépasse la profondeur utile (600 mm)."
        );
    }
}
