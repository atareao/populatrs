use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::auth::AppState;
use crate::models::ScheduleConfig;
use crate::scheduler::next_cron_run_at_string;

/// Calculate the next upcoming fire time for a cron expression in the given timezone.
fn next_cron_run(cron_expr: &str, tz_name: &str) -> Option<String> {
    next_cron_run_at_string(
        &ScheduleConfig {
            cron_expression: cron_expr.to_string(),
            timezone: tz_name.to_string(),
        },
        chrono::Utc::now(),
    )
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
        Ok(()) => {
            let next_run_at = next_cron_run_at_string(&schedule, chrono::Utc::now());

            {
                let mut timing = state.scheduler_status.lock().await;
                timing.next_run_at = next_run_at.clone();
            }

            let _ = state.schedule_change_tx.send(());

            Json(json!({
                "status": "ok",
                "message": "Schedule updated",
                "next_run_at": next_run_at,
            }))
            .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to update schedule: {e}")})),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::Config;
    use crate::db::Database;
    use crate::models::PublisherManager;
    use axum::body::to_bytes;
    use tempfile::TempDir;
    use tokio::sync::broadcast;

    async fn test_state() -> (Arc<AppState>, broadcast::Receiver<()>, TempDir) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.db");
        let db = Database::open(&path).await.unwrap();
        let (log_tx, _) = broadcast::channel(16);
        let (schedule_change_tx, schedule_change_rx) = broadcast::channel(16);

        let state = Arc::new(AppState {
            config: Config {
                data_dir: dir.path().to_path_buf(),
                database_url: path,
                ..Config::load()
            },
            db,
            oidc_metadata: None,
            jwt_validator: Arc::new(crate::auth::JwtValidator::dev()),
            oidc_states: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            oauth_states: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            log_tx,
            schedule_change_tx,
            publisher_manager: Arc::new(PublisherManager::new()),
            scheduler_status: Default::default(),
        });

        (state, schedule_change_rx, dir)
    }

    #[tokio::test]
    async fn test_update_refreshes_scheduler_status_and_notifies_scheduler() {
        let (state, mut schedule_change_rx, _dir) = test_state().await;
        let schedule = ScheduleConfig {
            cron_expression: "*/5 * * * *".to_string(),
            timezone: "UTC".to_string(),
        };

        let response = update(State(state.clone()), Json(schedule))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(payload["next_run_at"].is_string());
        assert!(schedule_change_rx.recv().await.is_ok());

        let timing = state.scheduler_status.lock().await;
        assert!(timing.next_run_at.is_some());
    }
}
