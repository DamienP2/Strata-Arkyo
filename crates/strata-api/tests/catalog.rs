//! Tests de bout en bout du catalogue (lot 3) : référentiels, fiches, import, export, échange.

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
    bytes: Vec<u8>,
    cookie: Option<String>,
}

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    content_type: &str,
    body: Vec<u8>,
    cookie: Option<&str>,
) -> Resp {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header(CSRF_HEADER, "1")
        .header(header::CONTENT_TYPE, content_type);
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
    }
    let res = app
        .clone()
        .oneshot(req.body(Body::from(body)).unwrap())
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
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    Resp {
        status,
        body,
        bytes,
        cookie,
    }
}

async fn call(app: &Router, method: &str, uri: &str, body: Option<Value>, cookie: &str) -> Resp {
    let b = body.map(|b| b.to_string().into_bytes()).unwrap_or_default();
    send(app, method, uri, "application/json", b, Some(cookie)).await
}

async fn owner(app: &Router) -> String {
    let r = send(
        app,
        "POST",
        "/api/setup",
        "application/json",
        json!({"email": "chef@studio.fr", "name": "Chef", "password": "mot-de-passe-solide"})
            .to_string()
            .into_bytes(),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    r.cookie.unwrap()
}

async fn member(app: &Router, owner: &str, email: &str, role: &str) -> String {
    let r = call(
        app,
        "POST",
        "/api/members",
        Some(json!({"email": email, "role_id": role})),
        owner,
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let temp = r.body["temporary_password"].as_str().unwrap().to_string();
    let l = send(
        app,
        "POST",
        "/api/auth/login",
        "application/json",
        json!({"login": email, "password": temp})
            .to_string()
            .into_bytes(),
        None,
    )
    .await;
    let cookie = l.cookie.unwrap();
    let r = call(
        app,
        "POST",
        "/api/me/password",
        Some(json!({"current": temp, "new": "nouveau-secret"})),
        &cookie,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    cookie
}

async fn brand(app: &Router, c: &str, name: &str) -> String {
    let r = call(
        app,
        "POST",
        "/api/catalog/refs/brands",
        Some(json!({"name": name})),
        c,
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    r.body["id"].as_str().unwrap().to_string()
}

async fn item(app: &Router, c: &str, kind: &str, data: Value) -> Value {
    let r = call(
        app,
        "POST",
        "/api/catalog/items",
        Some(json!({"kind": kind, "data": data})),
        c,
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    r.body
}

fn switcher(brand: &str) -> Value {
    json!({
        "name": "ATEM 2 M/E Constellation HD",
        "reference": "SWATEMCONST2ME",
        "brand_id": brand,
        "functions": ["V"],
        "format": {"kind": "rack", "height_u": 2, "depth_mm": 390},
        "power_w": 150,
        "ports": [
            {"face": "rear", "connector_id": "bnc", "signal_id": "sdi-3g", "direction": "in", "quantity": 20},
            {"face": "rear", "connector_id": "bnc", "signal_id": "sdi-3g", "direction": "out", "quantity": 12}
        ]
    })
}

#[tokio::test]
async fn starter_pack_and_rights() {
    let app = app().await;
    let o = owner(&app).await;
    let r = call(&app, "GET", "/api/catalog/refs", None, &o).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["functions"].as_array().unwrap().len(), 11);
    assert_eq!(r.body["phases"].as_array().unwrap().len(), 10);
    assert!(
        r.body["signals"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == "sdi-12g")
    );

    // Technicien : catalogue en lecture.
    let tech = member(&app, &o, "tech@studio.fr", "technician").await;
    assert_eq!(
        call(&app, "GET", "/api/catalog/items", None, &tech)
            .await
            .status,
        StatusCode::OK
    );
    let r = call(
        &app,
        "POST",
        "/api/catalog/items",
        Some(json!({"kind": "adapters", "data": {}})),
        &tech,
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);

    // Chef de projet : fiches en écriture, mais pas les référentiels.
    let lead = member(&app, &o, "lead@studio.fr", "project_lead").await;
    item(&app, &lead, "adapters", json!({"name": "BNC F vers HD-BNC M", "input": {"connector_id": "bnc", "gender": "F"}, "outputs": [{"connector_id": "hd-bnc"}]})).await;
    let r = call(
        &app,
        "POST",
        "/api/catalog/refs/brands",
        Some(json!({"name": "AJA"})),
        &lead,
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(r.body["message"].as_str().unwrap().contains("Paramètres"));
}

#[tokio::test]
async fn references_rules() {
    let app = app().await;
    let o = owner(&app).await;
    let b = brand(&app, &o, "Blackmagic Design").await;
    let r = call(
        &app,
        "POST",
        "/api/catalog/refs/brands",
        Some(json!({"name": "blackmagic DESIGN"})),
        &o,
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "nom unique sans casse");
    let r = call(
        &app,
        "POST",
        "/api/catalog/refs/brands",
        Some(json!({"name": "Sony", "data": {"website": "sony.com"}})),
        &o,
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(r.body["fields"][0]["field"], "website");

    // Une fonction a pour identifiant sa clé.
    let r = call(&app, "POST", "/api/catalog/refs/functions", Some(json!({"name": "Réalité étendue", "data": {"key": "xr", "color": "#112233", "cable_prefix": "XR"}})), &o).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    assert_eq!(r.body["id"], "XR");

    // Révision dépassée.
    let r = call(
        &app,
        "PATCH",
        &format!("/api/catalog/refs/brands/{b}"),
        Some(json!({"rev": 1, "name": "Blackmagic"})),
        &o,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let r = call(
        &app,
        "PATCH",
        &format!("/api/catalog/refs/brands/{b}"),
        Some(json!({"rev": 1, "name": "BMD"})),
        &o,
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (StatusCode::CONFLICT, Some("stale"))
    );

    // Utilisée : on désactive, on ne supprime pas.
    item(&app, &o, "equipment", switcher(&b)).await;
    let r = call(
        &app,
        "DELETE",
        &format!("/api/catalog/refs/brands/{b}"),
        None,
        &o,
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    let r = call(&app, "GET", "/api/catalog/refs/brands", None, &o).await;
    assert_eq!(r.body["items"][0]["uses"], 1);
    let r = call(
        &app,
        "PATCH",
        &format!("/api/catalog/refs/brands/{b}"),
        Some(json!({"rev": 2, "active": false})),
        &o,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);

    // Catégories système protégées.
    let r = call(
        &app,
        "DELETE",
        "/api/catalog/refs/system-categories/sys-video",
        None,
        &o,
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);

    // Sous-type : un seul niveau, fonction héritée.
    let r = call(
        &app,
        "POST",
        "/api/catalog/refs/types",
        Some(json!({"name": "Mélangeur", "data": {"function": "V"}})),
        &o,
    )
    .await;
    let parent = r.body["id"].as_str().unwrap().to_string();
    let r = call(
        &app,
        "POST",
        "/api/catalog/refs/types",
        Some(json!({"name": "Mélangeur live", "parent_id": parent, "data": {"function": "A"}})),
        &o,
    )
    .await;
    assert_eq!(r.body["data"]["function"], "V");
    let child = r.body["id"].as_str().unwrap().to_string();
    let r = call(
        &app,
        "POST",
        "/api/catalog/refs/types",
        Some(json!({"name": "Trop profond", "parent_id": child, "data": {}})),
        &o,
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);

    // Préréglages : effacés puis remis.
    let dims = call(&app, "GET", "/api/catalog/refs/dimensions", None, &o)
        .await
        .body["items"]
        .as_array()
        .unwrap()
        .clone();
    let id = dims[0]["id"].as_str().unwrap();
    assert_eq!(
        call(
            &app,
            "DELETE",
            &format!("/api/catalog/refs/dimensions/{id}"),
            None,
            &o
        )
        .await
        .status,
        StatusCode::NO_CONTENT
    );
    let r = call(
        &app,
        "POST",
        "/api/catalog/refs/dimensions/restore",
        None,
        &o,
    )
    .await;
    assert_eq!(r.body["restored"], 1);
}

#[tokio::test]
async fn records_lifecycle() {
    let app = app().await;
    let o = owner(&app).await;
    let b = brand(&app, &o, "Blackmagic Design").await;

    // Erreurs champ par champ.
    let mut bad = switcher(&b);
    bad["format"]["height_u"] = json!(60);
    bad["ports"][0]["signal_id"] = json!("dante");
    let r = call(
        &app,
        "POST",
        "/api/catalog/items",
        Some(json!({"kind": "equipment", "data": bad})),
        &o,
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let fields: Vec<&str> = r.body["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["field"].as_str().unwrap())
        .collect();
    assert!(fields.contains(&"format.height_u"), "{fields:?}");
    assert!(
        fields.iter().any(|f| f.starts_with("ports[0]")),
        "{fields:?}"
    );

    let created = item(&app, &o, "equipment", switcher(&b)).await;
    let id = created["item"]["id"].as_str().unwrap().to_string();
    assert!(created["duplicates"].as_array().unwrap().is_empty());
    // Même référence : signalé, pas bloqué.
    let again = item(&app, &o, "equipment", switcher(&b)).await;
    assert_eq!(again["duplicates"][0]["reason"], "Même référence");

    // Câble sans nom : nom proposé à partir des connecteurs.
    let model = item(
        &app,
        &o,
        "cable-models",
        json!({"name": "Belden 1694A", "impedance_ohm": 75, "bandwidth_mhz": 4500}),
    )
    .await;
    let model_id = model["item"]["id"].as_str().unwrap().to_string();
    let cable = item(&app, &o, "cables", json!({"left": {"connector_id": "bnc"}, "right": {"connector_id": "bnc"}, "model_id": model_id, "usages": ["sdi-3g"], "lengths_m": [1, 3, 5]})).await;
    assert_eq!(cable["item"]["name"], "BNC M ↔ BNC M");
    let cable_id = cable["item"]["id"].as_str().unwrap().to_string();

    // Compatibilité du modèle avec les signaux.
    let r = call(
        &app,
        "GET",
        &format!("/api/catalog/compat?model_id={model_id}&signal_id=sdi-3g"),
        None,
        &o,
    )
    .await;
    assert_eq!(r.body["result"], "compatible");
    let r = call(
        &app,
        "GET",
        &format!("/api/catalog/compat?model_id={model_id}&signal_id=sdi-12g"),
        None,
        &o,
    )
    .await;
    assert_eq!(r.body["result"], "warning");

    // Modification avec révision, puis révision périmée.
    let mut data = created["item"]["data"].clone();
    data["weight_kg"] = json!(6.5);
    let r = call(
        &app,
        "PUT",
        &format!("/api/catalog/items/{id}"),
        Some(json!({"rev": 1, "data": data})),
        &o,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["item"]["rev"], 2);
    let r = call(
        &app,
        "PUT",
        &format!("/api/catalog/items/{id}"),
        Some(json!({"rev": 1, "data": data})),
        &o,
    )
    .await;
    assert_eq!(r.body["code"], "stale");

    // Dupliquer.
    let r = call(
        &app,
        "POST",
        &format!("/api/catalog/items/{id}/duplicate"),
        None,
        &o,
    )
    .await;
    assert_eq!(
        r.body["item"]["name"],
        "ATEM 2 M/E Constellation HD (copie)"
    );
    let r = call(
        &app,
        "POST",
        &format!("/api/catalog/items/{id}/duplicate"),
        None,
        &o,
    )
    .await;
    assert_eq!(
        r.body["item"]["name"],
        "ATEM 2 M/E Constellation HD (copie 2)"
    );

    // Recherche sans accents ni casse, sur le nom ou la référence.
    let r = call(
        &app,
        "GET",
        "/api/catalog/items?kind=equipment&q=swatemconst",
        None,
        &o,
    )
    .await;
    assert_eq!(r.body["total"], 4);

    // Suppression groupée : le modèle utilisé par un câble reste, sauf si le câble part aussi.
    let r = call(
        &app,
        "POST",
        "/api/catalog/items/delete",
        Some(json!({"ids": [model_id]})),
        &o,
    )
    .await;
    assert_eq!(r.body["deleted"], 0);
    assert_eq!(r.body["kept"][0]["used_by"][0]["id"], cable_id);
    let r = call(
        &app,
        "POST",
        "/api/catalog/items/delete",
        Some(json!({"ids": [model_id, cable_id, "inconnu"]})),
        &o,
    )
    .await;
    assert_eq!(
        (r.body["deleted"].as_i64(), r.body["missing"].as_i64()),
        (Some(2), Some(1))
    );
    let r = call(
        &app,
        "POST",
        "/api/catalog/items/delete",
        Some(json!({"ids": []})),
        &o,
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
}

const SHEET: &str = "Désignation;Fabricant;Ref.;Hauteur;Poids;Conso (W);Fonctions;Colonne X\n\
Grille 40x40;Blackmagik;VHUB40;2U;4,2;80;Vidéo;?\n\
Écran 24\";Sony;LMD-A240;;lourd;45;V;\n\
;Sony;SANS-NOM;1U;;;;\n\
Moniteur son;Genelec;8030C;;;;Audio;\n";

async fn analyze(app: &Router, c: &str, filename: &str, bytes: Vec<u8>) -> Resp {
    send(
        app,
        "POST",
        &format!("/api/catalog/import/analyze?filename={filename}"),
        "application/octet-stream",
        bytes,
        Some(c),
    )
    .await
}

#[tokio::test]
async fn spreadsheet_import() {
    let app = app().await;
    let o = owner(&app).await;
    brand(&app, &o, "Blackmagic Design").await;
    let sony = brand(&app, &o, "Sony").await;
    // Windows-1252, comme un export Excel français.
    let (bytes, _, _) = encoding_rs::WINDOWS_1252.encode(SHEET);
    let r = analyze(&app, &o, "materiel.csv", bytes.into_owned()).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["row_count"], 4);
    assert_eq!(r.body["headers"][0], "Désignation");
    let fields: Vec<Value> = r.body["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["field"].clone())
        .collect();
    assert_eq!(
        fields,
        [
            json!("name"),
            json!("brand"),
            json!("reference"),
            json!("height_u"),
            json!("weight_kg"),
            json!("power_w"),
            json!("functions"),
            Value::Null
        ]
    );
    let unknown = &r.body["unknown"]["brands"];
    let bm = unknown
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["value"] == "Blackmagik")
        .unwrap();
    assert_eq!(bm["suggestion"]["name"], "Blackmagic Design");
    let session = r.body["session_id"].as_str().unwrap().to_string();
    let mapping: Vec<Value> = r.body["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| json!({"column": m["column"], "field": m["field"]}))
        .collect();
    let bm_id = bm["suggestion"]["id"].clone();
    let values = json!({"brands": {"Blackmagik": {"action": "map", "id": bm_id}, "Genelec": {"action": "create"}}});

    // Essai à blanc : rien n'est écrit.
    let r = call(
        &app,
        "POST",
        &format!("/api/catalog/import/{session}/run"),
        Some(json!({"mapping": mapping, "values": values, "dry_run": true})),
        &o,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["created"], 3);
    assert_eq!(
        call(&app, "GET", "/api/catalog/items", None, &o).await.body["total"],
        0
    );

    let r = call(
        &app,
        "POST",
        &format!("/api/catalog/import/{session}/run"),
        Some(json!({"mapping": mapping, "values": values})),
        &o,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["created"], 3);
    assert_eq!(r.body["errors"][0]["row"], 3, "ligne sans nom");
    assert_eq!(r.body["refs_created"][0]["name"], "Genelec");
    let issues = r.body["issues"].as_array().unwrap();
    assert!(
        issues
            .iter()
            .any(|i| i["row"] == 2 && i["field"] == "weight_kg"),
        "{issues:?}"
    );
    assert!(
        issues
            .iter()
            .any(|i| i["row"] == 2 && i["field"] == "height_u"),
        "hauteur par défaut"
    );
    assert!(r.body["snapshot_id"].is_string());

    let list = call(
        &app,
        "GET",
        "/api/catalog/items?kind=equipment&q=grille",
        None,
        &o,
    )
    .await;
    let grid = &list.body["items"][0]["data"];
    assert_eq!(grid["format"]["height_u"], 2.0);
    assert_eq!(grid["weight_kg"], 4.2);
    assert_eq!(grid["functions"], json!(["V"]));
    let screen = &call(&app, "GET", "/api/catalog/items?q=lmd-a240", None, &o)
        .await
        .body["items"][0];
    assert_eq!(screen["brand_id"], sony);

    // Second passage : doublons ignorés par défaut, mis à jour sur demande.
    let r = analyze(&app, &o, "materiel.csv", SHEET.as_bytes().to_vec()).await;
    let session = r.body["session_id"].as_str().unwrap().to_string();
    let r = call(
        &app,
        "POST",
        &format!("/api/catalog/import/{session}/run"),
        Some(json!({"mapping": mapping, "values": values, "dry_run": true})),
        &o,
    )
    .await;
    assert_eq!(
        (
            r.body["created"].as_i64(),
            r.body["skipped"].as_array().unwrap().len()
        ),
        (Some(0), 3)
    );
    let r = call(&app, "POST", &format!("/api/catalog/import/{session}/run"), Some(json!({"mapping": mapping, "values": values, "duplicates": "update", "rows": {"4": "skip"}})), &o).await;
    assert_eq!(
        (
            r.body["updated"].as_i64(),
            r.body["skipped"].as_array().unwrap().len()
        ),
        (Some(2), 1)
    );

    // La session est consommée.
    let r = call(
        &app,
        "POST",
        &format!("/api/catalog/import/{session}/run"),
        Some(json!({"mapping": mapping})),
        &o,
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn export_reads_back() {
    let app = app().await;
    let o = owner(&app).await;
    let b = brand(&app, &o, "Blackmagic Design").await;
    item(&app, &o, "equipment", switcher(&b)).await;

    let csv = call(&app, "GET", "/api/catalog/export?format=csv", None, &o).await;
    assert_eq!(csv.status, StatusCode::OK);
    let text = String::from_utf8(csv.bytes.clone()).unwrap();
    assert!(text.starts_with('\u{feff}'));
    assert!(
        text.contains("ATEM 2 M/E Constellation HD;SWATEMCONST2ME;Blackmagic Design"),
        "{text}"
    );
    let r = analyze(&app, &o, "export.csv", csv.bytes).await;
    assert!(
        r.body["matches"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| !m["field"].is_null()),
        "{}",
        r.body["matches"]
    );
    assert!(r.body["unknown"]["brands"].as_array().unwrap().is_empty());

    let full = call(
        &app,
        "GET",
        "/api/catalog/export?format=csv&full=true",
        None,
        &o,
    )
    .await;
    let text = String::from_utf8(full.bytes).unwrap();
    assert_eq!(text.lines().count(), 3, "une ligne par groupe de ports");
    assert!(text.contains(";SDI 3G;entrée;F;20;"), "{text}");

    let xlsx = call(&app, "GET", "/api/catalog/export?format=xlsx", None, &o).await;
    assert_eq!(&xlsx.bytes[..2], b"PK");
    let r = analyze(&app, &o, "export.xlsx", xlsx.bytes).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["row_count"], 1);
    assert_eq!(r.body["preview"][0][9], "2");
}

#[tokio::test]
async fn exchange_between_sites_and_rollback() {
    let a = app().await;
    let oa = owner(&a).await;
    let b = brand(&a, &oa, "Blackmagic Design").await;
    item(&a, &oa, "equipment", switcher(&b)).await;
    let doc = call(
        &a,
        "GET",
        "/api/catalog/exchange?sections=refs/brands,equipment",
        None,
        &oa,
    )
    .await;
    assert_eq!(doc.status, StatusCode::OK);
    let doc: Value = serde_json::from_slice(&doc.bytes).unwrap();
    assert_eq!(doc["format"], "strata-arkyo/catalogue");
    assert!(doc["items"]["equipment"][0].get("created_by").is_none());

    // Autre site : fusion.
    let site = app().await;
    let o = owner(&site).await;
    let local = item(&site, &o, "adapters", json!({"name": "XLR3 F vers jack", "input": {"connector_id": "xlr3", "gender": "F"}, "outputs": [{"connector_id": "jack635"}]})).await;
    let r = call(
        &site,
        "POST",
        "/api/catalog/exchange",
        Some(json!({"document": doc, "mode": "merge"})),
        &o,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        (
            r.body["refs"]["added"].as_i64(),
            r.body["items"]["added"].as_i64()
        ),
        (Some(1), Some(1))
    );
    // Rejouer ne change rien.
    let r = call(
        &site,
        "POST",
        "/api/catalog/exchange",
        Some(json!({"document": doc})),
        &o,
    )
    .await;
    assert_eq!(
        (
            r.body["items"]["added"].as_i64(),
            r.body["items"]["unchanged"].as_i64()
        ),
        (Some(0), Some(1))
    );

    // Une fiche qui cite une marque absente est refusée, pas importée à moitié.
    let r = call(
        &site,
        "POST",
        "/api/catalog/exchange",
        Some(json!({"document": doc, "sections": "equipment", "mode": "replace"})),
        &o,
    )
    .await;
    assert_eq!(r.body["items"]["unchanged"], 0);
    let mut orphan = doc.clone();
    orphan["items"]["equipment"][0]["id"] = json!("autre");
    orphan["items"]["equipment"][0]["data"]["brand_id"] = json!("marque-absente");
    let r = call(
        &site,
        "POST",
        "/api/catalog/exchange",
        Some(json!({"document": orphan, "sections": "equipment"})),
        &o,
    )
    .await;
    assert_eq!(r.body["items"]["rejected"], 1);
    assert_eq!(r.body["problems"][0]["id"], "autre");

    // Remplacement des adaptateurs par un fichier qui n'en a pas, puis retour arrière.
    let mut empty = doc.clone();
    empty["items"]["adapters"] = json!([]);
    let r = call(
        &site,
        "POST",
        "/api/catalog/exchange",
        Some(json!({"document": empty, "sections": "adapters", "mode": "replace"})),
        &o,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        call(&site, "GET", "/api/catalog/items?kind=adapters", None, &o)
            .await
            .body["total"],
        0
    );
    let snap = r.body["snapshot_id"].as_str().unwrap();
    let r = call(
        &site,
        "POST",
        &format!("/api/catalog/snapshots/{snap}/restore"),
        None,
        &o,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let list = call(&site, "GET", "/api/catalog/items?kind=adapters", None, &o).await;
    assert_eq!(list.body["items"][0]["id"], local["item"]["id"]);
    assert!(
        call(&site, "GET", "/api/catalog/snapshots", None, &o)
            .await
            .body["snapshots"]
            .as_array()
            .unwrap()
            .len()
            >= 3
    );

    // Remplacer les marques par un fichier vide garde celle qu'une fiche cite encore.
    let mut no_brands = doc.clone();
    no_brands["refs"]["brands"] = json!([]);
    let r = call(
        &site,
        "POST",
        "/api/catalog/exchange",
        Some(json!({"document": no_brands, "sections": "refs/brands", "mode": "replace"})),
        &o,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["problems"][0]["name"], "Blackmagic Design");
    assert_eq!(
        call(&site, "GET", "/api/catalog/refs/brands", None, &o)
            .await
            .body["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // Mauvais fichier.
    let r = call(
        &site,
        "POST",
        "/api/catalog/exchange",
        Some(json!({"document": {"format": "autre"}})),
        &o,
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
}
