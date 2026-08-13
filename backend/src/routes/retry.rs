use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use std::sync::Arc;

use crate::auth::AppState;
use crate::models::RetryPolicy;

/// GET /api/settings/retry-policy
pub async fn get_retry_policy(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_retry_policy().await {
        Ok(policy) => Json(json!(policy)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to load retry policy: {e}")})),
        )
            .into_response(),
    }
}

/// PUT /api/settings/retry-policy
pub async fn update_retry_policy(
    State(state): State<Arc<AppState>>,
    Json(policy): Json<RetryPolicy>,
) -> impl IntoResponse {
    // Validate fields
    if policy.max_retries > 10 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "max_retries must be between 0 and 10"})),
        )
            .into_response();
    }
    if policy.base_delay_seconds == 0 || policy.base_delay_seconds > 3600 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "base_delay_seconds must be between 1 and 3600"})),
        )
            .into_response();
    }
    if policy.max_delay_seconds < policy.base_delay_seconds || policy.max_delay_seconds > 86400 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "max_delay_seconds must be >= base_delay_seconds and <= 86400"})),
        )
            .into_response();
    }
    if policy.backoff_multiplier < 1.0 || policy.backoff_multiplier > 10.0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "backoff_multiplier must be between 1.0 and 10.0"})),
        )
            .into_response();
    }

    match state.db.set_retry_policy(&policy).await {
        Ok(()) => Json(json!({"status": "ok"})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to save retry policy: {e}")})),
        )
            .into_response(),
    }
}
