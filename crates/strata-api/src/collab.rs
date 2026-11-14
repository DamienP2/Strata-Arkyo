//! Vue d'ensemble d'un projet (section 12) : une ligne par domaine (matériel, racks, câblage,
//! validation, fiche) et la liste « Ce qu'il reste à faire », chaque ligne menant à l'objet.

use crate::AppState;
use crate::auth::Member;
use crate::error::{ApiError, ApiResult};
use crate::projects::{article_label, item_views, read};
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{Value, json};
use std::collections::HashSet;
use strata_domain::project::{ArticleNature, completion};

pub fn routes() -> Router<AppState> {
    Router::new().route("/projects/{id}/todo", get(todo))
}

async fn todo(
    State(st): State<AppState>,
    m: Member,
    Path(pid): Path<String>,
) -> ApiResult<Json<Value>> {
    read(&m)?;
    let p = st
        .store
        .project(&pid)
        .await?
        .ok_or(ApiError::NotFound("Projet"))?;
    let base = format!("/projets/{pid}");
    let inv = st.store.inventory(&pid).await?;
    let mut items: Vec<Value> = Vec::new();

    // Matériel : équipements rangés dans un élément.
    let equip: Vec<_> = inv
        .articles
        .iter()
        .filter(|a| a.nature == ArticleNature::Equipment)
        .collect();
    let housed = equip.iter().filter(|a| a.container_id.is_some()).count();
    // Racks : exemplaires rackables posés dans leur baie.
    let ids: HashSet<String> = equip.iter().filter_map(|a| a.item_id.clone()).collect();
    let views = item_views(&st.store, &ids).await?;
    let (mut rack_total, mut rack_done) = (0, 0);
    for c in inv.containers.iter().filter(|c| c.data.rackable) {
        let placed: HashSet<String> = st
            .store
            .placements(&c.id)
            .await?
            .into_iter()
            .filter_map(|p| p.instance_id)
            .collect();
        for a in equip
            .iter()
            .filter(|a| a.container_id.as_deref() == Some(&c.id))
        {
            let rackable = a
                .item_id
                .as_ref()
                .and_then(|i| views.get(i))
                .is_some_and(|v| v.rackable);
            if !rackable {
                continue;
            }
            for i in inv.instances.iter().filter(|i| i.article_id == a.id) {
                rack_total += 1;
                if placed.contains(&i.id) {
                    rack_done += 1;
                } else {
                    let subject = format!("{} {}", article_label(a, &views), i.tag);
                    items.push(json!({
                        "kind": "rack", "group": "Non posé en baie", "severity": "warning",
                        "subject": subject, "detail": format!("À poser dans {}.", c.name),
                        "message": format!("{subject} n'est pas posé dans {}.", c.name),
                        "href": format!("{base}/racks"),
                    }));
                }
            }
        }
    }
    // Câblage : câbles numérotés et anomalies.
    let cables = st.store.cables(&pid).await?;
    let real: Vec<_> = cables.iter().filter(|c| !c.fields.attach).collect();
    let numbered = real.iter().filter(|c| c.fields.number.is_some()).count();
    let issues = crate::cabling::issues_of(&st.store, &pid).await?;
    // Toutes les anomalies (regroupées par type à l'affichage), chacune menant à son objet :
    // le ou les câbles dans la liste filtrée, sinon la vue du synoptique concernée.
    for i in issues.iter().filter(|i| i.severity != "info") {
        let href = if !i.cables.is_empty() {
            format!("{base}/cables?cable={}", i.cables.join(","))
        } else {
            match i.view.as_deref().and_then(|v| v.split_once(':')) {
                Some(("location", id)) => format!("{base}/synoptique/lieu/{id}"),
                Some(("container", id)) => format!("{base}/synoptique/element/{id}"),
                _ => format!("{base}/synoptique"),
            }
        };
        items.push(json!({
            "kind": "cabling", "group": i.title, "severity": i.severity,
            "subject": i.subject, "place": i.place, "detail": i.detail,
            "message": format!("{} : {}", i.subject, i.detail), "href": href,
        }));
    }
    for msg in crate::network::network_issues(&st.store, &pid)
        .await?
        .into_iter()
        .take(20)
    {
        items.push(json!({ "kind": "network", "severity": "warning", "message": msg, "href": format!("{base}/reseau") }));
    }
    let dmx = crate::drawings::dmx_conflicts(&st.store, &pid).await?;
    if dmx > 0 {
        items.push(json!({ "kind": "dmx", "severity": "error", "message": format!("{dmx} chevauchement(s) d'adresses DMX."), "href": format!("{base}/dessins") }));
    }
    // Validation : recette.
    let r = crate::recette::recette_view(&st.store, &pid).await?;
    let total = r["total"].as_u64().unwrap_or(0);
    let tested = total - r["counts"]["to_test"].as_u64().unwrap_or(0).min(total);
    // Fiche : champs manquants.
    let d = &p.data;
    let mut missing = Vec::new();
    for (v, l) in [
        (&d.client, "client"),
        (&d.venue, "lieu"),
        (&d.description, "description"),
        (&d.service_type, "type de prestation"),
    ] {
        if v.as_deref().is_none_or(|s| s.trim().is_empty()) {
            missing.push(l);
        }
    }
    if d.planning_start.is_none() || d.planning_end.is_none() {
        missing.push("période de planification");
    }
    if !missing.is_empty() {
        items.push(json!({ "kind": "sheet", "severity": "info", "message": format!("Fiche à compléter : {}.", missing.join(", ")), "href": base }));
    }
    let fiche = completion(
        d,
        inv.locations.len(),
        inv.containers.len(),
        inv.articles.len(),
    );
    // Chaque total dit ce qu'il compte : la note sous le pourcentage, le détail en infobulle.
    let line = |key: &str,
                label: &str,
                done: u64,
                total: u64,
                href: String,
                note: String,
                hint: String| json!({ "key": key, "label": label, "done": done, "total": total, "pct": (done * 100).checked_div(total).unwrap_or(100), "href": href, "note": note, "hint": hint });
    let pl = |n: u64, one: &str, many: &str| format!("{n} {}", if n > 1 { many } else { one });
    let n_points = |k: &str| {
        r[k].as_array().map_or(0, |gs| {
            gs.iter()
                .map(|g| g["points"].as_array().map_or(0, Vec::len) as u64)
                .sum::<u64>()
        })
    };
    let (rc_cables, rc_equip) = (n_points("cables"), n_points("equipment"));
    let rc_free = r["free"].as_array().map_or(0, Vec::len) as u64;
    let n_issues = issues.iter().filter(|i| i.severity != "info").count() as u64;
    let domains = vec![
        line(
            "material",
            "Matériel",
            housed as u64,
            equip.len() as u64,
            format!("{base}/inventaire"),
            format!(
                "{} rangée{} dans un élément sur {}",
                pl(housed as u64, "ligne d'équipement", "lignes d'équipement"),
                if housed > 1 { "s" } else { "" },
                equip.len()
            ),
            "Lignes d'équipement de l'inventaire (un modèle et sa quantité) rangées dans un élément : baie, pupitre, valise…".into(),
        ),
        line(
            "racks",
            "Racks",
            rack_done,
            rack_total,
            format!("{base}/racks"),
            format!(
                "{} en baie sur {rack_total}",
                pl(rack_done, "exemplaire posé", "exemplaires posés")
            ),
            "Exemplaires d'équipements rackables rangés dans une baie : posés à leur hauteur, ou encore à poser.".into(),
        ),
        line(
            "cabling",
            "Câblage",
            numbered as u64,
            real.len() as u64,
            format!("{base}/cables"),
            format!(
                "{} sur {} · {}",
                pl(numbered as u64, "câble numéroté", "câbles numérotés"),
                real.len(),
                pl(n_issues, "anomalie", "anomalies")
            ),
            "Câbles entre deux ports (les attaches d'adaptateurs ne comptent pas). Anomalies : erreurs et avertissements de la validation du synoptique.".into(),
        ),
        line(
            "validation",
            "Validation",
            tested,
            total,
            format!("{base}/recette"),
            format!(
                "{} sur {total} en recette",
                pl(tested, "point testé", "points testés")
            ),
            format!(
                "Points de recette : {}, {} et {}.",
                pl(rc_cables, "liaison (un par câble)", "liaisons (une par câble)"),
                pl(
                    rc_equip,
                    "équipement rangé dans un élément",
                    "équipements rangés dans un élément"
                ),
                pl(rc_free, "point libre", "points libres")
            ),
        ),
        line(
            "sheet",
            "Fiche",
            fiche as u64,
            100,
            base.clone(),
            format!("Fiche remplie à {fiche} %"),
            "Champs de la fiche renseignés, plus au moins un emplacement, un élément et du matériel.".into(),
        ),
    ];
    let rank = |v: &Value| match v["severity"].as_str() {
        Some("error") => 0,
        Some("warning") => 1,
        _ => 2,
    };
    items.sort_by_key(rank);
    Ok(Json(json!({ "domains": domains, "items": items })))
}
