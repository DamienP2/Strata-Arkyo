//! Source GitHub : manifeste et paquet lus dans les Releases d'un dépôt privé, avec jeton.
//! L'API GitHub est simulée localement.

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use ed25519_dalek::SigningKey;
use serde_json::json;
use std::sync::Arc;
use strata_update::signing::sha256_hex;
use strata_update::{ReleaseManifest, TrustedKeys, host_arch, manifest, package};

struct Srv {
    base: String,
    manifest: Vec<u8>,
    package: Vec<u8>,
    package_name: String,
}

fn authorized(h: &HeaderMap) -> bool {
    h.get("authorization").and_then(|v| v.to_str().ok()) == Some("Bearer jeton-lecture")
}

fn release(s: &Srv, tag: &str) -> serde_json::Value {
    json!({
        "tag_name": tag, "draft": false, "prerelease": false,
        "assets": [
            {"name": "manifest.json", "url": format!("{}/assets/1", s.base)},
            {"name": s.package_name, "url": format!("{}/assets/2", s.base)},
        ]
    })
}

#[tokio::test]
async fn reads_releases_of_a_private_repository() {
    let key = SigningKey::generate(&mut rand_core::OsRng);
    let tmp = tempfile::tempdir().unwrap();
    let path = package::pack(b"programme 0.3.0", "0.3.0", host_arch(), &key, tmp.path()).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    let m = ReleaseManifest {
        format: 1,
        channel: "stable".into(),
        version: "0.3.0".into(),
        arch: host_arch().into(),
        package: format!("packages/{name}"),
        sha256: sha256_hex(&bytes),
        size: bytes.len() as u64,
        published_at: "2026-11-12T12:00:00Z".into(),
        notes: vec!["Essai".into()],
    };
    let env = serde_json::to_vec(&m.sign(&key).unwrap()).unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let srv = Arc::new(Srv {
        base: base.clone(),
        manifest: env,
        package: bytes.clone(),
        package_name: name,
    });
    let app = Router::new()
        .route(
            "/repos/o/r/releases/latest",
            get(|State(s): State<Arc<Srv>>, h: HeaderMap| async move {
                if !authorized(&h) {
                    return StatusCode::NOT_FOUND.into_response();
                }
                Json(release(&s, "v0.3.0")).into_response()
            }),
        )
        .route(
            "/repos/o/r/releases/tags/v0.3.0",
            get(|State(s): State<Arc<Srv>>, h: HeaderMap| async move {
                if !authorized(&h) {
                    return StatusCode::NOT_FOUND.into_response();
                }
                Json(release(&s, "v0.3.0")).into_response()
            }),
        )
        .route(
            "/assets/{id}",
            get(
                |State(s): State<Arc<Srv>>,
                 axum::extract::Path(id): axum::extract::Path<u32>,
                 h: HeaderMap| async move {
                    let raw = h.get("accept").and_then(|v| v.to_str().ok())
                        == Some("application/octet-stream");
                    if !authorized(&h) || !raw {
                        return StatusCode::NOT_FOUND.into_response();
                    }
                    let body = if id == 1 {
                        s.manifest.clone()
                    } else {
                        s.package.clone()
                    };
                    body.into_response()
                },
            ),
        )
        .with_state(srv);
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    // SAFETY : seul test de ce fichier, aucune autre tâche ne lit l'environnement en parallèle.
    unsafe {
        std::env::set_var("STRATA_GITHUB_API", &base);
        std::env::remove_var("STRATA_UPDATE_TOKEN");
    }
    let keys = TrustedKeys::new(vec![key.verifying_key()]);

    // Sans jeton, le dépôt privé reste invisible.
    let e = manifest::fetch_manifest("github:o/r", "stable", &keys)
        .await
        .unwrap_err();
    assert!(e.to_string().contains("404"), "{e}");

    unsafe { std::env::set_var("STRATA_UPDATE_TOKEN", "jeton-lecture") };
    let got = manifest::fetch_manifest("github:o/r", "stable", &keys)
        .await
        .unwrap();
    assert_eq!(got.version, "0.3.0");
    let pkg = manifest::fetch_package("https://github.com/o/r", &got)
        .await
        .unwrap();
    assert_eq!(pkg, bytes);
    assert_eq!(sha256_hex(&pkg), got.sha256);

    // Une autre clé : la signature du manifeste est refusée.
    let other = TrustedKeys::new(vec![
        SigningKey::generate(&mut rand_core::OsRng).verifying_key(),
    ]);
    assert!(
        manifest::fetch_manifest("github:o/r", "stable", &other)
            .await
            .is_err()
    );
}
