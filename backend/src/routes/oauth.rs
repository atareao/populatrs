use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
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

/// Query params for the GET /oauth/callback endpoint (called by OAuth provider).
#[derive(Debug, Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: String,
    pub state: Option<String>,
}

/// `GET /api/publishers/{id}/oauth/authorize`
///
/// Generates an OAuth 2.0 authorization URL for an X/Twitter or LinkedIn
/// publisher. The OAuth state is stored in `AppState.oauth_states` so it
/// can be retrieved when the callback arrives.
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
            template: x_pub.template.clone(),
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

        // Obtener el user_id de LinkedIn automáticamente
        let user_id = match li_pub.get_user_profile(&access_token).await {
            Ok(profile_urn) => {
                tracing::info!("LinkedIn user profile URN: {}", profile_urn);
                // Extraer solo el ID del URN (urn:li:person:XXXXX → XXXXX)
                let id = profile_urn
                    .strip_prefix("urn:li:person:")
                    .unwrap_or(&profile_urn)
                    .strip_prefix("urn:li:")
                    .unwrap_or(&profile_urn)
                    .to_string();
                Some(id)
            }
            Err(e) => {
                tracing::warn!("Failed to get LinkedIn user profile: {}", e);
                None
            }
        };

        let updated = PublisherConfig::LinkedIn {
            client_id: li_pub.client_id.clone(),
            client_secret: li_pub.client_secret.clone(),
            access_token: Some(access_token),
            refresh_token,
            user_id,
            redirect_uri: Some(li_pub.redirect_uri.clone()),
            template: li_pub.template.clone(),
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

/// Helper: find publisher_id from the stored OAuth state value.
/// Iterates the oauth_states map looking for a matching state value.
fn resolve_publisher_id(
    states: &std::collections::HashMap<String, (String, Instant)>,
    target_state: &str,
) -> Option<String> {
    for (key, (stored_state, _)) in states {
        if stored_state == target_state {
            // key format is "linkedin:{id}" or "x:{id}"
            if let Some(id) = key.split(':').nth(1) {
                return Some(id.to_string());
            }
        }
    }
    None
}

/// `GET /oauth/callback`
///
/// Public endpoint called by the OAuth provider (LinkedIn, X) after the user
/// authorizes the application. Exchanges the authorization code for tokens,
/// persists them, and returns an HTML page that closes the popup window.
pub async fn callback_get(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OAuthCallbackQuery>,
) -> impl IntoResponse {
    let code = &query.code;
    let state_param = query.state.as_deref().unwrap_or("");

    // ── 1. Resolve publisher_id from state ──
    let (publisher_id, stored_state, is_linkedin) = {
        let states = state.oauth_states.lock().await;
        if let Some(id) = resolve_publisher_id(&states, state_param) {
            let stored = states.get(&format!("linkedin:{id}"));
            match stored {
                Some((s, _)) if s == state_param => (id.clone(), s.clone(), true),
                _ => {
                    // Check X
                    let stored = states.get(&format!("x:{id}"));
                    match stored {
                        Some((s, _)) => (id, s.clone(), false),
                        None => {
                            return (
                                StatusCode::BAD_REQUEST,
                                Html(oauth_result_html(false, "No matching OAuth state found")),
                            )
                                .into_response();
                        }
                    }
                }
            }
        } else {
            return (
                StatusCode::BAD_REQUEST,
                Html(oauth_result_html(false, "No matching OAuth state found")),
            )
                .into_response();
        }
    };

    // Remove stored state
    {
        let mut states = state.oauth_states.lock().await;
        if is_linkedin {
            states.remove(&format!("linkedin:{publisher_id}"));
        } else {
            states.remove(&format!("x:{publisher_id}"));
        }
    }

    // ── 2. Validate state ──
    if state_param != stored_state {
        return (
            StatusCode::UNAUTHORIZED,
            Html(oauth_result_html(false, "OAuth state mismatch")),
        )
            .into_response();
    }

    // ── 3. Load publisher config ──
    let config = match state.db.get_publisher(&publisher_id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Html(oauth_result_html(false, "Publisher not found")),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(oauth_result_html(false, &format!("Database error: {e}"))),
            )
                .into_response();
        }
    };

    // ── 4. Create publisher and exchange code ──
    let publisher = match create_publisher(publisher_id.clone(), &config) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(oauth_result_html(
                    false,
                    &format!("Failed to create publisher: {e}"),
                )),
            )
                .into_response();
        }
    };

    if is_linkedin {
        let li_pub = match publisher.as_any().downcast_ref::<LinkedInPublisher>() {
            Some(p) => p,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Html(oauth_result_html(
                        false,
                        "Publisher is not a LinkedIn publisher",
                    )),
                )
                    .into_response();
            }
        };

        let (access_token, refresh_token, _) = match li_pub.exchange_code_for_tokens(code).await {
            Ok(tokens) => tokens,
            Err(e) => {
                return (
                    StatusCode::BAD_GATEWAY,
                    Html(oauth_result_html(
                        false,
                        &format!("Token exchange failed: {e}"),
                    )),
                )
                    .into_response();
            }
        };

        // Obtener el user_id de LinkedIn automáticamente
        let user_id = match li_pub.get_user_profile(&access_token).await {
            Ok(profile_urn) => {
                tracing::info!("LinkedIn user profile URN: {}", profile_urn);
                let id = profile_urn
                    .strip_prefix("urn:li:person:")
                    .unwrap_or(&profile_urn)
                    .strip_prefix("urn:li:")
                    .unwrap_or(&profile_urn)
                    .to_string();
                Some(id)
            }
            Err(e) => {
                tracing::warn!("Failed to get LinkedIn user profile: {}", e);
                None
            }
        };

        let updated = PublisherConfig::LinkedIn {
            client_id: li_pub.client_id.clone(),
            client_secret: li_pub.client_secret.clone(),
            access_token: Some(access_token),
            refresh_token,
            user_id,
            redirect_uri: Some(li_pub.redirect_uri.clone()),
            template: li_pub.template.clone(),
        };

        if let Err(e) = state
            .db
            .upsert_publisher(&publisher_id, &updated, true)
            .await
        {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(oauth_result_html(
                    false,
                    &format!("Failed to save tokens: {e}"),
                )),
            )
                .into_response();
        }

        (
            StatusCode::OK,
            Html(oauth_result_html(true, "LinkedIn connected successfully!")),
        )
            .into_response()
    } else {
        // X/Twitter
        let x_pub = match publisher.as_any().downcast_ref::<XPublisher>() {
            Some(p) => p,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Html(oauth_result_html(
                        false,
                        "Publisher is not an X/Twitter publisher",
                    )),
                )
                    .into_response();
            }
        };

        let code_verifier = {
            let mut states = state.oauth_states.lock().await;
            states
                .remove(&format!("x:{publisher_id}"))
                .map(|(v, _)| v)
                .unwrap_or_else(|| "challenge".to_string())
        };

        let (access_token, refresh_token, _) =
            match x_pub.exchange_code_for_tokens(code, &code_verifier).await {
                Ok(tokens) => tokens,
                Err(e) => {
                    return (
                        StatusCode::BAD_GATEWAY,
                        Html(oauth_result_html(
                            false,
                            &format!("Token exchange failed: {e}"),
                        )),
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
            template: x_pub.template.clone(),
        };

        if let Err(e) = state
            .db
            .upsert_publisher(&publisher_id, &updated, true)
            .await
        {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(oauth_result_html(
                    false,
                    &format!("Failed to save tokens: {e}"),
                )),
            )
                .into_response();
        }

        (
            StatusCode::OK,
            Html(oauth_result_html(true, "X/Twitter connected successfully!")),
        )
            .into_response()
    }
}

/// Generates an HTML page that displays the OAuth result and closes the popup.
fn oauth_result_html(success: bool, message: &str) -> String {
    let status = if success { "success" } else { "error" };
    let title = if success {
        "✅ Connected!"
    } else {
        "❌ Connection failed"
    };
    let color = if success { "#22c55e" } else { "#ef4444" };
    let icon = if success { "✅" } else { "❌" };
    let escaped_msg = message
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>OAuth {status}</title>
  <style>
    * {{ margin: 0; padding: 0; box-sizing: border-box; }}
    body {{
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
      background: #0f0f1a; color: #e0e0e0;
      display: flex; justify-content: center; align-items: center;
      min-height: 100vh; text-align: center;
    }}
    .card {{ background: #1a1a2e; padding: 2rem; border-radius: 12px; max-width: 420px; }}
    .icon {{ font-size: 3rem; margin-bottom: 1rem; }}
    .title {{ font-size: 1.25rem; font-weight: 600; margin-bottom: 0.5rem; }}
    .message {{ color: #a0a0b0; font-size: 0.9rem; margin-bottom: 1.5rem; }}
    .btn {{
      background: {color}; color: white; border: none;
      padding: 0.5rem 1.5rem; border-radius: 6px; cursor: pointer;
      font-size: 0.9rem;
    }}
    .btn:hover {{ opacity: 0.9; }}
  </style>
</head>
<body>
  <div class="card">
    <div class="icon">{icon}</div>
    <div class="title">{title}</div>
    <div class="message">{escaped_msg}</div>
    <button class="btn" onclick="window.close()">Close window</button>
  </div>
  <script>
    // Try to post message to opener and close
    if (window.opener) {{
      window.opener.postMessage({{ type: 'oauth-{status}', publisher: '' }}, '*');
      setTimeout(() => window.close(), 1500);
    }}
  </script>
</body>
</html>"#
    )
}
