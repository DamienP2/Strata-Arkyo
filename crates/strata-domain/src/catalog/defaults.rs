//! Pack de départ du catalogue : fonctions et phases fournies (section 5.5), catégories
//! système, connecteurs et signaux courants du broadcast, préréglages de dimensions.
//! Chaque entrée a un identifiant stable, ce qui permet de restaurer les valeurs d'origine.

use super::model::RefKind;
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct DefaultRef {
    pub kind: RefKind,
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub data: Value,
    /// Fourni : modifiable, jamais supprimé.
    pub builtin: bool,
    /// Protégé : ni modifiable ni supprimable.
    pub protected: bool,
}

fn r(kind: RefKind, id: &str, name: &str, data: Value) -> DefaultRef {
    DefaultRef {
        kind,
        id: id.into(),
        name: name.into(),
        parent_id: None,
        data,
        builtin: false,
        protected: false,
    }
}

/// Fonctions fournies : clé, libellé, couleur, préfixe de câble.
const FUNCTIONS: &[(&str, &str, &str)] = &[
    ("V", "Vidéo", "#2459E0"),
    ("A", "Audio", "#0E8550"),
    ("L", "Lumières", "#B78400"),
    ("C", "Contrôle", "#7A44C9"),
    ("P", "Énergie", "#CF3A2F"),
    ("N", "Data", "#0A8797"),
    ("S", "Synchronisation", "#BD3489"),
    ("R", "RF", "#D06006"),
    ("I", "Intercom", "#69788C"),
    ("F", "Flux IP", "#1F3DF0"),
    ("X", "Autres", "#8A919B"),
];

const PHASES: &[(&str, &str)] = &[
    ("acquisition", "Acquisition"),
    ("production", "Production"),
    ("post-production", "Post-production"),
    ("contribution", "Contribution"),
    ("distribution", "Distribution"),
    ("conversion", "Conversion"),
    ("monitoring", "Monitoring"),
    ("stockage", "Stockage"),
    ("infrastructure", "Infrastructure"),
    ("autre", "Autre"),
];

/// Connecteurs : identifiant (= symbole), nom, variantes, couleur, fonctions.
type ConnectorRow = (
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static str,
    &'static [&'static str],
);
/// Connecteurs vendus en coque standard (format D) pour les panels.
const D_FORMAT: &[&str] = &[
    "bnc",
    "hd-bnc",
    "xlr3",
    "xlr4",
    "xlr5",
    "jack635",
    "rca",
    "rj45",
    "ethercon",
    "lc",
    "hdmi",
    "usb",
    "powercon",
    "powercon-true1",
    "opticalcon-duo",
    "sc",
    "toslink",
];

const CONNECTORS: &[ConnectorRow] = &[
    (
        "bnc",
        "BNC",
        &["75 Ω", "50 Ω"],
        "#2459E0",
        &["V", "S", "A", "R"],
    ),
    ("hd-bnc", "HD-BNC", &["75 Ω"], "#2459E0", &["V", "S"]),
    ("din-1023", "DIN 1.0/2.3", &["75 Ω"], "#2459E0", &["V", "S"]),
    ("xlr3", "XLR 3", &[], "#0E8550", &["A", "S"]),
    ("xlr4", "XLR 4", &[], "#69788C", &["I", "P"]),
    ("xlr5", "XLR 5", &[], "#B78400", &["L"]),
    ("jack635", "Jack 6,35", &["TS", "TRS"], "#0E8550", &["A"]),
    ("jack35", "Jack 3,5", &["TRS", "TRRS"], "#0E8550", &["A"]),
    ("rca", "RCA", &[], "#0E8550", &["A", "V"]),
    (
        "dsub25",
        "D-Sub 25",
        &["Tascam", "Yamaha"],
        "#0E8550",
        &["A"],
    ),
    ("dsub9", "D-Sub 9", &["RS-422", "RS-232"], "#7A44C9", &["C"]),
    (
        "rj45",
        "RJ45",
        &["Cat 5e", "Cat 6", "Cat 6a"],
        "#0A8797",
        &["N", "F", "A", "C", "I"],
    ),
    (
        "ethercon",
        "etherCON",
        &["Cat 5e", "Cat 6a"],
        "#0A8797",
        &["N", "F", "A", "C", "I", "L"],
    ),
    (
        "lc",
        "LC duplex",
        &["Monomode", "Multimode"],
        "#0A8797",
        &["N", "F", "V"],
    ),
    (
        "sc",
        "SC",
        &["Monomode", "Multimode"],
        "#0A8797",
        &["N", "V"],
    ),
    (
        "opticalcon-duo",
        "opticalCON DUO",
        &["Monomode", "Multimode"],
        "#0A8797",
        &["N", "F", "V"],
    ),
    (
        "opticalcon-quad",
        "opticalCON QUAD",
        &["Monomode", "Multimode"],
        "#0A8797",
        &["N", "F", "V"],
    ),
    (
        "smpte311",
        "SMPTE 311 (fibre caméra)",
        &[],
        "#2459E0",
        &["V"],
    ),
    ("hdmi", "HDMI", &[], "#2459E0", &["V", "A"]),
    ("displayport", "DisplayPort", &[], "#2459E0", &["V"]),
    ("usb", "USB-C", &[], "#7A44C9", &["C", "N", "P"]),
    ("toslink", "Toslink", &[], "#0E8550", &["A"]),
    (
        "speakon",
        "speakON",
        &["NL2", "NL4", "NL8"],
        "#0E8550",
        &["A"],
    ),
    ("powercon", "powerCON", &[], "#CF3A2F", &["P"]),
    ("powercon-true1", "powerCON TRUE1", &[], "#CF3A2F", &["P"]),
    ("iec-c13", "IEC C13 / C14", &[], "#CF3A2F", &["P"]),
    ("schuko", "Schuko", &[], "#CF3A2F", &["P"]),
    ("cee16-1p", "CEE 16 A monophasé", &[], "#CF3A2F", &["P"]),
    ("cee16-3p", "CEE 16 A triphasé", &[], "#CF3A2F", &["P"]),
    ("cee32-3p", "CEE 32 A triphasé", &[], "#CF3A2F", &["P"]),
    ("sma", "SMA", &[], "#D06006", &["R"]),
    ("n-type", "Type N", &[], "#D06006", &["R"]),
];

/// Signaux : identifiant, nom, fonction, impédance, bande passante minimale (MHz), connecteurs,
/// IP, multicast, redondance, préfixe.
#[allow(clippy::type_complexity)]
const SIGNALS: &[(
    &str,
    &str,
    &str,
    Option<f64>,
    Option<f64>,
    &[&str],
    bool,
    bool,
    Option<&str>,
    &str,
)] = &[
    (
        "sdi-12g",
        "SDI 12G",
        "V",
        Some(75.0),
        Some(12000.0),
        &["bnc", "hd-bnc", "din-1023"],
        false,
        false,
        None,
        "V",
    ),
    (
        "sdi-3g",
        "SDI 3G",
        "V",
        Some(75.0),
        Some(3000.0),
        &["bnc", "hd-bnc", "din-1023"],
        false,
        false,
        None,
        "V",
    ),
    (
        "sdi-hd",
        "SDI HD",
        "V",
        Some(75.0),
        Some(1500.0),
        &["bnc", "hd-bnc", "din-1023"],
        false,
        false,
        None,
        "V",
    ),
    (
        "camera-fibre",
        "Fibre caméra SMPTE",
        "V",
        None,
        None,
        &["smpte311"],
        false,
        false,
        None,
        "V",
    ),
    (
        "hdmi",
        "HDMI",
        "V",
        None,
        None,
        &["hdmi"],
        false,
        false,
        None,
        "V",
    ),
    (
        "ref-sync",
        "Référence de synchro",
        "S",
        Some(75.0),
        Some(100.0),
        &["bnc", "din-1023"],
        false,
        false,
        None,
        "S",
    ),
    (
        "timecode",
        "Timecode LTC",
        "S",
        None,
        None,
        &["xlr3", "bnc"],
        false,
        false,
        None,
        "S",
    ),
    (
        "aes3",
        "AES3",
        "A",
        Some(110.0),
        Some(6.0),
        &["xlr3", "dsub25"],
        false,
        false,
        None,
        "A",
    ),
    (
        "aes3id",
        "AES3id",
        "A",
        Some(75.0),
        Some(6.0),
        &["bnc"],
        false,
        false,
        None,
        "A",
    ),
    (
        "audio-analog",
        "Audio analogique",
        "A",
        None,
        None,
        &["xlr3", "jack635", "jack35", "rca", "dsub25"],
        false,
        false,
        None,
        "A",
    ),
    (
        "madi",
        "MADI",
        "A",
        Some(75.0),
        Some(125.0),
        &["bnc", "sc", "lc"],
        false,
        false,
        None,
        "A",
    ),
    (
        "dante",
        "Dante",
        "A",
        None,
        None,
        &["rj45", "ethercon"],
        true,
        true,
        Some("primary_secondary"),
        "A",
    ),
    (
        "st2110",
        "SMPTE ST 2110",
        "F",
        None,
        None,
        &["rj45", "lc", "opticalcon-duo"],
        true,
        true,
        Some("ab"),
        "F",
    ),
    (
        "ethernet",
        "Ethernet",
        "N",
        None,
        None,
        &["rj45", "ethercon", "lc", "sc", "opticalcon-duo"],
        true,
        false,
        None,
        "N",
    ),
    (
        "dmx512",
        "DMX512",
        "L",
        Some(120.0),
        None,
        &["xlr5", "xlr3"],
        false,
        false,
        None,
        "L",
    ),
    (
        "intercom-4w",
        "Intercom 4 fils",
        "I",
        None,
        None,
        &["xlr4", "rj45"],
        false,
        false,
        None,
        "I",
    ),
    (
        "rs422",
        "RS-422",
        "C",
        None,
        None,
        &["dsub9"],
        false,
        false,
        None,
        "C",
    ),
    (
        "power-230",
        "Alimentation 230 V",
        "P",
        None,
        None,
        &[
            "schuko",
            "iec-c13",
            "powercon",
            "powercon-true1",
            "cee16-1p",
        ],
        false,
        false,
        None,
        "P",
    ),
    (
        "power-3p",
        "Alimentation triphasée",
        "P",
        None,
        None,
        &["cee16-3p", "cee32-3p"],
        false,
        false,
        None,
        "P",
    ),
    (
        "rf",
        "RF",
        "R",
        Some(50.0),
        None,
        &["bnc", "sma", "n-type"],
        false,
        false,
        None,
        "R",
    ),
];

/// Préréglages de dimensions : genre, valeurs.
pub const DIMENSIONS: &[(&str, &[f64])] = &[
    (
        "height_u",
        &[0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0],
    ),
    ("width", &[1.0, 0.5, 1.0 / 3.0, 0.25]),
    (
        "depth_mm",
        &[200.0, 300.0, 400.0, 450.0, 500.0, 600.0, 800.0],
    ),
    ("rear_reserve_mm", &[100.0, 150.0, 200.0]),
    (
        "cable_length_m",
        &[0.5, 1.0, 2.0, 3.0, 5.0, 10.0, 15.0, 20.0, 25.0, 50.0, 100.0],
    ),
    ("bend_radius_mm", &[30.0, 50.0, 80.0, 120.0]),
];

fn dimension_name(kind: &str, v: f64) -> String {
    let n = if v.fract() == 0.0 {
        format!("{v:.0}")
    } else if (v - 1.0 / 3.0).abs() < 1e-9 {
        "1/3".into()
    } else {
        format!("{v}").replace('.', ",")
    };
    match kind {
        "height_u" => format!("{n} U"),
        "width" => match n.as_str() {
            "1" => "pleine".into(),
            "0,5" => "1/2".into(),
            "0,25" => "1/4".into(),
            other => other.into(),
        },
        "cable_length_m" => format!("{n} m"),
        _ => format!("{n} mm"),
    }
}

pub fn dimension_defaults() -> Vec<DefaultRef> {
    let mut out = Vec::new();
    for (kind, values) in DIMENSIONS {
        for v in *values {
            let reel = *kind == "cable_length_m" && *v >= 50.0;
            let id = format!(
                "dim-{kind}-{}",
                format!("{v:.3}")
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .replace('.', "_")
            );
            let mut d = r(
                RefKind::Dimension,
                &id,
                &dimension_name(kind, *v),
                json!({ "kind": kind, "value": v, "reel": reel }),
            );
            d.builtin = true;
            out.push(d);
        }
    }
    out
}

/// Tout le pack de départ, dans l'ordre d'insertion (les dépendances d'abord).
pub fn defaults() -> Vec<DefaultRef> {
    let mut out = Vec::new();
    for (key, label, color) in FUNCTIONS {
        let mut d = r(
            RefKind::Function,
            key,
            label,
            json!({ "key": key, "color": color, "cable_prefix": key }),
        );
        d.builtin = true;
        out.push(d);
    }
    for (id, name) in PHASES {
        let mut d = r(RefKind::Phase, id, name, json!({}));
        d.builtin = true;
        out.push(d);
    }
    for (id, name, function) in [
        ("sys-video", "Vidéo", "V"),
        ("sys-audio", "Audio", "A"),
        ("sys-lumieres", "Lumières", "L"),
        ("sys-reseau", "Réseau", "N"),
    ] {
        let mut d = r(
            RefKind::SystemCategory,
            id,
            name,
            json!({ "function": function }),
        );
        d.builtin = true;
        d.protected = true;
        out.push(d);
    }
    for (id, name, variants, color, functions) in CONNECTORS {
        out.push(r(
            RefKind::Connector,
            id,
            name,
            json!({ "variants": variants, "symbol": id, "color": color, "functions": functions, "d_format": D_FORMAT.contains(id) }),
        ));
    }
    for (id, name, function, imp, bw, connectors, ip, multicast, redundancy, prefix) in SIGNALS {
        let color = FUNCTIONS
            .iter()
            .find(|f| f.0 == *function)
            .map(|f| f.2)
            .unwrap_or("#8A919B");
        out.push(r(
            RefKind::Signal,
            id,
            name,
            json!({
                "color": color, "function": function, "prefix": prefix, "suffix": "",
                "impedance_ohm": imp, "min_bandwidth_mhz": bw, "connectors": connectors,
                "ip": ip, "multicast": multicast, "redundancy": redundancy, "default": false
            }),
        ));
    }
    out.extend(dimension_defaults());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::validate::{RefIndex, validate_ref};
    use crate::catalog::*;

    /// Le pack de départ respecte ses propres règles.
    #[test]
    fn defaults_pass_validation() {
        let all = defaults();
        let mut ix = RefIndex::default();
        for d in &all {
            match d.kind {
                RefKind::Function => {
                    ix.functions.insert(d.id.clone());
                }
                RefKind::Connector => {
                    ix.connectors.insert(
                        d.id.clone(),
                        serde_json::from_value(d.data.clone()).unwrap(),
                    );
                }
                _ => {}
            }
        }
        for d in all.iter().filter(|d| !d.protected) {
            validate_ref(d.kind, &d.name, &d.data, d.parent_id.as_deref(), &ix)
                .unwrap_or_else(|e| panic!("{}: {e:?}", d.id));
        }
        assert_eq!(
            all.iter().filter(|d| d.kind == RefKind::Function).count(),
            11
        );
        assert_eq!(all.iter().filter(|d| d.kind == RefKind::Phase).count(), 10);
        assert!(all.iter().any(|d| d.name == "1/3"));
        let ids: std::collections::HashSet<_> =
            all.iter().map(|d| (d.kind, d.id.clone())).collect();
        assert_eq!(ids.len(), all.len(), "identifiants uniques");
    }
}
