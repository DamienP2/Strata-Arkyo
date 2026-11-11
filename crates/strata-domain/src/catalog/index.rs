//! Index des référentiels et liens entre fiches : ce qui est utilisé par quoi.

use super::model::*;
use super::validate::RefIndex;
use serde_json::Value;

/// Référence d'un objet du catalogue, référentiel ou fiche.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    Ref(RefKind, String),
    Item(String),
}

impl RefIndex {
    /// Construit l'index à partir des référentiels `(genre, id, parent, données)`
    /// et des fiches `(genre, id, données)`.
    pub fn build<'a>(
        refs: impl IntoIterator<Item = (RefKind, &'a str, Option<&'a str>, &'a Value)>,
        items: impl IntoIterator<Item = (ItemKind, &'a str, &'a Value)>,
    ) -> RefIndex {
        let mut ix = RefIndex::default();
        let mut types = Vec::new();
        for (kind, id, parent, data) in refs {
            match kind {
                RefKind::Function => {
                    ix.functions.insert(id.to_string());
                }
                RefKind::Signal => {
                    if let Ok(d) = serde_json::from_value(data.clone()) {
                        ix.signals.insert(id.to_string(), d);
                    }
                }
                RefKind::Connector => {
                    if let Ok(d) = serde_json::from_value(data.clone()) {
                        ix.connectors.insert(id.to_string(), d);
                    }
                }
                RefKind::Brand => {
                    ix.brands.insert(id.to_string());
                }
                RefKind::Category => {
                    ix.categories.insert(id.to_string());
                }
                RefKind::EquipmentType => {
                    let f = data
                        .get("function")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    types.push((id.to_string(), f, parent.map(str::to_string)));
                }
                RefKind::Phase => {
                    ix.phases.insert(id.to_string());
                }
                RefKind::SystemCategory | RefKind::Dimension => {}
            }
        }
        // Un sous-type porte la fonction de son parent.
        let own: std::collections::HashMap<String, String> = types
            .iter()
            .map(|(id, f, _)| (id.clone(), f.clone()))
            .collect();
        for (id, f, parent) in types {
            let f = parent
                .as_ref()
                .and_then(|p| own.get(p).cloned())
                .unwrap_or(f);
            ix.types.insert(id, (f, parent));
        }
        for (kind, id, data) in items {
            match kind {
                ItemKind::CableModel => {
                    ix.cable_models.insert(id.to_string());
                }
                ItemKind::Equipment => {
                    if let Some(Modular::Component { slots }) = data
                        .get("modular")
                        .and_then(|m| serde_json::from_value::<Modular>(m.clone()).ok())
                    {
                        ix.components.insert(id.to_string(), slots);
                    }
                }
                _ => {}
            }
        }
        ix
    }
}

fn r(kind: RefKind, id: &str) -> Target {
    Target::Ref(kind, id.to_string())
}

/// Objets dont dépend un référentiel.
pub fn ref_dependencies(kind: RefKind, parent: Option<&str>, data: &Value) -> Vec<Target> {
    let mut out = Vec::new();
    let s = |k: &str| {
        data.get(k)
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };
    let list = |k: &str| -> Vec<String> {
        data.get(k)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    match kind {
        RefKind::Signal => {
            out.extend(s("function").map(|f| r(RefKind::Function, &f)));
            out.extend(list("connectors").iter().map(|c| r(RefKind::Connector, c)));
        }
        RefKind::Connector => out.extend(list("functions").iter().map(|f| r(RefKind::Function, f))),
        RefKind::SystemCategory => out.extend(s("function").map(|f| r(RefKind::Function, &f))),
        RefKind::Category => out.extend(parent.map(|p| r(RefKind::Category, p))),
        RefKind::EquipmentType => {
            out.extend(parent.map(|p| r(RefKind::EquipmentType, p)));
            if parent.is_none() {
                out.extend(s("function").map(|f| r(RefKind::Function, &f)));
            }
        }
        _ => {}
    }
    out
}

/// Objets dont dépend une fiche. Une fiche illisible ne dépend de rien.
pub fn item_dependencies(kind: ItemKind, data: &Value) -> Vec<Target> {
    let mut out = Vec::new();
    let opt = |o: &Option<String>, k: RefKind, out: &mut Vec<Target>| {
        if let Some(v) = o {
            out.push(r(k, v));
        }
    };
    match kind {
        ItemKind::Equipment => {
            let Ok(x) = serde_json::from_value::<Equipment>(data.clone()) else {
                return out;
            };
            opt(&x.brand_id, RefKind::Brand, &mut out);
            opt(&x.category_id, RefKind::Category, &mut out);
            opt(&x.type_id, RefKind::EquipmentType, &mut out);
            out.extend(x.functions.iter().map(|f| r(RefKind::Function, f)));
            out.extend(x.phases.iter().map(|p| r(RefKind::Phase, p)));
            for p in &x.ports {
                out.push(r(RefKind::Connector, &p.connector_id));
                opt(&p.signal_id, RefKind::Signal, &mut out);
            }
            if let Some(ip) = &x.ip {
                out.extend(ip.flows.iter().map(|f| r(RefKind::Signal, &f.signal_id)));
            }
            if let Some(Modular::Host {
                default_composition,
                ..
            }) = &x.modular
            {
                out.extend(
                    default_composition
                        .iter()
                        .map(|s| Target::Item(s.equipment_id.clone())),
                );
            }
        }
        ItemKind::Cable => {
            let Ok(x) = serde_json::from_value::<Cable>(data.clone()) else {
                return out;
            };
            opt(&x.function, RefKind::Function, &mut out);
            opt(&x.brand_id, RefKind::Brand, &mut out);
            out.push(r(RefKind::Connector, &x.left.connector_id));
            out.push(r(RefKind::Connector, &x.right.connector_id));
            out.extend(x.usages.iter().map(|s| r(RefKind::Signal, s)));
            if let Some(m) = &x.model_id {
                out.push(Target::Item(m.clone()));
            }
        }
        ItemKind::CableModel => {
            let Ok(x) = serde_json::from_value::<CableModel>(data.clone()) else {
                return out;
            };
            opt(&x.function, RefKind::Function, &mut out);
            opt(&x.brand_id, RefKind::Brand, &mut out);
        }
        ItemKind::Adapter => {
            let Ok(x) = serde_json::from_value::<Adapter>(data.clone()) else {
                return out;
            };
            opt(&x.function, RefKind::Function, &mut out);
            out.push(r(RefKind::Connector, &x.input.connector_id));
            out.extend(
                x.outputs
                    .iter()
                    .map(|o| r(RefKind::Connector, &o.connector_id)),
            );
        }
    }
    out.sort_by_key(|t| format!("{t:?}"));
    out.dedup();
    out
}

/// Fiche en clair : nom, référence et marque, pour la liste et la recherche de doublons.
pub fn item_summary(data: &Value) -> (String, Option<String>, Option<String>) {
    let s = |k: &str| {
        data.get(k)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };
    (s("name").unwrap_or_default(), s("reference"), s("brand_id"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn subtypes_inherit_function() {
        let a = json!({"function": "V"});
        let b = json!({"function": "A"});
        let ix = RefIndex::build(
            [
                (RefKind::EquipmentType, "t1", None, &a),
                (RefKind::EquipmentType, "t2", Some("t1"), &b),
            ],
            [],
        );
        assert_eq!(ix.types["t2"].0, "V");
    }

    #[test]
    fn cable_dependencies() {
        let c = json!({"name": "", "left": {"connector_id": "bnc"}, "right": {"connector_id": "bnc", "gender": "M"}, "usages": ["sdi-3g"], "model_id": "m1"});
        let d = item_dependencies(ItemKind::Cable, &c);
        assert!(d.contains(&Target::Item("m1".into())));
        assert!(d.contains(&Target::Ref(RefKind::Connector, "bnc".into())));
        assert_eq!(
            d.iter()
                .filter(|t| **t == Target::Ref(RefKind::Connector, "bnc".into()))
                .count(),
            1
        );
    }
}
