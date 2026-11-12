//! Source GitHub : dernière version lue dans les Releases, programme téléchargé et vérifié
//! par son empreinte (SHA256SUMS). L'API GitHub est simulée localement.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use serde_json::json;
use std::sync::Arc;
use strata_update::github::{self, binary_name};
use strata_update::signing::sha256_hex;
use strata_update::{host_arch, manifest};

struct Srv {
    base: String,
    program: Vec<u8>,
    sums: String,
    /// Dépôt privé : le jeton est exigé.
    private: bool,
}

fn allowed(s: &Srv, h: &HeaderMap) -> bool {
    !s.private
        || h.get("authorization").and_then(|v| v.to_str().ok()) == Some("Bearer jeton-lecture")
}

fn release(s: &Srv) -> serde_json::Value {
    json!({
        "tag_name": "v0.3.0", "draft": false, "prerelease": false,
        "published_at": "2026-11-12T12:00:00Z",
        "body": "## Nouveautés\n* Ports en trois colonnes\n* Mises à jour simplifiées\n",
        "assets": [
            {"name": binary_name("0.3.0", host_arch()), "size": s.program.len(), "url": format!("{}/assets/1", s.base)},
            {"name": "SHA256SUMS", "size": s.sums.len(), "url": format!("{}/assets/2", s.base)},
        ]
    })
}

async fn serve(private: bool, program: Vec<u8>, sums: String) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let srv = Arc::new(Srv {
        base: base.clone(),
        program,
        sums,
        private,
    });
    let rel = |State(s): State<Arc<Srv>>, h: HeaderMap| async move {
        if !allowed(&s, &h) {
            return StatusCode::NOT_FOUND.into_response();
        }
        Json(release(&s)).into_response()
    };
    let app = Router::new()
        .route("/repos/o/r/releases/latest", get(rel))
        .route("/repos/o/r/releases/tags/v0.3.0", get(rel))
        .route(
            "/assets/{id}",
            get(
                |State(s): State<Arc<Srv>>, Path(id): Path<u32>, h: HeaderMap| async move {
                    let raw = h.get("accept").and_then(|v| v.to_str().ok())
                        == Some("application/octet-stream");
                    if !allowed(&s, &h) || !raw {
                        return StatusCode::NOT_FOUND.into_response();
                    }
                    if id == 1 {
                        s.program.clone().into_response()
                    } else {
                        s.sums.clone().into_response()
                    }
                },
            ),
        )
        .with_state(srv);
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    base
}

#[tokio::test]
async fn latest_release_and_program() {
    let program = b"#!/bin/sh\necho 0.3.0\n".to_vec();
    let name = binary_name("0.3.0", host_arch());
    let sums = format!(
        "{}  {name}\n{}  strata-arkyo_0.3.0_amd64.deb\n",
        sha256_hex(&program),
        "c".repeat(64)
    );

    // Dépôt public : rien à configurer.
    let public = serve(false, program.clone(), sums.clone()).await;
    // Dépôt privé : jeton exigé.
    let private = serve(true, program.clone(), sums).await;
    let repo = github::parse("github:o/r").unwrap();

    // SAFETY : seul test de ce fichier, l'environnement n'est pas lu en parallèle.
    unsafe {
        std::env::set_var("STRATA_GITHUB_API", &public);
        std::env::remove_var("STRATA_UPDATE_TOKEN");
    }
    let m = github::latest(&repo, "stable", host_arch()).await.unwrap();
    assert_eq!(
        (m.version.as_str(), m.package.as_str()),
        ("0.3.0", name.as_str())
    );
    assert_eq!(m.sha256, sha256_hex(&program));
    assert_eq!(
        m.notes,
        vec!["Ports en trois colonnes", "Mises à jour simplifiées"]
    );
    let bytes = manifest::fetch_package("https://github.com/o/r", &m)
        .await
        .unwrap();
    assert_eq!(sha256_hex(&bytes), m.sha256);

    unsafe { std::env::set_var("STRATA_GITHUB_API", &private) };
    let e = github::latest(&repo, "stable", host_arch())
        .await
        .unwrap_err();
    assert!(e.to_string().contains("404"), "{e}");
    unsafe { std::env::set_var("STRATA_UPDATE_TOKEN", "jeton-lecture") };
    assert_eq!(
        github::latest(&repo, "stable", host_arch())
            .await
            .unwrap()
            .version,
        "0.3.0"
    );

    // Une architecture absente de la Release est signalée clairement.
    let e = github::latest(&repo, "stable", "riscv64")
        .await
        .unwrap_err();
    assert!(e.to_string().contains("ne contient pas encore"), "{e}");
}
