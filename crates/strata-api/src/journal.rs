//! Lecture humaine du journal : libellés français des actions et des objets, nom de l'objet
//! (numéro du câble, nom du matériel…), ce qui a changé et le projet concerné. Sert au journal
//! d'administration et à l'historique d'un projet.

use crate::error::ApiResult;
use crate::projects::item_views;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use strata_domain::project::Accessory;
use strata_store::{AuditEntry, Store};

/// Libellé d'une action.
pub(crate) fn action_label(a: &str) -> &str {
    match a {
        "create" => "Création",
        "update" => "Modification",
        "delete" => "Suppression",
        "duplicate" => "Duplication",
        "restore" => "Restauration",
        "import" => "Import",
        "exchange" => "Échange du catalogue",
        "login" => "Connexion",
        "remove" => "Retrait",
        "reactivate" => "Réactivation",
        "reset_password" | "password_reset" => "Réinitialisation du mot de passe",
        "password_change" => "Changement de mot de passe",
        "move" => "Déplacement",
        "place" => "Pose en baie",
        "unplace" => "Retrait de la baie",
        "clear" => "Effacement",
        "number" => "Numérotation",
        "restore_stage" => "Restauration préparée",
        "restore_cancel" => "Restauration annulée",
        "update_schedule" => "Programmation des mises à jour",
        "update_switch" => "Passage à une nouvelle version",
        "update_rollback" => "Retour à la version précédente",
        _ => a,
    }
}

/// Libellé d'un type d'objet.
pub(crate) fn object_label(t: &str) -> String {
    if let Some(r) = t.strip_prefix("ref:") {
        let r = match r {
            "functions" => "fonctions",
            "types" => "types",
            "brands" => "marques",
            "connectors" => "connecteurs",
            "signals" => "signaux",
            "categories" => "catégories",
            "dimensions" => "dimensions",
            x => x,
        };
        return format!("Référentiel des {r}");
    }
    match t {
        "account" => "Compte",
        "role" => "Rôle",
        "site" => "Réglages du site",
        "settings" => "Réglages",
        "catalog_item" => "Fiche du catalogue",
        "catalog" => "Catalogue",
        "catalog_snapshot" => "Instantané du catalogue",
        "panel" => "Panel de brassage",
        "regie_template" => "Modèle de régie",
        "container_template" => "Modèle d'élément",
        "engine" => "Application",
        "backup" => "Sauvegarde",
        "project" => "Projet",
        "regie" => "Régie",
        "location" => "Emplacement",
        "container" => "Élément",
        "article" => "Matériel",
        "instance" => "Exemplaire",
        "placement" => "Pose en baie",
        "rack" => "Baie",
        "circuit" => "Circuit électrique",
        "cable" => "Câble",
        "harness" => "Faisceau",
        "annotation" => "Annotation du synoptique",
        "drawing" => "Dessin",
        "plan" => "Plan",
        "network" => "Réseau",
        "task" => "Tâche",
        "point" => "Point de restauration",
        "link" => "Lien entre projets",
        "check" => "Contrôles d'atelier",
        "label_sheet" => "Planche d'étiquettes",
        x => x,
    }
    .to_string()
}

/// Champs dont on montre le changement, avec leur libellé.
fn field_label(k: &str) -> Option<&'static str> {
    Some(match k {
        "name" => "nom",
        "title" => "titre",
        "label" => "libellé",
        "number" => "numéro",
        "quantity" => "quantité",
        "status" => "statut",
        "u_start" => "hauteur",
        "face" => "face",
        "container_id" => "élément",
        "location_id" => "emplacement",
        "tag" => "repère",
        "serial" => "n° de série",
        "mac" => "adresse MAC",
        "color" => "couleur",
        "notes" => "notes",
        "length_m" => "longueur",
        "client" => "client",
        "venue" => "lieu",
        "description" => "description",
        "a" | "b" => "extrémité",
        "harness_id" => "faisceau",
        "circuit_id" => "circuit électrique",
        "shelf_id" => "étagère",
        "col" => "colonne",
        "width" => "largeur",
        "waypoints" => "tracé",
        "style" => "style du tracé",
        "cable_model_id" => "modèle de câble",
        "external" => "port externe",
        "accessory" => "réglages de l'accessoire",
        "item_id" => "fiche du catalogue",
        "height_u" => "hauteur",
        "position" => "ordre",
        "rating_a" => "calibre",
        "phase" => "phase",
        "code" => "code",
        "active" => "actif",
        "reference" => "référence",
        "data" => "caractéristiques",
        "planning_start" | "planning_end" => "préparation",
        "shoot_start" | "shoot_end" => "exploitation",
        "assignee" => "responsable",
        "due" => "échéance",
        "role_id" => "rôle",
        "email" => "adresse",
        _ => return None,
    })
}

fn status_label(s: &str) -> &str {
    match s {
        "preparation" => "en préparation",
        "ready" => "prêt",
        "in_progress" => "en cours",
        "done" => "terminé",
        "cancelled" => "annulé",
        "validated" => "validé",
        "todo" => "à faire",
        x => x,
    }
}

/// Valeur courte et lisible, ou rien si elle ne s'affiche pas en une ligne.
fn short(k: &str, v: &Value) -> Option<String> {
    match v {
        Value::Null => Some("—".into()),
        Value::String(s) if k == "status" => Some(status_label(s).to_string()),
        Value::String(s) if k.ends_with("_id") || k == "a" || k == "b" => {
            let _ = s;
            None
        }
        Value::String(s) if s.chars().count() <= 40 => Some(s.clone()),
        Value::Number(n) if k == "u_start" => Some(format!("U{n}")),
        Value::Number(n) => Some(n.to_string().replace('.', ",")),
        Value::Bool(b) => Some(if *b { "oui" } else { "non" }.into()),
        _ => None,
    }
}

/// Ce qui a changé entre avant et après : « quantité 1 → 2, nom ».
fn changes(before: &Value, after: &Value) -> String {
    let (Some(b), Some(a)) = (before.as_object(), after.as_object()) else {
        return String::new();
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (k, av) in a {
        let bv = b.get(k).unwrap_or(&Value::Null);
        if bv == av {
            continue;
        }
        let Some(l) = field_label(k) else { continue };
        if !seen.insert(l) {
            continue;
        }
        match (short(k, bv), short(k, av)) {
            (Some(x), Some(y)) => out.push(format!("{l} {x} → {y}")),
            _ => out.push(l.to_string()),
        }
    }
    let s = out.join(", ");
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => s,
    }
}

struct Ctx {
    projects: HashMap<String, String>,
    items: HashMap<String, String>,
    articles: HashMap<String, String>,
}

fn s<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty())
}

fn article_name(a: &Value, items: &HashMap<String, String>) -> Option<String> {
    if let Some(acc) = a
        .get("accessory")
        .filter(|x| !x.is_null())
        .and_then(|x| serde_json::from_value::<Accessory>(x.clone()).ok())
    {
        return Some(acc.label());
    }
    s(a, "item_id").map(|i| {
        items
            .get(i)
            .cloned()
            .unwrap_or_else(|| "Fiche supprimée du catalogue".into())
    })
}

/// Nom de l'objet de l'entrée.
fn subject(e: &AuditEntry, c: &Ctx) -> String {
    let v = e
        .after
        .as_ref()
        .or(e.before.as_ref())
        .unwrap_or(&Value::Null);
    match e.object_type.as_str() {
        "cable" => match v.get("number").and_then(Value::as_str) {
            Some(n) => n.to_string(),
            None if e.object_id == "table" => "Saisie en tableau".into(),
            None if v.is_null() => String::new(),
            None => "non numéroté".into(),
        },
        "article" => article_name(v, &c.items).unwrap_or_default(),
        "placement" => {
            let art = s(v, "article_id").and_then(|a| c.articles.get(a)).cloned();
            match (art, v.get("u_start").and_then(Value::as_f64)) {
                (Some(a), Some(u)) => format!("{a} · U{}", u.to_string().replace('.', ",")),
                (Some(a), None) => a,
                (None, Some(u)) => format!("U{}", u.to_string().replace('.', ",")),
                _ => String::new(),
            }
        }
        "instance" => {
            let art = s(v, "article_id").and_then(|a| c.articles.get(a)).cloned();
            let tag = s(v, "tag").unwrap_or_default();
            match (art, s(v, "label")) {
                (Some(a), Some(l)) => format!("{a} {tag} « {l} »"),
                (Some(a), None) => format!("{a} {tag}"),
                (None, Some(l)) => format!("{tag} « {l} »"),
                _ => tag.to_string(),
            }
        }
        "catalog" if e.action == "exchange" => {
            let n = |k: &str| v["items"][k].as_u64().unwrap_or(0);
            format!("{} ajoutée(s), {} mise(s) à jour", n("added"), n("updated"))
        }
        "project" => s(v, "name")
            .map(str::to_string)
            .or_else(|| c.projects.get(&e.object_id).cloned())
            .unwrap_or_default(),
        _ => ["name", "title", "label", "file", "number"]
            .iter()
            .find_map(|k| s(v, k))
            .unwrap_or_default()
            .to_string(),
    }
}

/// Précision : ce qui a changé, ou la position.
fn detail(e: &AuditEntry) -> String {
    let (b, a) = (e.before.as_ref(), e.after.as_ref());
    match (e.action.as_str(), b, a) {
        ("move", Some(b), Some(a)) | ("update", Some(b), Some(a)) => changes(b, a),
        ("place", _, Some(a)) => {
            let face = match s(a, "face") {
                Some("rear") => " · face arrière",
                _ => "",
            };
            match a.get("u_start").and_then(Value::as_f64) {
                Some(u) => format!("Posé en U{}{face}", u.to_string().replace('.', ",")),
                None => String::new(),
            }
        }
        ("create", _, Some(a)) if e.object_type == "article" => a
            .get("quantity")
            .and_then(Value::as_u64)
            .map(|q| format!("Quantité {q}"))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// Entrées du journal enrichies pour l'affichage.
pub(crate) async fn describe(store: &Store, entries: Vec<AuditEntry>) -> ApiResult<Vec<Value>> {
    let mut projects: HashMap<String, String> = store
        .list_projects()
        .await?
        .into_iter()
        .map(|p| (p.id, p.data.name))
        .collect();
    // Projets supprimés : leur nom reste dans leurs propres entrées.
    for e in &entries {
        if e.object_type == "project" && !projects.contains_key(&e.object_id) {
            if let Some(n) = e
                .after
                .as_ref()
                .or(e.before.as_ref())
                .and_then(|v| s(v, "name"))
            {
                projects.insert(e.object_id.clone(), n.to_string());
            }
        }
    }
    // Matériel : fiches du catalogue des articles cités, directement ou par une pose.
    let mut article_ids: HashSet<String> = HashSet::new();
    let mut item_ids: HashSet<String> = HashSet::new();
    for e in &entries {
        for v in [e.before.as_ref(), e.after.as_ref()].into_iter().flatten() {
            match e.object_type.as_str() {
                "placement" | "instance" => {
                    if let Some(a) = s(v, "article_id") {
                        article_ids.insert(a.to_string());
                    }
                }
                "article" => {
                    if let Some(i) = s(v, "item_id") {
                        item_ids.insert(i.to_string());
                    }
                }
                _ => {}
            }
        }
    }
    let mut arts: HashMap<String, Value> = HashMap::new();
    for id in &article_ids {
        if let Some(a) = store.article(id).await? {
            let v = serde_json::to_value(&a).unwrap_or(Value::Null);
            if let Some(i) = s(&v, "item_id") {
                item_ids.insert(i.to_string());
            }
            arts.insert(id.clone(), v);
        }
    }
    let items: HashMap<String, String> = item_views(store, &item_ids)
        .await?
        .into_iter()
        .map(|(k, v)| (k, v.name))
        .collect();
    let articles = arts
        .iter()
        .filter_map(|(k, v)| article_name(v, &items).map(|n| (k.clone(), n)))
        .collect();
    let c = Ctx {
        projects,
        items,
        articles,
    };
    Ok(entries
        .iter()
        .map(|e| {
            let pid = e.project_id.clone().or_else(|| {
                (e.object_type == "project").then(|| e.object_id.clone())
            });
            json!({
                "id": e.id, "at": e.at, "actor_id": e.actor_id,
                "action": e.action, "object_type": e.object_type, "object_id": e.object_id,
                "action_label": action_label(&e.action),
                "object_label": object_label(&e.object_type),
                "subject": subject(e, &c),
                "detail": detail(e),
                "project_id": pid,
                "project_name": pid.as_ref().map(|p| c.projects.get(p).cloned().unwrap_or_else(|| "Projet supprimé".into())),
            })
        })
        .collect())
}
