use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::auth::AppState;
use crate::models::PublisherConfig;
use crate::publisher::{create_publisher, LinkedInPublisher, XPublisher};

/// Payload sent by the frontend after the OAuth provider redirects the user
/// back with an authorization code.
#[derive(Debug, Deserialize)]
pub struct OAuthCallbackPayload {
    pub code: String,
    pub state: Option<String>,
}

/// `GET /api/publishers/{id}/oauth/authorize`
///
/// Generates an OAuth 2.0 authorization URL for an X/Twitter or LinkedIn
/// publisher.  The OAuth state (code_verifier for X, state parameter for
/// LinkedIn) is stored in `AppState.oauth_states` so it can be retrieved
/// when the callback arrives.
pub async fn authorize(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // ── 1. Load publisher config from DB ──
    let config = match state.db.get_publisher(&id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "ok": false, "error": "Publisher not found" })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": format!("Database error: {e}") })),
            )
                .into_response();
        }
    };

    // ── 2. Create publisher instance ──
    let publisher = match create_publisher(id.clone(), &config) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": format!("Failed to create publisher: {e}") })),
            )
                .into_response();
        }
    };

    // ── 3. Downcast and generate auth URL ──

    // X/Twitter uses OAuth 2.0 PKCE — store the code_verifier
    if let Some(x_pub) = publisher.as_any().downcast_ref::<XPublisher>() {
        let (auth_url, code_verifier) = x_pub.generate_auth_url(None);
        let mut states = state.oauth_states.lock().await;
        states.insert(format!("x:{id}"), (code_verifier, Instant::now()));
        return Json(json!({ "ok": true, "url": auth_url })).into_response();
    }

    // LinkedIn uses standard OAuth 2.0 — store the state parameter
    if let Some(li_pub) = publisher.as_any().downcast_ref::<LinkedInPublisher>() {
        let oauth_state = uuid::Uuid::new_v4().to_string();
        let auth_url = li_pub.generate_auth_url(Some(oauth_state.clone()));
        let mut states = state.oauth_states.lock().await;
        states.insert(format!("linkedin:{id}"), (oauth_state, Instant::now()));
        return Json(json!({ "ok": true, "url": auth_url })).into_response();
    }

    (
        StatusCode::BAD_REQUEST,
        Json(
            json!({ "ok": false, "error": "Publisher type does not support OAuth authorization" }),
        ),
    )
        .into_response()
}

/// `POST /api/publishers/{id}/oauth/callback`
///
/// Exchanges the OAuth authorization code for access and refresh tokens,
/// then persists the updated publisher configuration in the database.
pub async fn callback(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<OAuthCallbackPayload>,
) -> impl IntoResponse {
    // ── 1. Load publisher config from DB ──
    let config = match state.db.get_publisher(&id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "ok": false, "error": "Publisher not found" })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": format!("Database error: {e}") })),
            )
                .into_response();
        }
    };

    // ── 2. Create publisher instance ──
    let publisher = match create_publisher(id.clone(), &config) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": format!("Failed to create publisher: {e}") })),
            )
                .into_response();
        }
    };

    // ── 3. Downcast, exchange code, persist ──

    // ── X/Twitter callback ──
    if let Some(x_pub) = publisher.as_any().downcast_ref::<XPublisher>() {
        let code_verifier = {
            let mut states = state.oauth_states.lock().await;
            states
                .remove(&format!("x:{id}"))
                .map(|(v, _)| v)
                .unwrap_or_else(|| "challenge".to_string())
        };

        let (access_token, refresh_token, _expires_in) = match x_pub
            .exchange_code_for_tokens(&payload.code, &code_verifier)
            .await
        {
            Ok(tokens) => tokens,
            Err(e) => {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({ "ok": false, "error": format!("Token exchange failed: {e}") })),
                )
                    .into_response();
            }
        };

        let updated = PublisherConfig::X {
            client_id: x_pub.client_id.clone(),
            client_secret: x_pub.client_secret.clone(),
            access_token: Some(access_token),
            refresh_token,
            redirect_uri: Some(x_pub.redirect_uri.clone()),
            template: Some(x_pub.template.clone()),
        };

        if let Err(e) = state.db.upsert_publisher(&id, &updated, true).await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": format!("Failed to save tokens: {e}") })),
            )
                .into_response();
        }

        return Json(json!({ "ok": true, "message": "OAuth completed for X/Twitter" }))
            .into_response();
    }

    // ── LinkedIn callback ──
    if let Some(li_pub) = publisher.as_any().downcast_ref::<LinkedInPublisher>() {
        // Validate state if the frontend provided one
        if let Some(ref cb_state) = payload.state {
            let mut states = state.oauth_states.lock().await;
            let stored = states.remove(&format!("linkedin:{id}"));
            match stored {
                Some((ref stored_state, _)) if stored_state == cb_state => { /* ok */ }
                Some(_) => {
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(json!({ "ok": false, "error": "OAuth state mismatch" })),
                    )
                        .into_response();
                }
                None => {
                    tracing::warn!("No stored OAuth state found for LinkedIn publisher {id}");
                }
            }
        }

        let (access_token, refresh_token, _expires_in) = match li_pub
            .exchange_code_for_tokens(&payload.code)
            .await
        {
            Ok(tokens) => tokens,
            Err(e) => {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({ "ok": false, "error": format!("Token exchange failed: {e}") })),
                )
                    .into_response();
            }
        };

        let updated = PublisherConfig::LinkedIn {
            client_id: li_pub.client_id.clone(),
            client_secret: li_pub.client_secret.clone(),
            access_token: Some(access_token),
            refresh_token,
            user_id: li_pub.user_id.clone(),
            redirect_uri: Some(li_pub.redirect_uri.clone()),
            template: Some(li_pub.template.clone()),
        };

        if let Err(e) = state.db.upsert_publisher(&id, &updated, true).await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": format!("Failed to save tokens: {e}") })),
            )
                .into_response();
        }

        return Json(json!({ "ok": true, "message": "OAuth completed for LinkedIn" }))
            .into_response();
    }

    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "ok": false, "error": "Publisher type does not support OAuth callback" })),
    )
        .into_response()
}
