use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive},
        IntoResponse, Sse,
    },
};
use tokio::sync::broadcast;

use crate::auth::{AppState, LogEntry};

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
        // Collect field values
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

        // Non-blocking send — ignore errors if no receivers
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
/// Uses a polling loop over the broadcast receiver.
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
