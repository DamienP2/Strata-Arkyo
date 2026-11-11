//! Tests de bout en bout des racks (lot 5) : placement, contrôles, bilan électrique, verrou.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use std::net::SocketAddr;
use strata_api::{AppState, CSRF_HEADER, router};
use strata_store::Store;
use tower::ServiceExt;

async fn app() -> Router {
    let store = Store::open_in_memory().await.unwrap();
    router(AppState::new(store, false, None)).layer(MockConnectInfo(
        "127.0.0.1:50000".parse::<SocketAddr>().unwrap(),
    ))
}

struct Resp {
    status: StatusCode,
    body: Value,
    cookie: Option<String>,
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    cookie: Option<&str>,
) -> Resp {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header(CSRF_HEADER, "1")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
    }
    let res = app
        .clone()
        .oneshot(
            req.body(Body::from(body.map(|b| b.to_string()).unwrap_or_default()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.split(';').next().unwrap().to_string());
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    Resp {
        status,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        cookie,
    }
}

fn day(n: i64) -> String {
    (chrono::Local::now().date_naive() + chrono::Duration::days(n))
        .format("%Y-%m-%d")
        .to_string()
}

struct Setup {
    app: Router,
    o: String,
    lea: String,
    pid: String,
    cid: String,
    mixer: String,
    half: String,
    free: String,
}

async fn setup() -> Setup {
    let app = app().await;
    let o = call(
        &app,
        "POST",
        "/api/setup",
        Some(json!({"email": "chef@studio.fr", "name": "Chef", "password": "mot-de-passe-solide"})),
        None,
    )
    .await
    .cookie
    .unwrap();
    let r = call(
        &app,
        "POST",
        "/api/members",
        Some(json!({"email": "lea@studio.fr", "name": "Léa", "role_id": "technician"})),
        Some(&o),
    )
    .await;
    let temp = r.body["temporary_password"].as_str().unwrap().to_string();
    let lea = call(
        &app,
        "POST",
        "/api/auth/login",
        Some(json!({"login": "lea@studio.fr", "password": temp})),
        None,
    )
    .await
    .cookie
    .unwrap();
    call(
        &app,
        "POST",
        "/api/me/password",
        Some(json!({"current": temp, "new": "nouveau-secret"})),
        Some(&lea),
    )
    .await;
    let item = |name: &str, format: Value, w: f64| json!({"kind": "equipment", "data": {"name": name, "functions": ["V"], "format": format, "power_w": w, "weight_kg": 5}});
    let mut ids = Vec::new();
    for b in [
        item(
            "Mélangeur 2U",
            json!({"kind": "rack", "height_u": 2, "depth_mm": 500, "rear_reserve_mm": 150}),
            2300.0,
        ),
        item(
            "Convertisseur 1/2",
            json!({"kind": "rack", "height_u": 1, "width": "half", "depth_mm": 200}),
            30.0,
        ),
        item(
            "Moniteur de table",
            json!({"kind": "free", "length_mm": 300, "width_mm": 200, "height_mm": 150}),
            40.0,
        ),
    ] {
        ids.push(
            call(&app, "POST", "/api/catalog/items", Some(b), Some(&o))
                .await
                .body["item"]["id"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }
    let r = call(&app, "POST", "/api/projects", Some(json!({
        "name": "Finale", "intervention": "prestation", "regie": "car", "planning_start": day(1), "planning_end": day(2), "shoot_start": day(3), "shoot_end": day(4),
        "location": {"name": "Car régie"}, "container": {"name": "Baie V1", "kind": "rack", "rackable": true, "height_u": 12, "depth_mm": 600, "rear_rails_mm": 50}
    })), Some(&o)).await;
    let pid = r.body["project"]["id"].as_str().unwrap().to_string();
    let cid = r.body["container"]["id"].as_str().unwrap().to_string();
    let mut arts = Vec::new();
    for (item, q) in [(&ids[0], 3), (&ids[1], 3), (&ids[2], 1)] {
        let a = call(
            &app,
            "POST",
            &format!("/api/projects/{pid}/articles"),
            Some(
                json!({"container_id": cid, "nature": "equipment", "item_id": item, "quantity": q}),
            ),
            Some(&o),
        )
        .await;
        arts.push(a.body["article"]["id"].as_str().unwrap().to_string());
    }
    Setup {
        app,
        o,
        lea,
        pid,
        cid,
        mixer: arts[0].clone(),
        half: arts[1].clone(),
        free: arts[2].clone(),
    }
}

fn place(article: &str, u: i64) -> Value {
    json!({"article_id": article, "face": "front", "u_start": u})
}

#[tokio::test]
async fn placement_rules() {
    let Setup {
        app,
        o,
        cid,
        mixer,
        half,
        free,
        ..
    } = setup().await;
    let rack = call(
        &app,
        "GET",
        &format!("/api/containers/{cid}/rack"),
        None,
        Some(&o),
    )
    .await;
    assert_eq!(
        rack.body["pending"].as_array().unwrap().len(),
        7,
        "3 + 3 exemplaires + 1 non rackable"
    );
    assert_eq!(rack.body["faces"], json!(["front", "rear", "rear_inner"]));

    // Sans la main : refusé.
    let r = call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/placements"),
        Some(place(&mixer, 1)),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/containers/{cid}/rack/lock"),
            None,
            Some(&o)
        )
        .await
        .body["mine"],
        true
    );

    let a = call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/placements"),
        Some(place(&mixer, 1)),
        Some(&o),
    )
    .await;
    assert_eq!(a.status, StatusCode::CREATED, "{}", a.body);
    // Profondeur : 500 + 150 = 650 > 600.
    assert!(
        a.body["warning"].as_str().unwrap().contains("650 mm"),
        "{}",
        a.body
    );
    // Collision sur 2 : la place libre la plus proche est 3.
    let b = call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/placements"),
        Some(place(&mixer, 2)),
        Some(&o),
    )
    .await;
    assert_eq!(
        (
            b.body["placement"]["u_start"].as_i64(),
            b.body["adjusted"].as_bool()
        ),
        (Some(3), Some(true))
    );
    // Trop haut : ramené à 12 − 2 + 1 = 11.
    let c = call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/placements"),
        Some(place(&mixer, 50)),
        Some(&o),
    )
    .await;
    assert_eq!(c.body["placement"]["u_start"], 11);
    // Plus d'exemplaire à poser.
    let r = call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/placements"),
        Some(place(&mixer, 6)),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    // Demi-largeur : deux côte à côte, la troisième colonne n'existe pas.
    for col in [0, 1] {
        let r = call(
            &app,
            "POST",
            &format!("/api/containers/{cid}/placements"),
            Some(json!({"article_id": half, "face": "front", "u_start": 5, "col": col})),
            Some(&o),
        )
        .await;
        assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    }
    let r = call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/placements"),
        Some(json!({"article_id": half, "face": "front", "u_start": 5, "col": 2})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    // Non rackable.
    let r = call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/placements"),
        Some(place(&free, 7)),
        Some(&o),
    )
    .await;
    assert_eq!(r.body["message"], "Cet équipement se pose sur une étagère.");

    // Déplacer sur une position occupée.
    let pid = b.body["placement"]["id"].as_str().unwrap();
    let r = call(
        &app,
        "PATCH",
        &format!("/api/placements/{pid}"),
        Some(json!({"rev": 1, "face": "front", "u_start": 1, "width": "full"})),
        Some(&o),
    )
    .await;
    assert!(
        r.body["message"]
            .as_str()
            .unwrap()
            .starts_with("Position occupée par « Mélangeur 2U »"),
        "{}",
        r.body
    );
    // Face arrière intérieure : permise, rails arrière définis.
    let r = call(
        &app,
        "PATCH",
        &format!("/api/placements/{pid}"),
        Some(json!({"rev": 1, "face": "rear_inner", "u_start": 1, "width": "full"})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);

    // Bilan électrique : deux mélangeurs (2 × 2 300 W) sur un circuit 16 A en L1.
    let circ = call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/circuits"),
        Some(json!({"name": "PDU A · prise 1", "phase": "L1", "rating_a": 16})),
        Some(&o),
    )
    .await;
    let circ = circ.body["id"].as_str().unwrap().to_string();
    let rack = call(
        &app,
        "GET",
        &format!("/api/containers/{cid}/rack"),
        None,
        Some(&o),
    )
    .await;
    for e in rack.body["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["label"] == "Mélangeur 2U")
        .take(2)
    {
        let p = &e["placement"];
        call(&app, "PATCH", &format!("/api/placements/{}", p["id"].as_str().unwrap()), Some(json!({"rev": p["rev"], "face": p["face"], "u_start": p["u_start"], "width": "full", "circuit_id": circ})), Some(&o)).await;
    }
    let rack = call(
        &app,
        "GET",
        &format!("/api/containers/{cid}/rack"),
        None,
        Some(&o),
    )
    .await;
    assert_eq!(rack.body["electrical"]["circuits"][0]["amps"], 20.0);
    assert_eq!(rack.body["electrical"]["circuits"][0]["over"], true);
    assert_eq!(
        rack.body["electrical"]["unassigned_watts"], 2360.0,
        "le troisième mélangeur et les convertisseurs"
    );

    // Réduire la hauteur : la liste de ce qui dépasse, puis retrait après confirmation.
    let c = call(
        &app,
        "GET",
        &format!("/api/containers/{cid}/rack"),
        None,
        Some(&o),
    )
    .await
    .body["container"]
        .clone();
    let mut body = c.clone();
    body["height_u"] = json!(6);
    let r = call(
        &app,
        "PATCH",
        &format!("/api/containers/{cid}"),
        Some(body.clone()),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert!(
        r.body["message"]
            .as_str()
            .unwrap()
            .starts_with("1 élément(s) dépasseraient"),
        "{}",
        r.body
    );
    body["confirm_remove"] = json!(true);
    assert_eq!(
        call(
            &app,
            "PATCH",
            &format!("/api/containers/{cid}"),
            Some(body),
            Some(&o)
        )
        .await
        .status,
        StatusCode::OK
    );

    // Vider le rack.
    let r = call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/placements/clear"),
        None,
        Some(&o),
    )
    .await;
    assert_eq!(r.body["removed"], 4);
}

#[tokio::test]
async fn one_editor_at_a_time() {
    let Setup {
        app,
        o,
        lea,
        cid,
        mixer,
        ..
    } = setup().await;
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/containers/{cid}/rack/lock"),
            None,
            Some(&o)
        )
        .await
        .body["mine"],
        true
    );
    let r = call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/rack/lock"),
        None,
        Some(&lea),
    )
    .await;
    assert_eq!(
        (r.body["mine"].as_bool(), r.body["account_name"].as_str()),
        (Some(false), Some("Chef"))
    );
    let r = call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/placements"),
        Some(place(&mixer, 1)),
        Some(&lea),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert!(
        r.body["message"]
            .as_str()
            .unwrap()
            .starts_with("Chef a la main")
    );
    // Léa voit l'élévation en lecture : la main est indiquée.
    let v = call(
        &app,
        "GET",
        &format!("/api/containers/{cid}/rack"),
        None,
        Some(&lea),
    )
    .await;
    assert_eq!(
        (
            v.body["lock"]["account_name"].as_str(),
            v.body["lock"]["mine"].as_bool()
        ),
        (Some("Chef"), Some(false))
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/containers/{cid}/rack/lock/request"),
            None,
            Some(&lea)
        )
        .await
        .status,
        StatusCode::ACCEPTED
    );
    // Le chef cède la main ; Léa la prend.
    call(
        &app,
        "DELETE",
        &format!("/api/containers/{cid}/rack/lock"),
        None,
        Some(&o),
    )
    .await;
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/containers/{cid}/rack/lock"),
            None,
            Some(&lea)
        )
        .await
        .body["mine"],
        true
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/containers/{cid}/placements"),
            Some(place(&mixer, 1)),
            Some(&lea)
        )
        .await
        .status,
        StatusCode::CREATED
    );
    // Sa déconnexion libère la main.
    call(&app, "POST", "/api/auth/logout", None, Some(&lea)).await;
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/containers/{cid}/rack/lock"),
            None,
            Some(&o)
        )
        .await
        .body["mine"],
        true
    );
    // Consultation : pas le droit de concevoir.
    let r = call(
        &app,
        "POST",
        "/api/members",
        Some(json!({"email": "voir@studio.fr", "role_id": "viewer"})),
        Some(&o),
    )
    .await;
    let temp = r.body["temporary_password"].as_str().unwrap().to_string();
    let v = call(
        &app,
        "POST",
        "/api/auth/login",
        Some(json!({"login": "voir@studio.fr", "password": temp})),
        None,
    )
    .await
    .cookie
    .unwrap();
    call(
        &app,
        "POST",
        "/api/me/password",
        Some(json!({"current": temp, "new": "nouveau-secret"})),
        Some(&v),
    )
    .await;
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/containers/{cid}/rack/lock"),
            None,
            Some(&v)
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn shelves_carry_non_rack_equipment() {
    let Setup {
        app, o, cid, free, ..
    } = setup().await;
    let rack = call(
        &app,
        "GET",
        &format!("/api/containers/{cid}/rack"),
        None,
        Some(&o),
    )
    .await;
    let pid = rack.body["container"]["project_id"]
        .as_str()
        .unwrap()
        .to_string();
    let shelf = call(&app, "POST", &format!("/api/projects/{pid}/articles"), Some(json!({"container_id": cid, "nature": "accessory", "accessory": {"kind": "shelf", "depth_mm": 450, "height_u": 1}})), Some(&o)).await;
    let shelf_art = shelf.body["article"]["id"].as_str().unwrap().to_string();
    call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/rack/lock"),
        None,
        Some(&o),
    )
    .await;
    let s = call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/placements"),
        Some(place(&shelf_art, 8)),
        Some(&o),
    )
    .await;
    let sid = s.body["placement"]["id"].as_str().unwrap().to_string();
    let m = call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/placements"),
        Some(json!({"article_id": free, "face": "front", "u_start": 1, "shelf_id": sid})),
        Some(&o),
    )
    .await;
    assert_eq!(m.status, StatusCode::CREATED, "{}", m.body);
    assert_eq!(
        m.body["placement"]["u_start"], 8,
        "l'équipement prend la position de l'étagère"
    );
    // L'étagère monte : ce qu'elle porte la suit.
    let r = call(
        &app,
        "PATCH",
        &format!("/api/placements/{sid}"),
        Some(json!({"rev": 1, "face": "front", "u_start": 9, "width": "full"})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let rack = call(
        &app,
        "GET",
        &format!("/api/containers/{cid}/rack"),
        None,
        Some(&o),
    )
    .await;
    let on_shelf = rack.body["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["label"] == "Moniteur de table")
        .unwrap();
    assert_eq!(
        (
            on_shelf["placement"]["u_start"].as_i64(),
            on_shelf["placement"]["shelf_id"].as_str()
        ),
        (Some(9), Some(sid.as_str()))
    );
    // Retirer l'étagère retire ce qu'elle porte.
    call(
        &app,
        "DELETE",
        &format!("/api/placements/{sid}"),
        None,
        Some(&o),
    )
    .await;
    let rack = call(
        &app,
        "GET",
        &format!("/api/containers/{cid}/rack"),
        None,
        Some(&o),
    )
    .await;
    assert!(rack.body["elements"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn templates_round_trip() {
    let Setup {
        app,
        o,
        cid,
        mixer,
        half,
        ..
    } = setup().await;
    call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/rack/lock"),
        None,
        Some(&o),
    )
    .await;
    call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/placements"),
        Some(place(&mixer, 1)),
        Some(&o),
    )
    .await;
    call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/placements"),
        Some(json!({"article_id": half, "face": "rear", "u_start": 5, "col": 1})),
        Some(&o),
    )
    .await;
    call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/circuits"),
        Some(json!({"name": "PDU A", "phase": "L2", "rating_a": 16})),
        Some(&o),
    )
    .await;

    let t = call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/save-as-template"),
        Some(json!({"name": "Baie vidéo type"})),
        Some(&o),
    )
    .await;
    assert_eq!(t.status, StatusCode::CREATED, "{}", t.body);
    assert_eq!(
        (t.body["status"].as_str(), t.body["cable_digit"].as_i64()),
        (Some("draft"), Some(1))
    );
    assert_eq!(t.body["summary"]["front_pct"], 17.0, "2 U sur 12");
    let tid = t.body["id"].as_str().unwrap().to_string();
    // Même nom : suffixe automatique.
    let t2 = call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/save-as-template"),
        Some(json!({"name": "baie vidéo TYPE"})),
        Some(&o),
    )
    .await;
    assert_eq!(t2.body["name"], "baie vidéo TYPE 2");

    // Un brouillon ne se copie pas.
    let rack = call(
        &app,
        "GET",
        &format!("/api/containers/{cid}/rack"),
        None,
        Some(&o),
    )
    .await;
    let pid = rack.body["container"]["project_id"]
        .as_str()
        .unwrap()
        .to_string();
    let lid = rack.body["container"]["location_id"]
        .as_str()
        .unwrap()
        .to_string();
    let body = json!({"template_id": tid, "location_id": lid});
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/projects/{pid}/containers/from-template"),
            Some(body.clone()),
            Some(&o)
        )
        .await
        .status,
        StatusCode::CONFLICT
    );
    call(
        &app,
        "PATCH",
        &format!("/api/templates/{tid}"),
        Some(json!({"rev": 1, "status": "active"})),
        Some(&o),
    )
    .await;
    let list = call(&app, "GET", "/api/templates?status=active", None, Some(&o)).await;
    assert_eq!(list.body["templates"].as_array().unwrap().len(), 1);

    // Copie : contenant, articles, exemplaires, circuits et placements.
    let made = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/containers/from-template"),
        Some(body),
        Some(&o),
    )
    .await;
    assert_eq!(made.status, StatusCode::CREATED, "{}", made.body);
    assert_eq!(made.body["container"]["name"], "Baie vidéo type");
    let ncid = made.body["container"]["id"].as_str().unwrap();
    let copy = call(
        &app,
        "GET",
        &format!("/api/containers/{ncid}/rack"),
        None,
        Some(&o),
    )
    .await;
    let els = copy.body["elements"].as_array().unwrap();
    assert_eq!(els.len(), 2);
    assert!(els.iter().any(|e| e["placement"]["face"] == "rear"
        && e["placement"]["col"] == 1
        && e["placement"]["u_start"] == 5));
    assert_eq!(copy.body["circuits"][0]["phase"], "L2");
    assert_eq!(
        copy.body["pending"].as_array().unwrap().len(),
        5,
        "les autres exemplaires restent à placer"
    );
    // L'original n'a pas bougé.
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/containers/{cid}/rack"),
            None,
            Some(&o)
        )
        .await
        .body["elements"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    // Technicien : catalogue en lecture, la bibliothèque est en lecture seule.
    let lea = {
        let r = call(
            &app,
            "POST",
            "/api/members",
            Some(json!({"email": "x@studio.fr", "role_id": "technician"})),
            Some(&o),
        )
        .await;
        let temp = r.body["temporary_password"].as_str().unwrap().to_string();
        let c = call(
            &app,
            "POST",
            "/api/auth/login",
            Some(json!({"login": "x@studio.fr", "password": temp})),
            None,
        )
        .await
        .cookie
        .unwrap();
        call(
            &app,
            "POST",
            "/api/me/password",
            Some(json!({"current": temp, "new": "nouveau-secret"})),
            Some(&c),
        )
        .await;
        c
    };
    assert_eq!(
        call(
            &app,
            "DELETE",
            &format!("/api/templates/{tid}"),
            None,
            Some(&lea)
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn patch_panels_from_library_to_rack() {
    let Setup {
        app,
        o,
        lea,
        pid,
        cid,
        ..
    } = setup().await;
    let slot = |i: u32, c: &str, d: bool| json!({"index": i, "connector": c, "label": format!("IN {i}"), "d_format": d});
    let panel = json!({"name": "Patch vidéo", "height_u": 1, "per_u": 16, "rear_depth_mm": 80,
        "slots": [slot(1, "bnc", true), slot(2, "bnc", true), slot(3, "dsub9", true)],
        "groups": [{"row": 1, "start": 1, "len": 2, "label": "Caméras", "color": "#2459E0"}]});

    // Léa (technicienne) n'écrit pas dans la bibliothèque.
    let r = call(
        &app,
        "POST",
        "/api/panels",
        Some(json!({"panel": panel})),
        Some(&lea),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = call(
        &app,
        "POST",
        "/api/panels",
        Some(json!({"panel": panel})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let lib = r.body.clone();
    let id = lib["id"].as_str().unwrap();
    assert_eq!(lib["stats"]["used"], 3);
    // Le D-Sub 9 n'existe pas en format D : le format est retiré.
    assert_eq!(lib["panel"]["slots"][2]["d_format"], false);
    assert_eq!(lib["panel"]["slots"][0]["d_format"], true);

    // Chevauchement et nom pris.
    let mut bad = panel.clone();
    bad["slots"][1]["index"] = json!(1);
    let r = call(
        &app,
        "PUT",
        &format!("/api/panels/{id}"),
        Some(json!({"rev": lib["rev"], "panel": bad})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let r = call(
        &app,
        "POST",
        "/api/panels",
        Some(json!({"panel": panel})),
        Some(&o),
    )
    .await;
    assert_eq!(r.body["fields"][0]["field"], "name");
    let r = call(
        &app,
        "POST",
        &format!("/api/panels/{id}/duplicate"),
        None,
        Some(&o),
    )
    .await;
    assert_eq!(r.body["panel"]["name"], "Patch vidéo (copie)");

    // Copie dans le projet, pose dans la baie, modification sans toucher la bibliothèque.
    let r = call(&app, "POST", &format!("/api/projects/{pid}/articles"),
        Some(json!({"container_id": cid, "nature": "accessory", "accessory": {"kind": "panel", "panel": lib["panel"]}})), Some(&o)).await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let art = r.body["article"]["id"].as_str().unwrap().to_string();
    call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/rack/lock"),
        None,
        Some(&o),
    )
    .await;
    let r = call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/placements"),
        Some(place(&art, 12)),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let rack = call(
        &app,
        "GET",
        &format!("/api/containers/{cid}/rack"),
        None,
        Some(&o),
    )
    .await;
    let el = rack.body["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["placement"]["article_id"] == art)
        .unwrap()
        .clone();
    assert_eq!(el["label"], "Patch vidéo");
    assert_eq!(
        el["accessory"]["panel"]["slots"].as_array().unwrap().len(),
        3
    );

    let mut edited = lib["panel"].clone();
    edited["slots"][0]["label"] = json!("CAM 1");
    let r = call(
        &app,
        "PUT",
        &format!("/api/articles/{art}/panel"),
        Some(json!({"rev": 1, "panel": edited})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(
        r.body["article"]["accessory"]["panel"]["slots"][0]["label"],
        "CAM 1"
    );
    let mut taller = edited.clone();
    taller["height_u"] = json!(2);
    let r = call(
        &app,
        "PUT",
        &format!("/api/articles/{art}/panel"),
        Some(json!({"rev": 2, "panel": taller})),
        Some(&o),
    )
    .await;
    assert_eq!(
        r.body["fields"][0]["field"], "height_u",
        "posé : la hauteur ne change pas"
    );
    let r = call(&app, "GET", &format!("/api/panels/{id}"), None, Some(&o)).await;
    assert_eq!(
        r.body["panel"]["slots"][0]["label"], "IN 1",
        "la bibliothèque n'a pas bougé"
    );

    let r = call(&app, "DELETE", &format!("/api/panels/{id}"), None, Some(&o)).await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let r = call(&app, "GET", "/api/panels", None, Some(&o)).await;
    assert_eq!(r.body["panels"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn regies_main_custom_and_library() {
    let Setup {
        app,
        o,
        lea,
        pid,
        cid,
        ..
    } = setup().await;
    let r = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/regies"),
        None,
        Some(&lea),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let main = r.body["regies"][0].clone();
    assert_eq!(main["name"], "Finale — Régie principale");
    assert_eq!(main["kind"], "car");
    assert_eq!(main["main"], true);
    assert_eq!(main["members"][0]["id"], cid.as_str());
    assert_eq!(main["members"][0]["data"]["kind"], "rack");
    assert_eq!(main["totals"]["containers"], 1);
    assert_eq!(main["totals"]["total_u"], 12.0);
    let main_id = main["id"].as_str().unwrap().to_string();

    // Régie libre : doublon et contenant étranger refusés ; la principale ne se supprime pas.
    let r = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/regies"),
        Some(json!({"name": "Plateau", "kind": "flight_case", "members": [cid, cid]})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let r = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/regies"),
        Some(json!({"name": "Plateau", "kind": "flight_case", "members": ["inconnu"]})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let r = call(&app, "POST", &format!("/api/projects/{pid}/regies"), Some(json!({"name": "Plateau", "kind": "flight_case", "service_type": "Sport", "members": [cid]})), Some(&o)).await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let plateau = r.body["id"].as_str().unwrap().to_string();
    let r = call(
        &app,
        "PATCH",
        &format!("/api/regies/{plateau}"),
        Some(json!({"rev": 1, "name": "Plateau B", "members": []})),
        Some(&o),
    )
    .await;
    assert_eq!(r.body["name"], "Plateau B");
    assert_eq!(r.body["totals"]["containers"], 0);
    let r = call(
        &app,
        "PATCH",
        &format!("/api/regies/{plateau}"),
        Some(json!({"rev": 1, "name": "X"})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "révision dépassée");
    let r = call(
        &app,
        "DELETE",
        &format!("/api/regies/{main_id}"),
        None,
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);

    // Modèle de régie : Léa n'écrit pas dans la bibliothèque.
    let r = call(
        &app,
        "POST",
        &format!("/api/regies/{main_id}/save-as-template"),
        Some(json!({})),
        Some(&lea),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = call(
        &app,
        "POST",
        &format!("/api/regies/{main_id}/save-as-template"),
        Some(json!({})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["name"], "Finale");
    assert_eq!(r.body["project_id"], Value::Null);
    assert_eq!(r.body["members"][0]["name"], "Finale · Baie V1");
    let lib = r.body["id"].as_str().unwrap().to_string();
    let r = call(&app, "GET", "/api/regies", None, Some(&o)).await;
    assert_eq!(r.body["regies"].as_array().unwrap().len(), 1);

    // Copie dans le projet : un nouveau contenant, une nouvelle régie, la principale grandit.
    let p = call(&app, "GET", &format!("/api/projects/{pid}"), None, Some(&o)).await;
    let loc = p.body["inventory"]["locations"][0]["id"]
        .as_str()
        .or(p.body["locations"][0]["id"].as_str())
        .unwrap()
        .to_string();
    let r = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/regies/from-template"),
        Some(json!({"regie_id": lib, "location_id": loc})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["regie"]["members"][0]["name"], "Baie V1 2");
    let r = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/regies"),
        None,
        Some(&o),
    )
    .await;
    assert_eq!(r.body["regies"][0]["totals"]["containers"], 2);
    assert_eq!(r.body["regies"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn build_checks_follow_the_inventory() {
    let Setup {
        app,
        o,
        lea,
        pid,
        cid,
        mixer,
        free,
        ..
    } = setup().await;
    call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/rack/lock"),
        None,
        Some(&o),
    )
    .await;
    call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/placements"),
        Some(place(&mixer, 5)),
        Some(&o),
    )
    .await;
    let url = format!("/api/containers/{cid}/checks");
    let r = call(&app, "GET", &url, None, Some(&lea)).await;
    assert_eq!(r.status, StatusCode::OK);
    let keys: Vec<String> = r.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["key"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(keys.len(), 4, "la baie et trois articles");
    assert_eq!(keys[0], "rack");
    assert_eq!(
        keys[1],
        format!("article:{mixer}"),
        "le posé avant le reste"
    );
    assert!(
        r.body["items"][1]["detail"]
            .as_str()
            .unwrap()
            .contains("U5")
    );
    assert_eq!(r.body["progress"], json!({"done": 0, "total": 4}));

    // Léa coche en glissant sur deux lignes ; une clé inconnue est ignorée.
    let r = call(
        &app,
        "POST",
        &url,
        Some(json!({"keys": ["rack", format!("article:{mixer}"), "conn:x"], "checked": true})),
        Some(&lea),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["progress"]["done"], 2);
    assert_eq!(r.body["items"][0]["checked"]["name"], "Léa");
    assert_eq!(
        r.body["families"][0],
        json!({"key": "materiel", "label": "Matériel", "done": 2, "total": 4})
    );
    let r = call(
        &app,
        "POST",
        &url,
        Some(json!({"keys": ["rack"], "checked": false})),
        Some(&o),
    )
    .await;
    assert_eq!(r.body["progress"]["done"], 1);

    // Un article supprimé disparaît de la liste, sa coche avec.
    call(
        &app,
        "POST",
        &url,
        Some(json!({"keys": [format!("article:{free}")], "checked": true})),
        Some(&o),
    )
    .await;
    call(
        &app,
        "DELETE",
        &format!("/api/articles/{free}"),
        None,
        Some(&o),
    )
    .await;
    let r = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/checks"),
        None,
        Some(&o),
    )
    .await;
    assert_eq!(r.body["containers"][0]["done"], 1);
    assert_eq!(r.body["containers"][0]["total"], 3);

    let r = call(&app, "DELETE", &url, None, Some(&o)).await;
    assert_eq!(r.body["progress"], json!({"done": 0, "total": 3}));
}

#[tokio::test]
async fn monitor_wall_element_keeps_its_screens_through_templates() {
    let Setup {
        app, o, pid, mixer, ..
    } = setup().await;
    let p = call(&app, "GET", &format!("/api/projects/{pid}"), None, Some(&o)).await;
    let loc = p.body["locations"][0]["id"].as_str().unwrap().to_string();
    let r = call(&app, "POST", &format!("/api/projects/{pid}/containers"), Some(json!({
        "location_id": loc, "name": "Mur réal", "kind": "monitor_wall", "rackable": true, "height_u": 42,
        "details": {"wall": {"rows": 2, "cols": 3, "support": "Structure alu"}, "power_kva": 12}
    })), Some(&o)).await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert_eq!(r.body["rackable"], false, "un mur n'est pas rackable");
    assert_eq!(
        r.body["details"]["power_kva"],
        Value::Null,
        "champ d'un autre type écarté"
    );
    let wall = r.body["id"].as_str().unwrap().to_string();
    let item = call(&app, "POST", "/api/catalog/items", Some(json!({"kind": "equipment", "data": {"name": "Moniteur 55 pouces", "functions": ["V"], "format": {"kind": "free", "length_mm": 1230, "width_mm": 80, "height_mm": 710}, "power_w": 150, "weight_kg": 18}})), Some(&o)).await;
    let item = item.body["item"]["id"].as_str().unwrap().to_string();
    call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/articles"),
        Some(json!({"container_id": wall, "nature": "equipment", "item_id": item, "quantity": 2})),
        Some(&o),
    )
    .await;
    let p = call(&app, "GET", &format!("/api/projects/{pid}"), None, Some(&o)).await;
    let arts = p.body["articles"].as_array().unwrap();
    let screen_art = arts
        .iter()
        .find(|a| a["container_id"] == wall.as_str())
        .unwrap()["id"]
        .clone();
    let inst: Vec<String> = p.body["instances"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["article_id"] == screen_art)
        .map(|i| i["id"].as_str().unwrap().to_string())
        .collect();
    let foreign = p.body["instances"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["article_id"] == mixer.as_str())
        .unwrap()["id"]
        .clone();
    let c = p.body["containers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == wall.as_str())
        .unwrap()
        .clone();
    let r = call(
        &app,
        "PATCH",
        &format!("/api/containers/{wall}"),
        Some(json!({
            "rev": c["rev"], "location_id": loc, "name": "Mur réal", "kind": "monitor_wall",
            "details": {"wall": {"rows": 2, "cols": 3, "tiles": [
                {"row": 1, "col": 1, "w": 2, "h": 2, "instance_id": inst[0], "source": "PGM"},
                {"row": 1, "col": 3, "instance_id": inst[1], "source": "PVW"},
                {"row": 2, "col": 3, "instance_id": foreign, "source": "CAM 1"}]}}
        })),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let tiles = r.body["details"]["wall"]["tiles"].clone();
    assert_eq!(tiles[0]["instance_id"], inst[0].as_str());
    assert_eq!(
        tiles[2]["instance_id"],
        Value::Null,
        "un écran d'un autre élément est refusé"
    );

    // Modèle puis copie : chaque tuile retrouve l'écran recréé.
    let t = call(
        &app,
        "POST",
        &format!("/api/containers/{wall}/save-as-template"),
        Some(json!({})),
        Some(&o),
    )
    .await;
    assert_eq!(t.status, StatusCode::CREATED, "{:?}", t.body);
    let tid = t.body["id"].as_str().unwrap();
    call(
        &app,
        "PATCH",
        &format!("/api/templates/{tid}"),
        Some(json!({"rev": t.body["rev"], "status": "active"})),
        Some(&o),
    )
    .await;
    let r = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/containers/from-template"),
        Some(json!({"template_id": tid, "location_id": loc})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let copy = r.body["container"].clone();
    let p = call(&app, "GET", &format!("/api/projects/{pid}"), None, Some(&o)).await;
    let copy_art = p.body["articles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["container_id"] == copy["id"])
        .unwrap()["id"]
        .clone();
    let copy_inst: Vec<&str> = p.body["instances"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["article_id"] == copy_art)
        .map(|i| i["id"].as_str().unwrap())
        .collect();
    let t0 = copy["details"]["wall"]["tiles"][0]["instance_id"]
        .as_str()
        .unwrap();
    assert!(copy_inst.contains(&t0) && !inst.iter().any(|i| i == t0));
    assert_eq!(copy["details"]["wall"]["tiles"][0]["source"], "PGM");

    let r = call(
        &app,
        "GET",
        &format!("/api/containers/{wall}/checks"),
        None,
        Some(&o),
    )
    .await;
    assert!(
        r.body["items"][0]["detail"]
            .as_str()
            .unwrap()
            .starts_with("Mur d'écrans 2 × 3")
    );
}

#[tokio::test]
async fn sheet_settings_are_kept_per_project() {
    let Setup {
        app, o, lea, pid, ..
    } = setup().await;
    let url = format!("/api/projects/{pid}/settings/rack_sheet");
    let r = call(&app, "GET", &url, None, Some(&lea)).await;
    assert_eq!(r.body, json!({"data": null, "rev": 0}));
    let r = call(
        &app,
        "PUT",
        &url,
        Some(json!({"rev": 0, "data": {"paper": "A3"}})),
        Some(&lea),
    )
    .await;
    assert_eq!(r.body["rev"], 1);
    let r = call(
        &app,
        "PUT",
        &url,
        Some(json!({"rev": 0, "data": {"paper": "A1"}})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "révision dépassée");
    let r = call(&app, "GET", &url, None, Some(&o)).await;
    assert_eq!(r.body["data"]["paper"], "A3");
    let r = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/settings/inconnu"),
        None,
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);

    assert_eq!(
        call(&app, "GET", "/api/site", None, Some(&lea)).await.body["name"],
        "Strata Arkyo"
    );
    let r = call(
        &app,
        "PUT",
        "/api/site",
        Some(json!({"name": "Studios Atlantique"})),
        Some(&lea),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    call(
        &app,
        "PUT",
        "/api/site",
        Some(json!({"name": "Studios Atlantique"})),
        Some(&o),
    )
    .await;
    assert_eq!(
        call(&app, "GET", "/api/site", None, Some(&lea)).await.body["name"],
        "Studios Atlantique"
    );
}

#[tokio::test]
async fn synoptic_cables_port_to_port() {
    let Setup {
        app,
        o,
        lea,
        pid,
        cid,
        ..
    } = setup().await;
    let item = |name: &str, h: u32, ports: Value| json!({"kind": "equipment", "data": {"name": name, "functions": ["V"], "format": {"kind": "rack", "height_u": h, "depth_mm": 300}, "ports": ports}});
    let cam = call(&app, "POST", "/api/catalog/items", Some(item("Convertisseur fibre", 1, json!([
        {"connector_id": "bnc", "signal_id": "sdi-3g", "direction": "out", "gender": "F", "quantity": 2, "label": "SDI OUT"},
        {"connector_id": "xlr3", "signal_id": "aes3", "direction": "out", "gender": "M", "quantity": 1}
    ]))), Some(&o)).await.body["item"]["id"].as_str().unwrap().to_string();
    let mix = call(&app, "POST", "/api/catalog/items", Some(item("Mélangeur 1 M/E", 2, json!([
        {"connector_id": "bnc", "signal_id": "sdi-3g", "direction": "in", "gender": "F", "quantity": 4, "label": "IN"}
    ]))), Some(&o)).await.body["item"]["id"].as_str().unwrap().to_string();
    let mut arts = Vec::new();
    for it in [&cam, &mix] {
        arts.push(
            call(
                &app,
                "POST",
                &format!("/api/projects/{pid}/articles"),
                Some(json!({"container_id": cid, "nature": "equipment", "item_id": it})),
                Some(&o),
            )
            .await
            .body["article"]["id"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }
    let view = format!("container:{cid}");
    let r = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/synoptic?view={view}"),
        None,
        Some(&lea),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let nodes = r.body["nodes"].as_array().unwrap();
    let node = |name: &str| nodes.iter().find(|n| n["label"] == name).unwrap().clone();
    let conv = node("Convertisseur fibre");
    let mixer = node("Mélangeur 1 M/E");
    assert_eq!(conv["ports"].as_array().unwrap().len(), 3);
    assert_eq!(conv["ports"][1]["label"], "SDI OUT 2");
    assert_eq!(conv["ports"][0]["side"], "r");
    assert_eq!(mixer["ports"][0]["side"], "l");
    let out1 = conv["ports"][0]["ref"].as_str().unwrap().to_string();
    let aes = conv["ports"][2]["ref"].as_str().unwrap().to_string();
    let in1 = mixer["ports"][0]["ref"].as_str().unwrap().to_string();
    let in2 = mixer["ports"][1]["ref"].as_str().unwrap().to_string();

    // Sans la main : refus. Léa (technicienne) n'a pas le droit de conception.
    let url = format!("/api/projects/{pid}/cables");
    let r = call(
        &app,
        "POST",
        &url,
        Some(json!({"cables": [{"a": out1, "b": in1}]})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    let r = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/synoptic/lock"),
        None,
        Some(&lea),
    )
    .await;
    // Léa (technicienne) prend la main : le chef est en lecture seule jusqu'à ce qu'elle la rende.
    assert_eq!(r.body["mine"], true);
    let lock = format!("/api/projects/{pid}/synoptic/lock");
    let r = call(&app, "POST", &lock, None, Some(&o)).await;
    assert_eq!(r.body["mine"], false);
    assert_eq!(r.body["account_name"], "Léa");
    call(&app, "DELETE", &lock, None, Some(&lea)).await;
    let r = call(&app, "POST", &lock, None, Some(&o)).await;
    assert_eq!(r.body["mine"], true);

    let r = call(
        &app,
        "POST",
        &url,
        Some(json!({"cables": [{"a": out1, "b": in1, "number": "V-0001"}, {"a": aes, "b": in2}]})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    assert!(r.body["warnings"][0].as_array().unwrap().is_empty());
    let w: Vec<String> = serde_json::from_value(r.body["warnings"][1].clone()).unwrap();
    assert!(
        w.iter().any(|m| m.contains("Familles")) && w.iter().any(|m| m.contains("Connecteurs")),
        "{w:?}"
    );

    // Port occupé : refus nommé, puis remplacement confirmé.
    let r = call(
        &app,
        "POST",
        &url,
        Some(json!({"cables": [{"a": out1, "b": in2}]})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(r.body["code"], "occupied");
    assert_eq!(r.body["ports"].as_array().unwrap().len(), 2);
    let r = call(
        &app,
        "POST",
        &url,
        Some(json!({"cables": [{"a": out1, "b": in2}], "replace": true})),
        Some(&o),
    )
    .await;
    assert_eq!(r.body["replaced"].as_array().unwrap().len(), 2);
    let r = call(
        &app,
        "POST",
        &url,
        Some(json!({"cables": [{"a": "i:inconnu:0:1", "b": in1}]})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);

    // Longueur estimée une fois les deux équipements posés dans la baie.
    call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/rack/lock"),
        None,
        Some(&o),
    )
    .await;
    call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/placements"),
        Some(place(&arts[0], 1)),
        Some(&o),
    )
    .await;
    call(
        &app,
        "POST",
        &format!("/api/containers/{cid}/placements"),
        Some(place(&arts[1], 8)),
        Some(&o),
    )
    .await;
    let r = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/synoptic?view={view}"),
        None,
        Some(&o),
    )
    .await;
    let c = r.body["cables"][0].clone();
    assert_eq!(c["estimate_m"], 1.0, "{c:?}");
    let id = c["id"].as_str().unwrap();
    let r = call(
        &app,
        "PATCH",
        &format!("/api/cables/{id}"),
        Some(json!({"rev": c["rev"], "number": "V-0002", "color": "#2459E0", "notes": "Repris"})),
        Some(&o),
    )
    .await;
    assert_eq!(r.body["number"], "V-0002");
    call(
        &app,
        "PUT",
        &format!("/api/projects/{pid}/synoptic/layout"),
        Some(json!({"view": view, "positions": [{"node": conv["key"], "x": 40.4, "y": 80}]})),
        Some(&o),
    )
    .await;
    let r = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/synoptic?view={view}"),
        None,
        Some(&o),
    )
    .await;
    assert_eq!(
        r.body["layout"][conv["key"].as_str().unwrap()],
        json!({"x": 40.0, "y": 80.0})
    );
    assert_eq!(r.body["lock"]["mine"], true);
    let r = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/cables/delete"),
        Some(json!({"ids": [id]})),
        Some(&o),
    )
    .await;
    assert_eq!(r.body["deleted"], 1);
}

#[tokio::test]
async fn numbering_and_validation() {
    let Setup {
        app, o, pid, cid, ..
    } = setup().await;
    let item = |name: &str, ports: Value| json!({"kind": "equipment", "data": {"name": name, "functions": ["V"], "format": {"kind": "rack", "height_u": 1, "depth_mm": 300}, "ports": ports}});
    let conv = call(
        &app,
        "POST",
        "/api/catalog/items",
        Some(item(
            "Convertisseur",
            json!([
                {"connector_id": "bnc", "signal_id": "sdi-3g", "direction": "out", "quantity": 3},
                {"connector_id": "xlr3", "signal_id": "aes3", "direction": "out", "gender": "M"}
            ]),
        )),
        Some(&o),
    )
    .await
    .body["item"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let rec = call(
        &app,
        "POST",
        "/api/catalog/items",
        Some(item(
            "Enregistreur",
            json!([
                {"connector_id": "bnc", "signal_id": "sdi-3g", "direction": "in", "quantity": 6}
            ]),
        )),
        Some(&o),
    )
    .await
    .body["item"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    for it in [&conv, &rec] {
        call(
            &app,
            "POST",
            &format!("/api/projects/{pid}/articles"),
            Some(json!({"container_id": cid, "nature": "equipment", "item_id": it})),
            Some(&o),
        )
        .await;
    }
    call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/synoptic/lock"),
        None,
        Some(&o),
    )
    .await;
    let view = format!("container:{cid}");
    let s = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/synoptic?view={view}"),
        None,
        Some(&o),
    )
    .await
    .body;
    let node = |l: &str| {
        s["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["label"] == l)
            .unwrap()
            .clone()
    };
    let (c, r) = (node("Convertisseur"), node("Enregistreur"));
    let p = |n: &Value, i: usize| n["ports"][i]["ref"].as_str().unwrap().to_string();
    let made = call(&app, "POST", &format!("/api/projects/{pid}/cables"), Some(json!({"cables": [
        {"a": p(&c, 0), "b": p(&r, 0)}, {"a": p(&r, 1), "b": p(&c, 1)}, {"a": p(&c, 3), "b": p(&r, 2)}
    ]})), Some(&o)).await.body;
    let ids: Vec<String> = made["created"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_str().unwrap().to_string())
        .collect();

    let url = format!("/api/projects/{pid}/numbering");
    let r1 = call(
        &app,
        "POST",
        &url,
        Some(json!({"mode": "missing", "view": view})),
        Some(&o),
    )
    .await;
    assert_eq!(r1.status, StatusCode::OK, "{:?}", r1.body);
    assert_eq!(r1.body["changed"], 3);
    let num = |b: &Value, id: &str| {
        b["numbers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["id"] == id)
            .unwrap()["number"]
            .as_str()
            .unwrap()
            .to_string()
    };
    // La sortie du convertisseur sert de référence, même tracée depuis l'entrée.
    assert_eq!(num(&r1.body, &ids[0]), "BAIEV-V20001");
    assert_eq!(num(&r1.body, &ids[1]), "BAIEV-V20002");
    assert_eq!(num(&r1.body, &ids[2]), "BAIEV-A20001");
    let r2 = call(
        &app,
        "POST",
        &url,
        Some(json!({"mode": "missing", "view": view})),
        Some(&o),
    )
    .await;
    assert_eq!(r2.body["changed"], 0, "déjà numérotés");

    // Un numéro n'est jamais recyclé.
    call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/cables/delete"),
        Some(json!({"ids": [ids[1]]})),
        Some(&o),
    )
    .await;
    let n4 = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/cables"),
        Some(json!({"cables": [{"a": p(&c, 2), "b": p(&r, 3)}]})),
        Some(&o),
    )
    .await
    .body["created"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let r3 = call(
        &app,
        "POST",
        &url,
        Some(json!({"mode": "missing"})),
        Some(&o),
    )
    .await;
    assert_eq!(num(&r3.body, &n4), "BAIEV-V20003");

    // Règles du site, puis réglages ponctuels.
    call(
        &app,
        "PUT",
        "/api/site/numbering",
        Some(json!({"with_rack": false, "digits": 3})),
        Some(&o),
    )
    .await;
    let r4 = call(
        &app,
        "POST",
        &url,
        Some(json!({"mode": "selection", "ids": [n4]})),
        Some(&o),
    )
    .await;
    assert_eq!(num(&r4.body, &n4), "V204");
    let r5 = call(&app, "POST", &url, Some(json!({"mode": "selection", "ids": [n4], "rules": {"template": "{prefix}{seq}·{rack}", "digits": 4}})), Some(&o)).await;
    assert_eq!(num(&r5.body, &n4), "V005·BAIEV");
    let bad = call(
        &app,
        "POST",
        &url,
        Some(json!({"mode": "all", "rules": {"digits": 12}})),
        Some(&o),
    )
    .await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);

    // Saisie manuelle, doublon, validation, effacement.
    let r6 = call(
        &app,
        "POST",
        &url,
        Some(json!({"mode": "manual", "ids": [ids[0], n4], "first": "TX-0099"})),
        Some(&o),
    )
    .await;
    assert_eq!(
        (num(&r6.body, &ids[0]), num(&r6.body, &n4)),
        ("TX-0099".to_string(), "TX-0100".to_string())
    );
    call(
        &app,
        "POST",
        &url,
        Some(json!({"mode": "manual", "ids": [ids[2]], "first": "TX-0099"})),
        Some(&o),
    )
    .await;
    let v = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/synoptic/validate"),
        None,
        Some(&o),
    )
    .await
    .body;
    let codes: Vec<u64> = v["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["code"].as_u64().unwrap())
        .collect();
    assert!(codes.contains(&2), "numéro en double : {codes:?}");
    assert!(codes.contains(&4), "AES vers SDI : {codes:?}");
    assert!(
        !codes.contains(&7),
        "tout équipement à ports est câblé ; ceux du setup n'ont pas de port : {codes:?}"
    );
    assert!(v["counts"]["error"].as_u64().unwrap() >= 2);
    let r7 = call(
        &app,
        "POST",
        &url,
        Some(json!({"mode": "clear", "ids": [ids[2]]})),
        Some(&o),
    )
    .await;
    assert_eq!(r7.body["numbers"][0]["number"], Value::Null);
    let v = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/synoptic/validate"),
        None,
        Some(&o),
    )
    .await
    .body;
    assert!(
        v["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["code"] == 5),
        "sans numéro"
    );
}

#[tokio::test]
async fn cable_table_bulk_edit_and_export() {
    let Setup {
        app,
        o,
        lea,
        pid,
        cid,
        ..
    } = setup().await;
    let item = |name: &str, ports: Value| json!({"kind": "equipment", "data": {"name": name, "functions": ["V"], "format": {"kind": "rack", "height_u": 1, "depth_mm": 300}, "ports": ports}});
    let a = call(&app, "POST", "/api/catalog/items", Some(item("Source", json!([{"connector_id": "bnc", "signal_id": "sdi-3g", "direction": "out", "quantity": 3}]))), Some(&o)).await.body["item"]["id"].as_str().unwrap().to_string();
    let b = call(&app, "POST", "/api/catalog/items", Some(item("Moniteur", json!([{"connector_id": "bnc", "signal_id": "sdi-3g", "direction": "in", "quantity": 3}]))), Some(&o)).await.body["item"]["id"].as_str().unwrap().to_string();
    for it in [&a, &b] {
        call(
            &app,
            "POST",
            &format!("/api/projects/{pid}/articles"),
            Some(json!({"container_id": cid, "nature": "equipment", "item_id": it})),
            Some(&o),
        )
        .await;
    }
    call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/synoptic/lock"),
        None,
        Some(&o),
    )
    .await;
    let s = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/synoptic?view=container:{cid}"),
        None,
        Some(&o),
    )
    .await
    .body;
    let node = |l: &str| {
        s["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["label"] == l)
            .unwrap()
            .clone()
    };
    let (src, dst) = (node("Source"), node("Moniteur"));
    let p = |n: &Value, i: usize| n["ports"][i]["ref"].as_str().unwrap().to_string();
    call(&app, "POST", &format!("/api/projects/{pid}/cables"), Some(json!({"cables": [
        {"a": p(&src, 0), "b": p(&dst, 0), "number": "V-10"}, {"a": p(&src, 1), "b": p(&dst, 1), "number": "V-2"}, {"a": p(&src, 2), "b": p(&dst, 2)}
    ]})), Some(&o)).await;

    let t = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/cables/table"),
        None,
        Some(&lea),
    )
    .await
    .body;
    let rows = t["rows"].as_array().unwrap().clone();
    assert_eq!(
        rows.iter().map(|r| r["number"].clone()).collect::<Vec<_>>(),
        vec![json!("V-2"), json!("V-10"), Value::Null],
        "tri naturel"
    );
    assert_eq!(rows[0]["signal"], "SDI 3G");
    assert_eq!(rows[0]["a"]["element"], "Baie V1");
    assert_eq!(rows[0]["a"]["port"], "SDI 3G 2");
    assert_eq!(rows[0]["b"]["equipment"], "Moniteur");

    // Saisie en série : Léa (technicienne) sans prendre la main ; une ligne périmée revient en conflit.
    let url = format!("/api/projects/{pid}/cables/bulk");
    let r = call(
        &app,
        "POST",
        &url,
        Some(json!({"changes": [
            {"id": rows[2]["id"], "rev": rows[2]["rev"], "number": "V-11", "notes": "Retour réal"},
            {"id": rows[0]["id"], "rev": rows[0]["rev"], "color": "#0E8550"},
            {"id": rows[1]["id"], "rev": 99, "number": "X"}
        ]})),
        Some(&lea),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["updated"].as_array().unwrap().len(), 2);
    assert_eq!(r.body["conflicts"][0]["reason"], "stale");
    let r = call(&app, "POST", &url, Some(json!({"changes": [{"id": rows[0]["id"], "rev": rows[0]["rev"].as_i64().unwrap() + 1, "color": "vert"}]})), Some(&lea)).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let t = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/cables/table"),
        None,
        Some(&o),
    )
    .await
    .body;
    let row = t["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["number"] == "V-11")
        .unwrap()
        .clone();
    assert_eq!(row["notes"], "Retour réal");

    let r = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/cables/export?format=xlsx"),
        None,
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let r = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/cables/export?format=pdf"),
        None,
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn externals_attachments_harnesses_and_overview() {
    let Setup {
        app, o, pid, cid, ..
    } = setup().await;
    let item = |name: &str, ports: Value| json!({"kind": "equipment", "data": {"name": name, "functions": ["V"], "format": {"kind": "rack", "height_u": 1, "depth_mm": 300}, "ports": ports}});
    let src = call(&app, "POST", "/api/catalog/items", Some(item("Source", json!([{"connector_id": "bnc", "signal_id": "sdi-3g", "direction": "out", "gender": "F", "quantity": 4}]))), Some(&o)).await.body["item"]["id"].as_str().unwrap().to_string();
    let ad = call(&app, "POST", "/api/catalog/items", Some(json!({"kind": "adapters", "data": {"name": "BNC vers HD-BNC", "input": {"connector_id": "bnc", "gender": "M"}, "outputs": [{"connector_id": "hd-bnc", "gender": "F", "quantity": 1}]}})), Some(&o)).await;
    assert_eq!(ad.status, StatusCode::CREATED, "{:?}", ad.body);
    let ad = ad.body["item"]["id"].as_str().unwrap().to_string();
    let bad = call(&app, "POST", "/api/catalog/items", Some(json!({"kind": "adapters", "data": {"name": "XLR F-M", "input": {"connector_id": "xlr3", "gender": "F"}, "outputs": [{"connector_id": "xlr3", "gender": "M"}]}})), Some(&o)).await.body["item"]["id"].as_str().unwrap().to_string();
    call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/articles"),
        Some(json!({"container_id": cid, "nature": "equipment", "item_id": src})),
        Some(&o),
    )
    .await;
    let ad_art = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/articles"),
        Some(json!({"container_id": cid, "nature": "adapter", "item_id": ad})),
        Some(&o),
    )
    .await
    .body["article"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/articles"),
        Some(json!({"container_id": cid, "nature": "adapter", "item_id": bad})),
        Some(&o),
    )
    .await;
    let other = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/locations"),
        Some(json!({"name": "Bord terrain", "code": "BORD"})),
        Some(&o),
    )
    .await
    .body["id"]
        .as_str()
        .unwrap()
        .to_string();
    call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/synoptic/lock"),
        None,
        Some(&o),
    )
    .await;
    let s = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/synoptic?view=container:{cid}"),
        None,
        Some(&o),
    )
    .await
    .body;
    let node = |l: &str| {
        s["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["label"] == l)
            .unwrap()
            .clone()
    };
    let source = node("Source");
    let p = |i: usize| source["ports"][i]["ref"].as_str().unwrap().to_string();
    let ad_in = node("BNC vers HD-BNC")["ports"][0]["ref"]
        .as_str()
        .unwrap()
        .to_string();
    let bad_in = node("XLR F-M")["ports"][0]["ref"]
        .as_str()
        .unwrap()
        .to_string();

    // Attache : BNC mâle sur BNC femelle ; un XLR ne s'attache pas sur un BNC.
    let r = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/attach"),
        Some(json!({"adapter": ad_in, "port": p(0)})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/attach"),
        Some(json!({"adapter": bad_in, "port": p(1)})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);

    // Ports externes : vers un lieu (non rattaché puis rattaché), hors projet ; un port marqué une fois.
    let ext = format!("/api/projects/{pid}/external");
    let r = call(
        &app,
        "POST",
        &ext,
        Some(json!({"port": p(1), "location_id": other})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{:?}", r.body);
    let r = call(
        &app,
        "POST",
        &ext,
        Some(json!({"port": p(1), "label": "OB2"})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    call(
        &app,
        "POST",
        &ext,
        Some(json!({"port": p(2), "label": "Diffuseur · fibre A"})),
        Some(&o),
    )
    .await;
    let r = call(&app, "POST", &ext, Some(json!({"port": p(3)})), Some(&o)).await;
    assert_eq!(
        r.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "destination obligatoire"
    );
    let s = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/synoptic?view=container:{cid}"),
        None,
        Some(&o),
    )
    .await
    .body;
    let to_loc = s["cables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["b_info"]["location_id"] == other.as_str())
        .unwrap()
        .clone();
    assert_eq!(to_loc["b_info"]["resolved"], false);
    assert!(
        to_loc["b_info"]["label"]
            .as_str()
            .unwrap()
            .contains("non rattaché")
    );
    assert!(to_loc["warnings"].as_array().unwrap().is_empty());

    // Faisceau et vue générale.
    let h = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/harnesses"),
        Some(json!({"name": "Multi vidéo", "color": "#2459E0"})),
        Some(&o),
    )
    .await;
    assert_eq!(h.status, StatusCode::CREATED);
    let hid = h.body["id"].as_str().unwrap().to_string();
    let r = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/harnesses/assign"),
        Some(json!({"ids": [to_loc["id"]], "harness_id": hid})),
        Some(&o),
    )
    .await;
    assert_eq!(r.body["changed"], 1);
    let ov = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/synoptic?view=overview"),
        None,
        Some(&o),
    )
    .await
    .body;
    assert_eq!(ov["overview"]["links"][0]["external"], 1);
    assert_eq!(ov["harnesses"][0]["name"], "Multi vidéo");
    let car = ov["overview"]["locations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["id"] != other.as_str())
        .unwrap()
        .clone();
    assert_eq!(car["outside"], 1, "un port hors projet");

    // Numérotation : les ports externes comptent, pas l'attache.
    let r = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/numbering"),
        Some(json!({"mode": "missing"})),
        Some(&o),
    )
    .await;
    assert_eq!(r.body["changed"], 2, "{:?}", r.body);
    let t = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/cables/table"),
        None,
        Some(&o),
    )
    .await
    .body;
    assert_eq!(t["rows"].as_array().unwrap().len(), 2);
    assert!(
        t["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["b"]["equipment"] == "→ Bord terrain")
    );

    // Atelier : connexions et câbles à préparer.
    let w = call(
        &app,
        "GET",
        &format!("/api/containers/{cid}/checks"),
        None,
        Some(&o),
    )
    .await
    .body;
    let fam = |k: &str| {
        w["families"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["key"] == k)
            .unwrap()["total"]
            .as_u64()
            .unwrap()
    };
    assert_eq!(
        (fam("connexions"), fam("cables")),
        (3, 2),
        "{:?}",
        w["families"]
    );

    // Supprimer l'adaptateur supprime son attache.
    call(
        &app,
        "DELETE",
        &format!("/api/articles/{ad_art}"),
        None,
        Some(&o),
    )
    .await;
    let s = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/synoptic?view=container:{cid}"),
        None,
        Some(&o),
    )
    .await
    .body;
    assert!(
        !s["cables"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["fields"]["attach"] == true)
    );
    call(
        &app,
        "DELETE",
        &format!("/api/harnesses/{hid}"),
        None,
        Some(&o),
    )
    .await;
    let s = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/synoptic?view=container:{cid}"),
        None,
        Some(&o),
    )
    .await
    .body;
    assert!(
        s["cables"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["fields"]["harness_id"].is_null())
    );
}

#[tokio::test]
async fn annotations_and_zone_checks() {
    let Setup {
        app, o, pid, cid, ..
    } = setup().await;
    call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/synoptic/lock"),
        None,
        Some(&o),
    )
    .await;
    let view = format!("container:{cid}");
    let url = format!("/api/projects/{pid}/annotations");
    let z = call(
        &app,
        "POST",
        &url,
        Some(json!({"view": view, "kind": "zone", "data": {"x": 0, "y": 0, "w": 500, "h": 400}})),
        Some(&o),
    )
    .await;
    assert_eq!(z.status, StatusCode::CREATED, "{:?}", z.body);
    assert_eq!(z.body["data"]["stroke"], "dashed");
    let bad = call(
        &app,
        "POST",
        &url,
        Some(json!({"view": view, "kind": "label", "data": {"text": " "}})),
        Some(&o),
    )
    .await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);
    let l = call(&app, "POST", &url, Some(json!({"view": view, "kind": "label", "data": {"x": 10, "y": 10, "text": "Régie image", "bold": true, "size": 400}})), Some(&o)).await;
    assert_eq!(l.body["data"]["size"], 96.0);
    let v = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/synoptic/validate"),
        None,
        Some(&o),
    )
    .await
    .body;
    let codes: Vec<u64> = v["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["code"].as_u64().unwrap())
        .collect();
    assert!(codes.contains(&10) && codes.contains(&11), "{codes:?}");
    // Un nœud dans la zone, un nom : plus d'anomalie de zone.
    call(
        &app,
        "PUT",
        &format!("/api/projects/{pid}/synoptic/layout"),
        Some(json!({"view": view, "positions": [{"node": "i:x", "x": 100, "y": 100}]})),
        Some(&o),
    )
    .await;
    let zid = z.body["id"].as_str().unwrap();
    let r = call(&app, "PATCH", &format!("/api/annotations/{zid}"), Some(json!({"rev": z.body["rev"], "data": {"x": 0, "y": 0, "w": 500, "h": 400, "name": "Vidéo"}})), Some(&o)).await;
    assert_eq!(r.body["data"]["name"], "Vidéo");
    let v = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/synoptic/validate"),
        None,
        Some(&o),
    )
    .await
    .body;
    assert!(
        !v["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["code"] == 10 || i["code"] == 11)
    );
    let s = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/synoptic?view={view}"),
        None,
        Some(&o),
    )
    .await
    .body;
    assert_eq!(s["annotations"].as_array().unwrap().len(), 2);
    let r = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/annotations/delete"),
        Some(json!({"ids": [zid, l.body["id"]]})),
        Some(&o),
    )
    .await;
    assert_eq!(r.body["deleted"], 2);
}

#[tokio::test]
async fn preparation_lines_and_cabled_state() {
    let Setup {
        app,
        o,
        lea,
        pid,
        cid,
        ..
    } = setup().await;
    let item = |name: &str, ports: Value| json!({"kind": "equipment", "data": {"name": name, "functions": ["V"], "format": {"kind": "rack", "height_u": 1, "depth_mm": 300}, "ports": ports}});
    let a = call(&app, "POST", "/api/catalog/items", Some(item("Src", json!([{"connector_id": "bnc", "signal_id": "sdi-3g", "direction": "out", "gender": "F", "quantity": 3}]))), Some(&o)).await.body["item"]["id"].as_str().unwrap().to_string();
    let b = call(&app, "POST", "/api/catalog/items", Some(item("Dst", json!([{"connector_id": "bnc", "signal_id": "sdi-3g", "direction": "in", "gender": "F", "quantity": 3}]))), Some(&o)).await.body["item"]["id"].as_str().unwrap().to_string();
    for it in [&a, &b] {
        call(
            &app,
            "POST",
            &format!("/api/projects/{pid}/articles"),
            Some(json!({"container_id": cid, "nature": "equipment", "item_id": it})),
            Some(&o),
        )
        .await;
    }
    call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/synoptic/lock"),
        None,
        Some(&o),
    )
    .await;
    let s = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/synoptic?view=container:{cid}"),
        None,
        Some(&o),
    )
    .await
    .body;
    let node = |l: &str| {
        s["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["label"] == l)
            .unwrap()
            .clone()
    };
    let (src, dst) = (node("Src"), node("Dst"));
    let p = |n: &Value, i: usize| n["ports"][i]["ref"].as_str().unwrap().to_string();
    call(&app, "POST", &format!("/api/projects/{pid}/cables"), Some(json!({"cables": [
        {"a": p(&src, 0), "b": p(&dst, 0), "length_m": 2}, {"a": p(&src, 1), "b": p(&dst, 1), "length_m": 1.8}, {"a": p(&src, 2), "b": p(&dst, 2), "length_m": 5}
    ]})), Some(&o)).await;

    let v = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/prep"),
        None,
        Some(&lea),
    )
    .await
    .body;
    let fam = |k: &str| {
        v["families"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["key"] == k)
            .unwrap()
            .clone()
    };
    assert_eq!(fam("elements")["lines"].as_array().unwrap().len(), 1);
    let eq = fam("equipment");
    let mixer = eq["lines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["label"] == "Mélangeur 2U")
        .unwrap()
        .clone();
    assert_eq!(
        (mixer["min"].clone(), mixer["status"].clone()),
        (json!(3), json!("to_prepare"))
    );
    let cables = fam("cables")["lines"].as_array().unwrap().clone();
    assert_eq!(cables.len(), 2, "2 m (2 et 1,8 arrondi) et 5 m");
    let two = cables.iter().find(|l| l["min"] == 2).unwrap().clone();
    assert!(
        two["detail"]
            .as_str()
            .unwrap()
            .contains("BNC M ↔ BNC M · 2,0 m"),
        "{}",
        two["detail"]
    );
    assert_eq!(two["missing_model"], true);

    // Saisie : statut, réserve, approvisionnement ; une réserve négative est refusée.
    let url = format!("/api/projects/{pid}/prep/line");
    let r = call(&app, "PATCH", &url, Some(json!({"key": mixer["key"], "status": "to_configure", "reserve": 1, "supply": "rental"})), Some(&lea)).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = call(
        &app,
        "PATCH",
        &url,
        Some(json!({"key": mixer["key"], "reserve": -2})),
        Some(&lea),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let r = call(
        &app,
        "PATCH",
        &url,
        Some(json!({"key": mixer["key"], "status": "made"})),
        Some(&lea),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "statut d'une autre famille"
    );

    // Modèle de câble pour toute la ligne, puis liaison câblée.
    let model = call(
        &app,
        "POST",
        "/api/catalog/items",
        Some(json!({"kind": "cable-models", "data": {"name": "Coax 75 Ω HD"}})),
        Some(&o),
    )
    .await
    .body["item"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let r = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/prep/cable-model"),
        Some(json!({"key": two["key"], "model_id": model})),
        Some(&lea),
    )
    .await;
    assert_eq!(r.body["changed"], 2);
    let cid1 = two["cables"][0]["id"].as_str().unwrap();
    call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/prep/cabled"),
        Some(json!({"cable_id": cid1, "done": true, "note": "Repris au chantier"})),
        Some(&lea),
    )
    .await;
    let v = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/prep"),
        None,
        Some(&o),
    )
    .await
    .body;
    let cl = v["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["key"] == "cables")
        .unwrap()["lines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["label"] == "Coax 75 Ω HD")
        .unwrap()
        .clone();
    assert_eq!(cl["cabled"], 1);
    assert_eq!(
        cl["cables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == cid1)
            .unwrap()["done_by"],
        "Léa"
    );
    let m2 = v["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["key"] == "equipment")
        .unwrap()["lines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["label"] == "Mélangeur 2U")
        .unwrap()
        .clone();
    assert_eq!(
        (m2["total"].clone(), m2["supply"].clone()),
        (json!(4), json!("rental"))
    );
    assert_eq!(v["packing"][0]["content"].as_array().unwrap().len(), 5);
    let r = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/prep/export?format=xlsx"),
        None,
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
}
