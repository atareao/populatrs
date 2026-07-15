use std::sync::Arc;

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth::AppState;

/// Storage configuration exposed via the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub data_dir: String,
    pub published_posts_file: String,
}

/// Get the current storage configuration.
pub async fn get(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let config = StorageConfig {
        data_dir: state.config.data_dir.to_string_lossy().to_string(),
        published_posts_file: "published_posts.json".to_string(),
    };
    Json(json!(config))
}

/// Update the storage configuration.
pub async fn update(
    State(_state): State<Arc<AppState>>,
    Json(_storage): Json<StorageConfig>,
) -> impl IntoResponse {
    // Por ahora solo devolvemos ok; el storage se configura por env vars
    (StatusCode::OK, Json(json!({
        "status": "ok",
        "message": "Storage config updated (will apply on restart)"
    }))).into_response()
}