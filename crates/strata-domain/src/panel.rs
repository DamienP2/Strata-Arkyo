//! Panels de brassage (sections 6.3 et 9.5).
//!
//! Un panel compte `hauteur × emplacements par U` emplacements, numérotés à partir de 1,
//! rangée par rangée, de haut en bas et de gauche à droite. Un connecteur occupe un ou
//! plusieurs emplacements consécutifs d'une même rangée. L'étiquetage se fait par rangée :
//! des groupes de ports colorés, puis un libellé par port à l'intérieur de son groupe.

use crate::catalog::validate::FieldError;
use crate::text::normalize;
use serde::{Deserialize, Serialize};

pub const PER_U: [u32; 4] = [8, 12, 16, 24];
/// Au-delà de cette densité, la coque standard (format D) ne tient plus.
pub const D_FORMAT_MAX_PER_U: u32 = 16;

fn err(field: impl Into<String>, message: impl Into<String>) -> FieldError {
    FieldError {
        field: field.into(),
        message: message.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelSlot {
    /// Premier emplacement occupé, à partir de 1.
    pub index: u32,
    /// Connecteur du référentiel.
    pub connector: String,
    pub variant: Option<String>,
    pub label: String,
    pub notes: Option<String>,
    /// Largeur en emplacements.
    pub width: u32,
    /// Famille de signal : fixe la couleur d'un connecteur multi-familles.
    pub function: Option<String>,
    /// Coque standard (format D).
    pub d_format: bool,
}

impl Default for PanelSlot {
    fn default() -> Self {
        PanelSlot {
            index: 1,
            connector: String::new(),
            variant: None,
            label: String::new(),
            notes: None,
            width: 1,
            function: None,
            d_format: false,
        }
    }
}

/// Groupe de ports colorés d'une rangée (étiquetage, section 9.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelGroup {
    /// Rangée, à partir de 1.
    pub row: u32,
    /// Première colonne, à partir de 1.
    pub start: u32,
    pub len: u32,
    pub label: String,
    pub color: String,
}

impl Default for PanelGroup {
    fn default() -> Self {
        PanelGroup {
            row: 1,
            start: 1,
            len: 1,
            label: String::new(),
            color: "#2459E0".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelData {
    pub name: String,
    pub height_u: u32,
    pub per_u: u32,
    pub rear_depth_mm: u32,
    pub notes: Option<String>,
    pub slots: Vec<PanelSlot>,
    pub groups: Vec<PanelGroup>,
}

impl Default for PanelData {
    fn default() -> Self {
        PanelData {
            name: String::new(),
            height_u: 1,
            per_u: 16,
            rear_depth_mm: 80,
            notes: None,
            slots: Vec::new(),
            groups: Vec::new(),
        }
    }
}

/// Ce que le validateur doit savoir d'un connecteur du référentiel.
#[derive(Debug, Clone, Default)]
pub struct ConnectorInfo {
    pub variants: Vec<String>,
    pub d_format: bool,
}

/// Mode de pose d'un connecteur (section 6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "mode", content = "count")]
pub enum Fill {
    One,
    Row,
    All,
    Count(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PanelStats {
    pub total: u32,
    pub used: u32,
    pub free: u32,
    /// Taux de remplissage, de 0 à 100.
    pub fill_pct: u32,
}

impl PanelData {
    pub fn total(&self) -> u32 {
        self.height_u * self.per_u
    }

    pub fn rows(&self) -> u32 {
        self.height_u
    }

    /// Rangée (à partir de 1) d'un emplacement.
    pub fn row_of(&self, index: u32) -> u32 {
        (index - 1) / self.per_u + 1
    }

    /// Emplacements occupés, un booléen par emplacement (indice 0 = emplacement 1).
    pub fn occupancy(&self) -> Vec<bool> {
        let mut o = vec![false; self.total() as usize];
        for s in &self.slots {
            for i in s.index..s.index + s.width.max(1) {
                if let Some(c) = o.get_mut(i as usize - 1) {
                    *c = true;
                }
            }
        }
        o
    }

    pub fn stats(&self) -> PanelStats {
        let total = self.total();
        let used = self.occupancy().iter().filter(|b| **b).count() as u32;
        PanelStats {
            total,
            used,
            free: total - used,
            fill_pct: (used * 100 + total / 2).checked_div(total).unwrap_or(0),
        }
    }

    /// L'emplacement `index` peut-il recevoir un connecteur de `width` emplacements ?
    /// `ignore` écarte un connecteur existant (déplacement).
    pub fn fits(&self, index: u32, width: u32, ignore: Option<usize>) -> bool {
        let w = width.max(1);
        if index == 0 || index + w - 1 > self.total() {
            return false;
        }
        if self.row_of(index) != self.row_of(index + w - 1) {
            return false;
        }
        !self.slots.iter().enumerate().any(|(k, s)| {
            Some(k) != ignore && index < s.index + s.width.max(1) && s.index < index + w
        })
    }

    /// Pose `proto` à partir de `start` selon `mode`, en sautant les emplacements occupés.
    /// Renvoie le nombre de connecteurs posés.
    pub fn fill(&mut self, start: u32, mode: Fill, proto: &PanelSlot) -> u32 {
        let w = proto.width.max(1);
        let total = self.total();
        if start == 0 || start > total {
            return 0;
        }
        let (end, max) = match mode {
            Fill::One => (total, 1),
            Fill::Row => (self.row_of(start) * self.per_u, u32::MAX),
            Fill::All => (total, u32::MAX),
            Fill::Count(n) => (total, n),
        };
        let mut placed = 0;
        let mut i = start;
        while i <= end && placed < max {
            if i + w - 1 <= end && self.fits(i, w, None) {
                self.slots.push(PanelSlot {
                    index: i,
                    ..proto.clone()
                });
                placed += 1;
                i += w;
            } else {
                i += 1;
            }
        }
        self.slots.sort_by_key(|s| s.index);
        placed
    }

    /// Déplace le connecteur posé en `from` vers `to` ; refus si la place est prise.
    pub fn move_slot(&mut self, from: u32, to: u32) -> bool {
        let Some(k) = self.slots.iter().position(|s| s.index == from) else {
            return false;
        };
        if !self.fits(to, self.slots[k].width, Some(k)) {
            return false;
        }
        self.slots[k].index = to;
        self.slots.sort_by_key(|s| s.index);
        true
    }

    /// Change la hauteur ou la densité : tous les connecteurs et groupes sont effacés.
    pub fn resize(&mut self, height_u: u32, per_u: u32) {
        if (height_u, per_u) != (self.height_u, self.per_u) {
            self.height_u = height_u;
            self.per_u = per_u;
            self.slots.clear();
            self.groups.clear();
        }
    }
}

/// Valide et normalise un panel. Le format D est retiré quand il n'est plus possible
/// (densité trop forte ou connecteur qui n'existe pas en format D).
pub fn validate_panel(
    p: &mut PanelData,
    connector: &dyn Fn(&str) -> Option<ConnectorInfo>,
) -> Vec<FieldError> {
    let mut e = Vec::new();
    p.name = p.name.trim().to_string();
    if p.name.is_empty() {
        e.push(err("name", "Le nom est obligatoire."));
    } else if p.name.chars().count() > 100 {
        e.push(err("name", "100 caractères au plus."));
    }
    if !(1..=4).contains(&p.height_u) {
        e.push(err("height_u", "De 1 à 4 U."));
    }
    if !PER_U.contains(&p.per_u) {
        e.push(err("per_u", "8, 12, 16 ou 24 emplacements par U."));
    }
    if !(30..=300).contains(&p.rear_depth_mm) {
        e.push(err("rear_depth_mm", "De 30 à 300 mm."));
    }
    p.notes = p
        .notes
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if !e.is_empty() {
        return e;
    }

    p.slots.sort_by_key(|s| s.index);
    let snapshot = p.clone();
    for (k, s) in p.slots.iter_mut().enumerate() {
        let f = |name: &str| format!("slots.{k}.{name}");
        s.width = s.width.max(1);
        s.label = s.label.trim().to_string();
        s.notes = s
            .notes
            .as_ref()
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty());
        if s.label.chars().count() > 40 {
            e.push(err(f("label"), "40 caractères au plus."));
        }
        if !snapshot.fits(s.index, s.width, Some(k)) {
            e.push(err(
                f("index"),
                format!("Emplacement {} : déjà occupé ou hors du panel.", s.index),
            ));
        }
        match connector(&s.connector) {
            None => e.push(err(f("connector"), "Connecteur inconnu.")),
            Some(info) => {
                if let Some(v) = &s.variant {
                    if !info.variants.iter().any(|x| x == v) {
                        s.variant = None;
                    }
                }
                if s.d_format && (!info.d_format || p.per_u > D_FORMAT_MAX_PER_U) {
                    s.d_format = false;
                }
            }
        }
    }

    for (k, g) in p.groups.iter_mut().enumerate() {
        g.label = g.label.trim().to_string();
        let end = g.start + g.len.max(1) - 1;
        if g.row == 0 || g.row > p.height_u || g.start == 0 || end > p.per_u {
            e.push(err(format!("groups.{k}"), "Groupe hors de la rangée."));
        }
    }
    let mut seen: Vec<(u32, u32, u32)> = Vec::new();
    for (k, g) in p.groups.iter().enumerate() {
        let end = g.start + g.len.max(1) - 1;
        if seen
            .iter()
            .any(|(r, a, b)| *r == g.row && g.start <= *b && *a <= end)
        {
            e.push(err(format!("groups.{k}"), "Deux groupes se chevauchent."));
        }
        seen.push((g.row, g.start, end));
    }
    p.groups.sort_by_key(|g| (g.row, g.start));
    e
}

/// Nom unique parmi les panels de la bibliothèque, sans tenir compte de la casse.
pub fn same_name(a: &str, b: &str) -> bool {
    normalize(a) == normalize(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn(id: &str) -> Option<ConnectorInfo> {
        match id {
            "xlr3" => Some(ConnectorInfo {
                variants: vec![],
                d_format: true,
            }),
            "bnc" => Some(ConnectorInfo {
                variants: vec!["75 Ω".into()],
                d_format: true,
            }),
            "lc" => Some(ConnectorInfo::default()),
            _ => None,
        }
    }

    fn panel(per_u: u32, h: u32) -> PanelData {
        PanelData {
            name: "Panel".into(),
            height_u: h,
            per_u,
            ..Default::default()
        }
    }

    fn proto(c: &str, w: u32) -> PanelSlot {
        PanelSlot {
            connector: c.into(),
            width: w,
            ..Default::default()
        }
    }

    #[test]
    fn fill_skips_occupied_and_stays_in_row() {
        let mut p = panel(8, 2);
        assert_eq!(p.fill(3, Fill::One, &proto("xlr3", 1)), 1);
        assert_eq!(
            p.fill(1, Fill::Row, &proto("bnc", 1)),
            7,
            "saute l'emplacement 3"
        );
        assert_eq!(p.stats().used, 8);
        assert_eq!(p.fill(9, Fill::Count(3), &proto("lc", 2)), 3);
        assert_eq!(
            p.slots
                .iter()
                .filter(|s| s.connector == "lc")
                .map(|s| s.index)
                .collect::<Vec<_>>(),
            vec![9, 11, 13]
        );
        // Il reste les emplacements 15 et 16 : un connecteur double tient, pas deux.
        assert_eq!(p.fill(15, Fill::All, &proto("lc", 2)), 1);
        assert_eq!(
            p.stats(),
            PanelStats {
                total: 16,
                used: 16,
                free: 0,
                fill_pct: 100
            }
        );
        // Un connecteur large ne déborde pas sur la rangée suivante.
        let mut q = panel(8, 2);
        assert_eq!(q.fill(8, Fill::All, &proto("lc", 2)), 4);
        assert_eq!(q.slots[0].index, 9);
    }

    #[test]
    fn move_refuses_taken_slots() {
        let mut p = panel(12, 1);
        p.fill(1, Fill::Count(2), &proto("xlr3", 1));
        assert!(!p.move_slot(1, 2));
        assert!(p.move_slot(1, 5));
        assert_eq!(
            p.slots.iter().map(|s| s.index).collect::<Vec<_>>(),
            vec![2, 5]
        );
        p.resize(1, 12);
        assert_eq!(p.slots.len(), 2, "même taille : rien n'est effacé");
        p.resize(2, 12);
        assert!(p.slots.is_empty());
    }

    #[test]
    fn validation_cancels_d_format_and_checks_groups() {
        let mut p = panel(16, 1);
        p.fill(
            1,
            Fill::Count(2),
            &PanelSlot {
                d_format: true,
                ..proto("xlr3", 1)
            },
        );
        p.fill(
            3,
            Fill::One,
            &PanelSlot {
                d_format: true,
                ..proto("lc", 1)
            },
        );
        assert!(validate_panel(&mut p, &conn).is_empty());
        assert_eq!(
            p.slots.iter().map(|s| s.d_format).collect::<Vec<_>>(),
            vec![true, true, false]
        );
        p.per_u = 24;
        assert!(validate_panel(&mut p, &conn).is_empty());
        assert!(
            p.slots.iter().all(|s| !s.d_format),
            "24 par U : pas de format D"
        );

        p.groups = vec![
            PanelGroup {
                row: 1,
                start: 1,
                len: 4,
                ..Default::default()
            },
            PanelGroup {
                row: 1,
                start: 4,
                len: 2,
                ..Default::default()
            },
            PanelGroup {
                row: 1,
                start: 23,
                len: 3,
                ..Default::default()
            },
        ];
        let e = validate_panel(&mut p, &conn);
        assert_eq!(e.len(), 2, "{e:?}");

        let mut bad = panel(10, 5);
        bad.name = " ".into();
        assert_eq!(validate_panel(&mut bad, &conn).len(), 3);

        let mut over = panel(8, 1);
        over.slots = vec![
            proto("xlr3", 1),
            PanelSlot {
                index: 1,
                ..proto("zz", 1)
            },
        ];
        let e = validate_panel(&mut over, &conn);
        assert!(e.iter().any(|x| x.field == "slots.1.connector"));
        assert!(e.iter().any(|x| x.field.ends_with(".index")));
    }
}
