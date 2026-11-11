//! Mots de passe, jetons de session et extracteurs d'identité.

use crate::AppState;
use crate::error::ApiError;
use argon2::password_hash::{PasswordHash, SaltString};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;
use strata_domain::{Actor, Domain, Level, Permissions, RoleInfo};
use strata_store::Account;

pub const COOKIE: &str = "strata_session";
pub const SESSION_DAYS: i64 = 30;

/// Empreinte Argon2id d'un mot de passe.
pub fn hash_password(password: &str) -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    let salt = SaltString::encode_b64(&bytes).expect("sel Argon2");
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("hachage Argon2id")
        .to_string()
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|h| {
            Argon2::default()
                .verify_password(password.as_bytes(), &h)
                .is_ok()
        })
        .unwrap_or(false)
}

/// Empreinte de référence : un identifiant inconnu coûte le même temps qu'un mauvais mot de passe.
static DUMMY_HASH: LazyLock<String> = LazyLock::new(|| hash_password("strata-arkyo-dummy"));

pub fn burn_verification(password: &str) {
    let _ = verify_password(password, &DUMMY_HASH);
}

/// Nouveau jeton de session (256 bits) et son empreinte, seule stockée en base.
pub fn new_token() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let token = hex::encode(bytes);
    let hash = hash_token(&token);
    (token, hash)
}

pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub fn session_cookie(token: String, secure: bool) -> Cookie<'static> {
    Cookie::build((COOKIE, token))
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(secure)
        .path("/")
        .max_age(time::Duration::days(SESSION_DAYS))
        .build()
}

pub fn removal_cookie() -> Cookie<'static> {
    Cookie::build((COOKIE, ""))
        .path("/")
        .max_age(time::Duration::ZERO)
        .build()
}

/// Personne connectée, même si elle doit encore changer son mot de passe temporaire.
pub struct Session {
    pub account: Account,
    pub actor: Actor,
    pub token_hash: String,
}

impl Session {
    pub fn require(&self, domain: Domain, level: Level, message: &str) -> Result<(), ApiError> {
        if self.actor.allows(domain, level) {
            Ok(())
        } else {
            Err(ApiError::Forbidden(message.into()))
        }
    }

    /// Permissions effectives : le propriétaire a tout.
    pub fn effective_permissions(&self) -> Permissions {
        if self.actor.is_owner() {
            Permissions::all(Level::Admin)
        } else {
            self.actor.role.perms
        }
    }
}

impl FromRequestParts<AppState> for Session {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get(COOKIE)
            .map(|c| c.value().to_string())
            .ok_or(ApiError::Unauthenticated)?;
        let token_hash = hash_token(&token);
        let account = state
            .store
            .session_account(&token_hash)
            .await?
            .ok_or(ApiError::Unauthenticated)?;
        if !account.active || account.removed {
            return Err(ApiError::Unauthenticated);
        }
        let role = state
            .store
            .role(&account.role_id)
            .await?
            .ok_or(ApiError::Unauthenticated)?;
        state.store.touch_session(&token_hash).await?;
        let actor = Actor {
            account_id: account.id.clone(),
            role: RoleInfo {
                id: role.id,
                perms: role.permissions,
            },
        };
        Ok(Session {
            account,
            actor,
            token_hash,
        })
    }
}

/// Personne connectée qui a déjà remplacé son mot de passe temporaire.
pub struct Member(pub Session);

impl FromRequestParts<AppState> for Member {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let s = Session::from_request_parts(parts, state).await?;
        if s.account.must_change_password {
            return Err(ApiError::PasswordChangeRequired);
        }
        Ok(Member(s))
    }
}
