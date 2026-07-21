use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth::AppState;

/// Storage configuration exposed via the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub data_dir: String,
}

/// Get the current storage configuration.
pub async fn get(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let data_dir = state
        .db
        .get_setting("storage_data_dir")
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| state.config.data_dir.to_string_lossy().to_string());
    let config = StorageConfig { data_dir };
    Json(json!(config))
}

/// Update the storage configuration (persisted to DB settings).
pub async fn update(
    State(state): State<Arc<AppState>>,
    Json(storage): Json<StorageConfig>,
) -> impl IntoResponse {
    match state
        .db
        .set_setting("storage_data_dir", &storage.data_dir)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "status": "ok",
                "message": "Storage config updated. Server restart required to apply changes."
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": format!("Failed to save data_dir: {e}")
            })),
        )
            .into_response(),
    }
}
