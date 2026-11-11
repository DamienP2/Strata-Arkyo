//! Erreurs de l'API : un code stable pour le programme, un message en français pour la personne.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use strata_domain::{Refusal, ValidationError};
use strata_store::StoreError;

#[derive(Debug)]
pub enum ApiError {
    Unauthenticated,
    LoginFailed,
    PasswordChangeRequired,
    Forbidden(String),
    NotFound(&'static str),
    Conflict(String),
    Invalid(String),
    Csrf,
    Internal(String),
}

impl ApiError {
    fn parts(&self) -> (StatusCode, &'static str, String) {
        match self {
            ApiError::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "Connectez-vous pour continuer.".into(),
            ),
            ApiError::LoginFailed => (
                StatusCode::UNAUTHORIZED,
                "login_failed",
                "Identifiant ou mot de passe incorrect.".into(),
            ),
            ApiError::PasswordChangeRequired => (
                StatusCode::FORBIDDEN,
                "password_change_required",
                "Changez votre mot de passe temporaire pour continuer.".into(),
            ),
            ApiError::Forbidden(m) => (StatusCode::FORBIDDEN, "forbidden", m.clone()),
            ApiError::NotFound(what) => (
                StatusCode::NOT_FOUND,
                "not_found",
                format!("{what} introuvable."),
            ),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, "conflict", m.clone()),
            ApiError::Invalid(m) => (StatusCode::UNPROCESSABLE_ENTITY, "invalid", m.clone()),
            ApiError::Csrf => (
                StatusCode::FORBIDDEN,
                "csrf",
                "Requête refusée : l'en-tête X-Strata-Request manque.".into(),
            ),
            ApiError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "Erreur interne du serveur. Réessayez, ou consultez le journal du service.".into(),
            ),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if let ApiError::Internal(detail) = &self {
            tracing::error!(%detail, "erreur interne");
        }
        let (status, code, message) = self.parts();
        (status, Json(json!({ "code": code, "message": message }))).into_response()
    }
}

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::Conflict("email") => {
                ApiError::Conflict("Un compte utilise déjà cet e-mail.".into())
            }
            StoreError::Conflict("username") => {
                ApiError::Conflict("Cet identifiant est déjà pris.".into())
            }
            StoreError::Conflict("role_name") => {
                ApiError::Conflict("Un rôle porte déjà ce nom.".into())
            }
            StoreError::Conflict(_) => ApiError::Conflict("Cette valeur est déjà utilisée.".into()),
            StoreError::NotFound => ApiError::NotFound("Élément"),
            other => ApiError::Internal(other.to_string()),
        }
    }
}

impl From<Refusal> for ApiError {
    fn from(r: Refusal) -> Self {
        ApiError::Forbidden(r.to_string())
    }
}

impl From<ValidationError> for ApiError {
    fn from(v: ValidationError) -> Self {
        ApiError::Invalid(v.to_string())
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
