use std::str::FromStr;
use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono_tz::Tz;
use serde_json::json;

use crate::auth::AppState;
use crate::models::ScheduleConfig;

/// Calculate the next upcoming fire time for a cron expression in the given timezone.
fn next_cron_run(cron_expr: &str, tz_name: &str) -> Option<String> {
    let tz: Tz = tz_name.parse().ok()?;
    let schedule = cron::Schedule::from_str(cron_expr).ok()?;
    let next_local = schedule.upcoming(tz).next()?;
    let next_utc = next_local.with_timezone(&chrono::Utc);
    Some(next_utc.to_rfc3339())
}

/// Get the current schedule configuration.
pub async fn get(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    match state.db.get_schedule().await {
        Ok(schedule) => {
            let next_run = next_cron_run(&schedule.cron_expression, &schedule.timezone);
            Json(json!({
                "cron_expression": schedule.cron_expression,
                "timezone": schedule.timezone,
                "next_run_at": next_run,
            }))
        }
        Err(e) => Json(json!({
            "error": format!("Failed to get schedule: {e}"),
            "cron_expression": "0 * * * *",
            "timezone": "UTC",
            "next_run_at": null,
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
