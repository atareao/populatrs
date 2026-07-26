use std::sync::Arc;

use axum::{extract::State, Json};
use serde_json::json;

use crate::auth::AppState;
use crate::routes::storage::StorageConfig;

/// Dashboard status: summary of feeds, publishers, schedule & storage.
pub async fn dashboard(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let sched_timing = state.scheduler_status.lock().await;
    let last_run_at = sched_timing.last_run_at.clone();
    let next_run_at = sched_timing.next_run_at.clone();
    drop(sched_timing);

    match state.db.get_stats().await {
        Ok(stats) => {
            let storage = StorageConfig {
                data_dir: state.config.data_dir.to_string_lossy().to_string(),
            };
            Json(json!({
                "feeds": {
                    "total": stats.total_feeds,
                    "enabled": stats.enabled_feeds,
                    "disabled": stats.total_feeds - stats.enabled_feeds
                },
                "publishers": {
                    "total": stats.total_publishers
                },
                "published_posts": stats.total_published,
                "schedule": {
                    "cron_expression": stats.schedule.cron_expression,
                    "timezone": stats.schedule.timezone
                },
                "storage": storage,
                "last_run_at": last_run_at,
                "next_run_at": next_run_at
            }))
        }
        Err(e) => Json(json!({
            "error": format!("Failed to get stats: {e}"),
            "feeds": { "total": 0, "enabled": 0, "disabled": 0 },
            "publishers": { "total": 0 },
            "published_posts": 0,
            "schedule": { "cron_expression": "0 * * * *", "timezone": "UTC" },
            "storage": { "data_dir": "./data" },
            "last_run_at": null,
            "next_run_at": null
        })),
    }
}
