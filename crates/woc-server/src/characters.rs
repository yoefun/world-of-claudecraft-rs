//! Character list / create / enter-world REST API.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use woc_persist::{Character, CharacterSummary};

use crate::auth::{account_from_headers, ApiError};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateCharacterRequest {
    pub name: String,
    pub class_id: String,
}

#[derive(Debug, Serialize)]
pub struct CharacterListResponse {
    pub characters: Vec<CharacterSummary>,
}

#[derive(Debug, Serialize)]
pub struct EnterResponse {
    pub character: Character,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/characters",
            get(list_characters).post(create_character),
        )
        .route("/api/characters/{id}/enter", post(enter_character))
        .route("/api/characters/{id}", delete(delete_character))
}

async fn list_characters(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<CharacterListResponse>, ApiError> {
    let account_id = account_from_headers(&state.persist, &headers).await?;
    let chars = state.persist.list_characters(account_id).await?;
    Ok(Json(CharacterListResponse {
        characters: chars.iter().map(CharacterSummary::from).collect(),
    }))
}

async fn create_character(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CreateCharacterRequest>,
) -> Result<Json<Character>, ApiError> {
    let account_id = account_from_headers(&state.persist, &headers).await?;
    let character = state
        .persist
        .create_character(account_id, &body.name, &body.class_id)
        .await?;
    Ok(Json(character))
}

async fn enter_character(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<EnterResponse>, ApiError> {
    let account_id = account_from_headers(&state.persist, &headers).await?;
    let character = state.persist.enter_character(account_id, id).await?;
    Ok(Json(EnterResponse { character }))
}

async fn delete_character(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let account_id = account_from_headers(&state.persist, &headers).await?;
    state.persist.delete_character(account_id, id).await?;
    crate::game_ws::on_character_deleted(id, &state.persist).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}
