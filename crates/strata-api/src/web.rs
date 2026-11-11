//! Interface web embarquée dans le binaire (application à page unique).
//!
//! Les fichiers de `web/dist` sont inclus à la compilation. Toute adresse hors de `/api`
//! qui ne désigne pas un fichier reçoit `index.html` : l'interface choisit l'écran.

use axum::body::Body;
use axum::http::{HeaderValue, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use include_dir::{Dir, include_dir};

static WEB: Dir<'static> = include_dir!("$STRATA_WEB_DIR");

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "json" | "webmanifest" => "application/json",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn file(path: &str, bytes: &'static [u8]) -> Response {
    // Les fichiers de `assets/` portent une empreinte dans leur nom : on peut les garder longtemps.
    let cache = if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let mut res = Response::new(Body::from(bytes));
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type(path)),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    if path.ends_with(".html") {
        h.insert(
            header::REFERRER_POLICY,
            HeaderValue::from_static("same-origin"),
        );
        h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    }
    res
}

pub async fn serve(method: Method, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path == "api" || path.starts_with("api/") {
        return crate::error::ApiError::Missing("Cette adresse de l'API n'existe pas.".into())
            .into_response();
    }
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    if !path.is_empty() && !path.contains("..") {
        if let Some(f) = WEB.get_file(path) {
            return file(path, f.contents());
        }
        // Un fichier d'assets absent est une vraie erreur, pas un écran.
        if path.starts_with("assets/") || path.rsplit('/').next().is_some_and(|n| n.contains('.')) {
            return StatusCode::NOT_FOUND.into_response();
        }
    }
    match WEB.get_file("index.html") {
        Some(f) => file("index.html", f.contents()),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
