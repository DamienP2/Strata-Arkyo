//! Préparation et livraison (section 11.4).
//!
//! Les lignes se déduisent de l'inventaire et du câblage : un élément (unité de colisage),
//! un modèle d'équipement (tous ses exemplaires), un modèle d'adaptateur, de panel ou
//! d'accessoire, un type de câble (modèle, connecteurs et genres, longueur). Seuls le statut,
//! l'approvisionnement, la réserve et la note sont saisis.

use crate::catalog::validate::FieldError;
use serde::{Deserialize, Serialize};

fn err(field: impl Into<String>, message: impl Into<String>) -> FieldError {
    FieldError {
        field: field.into(),
        message: message.into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Elements,
    Equipment,
    Others,
    Cables,
}

impl Family {
    pub fn label(self) -> &'static str {
        match self {
            Family::Elements => "Éléments",
            Family::Equipment => "Équipements",
            Family::Others => "Adaptateurs, panels et accessoires",
            Family::Cables => "Câbles",
        }
    }

    /// Statuts possibles, dans l'ordre ; le dernier vaut « prêt ».
    pub fn statuses(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Family::Elements | Family::Others => &[
                ("to_prepare", "À préparer"),
                ("to_order", "À commander"),
                ("ordered", "Commandé"),
                ("received", "Réceptionné"),
                ("ready", "Prêt"),
            ],
            Family::Equipment => &[
                ("to_prepare", "À préparer"),
                ("to_order", "À commander"),
                ("ordered", "Commandé"),
                ("to_configure", "À configurer"),
                ("operational", "Opérationnel"),
            ],
            Family::Cables => &[
                ("to_order", "À commander"),
                ("ordered", "Commandé"),
                ("received", "Réceptionné"),
                ("made", "Confectionné"),
                ("cabled", "Câblé"),
            ],
        }
    }

    pub fn first(self) -> &'static str {
        self.statuses()[0].0
    }

    pub fn done(self) -> &'static str {
        self.statuses().last().map(|s| s.0).unwrap_or("")
    }

    pub fn of_key(key: &str) -> Option<Family> {
        match key.split(':').next()? {
            "element" => Some(Family::Elements),
            "equipment" => Some(Family::Equipment),
            "adapter" | "panel" | "accessory" => Some(Family::Others),
            "cable" => Some(Family::Cables),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Supply {
    #[default]
    Stock,
    Purchase,
    Rental,
}

/// Saisie d'une ligne : tout est facultatif, seul ce qui est fourni change.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LineState {
    pub status: Option<String>,
    pub supply: Option<Supply>,
    pub reserve: Option<i64>,
    pub note: Option<String>,
}

pub fn validate_line(family: Family, s: &mut LineState) -> Vec<FieldError> {
    let mut e = Vec::new();
    if let Some(st) = &s.status {
        if !family.statuses().iter().any(|(k, _)| k == st) {
            e.push(err("status", "Statut inconnu pour cette famille."));
        }
    }
    if s.reserve.is_some_and(|r| !(0..=9999).contains(&r)) {
        e.push(err(
            "reserve",
            "La réserve va de 0 à 9 999 ; la quantité ne descend jamais sous le minimum calculé.",
        ));
    }
    s.note = s.note.as_ref().map(|n| n.trim().to_string());
    if s.note.as_ref().is_some_and(|n| n.chars().count() > 1000) {
        e.push(err("note", "1 000 caractères au plus."));
    }
    e
}

/// Genre de la fiche qui se branche sur un port de châssis : l'opposé (neutre reste neutre).
pub fn plug_gender(port: Option<&str>) -> &'static str {
    match port {
        Some("F") => "M",
        Some("M") => "F",
        _ => "N",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_depend_on_the_family() {
        assert_eq!(Family::of_key("cable:x"), Some(Family::Cables));
        assert_eq!(Family::Equipment.done(), "operational");
        assert_eq!(Family::Cables.first(), "to_order");
        let mut ok = LineState {
            status: Some("made".into()),
            reserve: Some(2),
            ..Default::default()
        };
        assert!(validate_line(Family::Cables, &mut ok).is_empty());
        let mut bad = LineState {
            status: Some("made".into()),
            reserve: Some(-1),
            ..Default::default()
        };
        assert_eq!(validate_line(Family::Equipment, &mut bad).len(), 2);
        assert_eq!(plug_gender(Some("F")), "M");
    }
}
