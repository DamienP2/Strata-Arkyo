//! Paquet de mise à jour `.saupd` : archive tar compressée zstd contenant
//! `package.json` (description), `package.sig` (enveloppe signée de la description)
//! et `strata` (le programme). Le même paquet sert en ligne et hors ligne.

use crate::signing::{Envelope, TrustedKeys, sha256_hex};
use crate::{Result, UpdateError, host_arch, version};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const FORMAT: u32 = 1;
pub const PRODUCT: &str = "strata-arkyo";
const MAX_BINARY: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageInfo {
    pub format: u32,
    pub product: String,
    pub version: String,
    pub arch: String,
    /// Empreinte SHA-256 du programme `strata` contenu dans le paquet.
    pub sha256: String,
    pub size: u64,
    pub created_at: String,
}

/// Paquet dont la signature et l'empreinte ont été vérifiées.
#[derive(Debug, Clone)]
pub struct VerifiedPackage {
    pub info: PackageInfo,
    pub binary: Vec<u8>,
}

pub fn file_name(version: &str, arch: &str) -> String {
    format!("{PRODUCT}_{version}_{arch}.saupd")
}

/// Construit un paquet signé (outil de publication).
pub fn pack(
    binary: &[u8],
    version: &str,
    arch: &str,
    key: &SigningKey,
    out_dir: &Path,
) -> Result<PathBuf> {
    if !version::is_valid(version) {
        return Err(UpdateError::InvalidPackage(format!(
            "numéro de version invalide : {version}"
        )));
    }
    let info = PackageInfo {
        format: FORMAT,
        product: PRODUCT.into(),
        version: version.into(),
        arch: arch.into(),
        sha256: sha256_hex(binary),
        size: binary.len() as u64,
        created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    };
    let payload = serde_json::to_string_pretty(&info)?;
    let envelope = serde_json::to_string_pretty(&Envelope::sign(payload.clone(), key))?;

    std::fs::create_dir_all(out_dir)?;
    let path = out_dir.join(file_name(version, arch));
    let file = std::fs::File::create(&path)?;
    let encoder = zstd::Encoder::new(file, 9)?;
    let mut tar = tar::Builder::new(encoder);
    append(&mut tar, "package.json", payload.as_bytes(), 0o644)?;
    append(&mut tar, "package.sig", envelope.as_bytes(), 0o644)?;
    append(&mut tar, "strata", binary, 0o755)?;
    tar.into_inner()?.finish()?;
    Ok(path)
}

fn append<W: std::io::Write>(
    tar: &mut tar::Builder<W>,
    name: &str,
    data: &[u8],
    mode: u32,
) -> Result<()> {
    let mut h = tar::Header::new_gnu();
    h.set_size(data.len() as u64);
    h.set_mode(mode);
    h.set_mtime(0);
    h.set_cksum();
    tar.append_data(&mut h, name, data)?;
    Ok(())
}

/// Ouvre un paquet et vérifie, dans l'ordre : la signature de la description,
/// le produit, l'architecture, puis l'empreinte du programme.
pub fn open(path: &Path, keys: &TrustedKeys) -> Result<VerifiedPackage> {
    let file = std::fs::File::open(path)?;
    let decoder =
        zstd::Decoder::new(file).map_err(|e| UpdateError::InvalidPackage(e.to_string()))?;
    let mut archive = tar::Archive::new(decoder);
    let (mut info_text, mut sig_text, mut binary) = (None, None, None);
    let entries = archive
        .entries()
        .map_err(|e| UpdateError::InvalidPackage(e.to_string()))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| UpdateError::InvalidPackage(e.to_string()))?;
        let name = entry.path()?.to_string_lossy().to_string();
        if entry.size() > MAX_BINARY {
            return Err(UpdateError::InvalidPackage(format!(
                "{name} trop volumineux"
            )));
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        match name.as_str() {
            "package.json" => info_text = Some(String::from_utf8_lossy(&buf).to_string()),
            "package.sig" => sig_text = Some(buf),
            "strata" => binary = Some(buf),
            _ => {
                return Err(UpdateError::InvalidPackage(format!(
                    "entrée inattendue : {name}"
                )));
            }
        }
    }
    let (Some(info_text), Some(sig_text), Some(binary)) = (info_text, sig_text, binary) else {
        return Err(UpdateError::InvalidPackage(
            "description, signature ou programme manquant".into(),
        ));
    };
    let envelope: Envelope = serde_json::from_slice(&sig_text)?;
    let signed = keys.verify(&envelope)?;
    // La description lue doit être exactement celle qui a été signée.
    if signed != info_text {
        return Err(UpdateError::BadSignature);
    }
    let info: PackageInfo = serde_json::from_str(signed)?;
    if info.product != PRODUCT || info.format != FORMAT || !version::is_valid(&info.version) {
        return Err(UpdateError::InvalidPackage(
            "produit, format ou version inconnus".into(),
        ));
    }
    if info.arch != host_arch() {
        return Err(UpdateError::WrongArch {
            found: info.arch,
            expected: host_arch().into(),
        });
    }
    if sha256_hex(&binary) != info.sha256 || binary.len() as u64 != info.size {
        return Err(UpdateError::HashMismatch);
    }
    Ok(VerifiedPackage { info, binary })
}
