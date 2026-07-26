use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::auth::AppState;
use crate::models::ScheduleConfig;

/// Get the current schedule configuration.
pub async fn get(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    match state.db.get_schedule().await {
        Ok(schedule) => Json(json!(schedule)),
        Err(e) => Json(json!({
            "error": format!("Failed to get schedule: {e}"),
            "cron_expression": "0 * * * *",
            "timezone": "UTC"
        })),
    }
}

/// Update the schedule configuration.
pub async fn update(
    State(state): State<Arc<AppState>>,
    Json(schedule): Json<ScheduleConfig>,
) -> impl IntoResponse {
    match state.db.set_schedule(&schedule).await {
        Ok(()) => Json(json!({"status": "ok", "message": "Schedule updated"})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to update schedule: {e}")})),
        )
            .into_response(),
    }
}
