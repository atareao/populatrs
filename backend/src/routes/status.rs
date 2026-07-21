use std::sync::Arc;

use axum::{extract::State, Json};
use serde_json::json;

use crate::auth::AppState;
use crate::routes::storage::StorageConfig;

/// Dashboard status: summary of feeds, publishers, schedule & storage.
pub async fn dashboard(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
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
                    "interval_minutes": stats.schedule.default_interval_minutes,
                    "timezone": stats.schedule.timezone
                },
                "storage": storage
            }))
        }
        Err(e) => Json(json!({
            "error": format!("Failed to get stats: {e}"),
            "feeds": { "total": 0, "enabled": 0, "disabled": 0 },
            "publishers": { "total": 0 },
            "published_posts": 0,
            "schedule": { "interval_minutes": 60, "timezone": "UTC" },
            "storage": { "data_dir": "./data" }
        })),
    }
}
