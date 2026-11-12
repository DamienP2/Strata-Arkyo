//! Mises à jour de Strata Arkyo (architecture, « Installation Ubuntu et mises à jour OTA »).
//!
//! - Un **paquet** (`.saupd`) contient le programme et sa description signée en Ed25519.
//! - Un **manifeste** de canal (Stable, Bêta) annonce la dernière version et l'empreinte de son paquet.
//! - Chaque version s'installe dans `versions/<version>/` ; le lien `current` désigne celle qui tourne.
//! - Après une bascule, un compteur de démarrages ramène l'ancienne version au troisième échec.

pub mod fetch;
pub mod github;
pub mod layout;
pub mod manifest;
pub mod package;
pub mod signing;
pub mod version;

pub use layout::{BootDecision, InstalledVersion, Layout, UpdateState};
pub use manifest::ReleaseManifest;
pub use package::{PackageInfo, VerifiedPackage};
pub use signing::{Envelope, TrustedKeys};

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("signature invalide : ce fichier n'a pas été publié par l'éditeur de Strata Arkyo")]
    BadSignature,
    #[error(
        "aucune clé de publication dans cette version : les mises à jour signées sont désactivées"
    )]
    NoTrustedKey,
    #[error("empreinte SHA-256 différente de celle annoncée : fichier corrompu ou modifié")]
    HashMismatch,
    #[error("paquet invalide : {0}")]
    InvalidPackage(String),
    #[error("ce paquet vise l'architecture {found}, ce serveur est en {expected}")]
    WrongArch { found: String, expected: String },
    #[error("la version {0} est introuvable parmi les versions installées")]
    UnknownVersion(String),
    #[error("aucune version précédente à laquelle revenir")]
    NoPrevious,
    #[error("l'essai à blanc de la version {version} a échoué : {detail}")]
    SelftestFailed { version: String, detail: String },
    #[error("téléchargement impossible : {0}")]
    Fetch(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, UpdateError>;

/// Architecture de ce serveur, au format des paquets.
pub fn host_arch() -> &'static str {
    std::env::consts::ARCH
}
