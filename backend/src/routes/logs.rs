use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive},
        IntoResponse, Sse,
    },
    Json,
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::broadcast;

use crate::auth::{AppState, LogEntry};

// ───── SSE log stream (real-time) ─────

/// Create a tracing Layer that broadcasts log events to SSE clients.
/// Returns the broadcast sender and the layer.
pub fn log_layer() -> (
    broadcast::Sender<LogEntry>,
    Box<dyn tracing_subscriber::Layer<tracing_subscriber::Registry> + Send + Sync>,
) {
    let (tx, _) = broadcast::channel(2048);
    (tx.clone(), Box::new(LogLayer { tx }))
}

struct LogLayer {
    tx: broadcast::Sender<LogEntry>,
}

impl<S> tracing_subscriber::Layer<S> for LogLayer
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    LogLayer: 'static,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut message = String::new();
        let mut target = String::new();

        let mut visitor = LogVisitor {
            message: &mut message,
            target: &mut target,
        };
        event.record(&mut visitor);

        if target.is_empty() {
            target = event.metadata().target().to_string();
        }

        let entry = LogEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: format!("{:?}", event.metadata().level()),
            message,
            target,
        };

        let _ = self.tx.send(entry);
    }
}

struct LogVisitor<'a> {
    message: &'a mut String,
    target: &'a mut String,
}

impl<'a> tracing::field::Visit for LogVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            *self.message = format!("{:?}", value);
        } else if field.name() == "target" {
            *self.target = format!("{:?}", value);
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            *self.message = value.to_string();
        } else if field.name() == "target" {
            *self.target = value.to_string();
        }
    }
}

/// SSE endpoint: streams log entries in real-time.
pub async fn stream(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let rx = state.log_tx.subscribe();

    let stream = futures::stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Ok(entry) => {
                let json = serde_json::to_string(&entry).unwrap_or_default();
                Some((
                    Ok::<_, std::convert::Infallible>(Event::default().data(json)),
                    rx,
                ))
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                let note = serde_json::json!({
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "level": "WARN",
                    "message": format!("Log stream lagged — missed {n} messages"),
                    "target": "populatrs::logs"
                });
                let json = serde_json::to_string(&note).unwrap_or_default();
                Some((
                    Ok::<_, std::convert::Infallible>(Event::default().data(json)),
                    rx,
                ))
            }
            Err(broadcast::error::RecvError::Closed) => None,
        }
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}

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
        Some(d) if d >= 1 && d <= 365 => d,
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
