//! Tests de bout en bout des projets et de l'inventaire (lot 4), temps réel compris.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Request, StatusCode, header};
use futures_util::{SinkExt, StreamExt};
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
    bytes: Vec<u8>,
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
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-strata-client", "onglet-test");
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
    }
    let b = body.map(|b| b.to_string()).unwrap_or_default();
    let res = app
        .clone()
        .oneshot(req.body(Body::from(b)).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.split(';').next().unwrap().to_string());
    let bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    Resp {
        status,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        bytes,
        cookie,
    }
}

async fn owner(app: &Router) -> String {
    let r = call(
        app,
        "POST",
        "/api/setup",
        Some(json!({"email": "chef@studio.fr", "name": "Chef", "password": "mot-de-passe-solide"})),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    r.cookie.unwrap()
}

async fn member(app: &Router, o: &str, email: &str, role: &str) -> String {
    let r = call(
        app,
        "POST",
        "/api/members",
        Some(json!({"email": email, "role_id": role})),
        Some(o),
    )
    .await;
    let temp = r.body["temporary_password"].as_str().unwrap().to_string();
    let l = call(
        app,
        "POST",
        "/api/auth/login",
        Some(json!({"login": email, "password": temp})),
        None,
    )
    .await;
    let c = l.cookie.unwrap();
    call(
        app,
        "POST",
        "/api/me/password",
        Some(json!({"current": temp, "new": "nouveau-secret"})),
        Some(&c),
    )
    .await;
    c
}

fn days(n: i64) -> String {
    (chrono::Local::now().date_naive() + chrono::Duration::days(n))
        .format("%Y-%m-%d")
        .to_string()
}

fn wizard(name: &str) -> Value {
    json!({
        "name": name, "client": "Ligue Atlantique", "venue": "Arena Loire, Nantes", "service_type": "Sport",
        "intervention": "prestation", "regie": "car",
        "planning_start": days(1), "planning_end": days(3), "shoot_start": days(4), "shoot_end": days(5),
        "location": {"name": "Car régie CR-2"},
        "container": {"name": "Baie vidéo V1", "kind": "rack", "rackable": true, "height_u": 42, "depth_mm": 600, "empty_weight_kg": 60, "max_weight_kg": 300}
    })
}

async fn equipment(app: &Router, o: &str, name: &str, u: f64, kg: f64, w: f64) -> String {
    let r = call(app, "POST", "/api/catalog/items", Some(json!({"kind": "equipment", "data": {"name": name, "functions": ["V"], "format": {"kind": "rack", "height_u": u, "depth_mm": 450}, "weight_kg": kg, "power_w": w}})), Some(o)).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    r.body["item"]["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn wizard_creates_project_location_and_container() {
    let app = app().await;
    let o = owner(&app).await;
    let r = call(
        &app,
        "POST",
        "/api/projects",
        Some(wizard("Finale Coupe Atlantique")),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    assert_eq!(r.body["project"]["status"], "preparation");
    assert_eq!(r.body["location"]["code"], "CARRE");
    assert_eq!(r.body["container"]["cable_code"], "BAIEV");
    let pid = r.body["project"]["id"].as_str().unwrap().to_string();

    // Nom unique, sans casse ni espaces autour.
    let r = call(
        &app,
        "GET",
        "/api/projects/check-name?name=%20finale%20coupe%20ATLANTIQUE%20",
        None,
        Some(&o),
    )
    .await;
    assert_eq!(r.body["taken"], true);
    let r = call(
        &app,
        "POST",
        "/api/projects",
        Some(wizard("finale coupe atlantique")),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        r.body["fields"][0]["message"],
        "Un projet porte déjà ce nom."
    );

    // Date passée refusée à la création, régie fixe refusée pour une prestation.
    let mut bad = wizard("Autre");
    bad["shoot_start"] = json!(days(-2));
    bad["regie"] = json!("fixed");
    let r = call(&app, "POST", "/api/projects", Some(bad), Some(&o)).await;
    let f: Vec<&str> = r.body["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["field"].as_str().unwrap())
        .collect();
    assert!(f.contains(&"shoot_start") && f.contains(&"regie"), "{f:?}");

    // Contenant refusé : le projet est créé quand même et l'échec est rendu.
    let mut half = wizard("Concert");
    half["container"]["height_u"] = json!(99);
    let r = call(&app, "POST", "/api/projects", Some(half), Some(&o)).await;
    assert_eq!(r.status, StatusCode::CREATED);
    assert!(r.body["container"].is_null());
    assert_eq!(r.body["container_error"]["fields"][0]["field"], "height_u");

    // Fiche : le type d'intervention ne change pas ; les dates d'intégration sont vidées.
    let p = call(&app, "GET", &format!("/api/projects/{pid}"), None, Some(&o)).await;
    let mut data = p.body["project"].clone();
    data["intervention"] = json!("integration");
    data["commissioning"] = json!(days(20));
    data["status"] = json!("ready");
    let r = call(
        &app,
        "PATCH",
        &format!("/api/projects/{pid}"),
        Some(data),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        (r.body["intervention"].as_str(), r.body["status"].as_str()),
        (Some("prestation"), Some("ready"))
    );
    assert!(r.body["commissioning"].is_null());

    let list = call(&app, "GET", "/api/projects", None, Some(&o)).await;
    assert_eq!(list.body["projects"].as_array().unwrap().len(), 2);
    let dash = call(&app, "GET", "/api/dashboard", None, Some(&o)).await;
    assert_eq!(dash.body["projects_active"], 2);
    assert_eq!(dash.body["next"]["days"], 4);
    assert_eq!(
        dash.body["upcoming"][0]["project"]["name"],
        "Finale Coupe Atlantique"
    );
}

#[tokio::test]
async fn inventory_articles_instances_and_totals() {
    let app = app().await;
    let o = owner(&app).await;
    let mixer = equipment(&app, &o, "Mélangeur 4 M/E", 2.0, 12.0, 300.0).await;
    let rec = equipment(&app, &o, "Enregistreur 1U", 1.0, 3.5, 60.0).await;
    let r = call(
        &app,
        "POST",
        "/api/projects",
        Some(wizard("Finale")),
        Some(&o),
    )
    .await;
    let pid = r.body["project"]["id"].as_str().unwrap().to_string();
    let lid = r.body["location"]["id"].as_str().unwrap().to_string();
    let cid = r.body["container"]["id"].as_str().unwrap().to_string();

    // Ajout, puis ajout de la même référence : la quantité augmente.
    let a = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/articles"),
        Some(json!({"container_id": cid, "nature": "equipment", "item_id": rec, "quantity": 2})),
        Some(&o),
    )
    .await;
    assert_eq!(a.status, StatusCode::CREATED, "{}", a.body);
    let again = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/articles"),
        Some(json!({"container_id": cid, "nature": "equipment", "item_id": rec, "quantity": 2})),
        Some(&o),
    )
    .await;
    assert_eq!(
        (
            again.body["merged"].as_bool(),
            again.body["article"]["quantity"].as_i64()
        ),
        (Some(true), Some(4))
    );
    let aid = a.body["article"]["id"].as_str().unwrap().to_string();
    call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/articles"),
        Some(json!({"container_id": cid, "nature": "equipment", "item_id": mixer, "quantity": 1})),
        Some(&o),
    )
    .await;
    call(&app, "POST", &format!("/api/projects/{pid}/articles"), Some(json!({"container_id": cid, "nature": "accessory", "accessory": {"kind": "shelf", "depth_mm": 450, "height_u": 1}})), Some(&o)).await;

    // Trop grand : avertissement non bloquant.
    let big = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/articles"),
        Some(json!({"container_id": cid, "nature": "equipment", "item_id": mixer, "quantity": 30})),
        Some(&o),
    )
    .await;
    assert_eq!(big.status, StatusCode::CREATED);
    assert!(
        big.body["warnings"][0]
            .as_str()
            .unwrap()
            .starts_with("Place insuffisante"),
        "{}",
        big.body
    );
    let big_id = big.body["article"]["id"].as_str().unwrap().to_string();
    let r = call(
        &app,
        "PATCH",
        &format!("/api/articles/{big_id}"),
        Some(json!({"rev": big.body["article"]["rev"], "quantity": 1})),
        Some(&o),
    )
    .await;
    // L'ajout a fusionné avec le mélangeur déjà là (1 + 30), ramené ensuite à 1.
    assert_eq!(r.body["quantity"], 1);

    let p = call(&app, "GET", &format!("/api/projects/{pid}"), None, Some(&o)).await;
    let c = &p.body["containers"][0];
    // 4 × 1U + 1 × 2U + étagère 1U = 7 U ; poids 60 + 4 × 3,5 + 12 = 86 ; conso 4 × 60 + 300 = 540.
    assert_eq!(c["totals"]["used_u"], 7.0, "{}", c["totals"]);
    assert_eq!(c["totals"]["weight_kg"], 86.0);
    assert_eq!(c["totals"]["power_w"], 540.0);
    assert_eq!(
        p.body["articles"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["nature"] == "accessory")
            .unwrap()["label"],
        "Étagère 1U · 450mm"
    );
    let inst: Vec<&Value> = p.body["instances"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["article_id"] == aid.as_str())
        .collect();
    assert_eq!(inst.len(), 4);
    assert!(inst.iter().all(|i| i["tag"].as_str().unwrap().len() == 5));

    // Identité d'un exemplaire.
    let iid = inst[0]["id"].as_str().unwrap();
    let r = call(
        &app,
        "PATCH",
        &format!("/api/instances/{iid}"),
        Some(json!({"rev": 1, "serial": "SN-001", "mac": "001a2b3c4d5e"})),
        Some(&o),
    )
    .await;
    assert_eq!(r.body["mac"], "00:1A:2B:3C:4D:5E");
    let r = call(
        &app,
        "PATCH",
        &format!("/api/instances/{iid}"),
        Some(json!({"rev": 2, "mac": "zz"})),
        Some(&o),
    )
    .await;
    assert_eq!(r.body["fields"][0]["field"], "mac");

    // Déplacer un exemplaire vers les équipements libres d'un autre emplacement.
    let l2 = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/locations"),
        Some(json!({"name": "Bord terrain"})),
        Some(&o),
    )
    .await;
    let l2id = l2.body["id"].as_str().unwrap().to_string();
    let art = p.body["articles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == aid.as_str())
        .unwrap()
        .clone();
    let r = call(
        &app,
        "POST",
        &format!("/api/articles/{aid}/move"),
        Some(json!({"rev": art["rev"], "location_id": l2id, "instance_id": iid})),
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        (
            r.body["quantity"].as_i64(),
            r.body["container_id"].is_null()
        ),
        (Some(1), true)
    );
    // L'emplacement porte un équipement libre : sa suppression est refusée.
    let r = call(
        &app,
        "DELETE",
        &format!("/api/locations/{l2id}"),
        None,
        Some(&o),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    let r = call(
        &app,
        "DELETE",
        &format!("/api/locations/{lid}"),
        None,
        Some(&o),
    )
    .await;
    assert!(r.body["message"].as_str().unwrap().contains("contenants"));

    // Nom de contenant unique : suffixe automatique.
    let r = call(&app, "POST", &format!("/api/projects/{pid}/containers"), Some(json!({"location_id": lid, "name": "baie vidéo v1", "kind": "flight_case", "rackable": true, "height_u": 12, "depth_mm": 500})), Some(&o)).await;
    assert_eq!(r.body["name"], "baie vidéo v1 2");

    // Nomenclature, avec sous-totaux par emplacement.
    let x = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/inventory/export?format=csv&group=location"),
        None,
        Some(&o),
    )
    .await;
    let text = String::from_utf8(x.bytes).unwrap();
    assert!(text.contains("Sous-total · Car régie CR-2"), "{text}");
    assert!(text.lines().last().unwrap().starts_with("Total;"));

    // Historique agrégé du projet.
    let h = call(
        &app,
        "GET",
        &format!("/api/projects/{pid}/history"),
        None,
        Some(&o),
    )
    .await;
    let kinds: std::collections::HashSet<&str> = h.body["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["object_type"].as_str().unwrap())
        .collect();
    assert!(
        ["project", "location", "container", "article", "instance"]
            .iter()
            .all(|k| kinds.contains(k)),
        "{kinds:?}"
    );

    // Suppression du projet : tout part avec lui.
    assert_eq!(
        call(
            &app,
            "DELETE",
            &format!("/api/projects/{pid}"),
            None,
            Some(&o)
        )
        .await
        .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&app, "GET", &format!("/api/projects/{pid}"), None, Some(&o))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn tasks_and_rights() {
    let app = app().await;
    let o = owner(&app).await;
    let r = call(
        &app,
        "POST",
        "/api/projects",
        Some(wizard("Finale")),
        Some(&o),
    )
    .await;
    let pid = r.body["project"]["id"].as_str().unwrap().to_string();
    let t = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/tasks"),
        Some(json!({"title": "Étiqueter la baie", "priority": "high", "due": days(2)})),
        Some(&o),
    )
    .await;
    assert_eq!(t.status, StatusCode::CREATED, "{}", t.body);
    let tid = t.body["id"].as_str().unwrap();
    let r = call(
        &app,
        "PATCH",
        &format!("/api/tasks/{tid}"),
        Some(json!({"rev": 1, "title": "Étiqueter la baie", "status": "done"})),
        Some(&o),
    )
    .await;
    assert_eq!(r.body["done_on"], days(0));
    let d = call(&app, "GET", "/api/dashboard", None, Some(&o)).await;
    assert_eq!(d.body["tasks_pct"], 100);

    // Consultation : lecture seule.
    let viewer = member(&app, &o, "voir@studio.fr", "viewer").await;
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/projects/{pid}"),
            None,
            Some(&viewer)
        )
        .await
        .status,
        StatusCode::OK
    );
    let r = call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/tasks"),
        Some(json!({"title": "x"})),
        Some(&viewer),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    // Révision dépassée.
    let r = call(
        &app,
        "PATCH",
        &format!("/api/tasks/{tid}"),
        Some(json!({"rev": 1, "title": "Autre"})),
        Some(&o),
    )
    .await;
    assert_eq!(r.body["code"], "stale");
}

#[tokio::test]
async fn live_events_and_presence() {
    let store = Store::open_in_memory().await.unwrap();
    let app = router(AppState::new(store, false, None));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap()
    });
    let base = format!("http://{addr}");
    let http = reqwest_like::Client::new(&base);
    let o = http
        .json(
            "POST",
            "/api/setup",
            json!({"email": "chef@studio.fr", "name": "Chef", "password": "mot-de-passe-solide"}),
            None,
        )
        .await
        .1
        .unwrap();
    let (body, _) = http
        .json("POST", "/api/projects", wizard("Finale"), Some(&o))
        .await;
    let pid = body["project"]["id"].as_str().unwrap().to_string();

    // Sans session : refusé.
    assert!(
        tokio_tungstenite::connect_async(format!("ws://{addr}/api/live"))
            .await
            .is_err()
    );
    let mut req = tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(
        format!("ws://{addr}/api/live"),
    )
    .unwrap();
    req.headers_mut().insert("cookie", o.parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
    ws.send(tokio_tungstenite::tungstenite::Message::Text(
        json!({"type": "join", "room": format!("project:{pid}")})
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let presence = next(&mut ws).await;
    assert_eq!(presence["type"], "presence");
    assert_eq!(presence["members"][0]["name"], "Chef");
    // Une écriture REST est diffusée dans la salle, avec l'onglet d'origine.
    http.json(
        "POST",
        &format!("/api/projects/{pid}/locations"),
        json!({"name": "Bord terrain"}),
        Some(&o),
    )
    .await;
    let ev = next(&mut ws).await;
    assert_eq!(
        (
            ev["type"].as_str(),
            ev["kind"].as_str(),
            ev["action"].as_str(),
            ev["client"].as_str()
        ),
        (
            Some("change"),
            Some("location"),
            Some("create"),
            Some("onglet-test")
        )
    );
    // Le projet montre qui est là.
    let (p, _) = http
        .json(
            "GET",
            &format!("/api/projects/{pid}"),
            Value::Null,
            Some(&o),
        )
        .await;
    assert_eq!(p["presence"][0]["name"], "Chef");
}

/// Prochain message JSON du WebSocket (3 s au plus).
async fn next<S>(ws: &mut S) -> Value
where
    S: futures_util::Stream<
            Item = Result<
                tokio_tungstenite::tungstenite::Message,
                tokio_tungstenite::tungstenite::Error,
            >,
        > + Unpin,
{
    let m = tokio::time::timeout(std::time::Duration::from_secs(3), ws.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    serde_json::from_str(&m.into_text().unwrap()).unwrap()
}

/// Petit client HTTP au-dessus de hyper (via axum) pour le test temps réel.
mod reqwest_like {
    use serde_json::Value;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    pub struct Client {
        host: String,
    }

    impl Client {
        pub fn new(base: &str) -> Client {
            Client {
                host: base.trim_start_matches("http://").to_string(),
            }
        }

        /// Renvoie le corps JSON et le cookie de session éventuel.
        pub async fn json(
            &self,
            method: &str,
            path: &str,
            body: Value,
            cookie: Option<&str>,
        ) -> (Value, Option<String>) {
            let mut s = tokio::net::TcpStream::connect(&self.host).await.unwrap();
            let b = if body.is_null() {
                String::new()
            } else {
                body.to_string()
            };
            let mut req = format!(
                "{method} {path} HTTP/1.1\r\nHost: {}\r\nX-Strata-Request: 1\r\nX-Strata-Client: onglet-test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
                self.host,
                b.len()
            );
            if let Some(c) = cookie {
                req.push_str(&format!("Cookie: {c}\r\n"));
            }
            req.push_str("\r\n");
            req.push_str(&b);
            s.write_all(req.as_bytes()).await.unwrap();
            let mut out = Vec::new();
            s.read_to_end(&mut out).await.unwrap();
            let text = String::from_utf8_lossy(&out).to_string();
            let (head, rest) = text.split_once("\r\n\r\n").unwrap();
            let cookie = head
                .lines()
                .find(|l| l.to_ascii_lowercase().starts_with("set-cookie:"))
                .map(|l| l[11..].trim().split(';').next().unwrap().to_string());
            // Corps éventuellement découpé (chunked) : on garde ce qui est entre accolades.
            let json = match (rest.find(['{', '[']), rest.rfind(['}', ']'])) {
                (Some(a), Some(z)) => serde_json::from_str(&rest[a..=z]).unwrap_or(Value::Null),
                _ => Value::Null,
            };
            (json, cookie)
        }
    }
}

#[tokio::test]
async fn catalog_items_used_in_projects_are_kept() {
    let app = app().await;
    let o = owner(&app).await;
    let rec = equipment(&app, &o, "Enregistreur 1U", 1.0, 3.5, 60.0).await;
    let r = call(
        &app,
        "POST",
        "/api/projects",
        Some(wizard("Finale")),
        Some(&o),
    )
    .await;
    let pid = r.body["project"]["id"].as_str().unwrap().to_string();
    let cid = r.body["container"]["id"].as_str().unwrap().to_string();
    call(
        &app,
        "POST",
        &format!("/api/projects/{pid}/articles"),
        Some(json!({"container_id": cid, "nature": "equipment", "item_id": rec, "quantity": 3})),
        Some(&o),
    )
    .await;
    let r = call(
        &app,
        "POST",
        "/api/catalog/items/delete",
        Some(json!({"ids": [rec]})),
        Some(&o),
    )
    .await;
    assert_eq!(r.body["deleted"], 0);
    assert_eq!(r.body["kept"][0]["used_by"][0]["name"], "1 projet");
    let r = call(
        &app,
        "GET",
        &format!("/api/catalog/items/{rec}"),
        None,
        Some(&o),
    )
    .await;
    assert_eq!(r.body["in_projects"]["quantity"], 3);
}
