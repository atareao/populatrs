use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use tracing::instrument;

use crate::auth::AppState;
use crate::models::FeedConfig;

/// List all feeds.
pub async fn list(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    match state.db.list_feeds().await {
        Ok(feeds) => Json(json!({
            "feeds": feeds,
            "total": feeds.len()
        })),
        Err(e) => Json(json!({
            "error": format!("Failed to list feeds: {e}"),
            "feeds": [],
            "total": 0
        })),
    }
}

/// Create a new feed.
pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(feed): Json<FeedConfig>,
) -> impl IntoResponse {
    // Check for duplicate ID
    match state.db.get_feed(&feed.id).await {
        Ok(Some(_)) => {
            return (
                StatusCode::CONFLICT,
                Json(json!({"error": format!("Feed '{}' already exists", feed.id)})),
            )
                .into_response();
        }
        Ok(None) => {}
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {e}")})),
            )
                .into_response();
        }
    }

    match state.db.create_feed(&feed).await {
        Ok(()) => (
            StatusCode::CREATED,
            Json(json!({"status": "ok", "message": "Feed created"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create feed: {e}")})),
        )
            .into_response(),
    }
}

/// Update an existing feed.
pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(feed): Json<FeedConfig>,
) -> impl IntoResponse {
    match state.db.update_feed(&id, &feed).await {
        Ok(true) => Json(json!({"status": "ok", "message": "Feed updated"})).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Feed not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to update feed: {e}")})),
        )
            .into_response(),
    }
}

/// Delete a feed.
pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.delete_feed(&id).await {
        Ok(true) => Json(json!({"status": "ok", "message": "Feed deleted"})).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Feed not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to delete feed: {e}")})),
        )
            .into_response(),
    }
}

/// Toggle feed enabled/disabled.
pub async fn toggle(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.toggle_feed(&id).await {
        Ok(Some(enabled)) => Json(json!({
            "status": "ok",
            "enabled": enabled,
            "message": if enabled { "Feed enabled" } else { "Feed disabled" }
        }))
        .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Feed not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to toggle feed: {e}")})),
        )
            .into_response(),
    }
}

/// Run a feed manually: fetch posts and return them.
#[instrument(skip(state))]
pub async fn run(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> impl IntoResponse {
    let feed_config = match state.db.get_feed(&id).await {
        Ok(Some(f)) => f,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Feed not found"})),
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
    };

    let feed = crate::models::Feed::new(feed_config.clone(), None);
    match feed.fetch_posts().await {
        Ok(posts) => {
            tracing::info!(count = posts.len(), feed_id = %id, "Manual feed run completed");
            (
                StatusCode::OK,
                Json(json!({
                    "status": "ok",
                    "feed_id": id,
                    "posts_count": posts.len(),
                    "posts": posts,
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(feed_id = %id, error = %e, "Manual feed run failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to fetch feed: {e}")})),
            )
                .into_response()
        }
    }
}
