//! Lecture d'une ressource par `https://`, `http://` ou `file://` (miroir local ou clé USB).

use crate::{Result, UpdateError};

const MAX: usize = 600 * 1024 * 1024;

pub async fn get(url: &str) -> Result<Vec<u8>> {
    get_with(url, &[]).await
}

/// Comme `get`, avec des en-têtes (jeton d'accès, type attendu). En cas de redirection vers
/// un autre hôte (stockage des fichiers GitHub), le jeton n'est pas retransmis.
pub async fn get_with(url: &str, headers: &[(&str, String)]) -> Result<Vec<u8>> {
    if let Some(path) = url.strip_prefix("file://") {
        return tokio::fs::read(path)
            .await
            .map_err(|e| UpdateError::Fetch(format!("{url} : {e}")));
    }
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(UpdateError::Fetch(format!(
            "adresse non prise en charge : {url}"
        )));
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .user_agent(concat!("strata-arkyo/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| UpdateError::Fetch(e.to_string()))?;
    let mut req = client.get(url);
    for (k, v) in headers {
        req = req.header(*k, v);
    }
    let res = req
        .send()
        .await
        .map_err(|e| UpdateError::Fetch(format!("{url} : {e}")))?;
    if !res.status().is_success() {
        let hint = match res.status().as_u16() {
            401 => " (jeton refusé : vérifiez STRATA_UPDATE_TOKEN)",
            403 => " (accès refusé ou limite de requêtes atteinte)",
            404 => " (introuvable : dépôt privé sans jeton, ou aucune version publiée)",
            _ => "",
        };
        return Err(UpdateError::Fetch(format!(
            "{url} : réponse {}{hint}",
            res.status()
        )));
    }
    let bytes = res
        .bytes()
        .await
        .map_err(|e| UpdateError::Fetch(e.to_string()))?;
    if bytes.len() > MAX {
        return Err(UpdateError::Fetch("fichier trop volumineux".into()));
    }
    Ok(bytes.to_vec())
}
