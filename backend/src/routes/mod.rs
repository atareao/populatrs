pub mod auth_routes;
pub mod feeds;
pub mod logs;
pub mod oauth;
pub mod publishers;
pub mod retry;
pub mod schedule;
pub mod settings;
pub mod status;
pub mod storage;
pub mod youtube;

use std::sync::Arc;

use axum::{middleware, routing, Json, Router};
use serde::Serialize;

use crate::auth::AppState;
use crate::middleware::require_auth;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

/// Build all routes for the populatrs web server.
/// State is applied externally via `.with_state()`.
pub fn api_routes() -> Router<Arc<AppState>> {
    // Public routes (no auth required)
    let public = Router::new()
        .route("/health", routing::get(health))
        .route("/auth/login", routing::get(auth_routes::login))
        .route("/auth/callback", routing::get(auth_routes::callback))
        .route("/auth/dev-login", routing::get(auth_routes::dev_login))
        .route("/oauth/callback", routing::get(oauth::callback_get));

    // Protected routes (auth required)
    let protected = Router::new()
        .route("/api/me", routing::get(auth_routes::me))
        .route("/api/feeds", routing::get(feeds::list).post(feeds::create))
        .route(
            "/api/feeds/{id}",
            routing::put(feeds::update)
                .delete(feeds::delete)
                .patch(feeds::toggle),
        )
        .route("/api/feeds/{id}/run", routing::post(feeds::run))
        .route("/api/feeds/{id}/publish", routing::post(feeds::publish))
        .route(
            "/api/feeds/resolve-youtube",
            routing::post(feeds::resolve_youtube),
        )
        .route(
            "/api/publishers",
            routing::get(publishers::list).post(publishers::create),
        )
        .route(
            "/api/publishers/{id}",
            routing::put(publishers::update_by_id)
                .delete(publishers::delete_publisher)
                .patch(publishers::toggle_publisher),
        )
        .route(
            "/api/publishers/{id}/test",
            routing::post(publishers::test_publisher),
        )
        .route(
            "/api/publishers/{id}/oauth/authorize",
            routing::get(oauth::authorize),
        )
        .route(
            "/api/publishers/{id}/oauth/callback",
            routing::post(oauth::callback),
        )
        .route(
            "/api/publishers/{id}/oauth/status",
            routing::get(oauth::status),
        )
        .route(
            "/api/schedule",
            routing::get(schedule::get).put(schedule::update),
        )
        .route(
            "/api/storage",
            routing::get(storage::get).put(storage::update),
        )
        .route(
            "/api/youtube",
            routing::get(youtube::get).put(youtube::update),
        )
        .route("/api/status", routing::get(status::dashboard))
        .route("/api/logs/stream", routing::get(logs::stream))
        .route("/api/logs/history", routing::get(logs::history))
        .route("/api/logs/republish", routing::post(logs::republish))
        .route(
            "/api/logs/retention",
            routing::get(logs::get_retention).put(logs::set_retention),
        )
        .route(
            "/api/settings/retry-policy",
            routing::get(retry::get_retry_policy).put(retry::update_retry_policy),
        )
        .route(
            "/api/settings/publish",
            routing::get(settings::get_publish_settings).put(settings::update_publish_settings),
        )
        .layer(middleware::from_fn(require_auth));

    public.merge(protected)
}
