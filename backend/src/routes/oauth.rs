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
use crate::publisher::{
    create_publisher, create_publisher_with_config_path, LinkedInPublisher, MastodonPublisher,
    ThreadsPublisher, XPublisher,
};

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
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

/// `GET /api/publishers/{id}/oauth/authorize`
///
/// Generates an OAuth 2.0 authorization URL for an X/Twitter, LinkedIn, or Threads
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
    let publisher = match create_publisher_with_config_path(
        id.clone(),
        &config,
        None,
        Some(Arc::new(state.db.clone())),
    ) {
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

    // X/Twitter uses OAuth 2.0 PKCE — store the state for validation
    if let Some(x_pub) = publisher.as_any().downcast_ref::<XPublisher>() {
        let oauth_state = uuid::Uuid::new_v4().to_string();
        let (auth_url, _code_verifier) = x_pub.generate_auth_url(Some(oauth_state.clone()));
        let mut states = state.oauth_states.lock().await;
        states.insert(format!("x:{id}"), (oauth_state, Instant::now()));
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

    // Threads uses standard OAuth 2.0 — store the state parameter
    if let Some(t_pub) = publisher.as_any().downcast_ref::<ThreadsPublisher>() {
        let oauth_state = uuid::Uuid::new_v4().to_string();
        let auth_url = t_pub.generate_auth_url(Some(oauth_state.clone()));
        let mut states = state.oauth_states.lock().await;
        states.insert(format!("threads:{id}"), (oauth_state, Instant::now()));
        return Json(json!({ "ok": true, "url": auth_url })).into_response();
    }

    // Mastodon uses OAuth 2.0 — optionally registers the app first
    if let Some(m_pub) = publisher.as_any().downcast_ref::<MastodonPublisher>() {
        return Box::pin(async move {
            // If no client_id, register the app first
            if m_pub.client_id.is_none() || m_pub.client_secret.is_none() {
                match m_pub.register_app().await {
                    Ok((client_id, client_secret)) => {
                        // Persist the credentials
                        let updated = PublisherConfig::Mastodon {
                            server_url: m_pub.server_url.clone(),
                            client_id: Some(client_id),
                            client_secret: Some(client_secret),
                            access_token: m_pub.access_token.clone(),
                            redirect_uri: Some(m_pub.redirect_uri.clone()),
                            template: m_pub.template.clone(),
                        };
                        if let Err(e) = state.db.upsert_publisher(&id, &updated, true).await {
                            return Json(json!({ "ok": false, "error": format!("Failed to save app credentials: {e}") })).into_response();
                        }
                        // Re-create publisher with updated config to get the new client_id/client_secret
                        match create_publisher(id.clone(), &updated) {
                            Ok(p) => {
                                if let Some(new_m_pub) = p.as_any().downcast_ref::<MastodonPublisher>() {
                                    let oauth_state = uuid::Uuid::new_v4().to_string();
                                    let auth_url_str = new_m_pub.generate_auth_url(Some(oauth_state.clone()));
                                    let mut states = state.oauth_states.lock().await;
                                    states.insert(format!("mastodon:{id}"), (oauth_state, Instant::now()));
                                    return Json(json!({ "ok": true, "url": auth_url_str })).into_response();
                                }
                            }
                            Err(e) => {
                                return Json(json!({ "ok": false, "error": format!("Failed to recreate publisher: {e}") })).into_response();
                            }
                        }
                    }
                    Err(e) => {
                        return Json(json!({ "ok": false, "error": format!("Failed to register Mastodon app: {e}") })).into_response();
                    }
                }
            }

            let oauth_state = uuid::Uuid::new_v4().to_string();
            let auth_url_str = m_pub.generate_auth_url(Some(oauth_state.clone()));
            let mut states = state.oauth_states.lock().await;
            states.insert(format!("mastodon:{id}"), (oauth_state, Instant::now()));
            Json(json!({ "ok": true, "url": auth_url_str })).into_response()
        })
        .await;
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
    let publisher = match create_publisher_with_config_path(
        id.clone(),
        &config,
        None,
        Some(Arc::new(state.db.clone())),
    ) {
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
        // Validate state if the frontend provided one
        if let Some(ref cb_state) = payload.state {
            let mut states = state.oauth_states.lock().await;
            let stored = states.remove(&format!("x:{id}"));
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
                    tracing::warn!("No stored OAuth state found for X publisher {id}");
                }
            }
        }

        // code_verifier is hardcoded to "challenge" in generate_auth_url
        let code_verifier = "challenge".to_string();

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
            reply_template: Some(x_pub.reply_template.clone()),
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

    // ── Threads callback ──
    if let Some(t_pub) = publisher.as_any().downcast_ref::<ThreadsPublisher>() {
        // Validate state if the frontend provided one
        if let Some(ref cb_state) = payload.state {
            let mut states = state.oauth_states.lock().await;
            let stored = states.remove(&format!("threads:{id}"));
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
                    tracing::warn!("No stored OAuth state found for Threads publisher {id}");
                }
            }
        }

        let (access_token, user_id, _short_expires_in) = match t_pub
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

        // Exchange the short-lived token for a long-lived one (60 days)
        let (long_lived_token, long_expires_in) =
            match t_pub.exchange_for_long_lived_token(&access_token).await {
                Ok(tokens) => tokens,
                Err(e) => {
                    tracing::warn!("Failed to exchange for long-lived Threads token: {}", e);
                    // Fall back to short-lived token
                    (access_token.clone(), _short_expires_in)
                }
            };

        // user_id comes directly from Meta's token exchange response
        // If missing, fall back to looking up via a placeholder
        let final_user_id = user_id.or_else(|| {
            tracing::warn!("Threads token exchange did not return user_id");
            None
        });

        let token_expires_at = Some(chrono::Utc::now().timestamp() + long_expires_in as i64);

        let updated = PublisherConfig::Threads {
            client_id: t_pub.client_id.clone(),
            client_secret: t_pub.client_secret.clone(),
            access_token: Some(long_lived_token),
            user_id: final_user_id,
            redirect_uri: Some(t_pub.redirect_uri.clone()),
            template: t_pub.template.clone(),
            token_expires_at,
        };

        if let Err(e) = state.db.upsert_publisher(&id, &updated, true).await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": format!("Failed to save tokens: {e}") })),
            )
                .into_response();
        }

        return Json(json!({ "ok": true, "message": "OAuth completed for Threads" }))
            .into_response();
    }

    // ── Mastodon callback ──
    if let Some(m_pub) = publisher.as_any().downcast_ref::<MastodonPublisher>() {
        // Validate state if the frontend provided one
        if let Some(ref cb_state) = payload.state {
            let mut states = state.oauth_states.lock().await;
            let stored = states.remove(&format!("mastodon:{id}"));
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
                    tracing::warn!("No stored OAuth state found for Mastodon publisher {id}");
                }
            }
        }

        let (access_token, _refresh_token, _expires_in) = match m_pub
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

        let updated = PublisherConfig::Mastodon {
            server_url: m_pub.server_url.clone(),
            client_id: m_pub.client_id.clone(),
            client_secret: m_pub.client_secret.clone(),
            access_token: Some(access_token),
            redirect_uri: Some(m_pub.redirect_uri.clone()),
            template: m_pub.template.clone(),
        };

        if let Err(e) = state.db.upsert_publisher(&id, &updated, true).await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "ok": false, "error": format!("Failed to save tokens: {e}") })),
            )
                .into_response();
        }

        return Json(json!({ "ok": true, "message": "OAuth completed for Mastodon" }))
            .into_response();
    }

    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "ok": false, "error": "Publisher type does not support OAuth callback" })),
    )
        .into_response()
}

/// `GET /api/publishers/{id}/oauth/status`
///
/// Returns the OAuth connection status for a publisher:
/// - connected: whether an access_token exists
/// - token_expires_at: when the token expires (Threads)
/// - has_refresh_token: whether a refresh_token is available (X, LinkedIn)
/// - publisher_type: the type name
pub async fn status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
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

    let (connected, token_expires_at, has_refresh_token, publisher_type) = match &config {
        PublisherConfig::X {
            access_token,
            refresh_token,
            ..
        } => (access_token.is_some(), None, refresh_token.is_some(), "x"),
        PublisherConfig::LinkedIn {
            access_token,
            refresh_token,
            ..
        } => (
            access_token.is_some(),
            None,
            refresh_token.is_some(),
            "linkedin",
        ),
        PublisherConfig::Threads {
            access_token,
            token_expires_at,
            ..
        } => (access_token.is_some(), *token_expires_at, false, "threads"),
        PublisherConfig::Mastodon { access_token, .. } => {
            (access_token.is_some(), None, false, "mastodon")
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "ok": false,
                    "error": "Publisher type does not support OAuth"
                })),
            )
                .into_response();
        }
    };

    Json(json!({
        "ok": true,
        "connected": connected,
        "token_expires_at": token_expires_at,
        "has_refresh_token": has_refresh_token,
        "publisher_type": publisher_type,
    }))
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
            // key format is "linkedin:{id}", "threads:{id}", or "x:{id}"
            if let Some(id) = key.split(':').nth(1) {
                return Some(id.to_string());
            }
        }
    }
    None
}

/// `GET /oauth/callback`
///
/// Public endpoint called by the OAuth provider (Threads, LinkedIn, X) after the user
/// authorizes the application. Exchanges the authorization code for tokens,
/// persists them, and returns an HTML page that closes the popup window.
pub async fn callback_get(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OAuthCallbackQuery>,
) -> impl IntoResponse {
    // Handle OAuth provider errors (e.g. user denies authorization)
    if let Some(error) = &query.error {
        let desc = query
            .error_description
            .as_deref()
            .unwrap_or("OAuth authorization denied");
        tracing::warn!(error = %error, description = %desc, "OAuth callback received error");
        return Html(oauth_result_html(false, desc)).into_response();
    }

    let code = match &query.code {
        Some(c) => c.clone(),
        None => {
            return Html(oauth_result_html(
                false,
                "Missing authorization code from OAuth provider",
            ))
            .into_response();
        }
    };
    let state_param = query.state.as_deref().unwrap_or("");

    // ── 1. Resolve publisher_id and type from state ──
    let (publisher_id, stored_state, oauth_type) = {
        let states = state.oauth_states.lock().await;
        if let Some(id) = resolve_publisher_id(&states, state_param) {
            // Try to determine the type from stored keys
            // Check in priority order: threads, linkedin, mastodon, x
            let stored_threads = states.get(&format!("threads:{id}"));
            let stored_linkedin = states.get(&format!("linkedin:{id}"));
            let stored_mastodon = states.get(&format!("mastodon:{id}"));
            let stored_x = states.get(&format!("x:{id}"));

            if let Some((s, _)) = stored_threads.filter(|(s, _)| s == state_param) {
                (id.clone(), s.clone(), "threads")
            } else if let Some((s, _)) = stored_linkedin.filter(|(s, _)| s == state_param) {
                (id.clone(), s.clone(), "linkedin")
            } else if let Some((s, _)) = stored_mastodon.filter(|(s, _)| s == state_param) {
                (id.clone(), s.clone(), "mastodon")
            } else if let Some((s, _)) = stored_x.filter(|(s, _)| s == state_param) {
                (id, s.clone(), "x")
            } else {
                return (
                    StatusCode::BAD_REQUEST,
                    Html(oauth_result_html(false, "No matching OAuth state found")),
                )
                    .into_response();
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
        states.remove(&format!("{oauth_type}:{publisher_id}"));
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
    let publisher = match create_publisher_with_config_path(
        publisher_id.clone(),
        &config,
        None,
        Some(Arc::new(state.db.clone())),
    ) {
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

    match oauth_type {
        "linkedin" => {
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

            let (access_token, refresh_token, _) =
                match li_pub.exchange_code_for_tokens(&code).await {
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
        }
        "mastodon" => {
            let m_pub = match publisher.as_any().downcast_ref::<MastodonPublisher>() {
                Some(p) => p,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Html(oauth_result_html(
                            false,
                            "Publisher is not a Mastodon publisher",
                        )),
                    )
                        .into_response();
                }
            };

            let (access_token, _refresh_token, _) =
                match m_pub.exchange_code_for_tokens(&code).await {
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

            let updated = PublisherConfig::Mastodon {
                server_url: m_pub.server_url.clone(),
                client_id: m_pub.client_id.clone(),
                client_secret: m_pub.client_secret.clone(),
                access_token: Some(access_token),
                redirect_uri: Some(m_pub.redirect_uri.clone()),
                template: m_pub.template.clone(),
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
                Html(oauth_result_html(true, "Mastodon connected successfully!")),
            )
                .into_response()
        }
        "x" | "twitter" => {
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

            // X/Twitter — code_verifier is hardcoded to "challenge" in generate_auth_url
            let code_verifier = "challenge".to_string();

            let (access_token, refresh_token, _) =
                match x_pub.exchange_code_for_tokens(&code, &code_verifier).await {
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
                reply_template: Some(x_pub.reply_template.clone()),
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
        "threads" => {
            let t_pub = match publisher.as_any().downcast_ref::<ThreadsPublisher>() {
                Some(p) => p,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Html(oauth_result_html(
                            false,
                            "Publisher is not a Threads publisher",
                        )),
                    )
                        .into_response();
                }
            };

            let (access_token, user_id, _short_expires_in) =
                match t_pub.exchange_code_for_tokens(&code).await {
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

            // Exchange for long-lived token
            let (long_lived_token, long_expires_in) =
                match t_pub.exchange_for_long_lived_token(&access_token).await {
                    Ok(tokens) => tokens,
                    Err(e) => {
                        tracing::warn!("Failed to exchange for long-lived Threads token: {}", e);
                        (access_token.clone(), _short_expires_in)
                    }
                };

            let final_user_id = user_id.or_else(|| {
                tracing::warn!("Threads token exchange did not return user_id");
                None
            });

            let token_expires_at = Some(chrono::Utc::now().timestamp() + long_expires_in as i64);

            let updated = PublisherConfig::Threads {
                client_id: t_pub.client_id.clone(),
                client_secret: t_pub.client_secret.clone(),
                access_token: Some(long_lived_token),
                user_id: final_user_id,
                redirect_uri: Some(t_pub.redirect_uri.clone()),
                template: t_pub.template.clone(),
                token_expires_at,
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
                Html(oauth_result_html(true, "Threads connected successfully!")),
            )
                .into_response()
        }
        _ => (
            StatusCode::BAD_REQUEST,
            Html(oauth_result_html(false, "Unknown OAuth type")),
        )
            .into_response(),
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── OAuthCallbackQuery deserialization ──

    #[test]
    fn test_oauth_callback_query_with_code_and_state() {
        let q: OAuthCallbackQuery = serde_json::from_str(
            r#"{"code": "abc123", "state": "def456", "error": null, "error_description": null}"#,
        )
        .unwrap();
        assert_eq!(q.code.as_deref(), Some("abc123"));
        assert_eq!(q.state.as_deref(), Some("def456"));
        assert!(q.error.is_none());
        assert!(q.error_description.is_none());
    }

    #[test]
    fn test_oauth_callback_query_with_error_no_code() {
        let q: OAuthCallbackQuery = serde_json::from_str(
            r#"{"error": "access_denied", "error_description": "User denied", "code": null, "state": null}"#,
        )
        .unwrap();
        assert!(q.code.is_none());
        assert!(q.state.is_none());
        assert_eq!(q.error.as_deref(), Some("access_denied"));
        assert_eq!(q.error_description.as_deref(), Some("User denied"));
    }

    #[test]
    fn test_oauth_callback_query_code_missing() {
        let q: OAuthCallbackQuery = serde_json::from_str(r#"{}"#).unwrap();
        assert!(q.code.is_none());
        assert!(q.state.is_none());
        assert!(q.error.is_none());
        assert!(q.error_description.is_none());
    }

    // ── OAuthCallbackPayload deserialization ──

    #[test]
    fn test_oauth_callback_payload_with_code() {
        let p: OAuthCallbackPayload =
            serde_json::from_str(r#"{"code": "auth_code_xyz", "state": "my_state"}"#).unwrap();
        assert_eq!(p.code, "auth_code_xyz");
        assert_eq!(p.state.as_deref(), Some("my_state"));
    }

    #[test]
    fn test_oauth_callback_payload_code_only() {
        let p: OAuthCallbackPayload = serde_json::from_str(r#"{"code": "just_code"}"#).unwrap();
        assert_eq!(p.code, "just_code");
        assert!(p.state.is_none());
    }

    // ── resolve_publisher_id ──

    #[test]
    fn test_resolve_publisher_id_found() {
        let mut map = std::collections::HashMap::new();
        map.insert("x:pub123".into(), ("state_abc".into(), Instant::now()));
        map.insert(
            "linkedin:pub456".into(),
            ("state_def".into(), Instant::now()),
        );
        assert_eq!(
            resolve_publisher_id(&map, "state_abc"),
            Some("pub123".to_string())
        );
        assert_eq!(
            resolve_publisher_id(&map, "state_def"),
            Some("pub456".to_string())
        );
    }

    #[test]
    fn test_resolve_publisher_id_not_found() {
        let map = std::collections::HashMap::new();
        assert_eq!(resolve_publisher_id(&map, "nonexistent"), None);
    }

    #[test]
    fn test_resolve_publisher_id_no_match_for_state() {
        let mut map = std::collections::HashMap::new();
        map.insert("x:pub1".into(), ("state1".into(), Instant::now()));
        assert_eq!(resolve_publisher_id(&map, "state2"), None);
    }

    // ── oauth_result_html ──

    #[test]
    fn test_oauth_result_html_success_contains_expected_strings() {
        let html = oauth_result_html(true, "Connected!");
        assert!(html.contains("OAuth success"));
        assert!(html.contains("✅ Connected!"));
        assert!(html.contains("Connected!"));
        assert!(html.contains("#22c55e"));
        assert!(html.contains("window.close"));
        assert!(html.contains("oauth-success"));
    }

    #[test]
    fn test_oauth_result_html_error_contains_expected_strings() {
        let html = oauth_result_html(false, "Something went wrong");
        assert!(html.contains("OAuth error"));
        assert!(html.contains("❌ Connection failed"));
        assert!(html.contains("Something went wrong"));
        assert!(html.contains("#ef4444"));
        assert!(html.contains("oauth-error"));
    }

    #[test]
    fn test_oauth_result_html_message_escaping() {
        let html = oauth_result_html(false, "Error: <script>alert('xss')</script>");
        // The template has its own <script> tag, so we verify the user message is escaped
        assert!(html.contains("&lt;script&gt;alert('xss')&lt;/script&gt;"));
        assert!(!html.contains("<script>alert('xss')</script>"));
    }

    #[test]
    fn test_oauth_result_html_message_quotes_escaped() {
        let html = oauth_result_html(true, r#"He said "hello""#);
        assert!(html.contains("&quot;"));
        assert!(!html.contains(r#""hello""#));
    }

    #[test]
    fn test_oauth_result_html_doctype() {
        let html = oauth_result_html(true, "ok");
        assert!(html.starts_with("<!DOCTYPE html>"));
    }
}
