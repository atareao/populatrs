use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth::AppState;

/// YouTube configuration exposed via the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YouTubeConfig {
    pub api_key: String,
}

/// Get the current YouTube configuration.
pub async fn get(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let config = state.db.get_youtube_config().await.unwrap_or(None);
    let api_key = config.map(|c| c.api_key).unwrap_or_default();
    Json(json!({ "api_key": api_key }))
}

/// Update the YouTube configuration (persisted to DB settings).
pub async fn update(
    State(state): State<Arc<AppState>>,
    Json(config): Json<YouTubeConfig>,
) -> impl IntoResponse {
    match state.db.set_youtube_api_key(&config.api_key).await {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({ "status": "ok", "message": "YouTube config saved" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to save YouTube config: {e}") })),
        )
            .into_response(),
    }
}
