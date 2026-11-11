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
