//! Règles des référentiels et des fiches. Chaque erreur nomme le champ fautif.

use super::model::*;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

fn err(field: impl Into<String>, message: impl Into<String>) -> FieldError {
    FieldError { field: field.into(), message: message.into() }
}

/// Ce que les règles doivent savoir des référentiels et des autres fiches.
#[derive(Debug, Clone, Default)]
pub struct RefIndex {
    /// Clés des fonctions.
    pub functions: HashSet<String>,
    pub signals: HashMap<String, SignalData>,
    pub connectors: HashMap<String, ConnectorData>,
    pub brands: HashSet<String>,
    /// Catégories attribuables (les catégories système ne le sont pas).
    pub categories: HashSet<String>,
    /// Type → (fonction effective, parent).
    pub types: HashMap<String, (String, Option<String>)>,
    pub phases: HashSet<String>,
    pub cable_models: HashSet<String>,
    /// Équipements composants modulaires → emplacements occupés.
    pub components: HashMap<String, u32>,
}

fn in_range(errors: &mut Vec<FieldError>, field: &str, v: Option<f64>, min: f64, max: f64, unit: &str) {
    if let Some(v) = v {
        if !v.is_finite() || v < min || v > max {
            errors.push(err(field, format!("Valeur attendue entre {min} et {max} {unit}.").replace(" .", ".")));
        }
    }
}

fn is_color(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())
}

fn is_url(u: &str) -> bool {
    (u.starts_with("https://") || u.starts_with("http://")) && u.len() > 8 && !u.contains(char::is_whitespace)
}

fn name_ok(errors: &mut Vec<FieldError>, field: &str, name: &str, max: usize) {
    let n = name.trim().chars().count();
    if n == 0 || n > max {
        errors.push(err(field, format!("Le nom compte de 1 à {max} caractères.")));
    }
}

fn code_ok(s: &str, max: usize) -> bool {
    s.len() <= max && s.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// Symboles de connecteurs qui existent en format D (coque standard de panel).
const D_SYMBOLS: &[&str] = &[
    "bnc", "hd-bnc", "xlr3", "xlr4", "xlr5", "ethercon", "rj45", "opticalcon-duo", "opticalcon-quad", "speakon",
    "powercon", "powercon-true1", "hdmi", "usb", "lc", "sc", "toslink", "jack635", "rca", "dsub9", "din-1023",
];

pub fn has_d_format(symbol: &str) -> bool {
    D_SYMBOLS.contains(&symbol)
}

/// Vérifie et normalise les données d'un référentiel. Renvoie les données à enregistrer.
pub fn validate_ref(kind: RefKind, name: &str, data: &Value, parent: Option<&str>, ix: &RefIndex) -> Result<Value, Vec<FieldError>> {
    let mut e = Vec::new();
    if kind.named() {
        name_ok(&mut e, "name", name, 100);
    }
    let out = match kind {
        RefKind::Function => {
            let mut d: FunctionData = from(data, &mut e);
            d.key = d.key.trim().to_uppercase();
            if d.key.is_empty() || d.key.len() > 16 || !d.key.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
                e.push(err("key", "La clé compte 1 à 16 caractères : majuscules, chiffres ou tiret bas."));
            }
            if !is_color(&d.color) {
                e.push(err("color", "La couleur s'écrit #RRVVBB."));
            }
            d.cable_prefix = d.cable_prefix.trim().to_uppercase();
            if !code_ok(&d.cable_prefix, 4) {
                e.push(err("cable_prefix", "Le préfixe compte au plus 4 majuscules ou chiffres."));
            }
            serde_json::to_value(d)
        }
        RefKind::Signal => {
            let mut d: SignalData = from(data, &mut e);
            if !ix.functions.contains(&d.function) {
                e.push(err("function", "Choisissez la fonction du signal."));
            }
            if !is_color(&d.color) {
                e.push(err("color", "La couleur s'écrit #RRVVBB."));
            }
            d.prefix = d.prefix.trim().to_uppercase();
            d.suffix = d.suffix.trim().to_uppercase();
            if !code_ok(&d.prefix, 4) || !code_ok(&d.suffix, 4) {
                e.push(err("prefix", "Préfixe et suffixe comptent au plus 4 majuscules ou chiffres."));
            }
            in_range(&mut e, "impedance_ohm", d.impedance_ohm, 1.0, 1000.0, "Ω");
            in_range(&mut e, "min_bandwidth_mhz", d.min_bandwidth_mhz, 0.0, 1_000_000.0, "MHz");
            d.connectors.dedup();
            if d.connectors.is_empty() {
                e.push(err("connectors", "Choisissez au moins un connecteur compatible."));
            }
            for c in &d.connectors {
                if !ix.connectors.contains_key(c) {
                    e.push(err("connectors", format!("Connecteur inconnu : {c}.")));
                }
            }
            if d.multicast && !d.ip {
                e.push(err("multicast", "Seul un signal IP peut être multicast."));
            }
            serde_json::to_value(d)
        }
        RefKind::Connector => {
            let mut d: ConnectorData = from(data, &mut e);
            d.variants = d.variants.iter().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).collect();
            let mut seen = HashSet::new();
            if d.variants.iter().any(|v| !seen.insert(crate::text::normalize(v))) {
                e.push(err("variants", "Chaque variante n'apparaît qu'une fois."));
            }
            if !is_color(&d.color) {
                e.push(err("color", "La couleur s'écrit #RRVVBB."));
            }
            for f in &d.functions {
                if !ix.functions.contains(f) {
                    e.push(err("functions", format!("Fonction inconnue : {f}.")));
                }
            }
            serde_json::to_value(d)
        }
        RefKind::Brand => {
            let mut d: BrandData = from(data, &mut e);
            d.website = d.website.map(|w| w.trim().to_string()).filter(|w| !w.is_empty());
            if d.website.as_deref().is_some_and(|w| !is_url(w)) {
                e.push(err("website", "Le site web commence par https:// ou http://."));
            }
            serde_json::to_value(d)
        }
        RefKind::SystemCategory => {
            e.push(err("name", "Les catégories système sont protégées."));
            Ok(Value::Null)
        }
        RefKind::Category => {
            let d: CategoryData = from(data, &mut e);
            if !d.color.is_empty() && !is_color(&d.color) {
                e.push(err("color", "La couleur s'écrit #RRVVBB."));
            }
            if let Some(p) = parent {
                if !ix.categories.contains(p) {
                    e.push(err("parent_id", "Catégorie parente inconnue."));
                }
            }
            serde_json::to_value(d)
        }
        RefKind::EquipmentType => {
            let mut d: TypeData = from(data, &mut e);
            match parent {
                Some(p) => match ix.types.get(p) {
                    // Un seul niveau de sous-type, qui hérite de la fonction du parent.
                    Some((function, None)) => d.function = function.clone(),
                    Some((_, Some(_))) => e.push(err("parent_id", "Un sous-type ne peut pas avoir de sous-type.")),
                    None => e.push(err("parent_id", "Type parent inconnu.")),
                },
                None => {
                    if !ix.functions.contains(&d.function) {
                        e.push(err("function", "Choisissez la fonction du type."));
                    }
                }
            }
            serde_json::to_value(d)
        }
        RefKind::Phase => Ok(Value::Object(Default::default())),
        RefKind::Dimension => {
            let d: Option<DimensionData> = serde_json::from_value(data.clone()).ok();
            match d {
                Some(d) if d.value.is_finite() && d.value > 0.0 => serde_json::to_value(d),
                Some(_) => {
                    e.push(err("value", "La valeur doit être strictement positive."));
                    Ok(Value::Null)
                }
                None => {
                    e.push(err("kind", "Indiquez le genre de préréglage et sa valeur."));
                    Ok(Value::Null)
                }
            }
        }
    };
    let out = out.unwrap_or(Value::Null);
    if e.is_empty() { Ok(out) } else { Err(e) }
}

fn from<T: serde::de::DeserializeOwned + Default>(data: &Value, e: &mut Vec<FieldError>) -> T {
    match serde_json::from_value(data.clone()) {
        Ok(v) => v,
        Err(x) => {
            e.push(err("data", format!("Données illisibles : {x}.")));
            T::default()
        }
    }
}

fn check_ref(e: &mut Vec<FieldError>, field: &str, value: &Option<String>, set: &HashSet<String>, what: &str) {
    if let Some(v) = value {
        if !set.contains(v) {
            e.push(err(field, format!("{what} inconnue.")));
        }
    }
}

fn check_custom(e: &mut Vec<FieldError>, custom: &[CustomField]) {
    for (i, c) in custom.iter().enumerate() {
        if c.name.trim().is_empty() || c.name.chars().count() > 100 {
            e.push(err(format!("custom[{i}].name"), "Le nom du champ compte de 1 à 100 caractères."));
        }
        if let CustomValue::Number { value: Some(v), min, max, .. } = &c.value {
            if min.is_some_and(|m| *v < m) || max.is_some_and(|m| *v > m) {
                e.push(err(format!("custom[{i}].value"), "La valeur sort des bornes du champ."));
            }
        }
    }
}

/// Règles d'un équipement (section 5.2).
pub fn validate_equipment(x: &Equipment, ix: &RefIndex) -> Vec<FieldError> {
    let mut e = Vec::new();
    name_ok(&mut e, "name", &x.name, 200);
    if x.reference.as_ref().is_some_and(|r| r.chars().count() > 100) {
        e.push(err("reference", "La référence compte au plus 100 caractères."));
    }
    check_ref(&mut e, "brand_id", &x.brand_id, &ix.brands, "Marque");
    check_ref(&mut e, "category_id", &x.category_id, &ix.categories, "Catégorie");
    for f in &x.functions {
        if !ix.functions.contains(f) {
            e.push(err("functions", format!("Fonction inconnue : {f}.")));
        }
    }
    for p in &x.phases {
        if !ix.phases.contains(p) {
            e.push(err("phases", "Phase inconnue."));
        }
    }
    if let Some(t) = &x.type_id {
        match ix.types.get(t) {
            None => e.push(err("type_id", "Type inconnu.")),
            Some((function, _)) if !x.functions.contains(function) => e.push(err(
                "functions",
                format!("La fonction {function} du type choisi doit figurer parmi les fonctions de l'équipement."),
            )),
            _ => {}
        }
    }

    match &x.format {
        Format::Rack { height_u, depth_mm, rear_reserve_mm, .. } => {
            if !(1.0..=48.0).contains(height_u) || (height_u * 2.0).fract() != 0.0 {
                e.push(err("format.height_u", "La hauteur va de 1 à 48 U, par demi-U."));
            }
            in_range(&mut e, "format.depth_mm", Some(*depth_mm), 0.0, 1200.0, "mm");
            in_range(&mut e, "format.rear_reserve_mm", *rear_reserve_mm, 0.0, 600.0, "mm");
        }
        Format::Free { length_mm, width_mm, height_mm } => {
            for (f, v) in [("format.length_mm", length_mm), ("format.width_mm", width_mm), ("format.height_mm", height_mm)] {
                in_range(&mut e, f, Some(*v), 0.0, 2000.0, "mm");
            }
        }
    }
    in_range(&mut e, "weight_kg", x.weight_kg, 0.0, 200.0, "kg");
    in_range(&mut e, "power_w", x.power_w, 0.0, 5000.0, "W");
    in_range(&mut e, "btu_h", x.btu_h, 0.0, 15000.0, "BTU/h");

    if let Some(ip) = &x.ip {
        for (i, f) in ip.flows.iter().enumerate() {
            if !ix.signals.contains_key(&f.signal_id) {
                e.push(err(format!("ip.flows[{i}].signal_id"), "Signal inconnu."));
            }
            if f.inputs > 1024 || f.outputs > 1024 {
                e.push(err(format!("ip.flows[{i}]"), "Au plus 1 024 entrées et 1 024 sorties."));
            }
        }
    }
    if let Some(dmx) = &x.dmx {
        if !(1..=512).contains(&dmx.default_channels) {
            e.push(err("dmx.default_channels", "De 1 à 512 canaux."));
        }
        for (i, m) in dmx.modes.iter().enumerate() {
            if m.name.trim().is_empty() || !(1..=512).contains(&m.channels) {
                e.push(err(format!("dmx.modes[{i}]"), "Chaque mode a un nom et de 1 à 512 canaux."));
            }
        }
    }
    match &x.modular {
        Some(Modular::Host { slots, default_composition }) => {
            if !(1..=40).contains(slots) {
                e.push(err("modular.slots", "Un châssis offre de 1 à 40 emplacements."));
            }
            let mut used = vec![false; *slots as usize + 1];
            for (i, p) in default_composition.iter().enumerate() {
                let Some(width) = ix.components.get(&p.equipment_id) else {
                    e.push(err(format!("modular.default_composition[{i}]"), "Ce n'est pas un composant modulaire connu."));
                    continue;
                };
                let end = p.slot + width - 1;
                if p.slot == 0 || end > *slots {
                    e.push(err(format!("modular.default_composition[{i}]"), "Hors des limites du châssis."));
                    continue;
                }
                for s in p.slot..=end {
                    if std::mem::replace(&mut used[s as usize], true) {
                        e.push(err(format!("modular.default_composition[{i}]"), "Slot occupé."));
                        break;
                    }
                }
            }
        }
        Some(Modular::Component { slots }) if !(1..=8).contains(slots) => {
            e.push(err("modular.slots", "Un composant occupe de 1 à 8 emplacements."));
        }
        _ => {}
    }

    for (i, p) in x.ports.iter().enumerate() {
        let f = |s: &str| format!("ports[{i}].{s}");
        match ix.connectors.get(&p.connector_id) {
            None => e.push(err(f("connector_id"), "Connecteur inconnu.")),
            Some(c) => {
                if let Some(v) = &p.variant {
                    if !c.variants.iter().any(|x| x == v) {
                        e.push(err(f("variant"), "Variante inconnue pour ce connecteur."));
                    }
                }
            }
        }
        if let Some(s) = &p.signal_id {
            match ix.signals.get(s) {
                None => e.push(err(f("signal_id"), "Signal inconnu.")),
                // Le signal est filtré selon le connecteur.
                Some(sig) if !sig.connectors.contains(&p.connector_id) => {
                    e.push(err(f("signal_id"), "Ce signal ne passe pas par ce connecteur."))
                }
                _ => {}
            }
        }
        if !(1..=1024).contains(&p.quantity) {
            e.push(err(f("quantity"), "De 1 à 1 024 ports par groupe."));
        }
    }
    check_custom(&mut e, &x.custom);
    for (i, d) in x.documents.iter().enumerate() {
        if d.title.trim().is_empty() || !is_url(d.url.trim()) {
            e.push(err(format!("documents[{i}]"), "Un document a un titre et un lien http(s)."));
        }
    }
    e
}

/// Nom proposé pour un câble : « ConnG G ↔ ConnD G ».
pub fn proposed_cable_name(left: &str, lg: EndGender, right: &str, rg: EndGender) -> String {
    format!("{left} {} ↔ {right} {}", lg.letter(), rg.letter())
}

pub fn validate_cable(x: &Cable, ix: &RefIndex) -> Vec<FieldError> {
    let mut e = Vec::new();
    name_ok(&mut e, "name", &x.name, 200);
    for (f, end) in [("left", &x.left), ("right", &x.right)] {
        if !ix.connectors.contains_key(&end.connector_id) {
            e.push(err(format!("{f}.connector_id"), "Connecteur inconnu."));
        }
    }
    check_ref(&mut e, "function", &x.function, &ix.functions, "Fonction");
    check_ref(&mut e, "brand_id", &x.brand_id, &ix.brands, "Marque");
    check_ref(&mut e, "model_id", &x.model_id, &ix.cable_models, "Modèle de câble");
    if x.lengths_m.iter().any(|l| !l.is_finite() || *l <= 0.0) {
        e.push(err("lengths_m", "Les longueurs sont strictement positives."));
    }
    in_range(&mut e, "custom_min_m", x.custom_min_m, 0.0, 10_000.0, "m");
    in_range(&mut e, "custom_max_m", x.custom_max_m, 0.0, 10_000.0, "m");
    if let (Some(a), Some(b)) = (x.custom_min_m, x.custom_max_m) {
        if a > b {
            e.push(err("custom_max_m", "La longueur sur mesure maximale dépasse la minimale."));
        }
    }
    e
}

pub fn validate_cable_model(x: &CableModel, ix: &RefIndex) -> Vec<FieldError> {
    let mut e = Vec::new();
    name_ok(&mut e, "name", &x.name, 100);
    check_ref(&mut e, "function", &x.function, &ix.functions, "Fonction");
    check_ref(&mut e, "brand_id", &x.brand_id, &ix.brands, "Marque");
    in_range(&mut e, "impedance_ohm", x.impedance_ohm, 1.0, 1000.0, "Ω");
    in_range(&mut e, "bandwidth_mhz", x.bandwidth_mhz, 0.0, 1_000_000.0, "MHz");
    in_range(&mut e, "diameter_mm", x.diameter_mm, 0.0, 100.0, "mm");
    check_custom(&mut e, &x.custom);
    if x.datasheet_url.as_deref().is_some_and(|u| !is_url(u.trim())) {
        e.push(err("datasheet_url", "La fiche technique est un lien http(s)."));
    }
    e
}

pub fn validate_adapter(x: &Adapter, ix: &RefIndex) -> Vec<FieldError> {
    let mut e = Vec::new();
    name_ok(&mut e, "name", &x.name, 200);
    check_ref(&mut e, "function", &x.function, &ix.functions, "Fonction");
    if !ix.connectors.contains_key(&x.input.connector_id) {
        e.push(err("input.connector_id", "Connecteur d'entrée inconnu."));
    }
    if x.outputs.is_empty() {
        e.push(err("outputs", "Un adaptateur a au moins une sortie."));
    }
    for (i, o) in x.outputs.iter().enumerate() {
        if !ix.connectors.contains_key(&o.connector_id) {
            e.push(err(format!("outputs[{i}].connector_id"), "Connecteur inconnu."));
        }
        if !(1..=1024).contains(&o.quantity) {
            e.push(err(format!("outputs[{i}].quantity"), "De 1 à 1 024."));
        }
    }
    e
}

/// Compatibilité d'un modèle de câble avec un signal (section 5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Compatibility {
    Compatible,
    /// Seule la bande passante manque.
    Warning,
    /// Impédance fausse.
    Incompatible,
    /// Il manque une valeur pour conclure.
    Unknown,
}

pub fn cable_compatibility(model: &CableModel, signal: &SignalData) -> Compatibility {
    if let (Some(m), Some(s)) = (model.impedance_ohm, signal.impedance_ohm) {
        if (m - s).abs() > 0.5 {
            return Compatibility::Incompatible;
        }
    }
    match (model.impedance_ohm, signal.impedance_ohm, model.bandwidth_mhz, signal.min_bandwidth_mhz) {
        (Some(_), Some(_), Some(bw), Some(min)) => {
            if bw >= min {
                Compatibility::Compatible
            } else {
                Compatibility::Warning
            }
        }
        _ => Compatibility::Unknown,
    }
}

/// Dissipation suggérée : puissance × 3,412, arrondie.
pub fn suggested_btu(power_w: f64) -> f64 {
    (power_w * 3.412).round()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ix() -> RefIndex {
        let mut ix = RefIndex::default();
        ix.functions.extend(["V".to_string(), "A".to_string()]);
        ix.connectors.insert("bnc".into(), ConnectorData { variants: vec!["75 Ω".into()], ..Default::default() });
        ix.connectors.insert("xlr3".into(), ConnectorData::default());
        ix.signals.insert(
            "sdi".into(),
            SignalData { function: "V".into(), connectors: vec!["bnc".into()], impedance_ohm: Some(75.0), min_bandwidth_mhz: Some(3000.0), ..Default::default() },
        );
        ix.types.insert("cam".into(), ("V".into(), None));
        ix.components.insert("carte".into(), 2);
        ix
    }

    fn eq() -> Equipment {
        Equipment { name: "Grille 40×40".into(), functions: vec!["V".into()], ..Default::default() }
    }

    fn fields(e: &[FieldError]) -> Vec<&str> {
        e.iter().map(|x| x.field.as_str()).collect()
    }

    #[test]
    fn a_minimal_equipment_is_valid() {
        assert!(validate_equipment(&eq(), &ix()).is_empty());
        assert_eq!(fields(&validate_equipment(&Equipment::default(), &ix())), ["name"]);
    }

    #[test]
    fn type_function_must_be_among_functions() {
        let mut x = eq();
        x.type_id = Some("cam".into());
        x.functions = vec!["A".into()];
        assert_eq!(fields(&validate_equipment(&x, &ix())), ["functions"]);
    }

    #[test]
    fn rack_bounds_and_half_units() {
        let mut x = eq();
        x.format = Format::Rack { height_u: 1.5, width: Width::Half, depth_mm: 450.0, rear_reserve_mm: Some(150.0) };
        assert!(validate_equipment(&x, &ix()).is_empty());
        x.format = Format::Rack { height_u: 1.25, width: Width::Full, depth_mm: 1300.0, rear_reserve_mm: Some(700.0) };
        assert_eq!(fields(&validate_equipment(&x, &ix())), ["format.height_u", "format.depth_mm", "format.rear_reserve_mm"]);
    }

    #[test]
    fn ports_signal_must_go_through_the_connector() {
        let mut x = eq();
        let port = |c: &str, s: Option<&str>| PortGroup {
            face: Face::Rear,
            connector_id: c.into(),
            variant: None,
            signal_id: s.map(str::to_string),
            direction: Direction::Out,
            gender: Gender::F,
            quantity: 40,
            label: None,
            network_role: None,
            redundancy: true,
        };
        x.ports = vec![port("bnc", Some("sdi"))];
        assert!(validate_equipment(&x, &ix()).is_empty());
        x.ports = vec![port("xlr3", Some("sdi"))];
        assert_eq!(fields(&validate_equipment(&x, &ix())), ["ports[0].signal_id"]);
    }

    #[test]
    fn modular_default_composition() {
        let mut x = eq();
        x.modular = Some(Modular::Host {
            slots: 4,
            default_composition: vec![
                SlotPlacement { equipment_id: "carte".into(), slot: 1 },
                SlotPlacement { equipment_id: "carte".into(), slot: 2 },
                SlotPlacement { equipment_id: "carte".into(), slot: 4 },
            ],
        });
        let errors = validate_equipment(&x, &ix());
        assert_eq!(errors.iter().map(|e| e.message.as_str()).collect::<Vec<_>>(), ["Slot occupé.", "Hors des limites du châssis."]);
    }

    #[test]
    fn adapters_need_one_output() {
        let a = Adapter { name: "BNC → HDMI".into(), input: AdapterIn { connector_id: "bnc".into(), ..Default::default() }, ..Default::default() };
        assert_eq!(fields(&validate_adapter(&a, &ix())), ["outputs"]);
    }

    #[test]
    fn cable_model_compatibility() {
        let sig = SignalData { impedance_ohm: Some(75.0), min_bandwidth_mhz: Some(12000.0), ..Default::default() };
        let m = |imp: Option<f64>, bw: Option<f64>| CableModel { impedance_ohm: imp, bandwidth_mhz: bw, ..Default::default() };
        assert_eq!(cable_compatibility(&m(Some(75.3), Some(12000.0)), &sig), Compatibility::Compatible);
        assert_eq!(cable_compatibility(&m(Some(75.0), Some(3000.0)), &sig), Compatibility::Warning);
        assert_eq!(cable_compatibility(&m(Some(50.0), Some(20000.0)), &sig), Compatibility::Incompatible);
        assert_eq!(cable_compatibility(&m(Some(50.0), None), &sig), Compatibility::Incompatible);
        assert_eq!(cable_compatibility(&m(Some(75.0), None), &sig), Compatibility::Unknown);
    }

    #[test]
    fn signals_and_types() {
        let ix = ix();
        let bad = serde_json::json!({"function": "Z", "color": "rouge", "connectors": []});
        let e = validate_ref(RefKind::Signal, "AES3", &bad, None, &ix).unwrap_err();
        assert_eq!(fields(&e), ["function", "color", "connectors"]);
        let sub = validate_ref(RefKind::EquipmentType, "Caméra épaule", &serde_json::json!({}), Some("cam"), &ix).unwrap();
        assert_eq!(sub["function"], "V", "le sous-type hérite de la fonction");
        assert_eq!(suggested_btu(330.0), 1126.0);
        assert!(has_d_format("xlr3") && !has_d_format("schuko"));
    }
}
