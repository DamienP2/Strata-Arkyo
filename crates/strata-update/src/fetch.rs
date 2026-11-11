//! Lecture d'une ressource par `https://`, `http://` ou `file://` (miroir local ou clé USB).

use crate::{Result, UpdateError};

const MAX: usize = 600 * 1024 * 1024;

pub async fn get(url: &str) -> Result<Vec<u8>> {
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
    let res = client
        .get(url)
        .send()
        .await
        .map_err(|e| UpdateError::Fetch(format!("{url} : {e}")))?;
    if !res.status().is_success() {
        return Err(UpdateError::Fetch(format!(
            "{url} : réponse {}",
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
