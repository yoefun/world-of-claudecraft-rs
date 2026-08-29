//! REST auth: register / login → bearer session token.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use woc_persist::{Persist, PersistError};

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub account_id: Uuid,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/register", post(register))
        .route("/api/login", post(login))
}

async fn register(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AuthRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let (account_id, token) = state
        .persist
        .register(&body.username, &body.password)
        .await?;
    Ok(Json(AuthResponse { token, account_id }))
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AuthRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let (account_id, token) = state.persist.login(&body.username, &body.password).await?;
    Ok(Json(AuthResponse { token, account_id }))
}

/// Resolve `Authorization: Bearer <token>` to an account id.
pub async fn account_from_headers(
    persist: &Persist,
    headers: &HeaderMap,
) -> Result<Uuid, ApiError> {
    let auth = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError(PersistError::Unauthorized))?;
    let token = auth
        .strip_prefix("Bearer ")
        .or_else(|| auth.strip_prefix("bearer "))
        .ok_or(ApiError(PersistError::Unauthorized))?;
    Ok(persist.account_id_for_token(token.trim()).await?)
}

pub struct ApiError(pub PersistError);

impl From<PersistError> for ApiError {
    fn from(value: PersistError) -> Self {
        Self(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            PersistError::UsernameTaken | PersistError::CharacterNameTaken => StatusCode::CONFLICT,
            PersistError::InvalidCredentials | PersistError::Unauthorized => {
                StatusCode::UNAUTHORIZED
            }
            PersistError::Forbidden => StatusCode::FORBIDDEN,
            PersistError::CharacterNotFound => StatusCode::NOT_FOUND,
            PersistError::InvalidInput(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = serde_json::json!({ "error": self.0.to_string() });
        (status, Json(body)).into_response()
    }
}
