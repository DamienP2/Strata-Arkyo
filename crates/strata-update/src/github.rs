//! Source de mise à jour GitHub : les versions sont les « Releases » d'un dépôt.
//!
//! `STRATA_UPDATE_URL=github:propriétaire/dépôt`. Chaque Release porte deux fichiers,
//! déposés par le workflow de publication : `manifest.json` (manifeste signé) et le paquet
//! `.saupd`. Le canal Stable lit la dernière Release publiée ; le canal Bêta lit aussi
//! les préversions. Pour un dépôt privé, `STRATA_UPDATE_TOKEN` porte un jeton en lecture
//! seule (contenu du dépôt). La signature et l'empreinte restent vérifiées comme ailleurs :
//! GitHub n'est qu'un transport.

use crate::{Result, UpdateError, fetch};
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

/// Télécharge un fichier joint à une Release, par son nom.
async fn asset(release: &Value, name: &str) -> Result<Vec<u8>> {
    let url = release["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|x| x["name"].as_str() == Some(name)))
        .and_then(|x| x["url"].as_str())
        .ok_or_else(|| {
            UpdateError::Fetch(format!(
                "la version {} ne contient pas le fichier {name}",
                release["tag_name"].as_str().unwrap_or("?")
            ))
        })?;
    fetch::get_with(url, &headers("application/octet-stream")).await
}

/// Manifeste signé de la Release du canal.
pub async fn manifest_bytes(repo: &Repo, channel: &str) -> Result<Vec<u8>> {
    let r = release_for(repo, channel).await?;
    asset(&r, "manifest.json").await
}

/// Paquet annoncé par un manifeste : cherché dans la Release `v<version>`.
pub async fn package_bytes(repo: &Repo, version: &str, file: &str) -> Result<Vec<u8>> {
    let url = format!(
        "{}/repos/{}/{}/releases/tags/v{version}",
        api_base(),
        repo.owner,
        repo.name
    );
    let r = json(&url).await?;
    asset(&r, file).await
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
}
