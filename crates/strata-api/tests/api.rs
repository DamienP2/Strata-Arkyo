//! Tests de bout en bout de l'API du socle : chaque test rejoue une règle du cahier des charges.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use std::net::SocketAddr;
use strata_api::{AppState, CSRF_HEADER, router};
use strata_store::Store;
use tower::ServiceExt;

const OWNER_PW: &str = "mot-de-passe-solide";

async fn app_from(addr: &str) -> Router {
    let store = Store::open_in_memory().await.unwrap();
    router(AppState::new(store, false, None))
        .layer(MockConnectInfo(addr.parse::<SocketAddr>().unwrap()))
}

async fn app() -> Router {
    app_from("127.0.0.1:50000").await
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
    call_with(app, method, uri, body, cookie, true).await
}

async fn call_with(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    cookie: Option<&str>,
    csrf: bool,
) -> Resp {
    let mut req = Request::builder().method(method).uri(uri);
    if csrf {
        req = req.header(CSRF_HEADER, "1");
    }
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
    }
    let req = match body {
        Some(b) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(b.to_string())),
        None => req.body(Body::empty()),
    }
    .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.split(';').next().unwrap().to_string());
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    Resp {
        status,
        body,
        cookie,
    }
}

/// Crée le propriétaire et renvoie son cookie de session.
async fn setup_owner(app: &Router) -> String {
    let r = call(
        app,
        "POST",
        "/api/setup",
        Some(json!({"email": "damien@studio.fr", "name": "Damien P.", "password": OWNER_PW})),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    r.cookie.expect("cookie de session")
}

async fn login(app: &Router, login: &str, password: &str) -> Resp {
    call(
        app,
        "POST",
        "/api/auth/login",
        Some(json!({"login": login, "password": password})),
        None,
    )
    .await
}

/// Crée un membre, le connecte et remplace son mot de passe temporaire. Renvoie (id, cookie).
async fn ready_member(app: &Router, owner: &str, email: &str, role: &str) -> (String, String) {
    let r = call(
        app,
        "POST",
        "/api/members",
        Some(json!({"email": email, "role_id": role})),
        Some(owner),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let id = r.body["member"]["id"].as_str().unwrap().to_string();
    let temp = r.body["temporary_password"].as_str().unwrap().to_string();
    let l = login(app, email, &temp).await;
    let cookie = l.cookie.unwrap();
    let r = call(
        app,
        "POST",
        "/api/me/password",
        Some(json!({"current": temp, "new": "nouveau-secret"})),
        Some(&cookie),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    (id, cookie)
}

#[tokio::test]
async fn health_answers_without_session() {
    let app = app().await;
    let r = call(&app, "GET", "/api/health", None, None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["status"], "ok");
    assert_eq!(r.body["schema"], 11);
}

#[tokio::test]
async fn first_admin_only_from_the_machine_and_only_once() {
    let remote = app_from("192.168.1.20:50000").await;
    let r = call(
        &remote,
        "POST",
        "/api/setup",
        Some(json!({"email": "a@b.fr", "password": OWNER_PW})),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);

    let app = app().await;
    assert_eq!(
        call(&app, "GET", "/api/setup", None, None).await.body["needed"],
        true
    );
    let cookie = setup_owner(&app).await;
    let me = call(&app, "GET", "/api/me", None, Some(&cookie)).await;
    assert_eq!(me.body["is_owner"], true);
    assert_eq!(me.body["permissions"]["parametres"], "admin");
    let r = call(
        &app,
        "POST",
        "/api/setup",
        Some(json!({"email": "x@b.fr", "password": OWNER_PW})),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn writes_need_the_csrf_header() {
    let app = app().await;
    let r = call_with(
        &app,
        "POST",
        "/api/auth/login",
        Some(json!({"login": "x", "password": "y"})),
        None,
        false,
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(r.body["code"], "csrf");
}

#[tokio::test]
async fn one_failure_message_whatever_the_cause() {
    let app = app().await;
    setup_owner(&app).await;
    let unknown = login(&app, "personne@studio.fr", OWNER_PW).await;
    let wrong = login(&app, "damien@studio.fr", "mauvais-mot-de-passe").await;
    assert_eq!(unknown.status, StatusCode::UNAUTHORIZED);
    assert_eq!(unknown.body, wrong.body);
    let ok = login(&app, "DAMIEN@studio.fr", OWNER_PW).await;
    assert_eq!(ok.status, StatusCode::OK);
}

#[tokio::test]
async fn temporary_password_must_be_changed_first() {
    let app = app().await;
    let owner = setup_owner(&app).await;
    let r = call(
        &app,
        "POST",
        "/api/members",
        Some(json!({"email": "lea@studio.fr", "name": "Léa Martin", "username": "lea"})),
        Some(&owner),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED);
    assert_eq!(r.body["member"]["role_id"], "technician");
    let temp = r.body["temporary_password"].as_str().unwrap().to_string();
    assert_eq!(temp.len(), 12);

    let lea = login(&app, "lea", &temp).await.cookie.unwrap();
    let blocked = call(&app, "GET", "/api/members", None, Some(&lea)).await;
    assert_eq!(blocked.body["code"], "password_change_required");

    let bad = call(
        &app,
        "POST",
        "/api/me/password",
        Some(json!({"current": "faux", "new": "nouveau-secret"})),
        Some(&lea),
    )
    .await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);
    let short = call(
        &app,
        "POST",
        "/api/me/password",
        Some(json!({"current": temp, "new": "court"})),
        Some(&lea),
    )
    .await;
    assert_eq!(short.status, StatusCode::UNPROCESSABLE_ENTITY);
    let ok = call(
        &app,
        "POST",
        "/api/me/password",
        Some(json!({"current": temp, "new": "nouveau-secret"})),
        Some(&lea),
    )
    .await;
    assert_eq!(ok.status, StatusCode::OK);
    assert_eq!(
        call(&app, "GET", "/api/members", None, Some(&lea))
            .await
            .status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn changing_password_closes_other_sessions() {
    let app = app().await;
    let first = setup_owner(&app).await;
    let second = login(&app, "damien@studio.fr", OWNER_PW)
        .await
        .cookie
        .unwrap();
    let r = call(
        &app,
        "POST",
        "/api/me/password",
        Some(json!({"current": OWNER_PW, "new": "encore-plus-solide"})),
        Some(&second),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(
        call(&app, "GET", "/api/me", None, Some(&first))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, "GET", "/api/me", None, Some(&second))
            .await
            .status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn members_without_rights_cannot_manage_members() {
    let app = app().await;
    let owner = setup_owner(&app).await;
    let (_, tech) = ready_member(&app, &owner, "hugo@studio.fr", "technician").await;
    let r = call(
        &app,
        "POST",
        "/api/members",
        Some(json!({"email": "x@studio.fr"})),
        Some(&tech),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    // La liste des membres reste visible de tout membre connecté.
    assert_eq!(
        call(&app, "GET", "/api/members", None, Some(&tech))
            .await
            .status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn nobody_is_created_owner_and_duplicates_are_refused() {
    let app = app().await;
    let owner = setup_owner(&app).await;
    let r = call(
        &app,
        "POST",
        "/api/members",
        Some(json!({"email": "x@studio.fr", "role_id": "owner"})),
        Some(&owner),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    call(
        &app,
        "POST",
        "/api/members",
        Some(json!({"email": "x@studio.fr"})),
        Some(&owner),
    )
    .await;
    let r = call(
        &app,
        "POST",
        "/api/members",
        Some(json!({"email": "X@studio.fr"})),
        Some(&owner),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn the_last_owner_stays_and_cannot_deactivate_himself() {
    let app = app().await;
    let owner = setup_owner(&app).await;
    let me = call(&app, "GET", "/api/me", None, Some(&owner)).await;
    let my_id = me.body["account"]["id"].as_str().unwrap().to_string();
    let r = call(
        &app,
        "PATCH",
        &format!("/api/members/{my_id}"),
        Some(json!({"active": false})),
        Some(&owner),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = call(
        &app,
        "PATCH",
        &format!("/api/members/{my_id}"),
        Some(json!({"role_id": "technician"})),
        Some(&owner),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "dernier propriétaire");

    // Avec un second propriétaire, le premier peut être rétrogradé.
    let (other, _) = ready_member(&app, &owner, "sofia@studio.fr", "technician").await;
    let r = call(
        &app,
        "PATCH",
        &format!("/api/members/{other}"),
        Some(json!({"role_id": "owner"})),
        Some(&owner),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let r = call(
        &app,
        "PATCH",
        &format!("/api/members/{my_id}"),
        Some(json!({"role_id": "technician"})),
        Some(&owner),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
}

#[tokio::test]
async fn a_member_admin_cannot_grant_above_his_own_level() {
    let app = app().await;
    let owner = setup_owner(&app).await;
    let perms = json!({"projets": "ecriture", "catalogue": "lecture", "membres": "admin"});
    let r = call(
        &app,
        "POST",
        "/api/roles",
        Some(json!({"name": "Responsable d'équipe", "color": "#15935A", "permissions": perms})),
        Some(&owner),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    let lead_role = r.body["id"].as_str().unwrap().to_string();
    let (lead_id, lead) = ready_member(&app, &owner, "lea@studio.fr", &lead_role).await;

    let too_high = json!({"parametres": "admin"});
    let r = call(
        &app,
        "POST",
        "/api/roles",
        Some(json!({"name": "Super", "permissions": too_high})),
        Some(&lead),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(
        r.body["message"],
        "Vous ne pouvez pas accorder un niveau supérieur au vôtre."
    );

    // Ni attribuer un rôle plus élevé que le sien.
    let (tech_id, _) = ready_member(&app, &owner, "hugo@studio.fr", "technician").await;
    let r = call(
        &app,
        "PATCH",
        &format!("/api/members/{tech_id}"),
        Some(json!({"role_id": "project_lead"})),
        Some(&lead),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    // Ni modifier son propre rôle.
    let r = call(
        &app,
        "PATCH",
        &format!("/api/members/{lead_id}"),
        Some(json!({"role_id": "viewer"})),
        Some(&lead),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    // Mais il gère un compte dans ses limites.
    let r = call(
        &app,
        "PATCH",
        &format!("/api/members/{tech_id}"),
        Some(json!({"role_id": "viewer"})),
        Some(&lead),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
}

#[tokio::test]
async fn deactivation_closes_sessions_and_refuses_login() {
    let app = app().await;
    let owner = setup_owner(&app).await;
    let (id, hugo) = ready_member(&app, &owner, "hugo@studio.fr", "technician").await;
    let r = call(
        &app,
        "PATCH",
        &format!("/api/members/{id}"),
        Some(json!({"active": false})),
        Some(&owner),
    )
    .await;
    assert_eq!(r.body["active"], false);
    assert_eq!(
        call(&app, "GET", "/api/me", None, Some(&hugo)).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(&app, "hugo@studio.fr", "nouveau-secret").await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn removed_member_is_kept_and_recreating_reactivates() {
    let app = app().await;
    let owner = setup_owner(&app).await;
    let (id, _) = ready_member(&app, &owner, "hugo@studio.fr", "technician").await;
    assert_eq!(
        call(
            &app,
            "DELETE",
            &format!("/api/members/{id}"),
            None,
            Some(&owner)
        )
        .await
        .status,
        StatusCode::NO_CONTENT
    );
    let listed = call(&app, "GET", "/api/members", None, Some(&owner)).await;
    assert_eq!(listed.body["members"].as_array().unwrap().len(), 1);
    let all = call(&app, "GET", "/api/members?removed=true", None, Some(&owner)).await;
    assert_eq!(all.body["members"].as_array().unwrap().len(), 2);

    let r = call(
        &app,
        "POST",
        "/api/members",
        Some(json!({"email": "hugo@studio.fr", "role_id": "viewer"})),
        Some(&owner),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["reactivated"], true);
    assert_eq!(r.body["member"]["id"], id.as_str());
    assert_eq!(r.body["member"]["role_id"], "viewer");
    assert_eq!(r.body["member"]["must_change_password"], true);
}

#[tokio::test]
async fn roles_duplicate_delete_and_builtins_stay() {
    let app = app().await;
    let owner = setup_owner(&app).await;
    let r = call(
        &app,
        "POST",
        "/api/roles/technician/duplicate",
        None,
        Some(&owner),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED);
    assert_eq!(r.body["name"], "Technicien (copie)");
    let copy = r.body["id"].as_str().unwrap().to_string();
    let r = call(
        &app,
        "POST",
        "/api/roles/technician/duplicate",
        None,
        Some(&owner),
    )
    .await;
    assert_eq!(r.body["name"], "Technicien (copie 2)");

    let (member, _) = ready_member(&app, &owner, "hugo@studio.fr", &copy).await;
    let r = call(
        &app,
        "DELETE",
        &format!("/api/roles/{copy}"),
        None,
        Some(&owner),
    )
    .await;
    assert_eq!(r.body["moved_members"], 1);
    let list = call(&app, "GET", "/api/members", None, Some(&owner)).await;
    let hugo = list.body["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == member.as_str())
        .unwrap()
        .clone();
    assert_eq!(hugo["role_id"], "technician");

    assert_eq!(
        call(&app, "DELETE", "/api/roles/viewer", None, Some(&owner))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    let r = call(
        &app,
        "PATCH",
        "/api/roles/owner",
        Some(json!({"name": "Chef"})),
        Some(&owner),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = call(
        &app,
        "POST",
        "/api/roles",
        Some(json!({"name": "technicien"})),
        Some(&owner),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn every_change_is_audited() {
    let app = app().await;
    let owner = setup_owner(&app).await;
    let (id, _) = ready_member(&app, &owner, "hugo@studio.fr", "technician").await;
    call(
        &app,
        "PATCH",
        &format!("/api/members/{id}"),
        Some(json!({"role_id": "viewer"})),
        Some(&owner),
    )
    .await;
    let r = call(&app, "GET", "/api/audit?limit=10", None, Some(&owner)).await;
    let last = &r.body["entries"][0];
    assert_eq!(last["action"], "update");
    assert_eq!(last["before"]["role_id"], "technician");
    assert_eq!(last["after"]["role_id"], "viewer");
}

#[tokio::test]
async fn web_app_is_served_for_screens_but_not_for_api() {
    let app = app().await;
    for uri in ["/", "/catalogue/equipment", "/admin/membres"] {
        let res = app
            .clone()
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "{uri}");
        let ct = res.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .to_string();
        assert!(ct.starts_with("text/html"), "{uri} : {ct}");
    }
    let r = call(&app, "GET", "/api/inconnu", None, None).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(r.body["code"], "not_found");
    let res = app
        .clone()
        .oneshot(
            Request::get("/assets/absent.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
