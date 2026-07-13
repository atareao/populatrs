use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;

use crate::auth::AppState;
use crate::models::PublisherConfig;

/// List all publishers.
pub async fn list(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    match state.db.list_publishers().await {
        Ok(publishers) => Json(json!({
            "publishers": publishers,
            "total": publishers.len()
        })),
        Err(e) => Json(json!({
            "error": format!("Failed to list publishers: {e}"),
            "publishers": {},
            "total": 0
        })),
    }
}

#[derive(Deserialize)]
pub struct UpdatePublisherPayload {
    pub config: PublisherConfig,
}

/// Update a publisher by ID.
pub async fn update_by_id(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdatePublisherPayload>,
) -> impl IntoResponse {
    match state.db.get_publisher(&id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": format!("Publisher '{}' not found", id)})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {e}")})),
            )
                .into_response();
        }
    }

    match state.db.upsert_publisher(&id, &payload.config).await {
        Ok(()) => Json(json!({"status": "ok", "message": "Publisher updated"})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to update publisher: {e}")})),
        )
            .into_response(),
    }
}