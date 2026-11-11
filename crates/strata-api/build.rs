//! Repère l'interface web compilée (`web/dist`). Sans elle (développement du moteur seul,
//! tests), une page d'attente est embarquée à la place.

use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let dist = std::env::var("STRATA_WEB_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest.join("../../web/dist"));
    println!("cargo:rerun-if-env-changed=STRATA_WEB_DIST");
    println!("cargo:rerun-if-changed={}", dist.display());
    let dir = if dist.join("index.html").is_file() {
        dist.canonicalize().unwrap()
    } else {
        let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("web-placeholder");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(
            out.join("index.html"),
            "<!doctype html><html lang=\"fr\"><meta charset=\"utf-8\"><title>Strata Arkyo</title>\
             <body style=\"font:16px/1.5 system-ui;margin:48px\"><h1>Strata Arkyo</h1>\
             <p>Le moteur tourne, mais cette version a été compilée sans l'interface web. \
             Compilez <code>web/</code> (<code>npm run build</code>) puis le moteur. L'API répond sous <code>/api</code>.</p></body></html>",
        )
        .unwrap();
        println!("cargo:warning=web/dist absent : interface web remplacée par une page d'attente");
        out
    };
    println!("cargo:rustc-env=STRATA_WEB_DIR={}", dir.display());
}
