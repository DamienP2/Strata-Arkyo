//! Numérotation des câbles (section 10.7).
//!
//! Format standard : `[élément][sép. 1][préfixe][sens][séquence][sép. 2][suffixe]`, par
//! exemple `RGA-S10001`. Le sens vaut 1 (entrée), 2 (sortie) ou 3 (bidirectionnel) ; le bloc
//! numérique, sens compris, compte de 3 à 10 chiffres. Format personnalisé : modèle libre avec
//! `{rack}`, `{sep1}`, `{prefix}`, `{dir}`, `{seq}`, `{sep2}`, `{suffix}` et du texte.
//! Réglages : site, puis élément, puis ponctuel.

use crate::catalog::validate::FieldError;
use serde::{Deserialize, Serialize};

pub const SEPARATORS: [&str; 6] = ["", "-", ".", "_", "/", "*"];

fn err(field: impl Into<String>, message: impl Into<String>) -> FieldError {
    FieldError {
        field: field.into(),
        message: message.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NumberingRules {
    /// Segment « élément » (code câble) en tête du numéro.
    pub with_rack: bool,
    pub sep1: String,
    pub sep2: String,
    /// Longueur du bloc numérique, sens compris.
    pub digits: u32,
    /// Modèle personnalisé ; vide : format standard.
    pub template: Option<String>,
    /// Ordre des équipements : de haut en bas (sinon de bas en haut), de gauche à droite.
    pub top_down: bool,
    pub left_right: bool,
}

impl Default for NumberingRules {
    fn default() -> Self {
        NumberingRules {
            with_rack: true,
            sep1: "-".into(),
            sep2: String::new(),
            digits: 5,
            template: None,
            top_down: true,
            left_right: true,
        }
    }
}

pub fn validate_rules(r: &mut NumberingRules) -> Vec<FieldError> {
    let mut e = Vec::new();
    if !(3..=10).contains(&r.digits) {
        e.push(err("digits", "De 3 à 10 chiffres, sens compris."));
    }
    for (f, s) in [("sep1", &r.sep1), ("sep2", &r.sep2)] {
        if !SEPARATORS.contains(&s.as_str()) {
            e.push(err(f, "Séparateur : - . _ / * ou aucun."));
        }
    }
    r.template = r
        .template
        .as_ref()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty());
    if let Some(t) = &r.template {
        if t.chars().count() > 80 {
            e.push(err("template", "80 caractères au plus."));
        } else if !t.contains("{seq}") {
            e.push(err("template", "Le modèle doit contenir {seq}."));
        }
    }
    e
}

/// Chiffre du sens : 1 entrée, 2 sortie, 3 bidirectionnel ou passage.
pub fn dir_digit(dir: crate::synoptic::Dir) -> u8 {
    use crate::synoptic::Dir;
    match dir {
        Dir::In => 1,
        Dir::Out => 2,
        Dir::Bidir | Dir::Through => 3,
    }
}

/// Code d'un élément pour la numérotation : son code câble, sinon les 4 premiers caractères
/// de son nom (lettres et chiffres, en majuscules).
pub fn rack_code(cable_code: &str, name: &str) -> String {
    let c = cable_code.trim();
    if !c.is_empty() {
        return c.to_string();
    }
    crate::text::normalize(name)
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(4)
        .collect::<String>()
        .to_uppercase()
}

pub struct Parts<'a> {
    pub rack: &'a str,
    pub prefix: &'a str,
    pub dir: u8,
    pub seq: u64,
    pub suffix: &'a str,
}

pub fn format(r: &NumberingRules, p: &Parts) -> String {
    let width = r.digits.saturating_sub(1).max(1) as usize;
    let seq = format!("{:0width$}", p.seq);
    match &r.template {
        Some(t) => t
            .replace("{rack}", p.rack)
            .replace("{sep1}", &r.sep1)
            .replace("{prefix}", p.prefix)
            .replace("{dir}", &p.dir.to_string())
            .replace("{seq}", &seq)
            .replace("{sep2}", &r.sep2)
            .replace("{suffix}", p.suffix),
        None => {
            let mut s = String::new();
            if r.with_rack && !p.rack.is_empty() {
                s.push_str(p.rack);
                s.push_str(&r.sep1);
            }
            s.push_str(p.prefix);
            s.push_str(&p.dir.to_string());
            s.push_str(&seq);
            if !p.suffix.is_empty() {
                s.push_str(&r.sep2);
                s.push_str(p.suffix);
            }
            s
        }
    }
}

/// Numéro suivant : on avance le numéro manuel de départ (« V-0100 » → « V-0101 »),
/// en gardant la largeur de son dernier bloc de chiffres.
pub fn bump(first: &str, k: u64) -> Option<String> {
    let end = first.rfind(|c: char| c.is_ascii_digit())? + 1;
    let start = first[..end]
        .rfind(|c: char| !c.is_ascii_digit())
        .map(|i| i + 1)
        .unwrap_or(0);
    let n: u64 = first[start..end].parse().ok()?;
    let w = end - start;
    Some(format!("{}{:0w$}{}", &first[..start], n + k, &first[end..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_and_custom_formats() {
        let r = NumberingRules::default();
        let p = Parts {
            rack: "RGA",
            prefix: "S",
            dir: 1,
            seq: 1,
            suffix: "",
        };
        assert_eq!(format(&r, &p), "RGA-S10001");
        let r2 = NumberingRules {
            with_rack: false,
            digits: 3,
            sep2: "/".into(),
            ..Default::default()
        };
        assert_eq!(
            format(
                &r2,
                &Parts {
                    suffix: "HD",
                    seq: 7,
                    dir: 2,
                    ..p
                }
            ),
            "S207/HD"
        );
        let mut c = NumberingRules {
            template: Some("{prefix}{seq}·{rack}".into()),
            ..Default::default()
        };
        assert!(validate_rules(&mut c).is_empty());
        assert_eq!(
            format(
                &c,
                &Parts {
                    rack: "CAR",
                    prefix: "V",
                    dir: 2,
                    seq: 12,
                    suffix: ""
                }
            ),
            "V0012·CAR"
        );
        let mut bad = NumberingRules {
            digits: 2,
            sep1: "+".into(),
            template: Some("{rack}".into()),
            ..Default::default()
        };
        assert_eq!(validate_rules(&mut bad).len(), 3);
        assert_eq!(rack_code("", "Régie vidéo A"), "REGI");
        assert_eq!(rack_code("RGA", "x"), "RGA");
        assert_eq!(bump("V-0099", 2).as_deref(), Some("V-0101"));
        assert_eq!(bump("CAM12B", 1).as_deref(), Some("CAM13B"));
        assert_eq!(bump("ABC", 1), None);
    }
}
