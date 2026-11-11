//! `strata-release` : outil de l'éditeur pour publier une version de Strata Arkyo.
//!
//! 1. `keygen` une seule fois : la clé privée reste chez l'éditeur, la clé publique
//!    est intégrée au moteur à la compilation (`STRATA_RELEASE_PUBKEY`).
//! 2. `pack` : fabrique le paquet signé `.saupd` à partir du programme compilé.
//! 3. `publish` : range le paquet et signe le manifeste du canal dans le dossier
//!    à servir par le serveur de publication (ou à copier sur une clé USB).

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use ed25519_dalek::SigningKey;
use std::path::{Path, PathBuf};
use std::process::Command;
use strata_update::signing::{decode_secret, encode_public, encode_secret, key_id, sha256_hex};
use strata_update::{ReleaseManifest, TrustedKeys, host_arch, package};

#[derive(Parser)]
#[command(
    name = "strata-release",
    version,
    about = "Publication des versions de Strata Arkyo"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Crée une paire de clés de publication.
    Keygen {
        /// Dossier de sortie : release.key (privée) et release.pub (publique).
        #[arg(long)]
        out: PathBuf,
    },
    /// Fabrique un paquet signé à partir du programme `strata` compilé.
    Pack {
        #[arg(long)]
        binary: PathBuf,
        /// Clé privée de publication.
        #[arg(long)]
        key: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Architecture visée (par défaut celle de cette machine).
        #[arg(long)]
        arch: Option<String>,
    },
    /// Range un paquet et signe le manifeste de son canal.
    Publish {
        #[arg(long)]
        package: PathBuf,
        /// stable ou beta.
        #[arg(long, default_value = "stable")]
        channel: String,
        #[arg(long)]
        key: PathBuf,
        /// Dossier servi par le serveur de publication.
        #[arg(long)]
        site: PathBuf,
        /// Une ligne de notes de version ; répéter l'option pour plusieurs lignes.
        #[arg(long = "note")]
        notes: Vec<String>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Cmd::Keygen { out } => keygen(&out),
        Cmd::Pack {
            binary,
            key,
            out,
            arch,
        } => pack(&binary, &key, &out, arch.as_deref()),
        Cmd::Publish {
            package,
            channel,
            key,
            site,
            notes,
        } => publish(&package, &channel, &key, &site, notes),
    }
}

fn read_key(path: &Path) -> Result<SigningKey> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("lecture de {}", path.display()))?;
    decode_secret(&text).context("clé privée illisible")
}

fn keygen(out: &Path) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let secret = out.join("release.key");
    if secret.exists() {
        bail!(
            "{} existe déjà : on ne remplace jamais une clé de publication",
            secret.display()
        );
    }
    let key = SigningKey::generate(&mut rand_core::OsRng);
    std::fs::write(&secret, encode_secret(&key) + "\n")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o600))?;
    }
    let public = encode_public(&key.verifying_key());
    std::fs::write(out.join("release.pub"), format!("{public}\n"))?;
    println!("Clé {} créée.", key_id(&key.verifying_key()));
    println!(
        "Gardez {} hors du dépôt, sur un support sauvegardé.",
        secret.display()
    );
    println!("Compilez le moteur avec : STRATA_RELEASE_PUBKEY={public} cargo deb -p strata");
    Ok(())
}

fn pack(binary: &Path, key: &Path, out: &Path, arch: Option<&str>) -> Result<()> {
    let key = read_key(key)?;
    // Chemin absolu : un nom nu serait cherché dans le PATH, pas dans le dossier courant.
    let binary = &std::fs::canonicalize(binary)
        .with_context(|| format!("programme introuvable : {}", binary.display()))?;
    let output = Command::new(binary)
        .args(["version", "--short"])
        .output()
        .context("lancement du programme")?;
    if !output.status.success() {
        bail!("le programme ne répond pas à « version --short »");
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let bytes = std::fs::read(binary)?;
    let arch = arch.unwrap_or(host_arch());
    let path = package::pack(&bytes, &version, arch, &key, out)?;
    println!(
        "Paquet {} ({} octets, version {version}, {arch}).",
        path.display(),
        std::fs::metadata(&path)?.len()
    );
    Ok(())
}

fn publish(pkg: &Path, channel: &str, key: &Path, site: &Path, notes: Vec<String>) -> Result<()> {
    if !matches!(channel, "stable" | "beta") {
        bail!("canal inconnu : {channel} (stable ou beta)");
    }
    let key = read_key(key)?;
    // Le paquet doit s'ouvrir avec la clé qui va signer le manifeste.
    let verified = package::open(pkg, &TrustedKeys::new(vec![key.verifying_key()]))
        .context("vérification du paquet")?;
    let bytes = std::fs::read(pkg)?;
    let name = pkg
        .file_name()
        .context("nom de paquet")?
        .to_string_lossy()
        .to_string();
    std::fs::create_dir_all(site.join("packages"))?;
    std::fs::create_dir_all(site.join(channel))?;
    std::fs::copy(pkg, site.join("packages").join(&name))?;
    let manifest = ReleaseManifest {
        format: 1,
        channel: channel.into(),
        version: verified.info.version.clone(),
        arch: verified.info.arch.clone(),
        package: format!("packages/{name}"),
        sha256: sha256_hex(&bytes),
        size: bytes.len() as u64,
        published_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        notes,
    };
    let env = manifest.sign(&key)?;
    std::fs::write(
        site.join(channel).join("manifest.json"),
        serde_json::to_vec_pretty(&env)?,
    )?;
    println!(
        "Version {} publiée sur le canal {channel} dans {}.",
        manifest.version,
        site.display()
    );
    Ok(())
}
