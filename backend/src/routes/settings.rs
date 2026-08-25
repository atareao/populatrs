use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::auth::AppState;

/// Publish settings returned by GET /api/settings/publish
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishSettings {
    /// Max posts per cycle (0 = publish all)
    pub max_posts: u64,
    /// Minimum date threshold (RFC3339 or YYYY-MM-DD, empty = no filter)
    pub min_date: String,
}

/// GET /api/settings/publish
pub async fn get_publish_settings(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let max_posts = state.db.get_max_posts().await.unwrap_or(1);
    let min_date = state
        .db
        .get_setting("min_date")
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    Json(json!({
        "max_posts": max_posts,
        "min_date": min_date,
    }))
    .into_response()
}

/// PUT /api/settings/publish
pub async fn update_publish_settings(
    State(state): State<Arc<AppState>>,
    Json(settings): Json<PublishSettings>,
) -> impl IntoResponse {
    // Validate max_posts
    if settings.max_posts > 100 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "max_posts must be between 0 and 100"})),
        )
            .into_response();
    }

    // Save max_posts
    if let Err(e) = state.db.set_max_posts(settings.max_posts).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to save max_posts: {e}")})),
        )
            .into_response();
    }

    // Save min_date (validated in db.set_min_date)
    if let Err(e) = state.db.set_min_date(&settings.min_date).await {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("{e}")})),
        )
            .into_response();
    }

    Json(json!({"status": "ok", "message": "Publish settings saved"}))
        .into_response()
}