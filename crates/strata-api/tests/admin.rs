//! Administration du site de bout en bout : droits, sauvegardes, restauration,
//! mises à jour hors ligne et en ligne, bascule et redémarrage.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Request, StatusCode, header};
use ed25519_dalek::SigningKey;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use strata_api::ops::Ops;
use strata_api::{AppState, CSRF_HEADER, VERSION, router};
use strata_store::Store;
use strata_update::{
    Layout, ReleaseManifest, TrustedKeys, host_arch, package, signing::sha256_hex,
};
use tokio::sync::watch;
use tower::ServiceExt;

struct Site {
    _tmp: tempfile::TempDir,
    root: std::path::PathBuf,
    app: Router,
    ops: Arc<Ops>,
    restart: watch::Receiver<bool>,
    owner: String,
    key: SigningKey,
}

/// Programme factice : répond à `version --short` et réussit `selftest`.
fn fake_binary(version: &str) -> Vec<u8> {
    format!("#!/bin/sh\nif [ \"$1\" = version ]; then echo {version}; exit 0; fi\nif [ \"$1\" = selftest ]; then exit 0; fi\nexit 1\n")
        .into_bytes()
}

/// Installation « par paquet » de la version en cours dans `root/opt`.
fn install_current(opt: &Path) {
    let dir = opt.join("versions").join(VERSION);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("strata"), fake_binary(VERSION)).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir.join("strata"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::os::unix::fs::symlink(&dir, opt.join("current")).unwrap();
}

async fn site(managed: bool, update_url: Option<String>) -> Site {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let data = root.join("data");
    std::fs::create_dir_all(&data).unwrap();
    if managed {
        install_current(&root.join("opt"));
    }
    let key = SigningKey::generate(&mut rand_core::OsRng);
    let store = Store::open(&data.join("strata.db")).await.unwrap();
    let (ops, restart) = Ops::new(
        data,
        Layout::new(root.join("opt")),
        TrustedKeys::new(vec![key.verifying_key()]),
        update_url,
        Duration::ZERO,
        VERSION,
    );
    let app = router(AppState {
        store,
        secure_cookies: false,
        ops: Some(ops.clone()),
    })
    .layer(MockConnectInfo(
        "127.0.0.1:1".parse::<SocketAddr>().unwrap(),
    ));
    let r = call(
        &app,
        "POST",
        "/api/setup",
        Some(json!({"email": "damien@studio.fr", "password": "mot-de-passe-solide"})),
        None,
    )
    .await;
    let owner = r.2.unwrap();
    Site {
        _tmp: tmp,
        root,
        app,
        ops,
        restart,
        owner,
        key,
    }
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    cookie: Option<&str>,
) -> (StatusCode, Value, Option<String>) {
    raw(
        app,
        method,
        uri,
        body.map(|b| ("application/json", b.to_string().into_bytes())),
        cookie,
    )
    .await
}

async fn raw(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<(&str, Vec<u8>)>,
    cookie: Option<&str>,
) -> (StatusCode, Value, Option<String>) {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header(CSRF_HEADER, "1");
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
    }
    let req = match body {
        Some((ct, b)) => req.header(header::CONTENT_TYPE, ct).body(Body::from(b)),
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
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        cookie,
    )
}

async fn wait_restart(rx: &mut watch::Receiver<bool>) {
    tokio::time::timeout(Duration::from_secs(10), rx.wait_for(|r| *r))
        .await
        .expect("redémarrage demandé")
        .unwrap();
}

#[tokio::test]
async fn only_site_admins_reach_the_administration() {
    let s = site(true, None).await;
    let (st, body, _) = call(
        &s.app,
        "POST",
        "/api/members",
        Some(json!({"email": "lea@studio.fr", "role_id": "project_lead"})),
        Some(&s.owner),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let temp = body["temporary_password"].as_str().unwrap();
    let (_, _, lea) = call(
        &s.app,
        "POST",
        "/api/auth/login",
        Some(json!({"login": "lea@studio.fr", "password": temp})),
        None,
    )
    .await;
    let lea = lea.unwrap();
    call(
        &s.app,
        "POST",
        "/api/me/password",
        Some(json!({"current": temp, "new": "nouveau-secret"})),
        Some(&lea),
    )
    .await;
    for uri in [
        "/api/admin/system",
        "/api/admin/update",
        "/api/admin/backups",
    ] {
        assert_eq!(
            call(&s.app, "GET", uri, None, Some(&lea)).await.0,
            StatusCode::FORBIDDEN,
            "{uri}"
        );
        assert_eq!(
            call(&s.app, "GET", uri, None, Some(&s.owner)).await.0,
            StatusCode::OK,
            "{uri}"
        );
    }
}

#[tokio::test]
async fn manual_backup_restore_staging_and_cancel() {
    let s = site(true, None).await;
    let (st, info, _) = call(
        &s.app,
        "POST",
        "/api/admin/backups",
        Some(json!({"label": "Avant la tournée"})),
        Some(&s.owner),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{info}");
    let name = info["name"].as_str().unwrap().to_string();
    assert_eq!(info["manifest"]["kind"], "manual");

    let (_, list, _) = call(&s.app, "GET", "/api/admin/backups", None, Some(&s.owner)).await;
    assert_eq!(list["backups"][0]["name"], name.as_str());
    assert!(list["last_error"].is_null());

    let res = s
        .app
        .clone()
        .oneshot(
            Request::get(format!("/api/admin/backups/{name}/download"))
                .header(header::COOKIE, &s.owner)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(
        !to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .is_empty()
    );

    let (st, body, _) = call(
        &s.app,
        "POST",
        &format!("/api/admin/backups/{name}/restore"),
        None,
        Some(&s.owner),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    let (_, list, _) = call(&s.app, "GET", "/api/admin/backups", None, Some(&s.owner)).await;
    assert!(!list["pending_restore"].is_null());
    assert_eq!(
        call(&s.app, "DELETE", "/api/admin/restore", None, Some(&s.owner))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&s.app, "DELETE", "/api/admin/restore", None, Some(&s.owner))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        call(
            &s.app,
            "GET",
            "/api/admin/backups/..%2F..%2Fetc%2Fpasswd/download",
            None,
            Some(&s.owner)
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn restore_with_restart_asks_for_a_restart() {
    let mut s = site(true, None).await;
    let (_, info, _) = call(&s.app, "POST", "/api/admin/backups", None, Some(&s.owner)).await;
    let name = info["name"].as_str().unwrap();
    let (st, _, _) = call(
        &s.app,
        "POST",
        &format!("/api/admin/backups/{name}/restore"),
        Some(json!({"restart": true})),
        Some(&s.owner),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    wait_restart(&mut s.restart).await;
}

#[tokio::test]
async fn offline_package_is_verified_tested_backed_up_then_switched() {
    let mut s = site(true, None).await;
    let pkg = package::pack(
        &fake_binary("9.0.0"),
        "9.0.0",
        host_arch(),
        &s.key,
        &s.root.join("dist"),
    )
    .unwrap();
    let bytes = std::fs::read(&pkg).unwrap();
    let (st, body, _) = raw(
        &s.app,
        "POST",
        "/api/admin/update/upload",
        Some(("application/octet-stream", bytes)),
        Some(&s.owner),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(body["scheduled"]["version"], "9.0.0");
    wait_restart(&mut s.restart).await;
    assert_eq!(s.ops.layout.current_version().as_deref(), Some("9.0.0"));
    assert_eq!(s.ops.layout.state().pending.as_deref(), Some("9.0.0"));
    // Une sauvegarde « avant mise à jour » a été prise.
    let (_, list, _) = call(&s.app, "GET", "/api/admin/backups", None, Some(&s.owner)).await;
    assert_eq!(list["backups"][0]["manifest"]["kind"], "pre-update");
}

#[tokio::test]
async fn a_package_signed_by_someone_else_is_refused() {
    let s = site(true, None).await;
    let pirate = SigningKey::generate(&mut rand_core::OsRng);
    let pkg = package::pack(
        &fake_binary("9.0.0"),
        "9.0.0",
        host_arch(),
        &pirate,
        &s.root.join("dist"),
    )
    .unwrap();
    let (st, body, _) = raw(
        &s.app,
        "POST",
        "/api/admin/update/upload",
        Some(("application/octet-stream", std::fs::read(&pkg).unwrap())),
        Some(&s.owner),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        body["message"].as_str().unwrap().contains("signature"),
        "{body}"
    );
    assert_eq!(s.ops.layout.current_version().as_deref(), Some(VERSION));
}

#[tokio::test]
async fn an_older_package_is_refused() {
    let s = site(true, None).await;
    let pkg = package::pack(
        &fake_binary("0.0.1"),
        "0.0.1",
        host_arch(),
        &s.key,
        &s.root.join("dist"),
    )
    .unwrap();
    let (st, _, _) = raw(
        &s.app,
        "POST",
        "/api/admin/update/upload",
        Some(("application/octet-stream", std::fs::read(&pkg).unwrap())),
        Some(&s.owner),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);
}

#[tokio::test]
async fn online_update_from_a_mirror_scheduled_tonight_then_cancelled() {
    let tmp_site = tempfile::tempdir().unwrap();
    let mirror = tmp_site.path();
    let s = site(true, Some(format!("file://{}", mirror.display()))).await;

    // Publication par l'éditeur : paquet + manifeste signé du canal stable.
    let pkg = package::pack(
        &fake_binary("9.1.0"),
        "9.1.0",
        host_arch(),
        &s.key,
        &mirror.join("packages"),
    )
    .unwrap();
    let bytes = std::fs::read(&pkg).unwrap();
    let m = ReleaseManifest {
        format: 1,
        channel: "stable".into(),
        version: "9.1.0".into(),
        arch: host_arch().into(),
        package: format!("packages/{}", pkg.file_name().unwrap().to_string_lossy()),
        sha256: sha256_hex(&bytes),
        size: bytes.len() as u64,
        published_at: "2026-11-11T00:00:00Z".into(),
        notes: vec!["Synoptique plus rapide".into()],
    };
    std::fs::create_dir_all(mirror.join("stable")).unwrap();
    std::fs::write(
        mirror.join("stable/manifest.json"),
        serde_json::to_vec(&m.sign(&s.key).unwrap()).unwrap(),
    )
    .unwrap();

    let (st, body, _) = call(
        &s.app,
        "POST",
        "/api/admin/update/check",
        None,
        Some(&s.owner),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(body["available"]["version"], "9.1.0");

    let (st, body, _) = call(
        &s.app,
        "POST",
        "/api/admin/update/install",
        Some(json!({"when": "night"})),
        Some(&s.owner),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    let (_, status, _) = call(&s.app, "GET", "/api/admin/update", None, Some(&s.owner)).await;
    assert_eq!(status["status"]["phase"], "scheduled");
    assert_eq!(status["installed"].as_array().unwrap().len(), 2, "{status}");
    // Rien n'a basculé avant l'heure.
    assert_eq!(s.ops.layout.current_version().as_deref(), Some(VERSION));
    assert_eq!(
        call(
            &s.app,
            "DELETE",
            "/api/admin/update/schedule",
            None,
            Some(&s.owner)
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );

    // Le canal Bêta n'a rien publié : la recherche le dit.
    call(
        &s.app,
        "PATCH",
        "/api/admin/settings",
        Some(json!({"update_channel": "beta"})),
        Some(&s.owner),
    )
    .await;
    let (st, _, _) = call(
        &s.app,
        "POST",
        "/api/admin/update/check",
        None,
        Some(&s.owner),
    )
    .await;
    assert_eq!(st, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn a_server_not_installed_by_package_does_not_update() {
    let s = site(false, None).await;
    let pkg = package::pack(
        &fake_binary("9.0.0"),
        "9.0.0",
        host_arch(),
        &s.key,
        &s.root.join("dist"),
    )
    .unwrap();
    let (st, body, _) = raw(
        &s.app,
        "POST",
        "/api/admin/update/upload",
        Some(("application/octet-stream", std::fs::read(&pkg).unwrap())),
        Some(&s.owner),
    )
    .await;
    assert_eq!(st, StatusCode::SERVICE_UNAVAILABLE, "{body}");
}

#[tokio::test]
async fn backup_dir_must_be_absolute_and_writable() {
    let s = site(true, None).await;
    let (st, _, _) = call(
        &s.app,
        "PATCH",
        "/api/admin/settings",
        Some(json!({"backup_dir": "relatif/nas"})),
        Some(&s.owner),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let nas = s.root.join("nas");
    let (st, body, _) = call(
        &s.app,
        "PATCH",
        "/api/admin/settings",
        Some(json!({"backup_dir": nas})),
        Some(&s.owner),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    call(&s.app, "POST", "/api/admin/backups", None, Some(&s.owner)).await;
    assert_eq!(
        std::fs::read_dir(&nas).unwrap().count(),
        2,
        "archive et sa fiche"
    );
}
