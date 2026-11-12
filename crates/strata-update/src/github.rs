//! Source de mise à jour GitHub : les versions sont les « Releases » du dépôt.
//!
//! `STRATA_UPDATE_URL=github:propriétaire/dépôt` (c'est la source par défaut). Chaque Release
//! `vX.Y.Z`, déposée par le workflow de publication, porte :
//! - `strata-X.Y.Z-<arch>` : le programme ;
//! - `SHA256SUMS` : l'empreinte de chaque fichier, vérifiée après téléchargement ;
//! - le paquet Debian, pour une première installation.
//!
//! Le canal Stable lit la dernière Release publiée ; le canal Bêta lit aussi les préversions.
//! Dépôt public : rien à configurer. Dépôt privé : `STRATA_UPDATE_TOKEN` (lecture seule).

use crate::{ReleaseManifest, Result, UpdateError, fetch};
use serde_json::Value;

/// Adresse de l'API, remplaçable pour les essais.
fn api_base() -> String {
    std::env::var("STRATA_GITHUB_API")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "https://api.github.com".into())
        .trim_end_matches('/')
        .to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub owner: String,
    pub name: String,
}

/// Reconnaît `github:proprio/depot` et `https://github.com/proprio/depot`.
pub fn parse(base: &str) -> Option<Repo> {
    let b = base.trim().trim_end_matches('/');
    let rest = b
        .strip_prefix("github:")
        .or_else(|| b.strip_prefix("https://github.com/"))?;
    let rest = rest.trim_end_matches(".git");
    let (owner, name) = rest.split_once('/')?;
    let ok = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    (ok(owner) && ok(name) && !name.contains('/')).then(|| Repo {
        owner: owner.into(),
        name: name.into(),
    })
}

/// Nom du programme dans une Release.
pub fn binary_name(version: &str, arch: &str) -> String {
    format!("strata-{version}-{arch}")
}

fn headers(accept: &str) -> Vec<(&'static str, String)> {
    let mut h = vec![
        ("Accept", accept.to_string()),
        ("X-GitHub-Api-Version", "2022-11-28".to_string()),
    ];
    if let Ok(t) = std::env::var("STRATA_UPDATE_TOKEN") {
        let t = t.trim();
        if !t.is_empty() {
            h.push(("Authorization", format!("Bearer {t}")));
        }
    }
    h
}

async fn json(url: &str) -> Result<Value> {
    let bytes = fetch::get_with(url, &headers("application/vnd.github+json")).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Release à lire pour un canal : la dernière publiée (Stable), ou la plus récente,
/// préversions comprises (Bêta). Les brouillons sont ignorés.
async fn release_for(repo: &Repo, channel: &str) -> Result<Value> {
    let base = format!("{}/repos/{}/{}", api_base(), repo.owner, repo.name);
    if channel == "beta" {
        let list = json(&format!("{base}/releases?per_page=20")).await?;
        list.as_array()
            .and_then(|l| l.iter().find(|r| r["draft"] != Value::Bool(true)).cloned())
            .ok_or_else(|| UpdateError::Fetch("aucune version publiée sur GitHub".into()))
    } else {
        json(&format!("{base}/releases/latest")).await
    }
}

fn asset<'a>(release: &'a Value, name: &str) -> Option<&'a Value> {
    release["assets"]
        .as_array()?
        .iter()
        .find(|x| x["name"].as_str() == Some(name))
}

async fn download(release: &Value, name: &str) -> Result<Vec<u8>> {
    let url = asset(release, name)
        .and_then(|x| x["url"].as_str())
        .ok_or_else(|| missing(release, name))?;
    fetch::get_with(url, &headers("application/octet-stream")).await
}

fn missing(release: &Value, name: &str) -> UpdateError {
    UpdateError::Fetch(format!(
        "la version {} ne contient pas encore le fichier {name} (publication en cours ?)",
        release["tag_name"].as_str().unwrap_or("?")
    ))
}

/// Empreinte d'un fichier dans un `SHA256SUMS` (« <empreinte>  <nom> »).
pub fn sum_for(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let mut it = l.split_whitespace();
        let (h, n) = (it.next()?, it.next()?);
        (n.trim_start_matches('*') == name && h.len() == 64).then(|| h.to_ascii_lowercase())
    })
}

/// Dernière version du canal, décrite comme un manifeste (non signé : l'empreinte vient de
/// `SHA256SUMS`, lu par HTTPS sur GitHub).
pub async fn latest(repo: &Repo, channel: &str, arch: &str) -> Result<ReleaseManifest> {
    let r = release_for(repo, channel).await?;
    let tag = r["tag_name"].as_str().unwrap_or_default();
    let version = tag.trim_start_matches('v').to_string();
    if !crate::version::is_valid(&version) {
        return Err(UpdateError::Fetch(format!(
            "étiquette de version illisible : {tag}"
        )));
    }
    let name = binary_name(&version, arch);
    let size = asset(&r, &name)
        .and_then(|a| a["size"].as_u64())
        .ok_or_else(|| missing(&r, &name))?;
    let sums = download(&r, "SHA256SUMS").await?;
    let sha256 = sum_for(&String::from_utf8_lossy(&sums), &name)
        .ok_or_else(|| UpdateError::Fetch(format!("empreinte de {name} absente de SHA256SUMS")))?;
    let notes = r["body"]
        .as_str()
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            l.trim()
                .strip_prefix("* ")
                .or_else(|| l.trim().strip_prefix("- "))
        })
        .take(12)
        .map(str::to_string)
        .collect();
    Ok(ReleaseManifest {
        format: 1,
        channel: if r["prerelease"] == Value::Bool(true) {
            "beta"
        } else {
            "stable"
        }
        .into(),
        version,
        arch: arch.into(),
        package: name,
        sha256,
        size,
        published_at: r["published_at"].as_str().unwrap_or_default().into(),
        notes,
    })
}

/// Programme annoncé par `latest`, cherché dans la Release `v<version>`.
pub async fn binary(repo: &Repo, m: &ReleaseManifest) -> Result<Vec<u8>> {
    let url = format!(
        "{}/repos/{}/{}/releases/tags/v{}",
        api_base(),
        repo.owner,
        repo.name,
        m.version
    );
    let r = json(&url).await?;
    download(&r, &m.package).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repositories() {
        let r = Repo {
            owner: "DamienP2".into(),
            name: "Strata-Arkyo".into(),
        };
        assert_eq!(parse("github:DamienP2/Strata-Arkyo"), Some(r.clone()));
        assert_eq!(
            parse("https://github.com/DamienP2/Strata-Arkyo/"),
            Some(r.clone())
        );
        assert_eq!(
            parse("https://github.com/DamienP2/Strata-Arkyo.git"),
            Some(r)
        );
        assert_eq!(parse("https://updates.exemple.fr/strata"), None);
        assert_eq!(parse("github:seul"), None);
        assert_eq!(parse("github:a/b/c"), None);
    }

    #[test]
    fn reads_checksums() {
        let h = "a".repeat(64);
        let sums = format!("{h}  strata-0.3.0-x86_64\n{}  autre.deb\n", "b".repeat(64));
        assert_eq!(sum_for(&sums, "strata-0.3.0-x86_64"), Some(h));
        assert_eq!(sum_for(&sums, "absent"), None);
    }
}
