use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::auth::AppState;
use crate::models::Post;

// ───── Historical feed log endpoints ─────

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// GET /api/logs/history — returns historical feed publication logs.
pub async fn history(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HistoryQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50).min(200);
    let offset = query.offset.unwrap_or(0);

    match state.db.list_feed_logs(limit, offset).await {
        Ok(response) => Json(json!(response)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to load feed logs: {e}")})),
        )
            .into_response(),
    }
}

/// GET /api/logs/retention — returns the current log retention days.
pub async fn get_retention(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let days = state.db.get_log_retention().await.unwrap_or(30);
    Json(json!({"retention_days": days}))
}

/// PUT /api/logs/retention — updates the log retention days.
pub async fn set_retention(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let days = match body.get("retention_days").and_then(|v| v.as_u64()) {
        Some(d) if (1..=365).contains(&d) => d,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "retention_days must be between 1 and 365"})),
            )
                .into_response();
        }
    };

    match state.db.set_log_retention(days).await {
        Ok(()) => Json(json!({"status": "ok", "retention_days": days})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to set retention: {e}")})),
        )
            .into_response(),
    }
}

// ───── Republish a single post to a single publisher ─────

#[derive(Deserialize)]
pub struct RepublishRequest {
    pub guid: String,
    pub feed_id: String,
    pub publisher_id: String,
}

/// POST /api/logs/republish — retries publishing a specific post to a single
/// publisher, updating the stored publish result so the UI tag refreshes.
pub async fn republish(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RepublishRequest>,
) -> impl IntoResponse {
    let guid = body.guid;
    let feed_id = body.feed_id;
    let publisher_id = body.publisher_id;

    // 1. The post must have been published before (we need its title/url/description).
    let published = match state.db.get_published_post(&guid, &feed_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Published post not found"})),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!(guid = %guid, feed_id = %feed_id, error = %e, "Failed to load published post for republish");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to load published post: {e}")})),
            )
                .into_response();
        }
    };

    // 2. The feed must still exist (to locate the per-feed template override).
    let feed = match state.db.get_feed(&feed_id).await {
        Ok(Some(f)) => f,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Feed not found"})),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!(feed_id = %feed_id, error = %e, "Failed to load feed for republish");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to load feed: {e}")})),
            )
                .into_response();
        }
    };

    // 3. Locate the per-feed template override for this publisher (if any).
    let template = feed
        .publishers
        .iter()
        .find(|b| b.publisher_id == publisher_id)
        .and_then(|b| b.template.clone())
        .filter(|t| !t.is_empty());

    // 4. Rebuild the post from the persisted data.
    let post = Post::new(
        guid.clone(),
        published.title,
        published.description,
        published.url,
        chrono::Utc::now(),
        feed_id.clone(),
    );

    // 5. Resolve the publisher instance.
    let publisher = match state.publisher_manager.get_publisher(&publisher_id) {
        Some(p) => p,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": format!("Publisher '{}' not found", publisher_id)})),
            )
                .into_response();
        }
    };

    // 6. Load retry policy and attempt the republish with retry.
    let policy = state.db.get_retry_policy().await.unwrap_or_default();

    tracing::info!(
        guid = %guid,
        feed_id = %feed_id,
        publisher_id = %publisher_id,
        "Republishing post with retry policy (max_retries={})",
        policy.max_retries
    );

    let result =
        crate::publish_with_retry(publisher, &post, template.as_deref(), &policy, &state.db).await;

    match result {
        Ok(msg) => {
            tracing::info!(
                guid = %guid,
                feed_id = %feed_id,
                publisher_id = %publisher_id,
                "Republish succeeded"
            );
            (
                StatusCode::OK,
                Json(json!({"status": "ok", "success": true, "message": msg})),
            )
                .into_response()
        }
        Err(e) => {
            // A failed publish attempt is a valid business outcome, not a server
            // error. Return 200 with success=false so the UI can surface the real
            // reason to the user.
            tracing::error!(
                guid = %guid,
                feed_id = %feed_id,
                publisher_id = %publisher_id,
                error = %e,
                "Republish failed"
            );
            (
                StatusCode::OK,
                Json(json!({"status": "error", "success": false, "message": e.to_string()})),
            )
                .into_response()
        }
    }
}
