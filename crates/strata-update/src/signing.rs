//! Enveloppes signées : un texte JSON et sa signature Ed25519.
//!
//! On signe les octets exacts du texte : aucune normalisation JSON n'entre en jeu.

use crate::{Result, UpdateError};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Envelope {
    /// Texte JSON signé, conservé tel quel.
    pub payload: String,
    /// Signature Ed25519 des octets de `payload`, en base64.
    pub signature: String,
    /// Empreinte de la clé publique qui a signé.
    pub key_id: String,
}

impl Envelope {
    pub fn sign(payload: String, key: &SigningKey) -> Envelope {
        let signature = B64.encode(key.sign(payload.as_bytes()).to_bytes());
        Envelope {
            payload,
            signature,
            key_id: key_id(&key.verifying_key()),
        }
    }
}

/// Empreinte courte d'une clé publique : 16 caractères hexadécimaux.
pub fn key_id(key: &VerifyingKey) -> String {
    hex::encode(&Sha256::digest(key.as_bytes())[..8])
}

pub fn encode_public(key: &VerifyingKey) -> String {
    B64.encode(key.as_bytes())
}

pub fn encode_secret(key: &SigningKey) -> String {
    B64.encode(key.to_bytes())
}

pub fn decode_public(text: &str) -> Option<VerifyingKey> {
    let bytes: [u8; 32] = B64.decode(text.trim()).ok()?.try_into().ok()?;
    VerifyingKey::from_bytes(&bytes).ok()
}

pub fn decode_secret(text: &str) -> Option<SigningKey> {
    let bytes: [u8; 32] = B64.decode(text.trim()).ok()?.try_into().ok()?;
    Some(SigningKey::from_bytes(&bytes))
}

/// Clés de publication auxquelles le moteur fait confiance.
#[derive(Debug, Clone, Default)]
pub struct TrustedKeys(Vec<VerifyingKey>);

impl TrustedKeys {
    pub fn new(keys: Vec<VerifyingKey>) -> TrustedKeys {
        TrustedKeys(keys)
    }

    /// Clés intégrées au programme à la compilation par la variable
    /// `STRATA_RELEASE_PUBKEY` (une ou plusieurs clés base64, séparées par des virgules).
    /// Sans elle, le programme refuse toute mise à jour signée.
    pub fn embedded() -> TrustedKeys {
        let keys = option_env!("STRATA_RELEASE_PUBKEY")
            .unwrap_or("")
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .filter_map(decode_public)
            .collect();
        TrustedKeys(keys)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn ids(&self) -> Vec<String> {
        self.0.iter().map(key_id).collect()
    }

    /// Vérifie l'enveloppe et renvoie le texte signé.
    pub fn verify<'a>(&self, env: &'a Envelope) -> Result<&'a str> {
        if self.0.is_empty() {
            return Err(UpdateError::NoTrustedKey);
        }
        let bytes: [u8; 64] = B64
            .decode(env.signature.trim())
            .ok()
            .and_then(|b| b.try_into().ok())
            .ok_or(UpdateError::BadSignature)?;
        let sig = Signature::from_bytes(&bytes);
        if self
            .0
            .iter()
            .any(|k| k.verify(env.payload.as_bytes(), &sig).is_ok())
        {
            Ok(&env.payload)
        } else {
            Err(UpdateError::BadSignature)
        }
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
