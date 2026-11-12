//! Manifeste d'un canal de publication : `<base>/<canal>/manifest.json`, enveloppe signée.

use crate::signing::{Envelope, TrustedKeys};
use crate::{Result, UpdateError, fetch, github};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseManifest {
    pub format: u32,
    pub channel: String,
    pub version: String,
    pub arch: String,
    /// Chemin du paquet, relatif à l'adresse de base du serveur de publication.
    pub package: String,
    /// Empreinte SHA-256 du fichier paquet, vérifiée avant de l'ouvrir.
    pub sha256: String,
    pub size: u64,
    pub published_at: String,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl ReleaseManifest {
    pub fn sign(&self, key: &SigningKey) -> Result<Envelope> {
        Ok(Envelope::sign(serde_json::to_string_pretty(self)?, key))
    }
}

pub fn manifest_url(base: &str, channel: &str) -> String {
    format!("{}/{channel}/manifest.json", base.trim_end_matches('/'))
}

pub fn package_url(base: &str, m: &ReleaseManifest) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        m.package.trim_start_matches('/')
    )
}

/// Lit et vérifie le manifeste du canal.
pub async fn fetch_manifest(
    base: &str,
    channel: &str,
    keys: &TrustedKeys,
) -> Result<ReleaseManifest> {
    let gh = github::parse(base);
    let bytes = match &gh {
        Some(repo) => github::manifest_bytes(repo, channel).await?,
        None => fetch::get(&manifest_url(base, channel)).await?,
    };
    let env: Envelope = serde_json::from_slice(&bytes)?;
    let m: ReleaseManifest = serde_json::from_str(keys.verify(&env)?)?;
    // Sur GitHub, le canal Bêta suit aussi les versions stables (la plus récente l'emporte).
    let channel_ok = m.channel == channel || (gh.is_some() && channel == "beta");
    if !channel_ok || m.package.contains("..") {
        return Err(UpdateError::InvalidPackage("manifeste incohérent".into()));
    }
    Ok(m)
}

/// Télécharge le paquet annoncé par le manifeste, quelle que soit la source.
pub async fn fetch_package(base: &str, m: &ReleaseManifest) -> Result<Vec<u8>> {
    match github::parse(base) {
        Some(repo) => {
            let file = m.package.rsplit('/').next().unwrap_or(&m.package);
            github::package_bytes(&repo, &m.version, file).await
        }
        None => fetch::get(&package_url(base, m)).await,
    }
}
