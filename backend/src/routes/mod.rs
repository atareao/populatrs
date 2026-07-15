pub mod auth_routes;
pub mod feeds;
pub mod publishers;
pub mod schedule;
pub mod status;
pub mod storage;

use std::sync::Arc;

use axum::{Json, Router, middleware, routing};
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
        .route("/auth/dev-login", routing::get(auth_routes::dev_login));

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
        .route("/api/publishers", routing::get(publishers::list))
        .route(
            "/api/publishers/{id}",
            routing::put(publishers::update_by_id),
        )
        .route("/api/schedule", routing::get(schedule::get).put(schedule::update))
        .route("/api/storage", routing::get(storage::get).put(storage::update))
        .route("/api/status", routing::get(status::dashboard))
        .layer(middleware::from_fn(require_auth));

    public.merge(protected)
}