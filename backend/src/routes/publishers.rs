use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::auth::AppState;
use crate::models::{Post, PublisherConfig};
use crate::publisher::create_publisher;

/// List all publishers.
pub async fn list(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    match state.db.list_publishers().await {
        Ok(publishers) => {
            // Transform (config, enabled) tuples into objects with enabled field
            let entries: serde_json::Value = publishers
                .into_iter()
                .map(|(id, (config, enabled))| {
                    let entry = serde_json::to_value(&config).unwrap_or_default();
                    (
                        id,
                        json!({
                            "type": entry.get("type"),
                            "config": entry.get("config"),
                            "enabled": enabled,
                        }),
                    )
                })
                .collect::<serde_json::Map<_, _>>()
                .into();
            Json(json!({
                "publishers": entries,
                "total": entries.as_object().map(|o| o.len()).unwrap_or(0),
            }))
        }
        Err(e) => Json(json!({
            "error": format!("Failed to list publishers: {e}"),
            "publishers": {},
            "total": 0
        })),
    }
}

#[derive(Deserialize)]
pub struct CreatePublisherPayload {
    pub id: String,
    pub config: PublisherConfig,
}

/// Create a new publisher.
pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreatePublisherPayload>,
) -> impl IntoResponse {
    // Check for duplicate ID
    match state.db.get_publisher(&payload.id).await {
        Ok(Some(_)) => {
            return (
                StatusCode::CONFLICT,
                Json(json!({"error": format!("Publisher '{}' already exists", payload.id)})),
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

    match state
        .db
        .upsert_publisher(&payload.id, &payload.config, true)
        .await
    {
        Ok(()) => (
            StatusCode::CREATED,
            Json(json!({"status": "ok", "message": "Publisher created"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create publisher: {e}")})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct UpdatePublisherPayload {
    pub config: PublisherConfig,
}

/// Test a publisher by sending a test message.
pub async fn test_publisher(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let config = match state.db.get_publisher(&id).await {
        Ok(Some(config)) => config,
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
    };

    let publisher = match create_publisher(id.clone(), &config) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("Failed to create publisher: {e}")})),
            )
                .into_response();
        }
    };

    let test_post = Post::new(
        "test-message".to_string(),
        "🧪 Test message from Populatrs".to_string(),
        Some(
            "This is a test message to verify your publisher configuration is working correctly."
                .to_string(),
        ),
        "https://github.com/lorenzocarbonell/populatrs".to_string(),
        chrono::Utc::now(),
        "test".to_string(),
    );

    match publisher.publish(&test_post, None).await {
        Ok(msg) => (
            StatusCode::OK,
            Json(json!({"status": "ok", "message": msg})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("Test failed: {e}")})),
        )
            .into_response(),
    }
}
/// Delete a publisher by ID.
pub async fn delete_publisher(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.delete_publisher(&id).await {
        Ok(true) => Json(json!({"status": "ok", "message": "Publisher deleted"})).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("Publisher '{}' not found", id)})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to delete publisher: {e}")})),
        )
            .into_response(),
    }
}
/// Toggle the enabled status of a publisher.
pub async fn toggle_publisher(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let current = match state.db.get_publisher_enabled(&id).await {
        Ok(Some(enabled)) => enabled,
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
    };

    let new_enabled = !current;
    match state.db.set_publisher_enabled(&id, new_enabled).await {
        Ok(true) => Json(json!({"status": "ok", "enabled": new_enabled})).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("Publisher '{}' not found", id)})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to toggle publisher: {e}")})),
        )
            .into_response(),
    }
}
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

    match state.db.upsert_publisher(&id, &payload.config, true).await {
        Ok(()) => Json(json!({"status": "ok", "message": "Publisher updated"})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to update publisher: {e}")})),
        )
            .into_response(),
    }
}
