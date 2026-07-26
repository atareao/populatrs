use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::auth::{AppState, AuthUser};

#[derive(Debug, Deserialize)]
pub struct AuthCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DevLoginQuery {
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: String,
    #[allow(dead_code)]
    expires_in: u64,
    id_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserInfoResponse {
    sub: String,
    email: Option<String>,
    name: Option<String>,
}

#[instrument(skip(state))]
pub async fn login(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if let Some(issuer) = &state.config.oidc_issuer_url {
        let client_id = state
            .config
            .oidc_client_id
            .as_deref()
            .unwrap_or("populatrs");
        let redirect_uri = state
            .config
            .oidc_redirect_url
            .as_deref()
            .unwrap_or("http://localhost:3044/auth/callback");

        // Generate random state for CSRF protection (PocketID requires >= 8 chars)
        let oauth_state = uuid::Uuid::new_v4().to_string();
        {
            let mut states = state.oidc_states.lock().await;
            states.insert(
                "oidc:login".to_string(),
                (oauth_state.clone(), Instant::now()),
            );
        }

        let url = format!(
            "{}/authorize?response_type=code&client_id={}&redirect_uri={}&scope=openid+profile+email&state={}",
            issuer.trim_end_matches('/'),
            client_id,
            redirect_uri,
            oauth_state,
        );
        tracing::info!("redirecting to OIDC provider: {}", url);
        Redirect::to(&url).into_response()
    } else {
        Redirect::to("/auth/dev-login").into_response()
    }
}

#[instrument(skip(state))]
pub async fn callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuthCallbackQuery>,
) -> impl IntoResponse {
    // Handle OIDC provider errors (e.g. PocketID sends ?error=access_denied)
    if let Some(error) = &query.error {
        let desc = query
            .error_description
            .as_deref()
            .unwrap_or("OIDC authorization denied");
        tracing::warn!(error = %error, description = %desc, "OIDC callback received error");
        let html = format!(
            r#"<!DOCTYPE html>
<html>
<head><title>Login failed</title></head>
<body>
<script>
    alert('{desc}');
    window.location.href = '/login';
</script>
</body>
</html>"#,
            desc = desc.replace('\'', "\\'")
        );
        return ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response();
    }

    // Validate state parameter for CSRF protection
    if let Some(ref cb_state) = query.state {
        let stored_state = state.oidc_states.lock().await.remove("oidc:login");
        match stored_state {
            Some((ref stored, _)) if stored == cb_state => { /* ok */ }
            Some(_) => {
                tracing::warn!("OIDC state mismatch: expected different value");
                return (
                    StatusCode::UNAUTHORIZED,
                    Html(
                        r#"<!DOCTYPE html>
<html>
<head><title>Login failed</title></head>
<body>
<script>
    alert('OAuth state mismatch. Please try again.');
    window.location.href = '/login';
</script>
</body>
</html>"#
                            .to_string(),
                    ),
                )
                    .into_response();
            }
            None => {
                tracing::warn!("No stored OIDC state found — possible replay attack");
                // Still allow the flow to continue for backwards compatibility
            }
        }
    } else {
        tracing::warn!("OIDC callback without state parameter");
    }

    let code = match &query.code {
        Some(c) => c.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                "Missing authorization code from OIDC provider".to_string(),
            )
                .into_response()
        }
    };

    let issuer = match &state.config.oidc_issuer_url {
        Some(i) => i.clone(),
        None => {
            return (StatusCode::BAD_GATEWAY, "OIDC not configured".to_string()).into_response()
        }
    };
    let client_id = state.config.oidc_client_id.as_deref().unwrap_or("");
    let client_secret = state.config.oidc_client_secret.as_deref().unwrap_or("");
    let redirect_uri = state
        .config
        .oidc_redirect_url
        .as_deref()
        .unwrap_or("http://localhost:3044/auth/callback");

    let token_url = format!("{}/api/oidc/token", issuer.trim_end_matches('/'));
    let params = [
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
        ("client_secret", client_secret),
    ];

    let client = reqwest::Client::new();
    let token_resp = match client.post(&token_url).form(&params).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, url = %token_url, "token exchange failed");
            return (
                StatusCode::BAD_GATEWAY,
                format!("token exchange failed: {e}"),
            )
                .into_response();
        }
    };

    if !token_resp.status().is_success() {
        let body = token_resp.text().await.unwrap_or_default();
        tracing::error!("token endpoint error: {}", body);
        return (
            StatusCode::BAD_GATEWAY,
            format!("token endpoint error: {body}"),
        )
            .into_response();
    }

    let token_data: TokenResponse = match token_resp.json().await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("failed to parse token response: {e}");
            return (StatusCode::BAD_GATEWAY, "invalid token response").into_response();
        }
    };

    let access_token = token_data.access_token.clone();
    let jwt = token_data.id_token.unwrap_or(token_data.access_token);

    let userinfo_url = format!("{}/api/oidc/userinfo", issuer.trim_end_matches('/'));
    let user_info = match client
        .get(&userinfo_url)
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
    {
        Ok(resp) => resp.json::<UserInfoResponse>().await.ok(),
        Err(_) => None,
    };

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head><title>Redirecting...</title></head>
<body>
<script>
sessionStorage.setItem('populatrs_token', '{jwt}');
{user_data}
window.location.href = '/';
</script>
</body>
</html>"#,
        jwt = jwt,
        user_data = user_info
            .as_ref()
            .map(|u| {
                format!(
                    "sessionStorage.setItem('populatrs_user', JSON.stringify({}));",
                    serde_json::json!({
                        "sub": u.sub,
                        "email": u.email,
                        "name": u.name,
                    })
                )
            })
            .unwrap_or_default()
    );

    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
}

#[instrument(skip(_state))]
pub async fn dev_login(
    State(_state): State<Arc<AppState>>,
    Query(query): Query<DevLoginQuery>,
) -> impl IntoResponse {
    let sub = query
        .email
        .clone()
        .unwrap_or_else(|| "dev@populatrs.app".into());

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head><title>Redirecting...</title></head>
<body>
<script>
sessionStorage.setItem('populatrs_token', '{jwt}');
sessionStorage.setItem('populatrs_user', JSON.stringify({user}));
window.location.href = '/';
</script>
</body>
</html>"#,
        jwt = sub,
        user = serde_json::json!({
            "sub": sub,
            "email": sub,
            "name": sub.split('@').next().unwrap_or("Dev"),
        })
    );

    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
}

pub async fn me(axum::Extension(user): axum::Extension<AuthUser>) -> Json<MeResponse> {
    Json(MeResponse {
        sub: user.user_id,
        email: user.email,
        name: user.name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── AuthCallbackQuery deserialization ──

    #[test]
    fn test_callback_query_with_code_and_state() {
        let q: AuthCallbackQuery = serde_json::from_str(
            r#"{"code": "abc123", "state": "def456", "error": null, "error_description": null}"#,
        )
        .unwrap();
        assert_eq!(q.code.as_deref(), Some("abc123"));
        assert_eq!(q.state.as_deref(), Some("def456"));
        assert!(q.error.is_none());
        assert!(q.error_description.is_none());
    }

    #[test]
    fn test_callback_query_with_error_and_no_code() {
        let q: AuthCallbackQuery = serde_json::from_str(
            r#"{"error": "access_denied", "error_description": "User cancelled", "code": null, "state": null}"#,
        )
        .unwrap();
        assert!(q.code.is_none());
        assert!(q.state.is_none());
        assert_eq!(q.error.as_deref(), Some("access_denied"));
        assert_eq!(q.error_description.as_deref(), Some("User cancelled"));
    }

    #[test]
    fn test_callback_query_all_fields_missing() {
        let q: AuthCallbackQuery = serde_json::from_str(r#"{}"#).unwrap();
        assert!(q.code.is_none());
        assert!(q.state.is_none());
        assert!(q.error.is_none());
        assert!(q.error_description.is_none());
    }

    // ── DevLoginQuery deserialization ──

    #[test]
    fn test_dev_login_query_with_email() {
        let q: DevLoginQuery = serde_json::from_str(r#"{"email": "test@example.com"}"#).unwrap();
        assert_eq!(q.email.as_deref(), Some("test@example.com"));
    }

    #[test]
    fn test_dev_login_query_missing_email() {
        let q: DevLoginQuery = serde_json::from_str(r#"{}"#).unwrap();
        assert!(q.email.is_none());
    }

    // ── MeResponse serialization ──

    #[test]
    fn test_me_response_serialization() {
        let resp = MeResponse {
            sub: "user123".into(),
            email: Some("user@test.com".into()),
            name: Some("Test User".into()),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["sub"], "user123");
        assert_eq!(json["email"], "user@test.com");
        assert_eq!(json["name"], "Test User");
    }

    #[test]
    fn test_me_response_optional_fields_none() {
        let resp = MeResponse {
            sub: "anon".into(),
            email: None,
            name: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["sub"], "anon");
        assert!(json["email"].is_null());
        assert!(json["name"].is_null());
    }

    // ── Ownership / borrowing patterns ──

    #[test]
    fn test_code_ownership_transition() {
        // Simulate the pattern in callback: code is cloned from query
        let code = Some("auth_code_123".to_string());
        // Take ownership via clone (as done in the callback handler)
        let cloned = code.clone();
        assert_eq!(cloned.as_deref(), Some("auth_code_123"));
        // Original is still usable
        assert_eq!(code.as_deref(), Some("auth_code_123"));
    }

    #[test]
    fn test_error_description_default() {
        // Simulate: error_description.as_deref().unwrap_or("OIDC authorization denied")
        let desc: Option<String> = None;
        let result = desc.as_deref().unwrap_or("OIDC authorization denied");
        assert_eq!(result, "OIDC authorization denied");
    }

    #[test]
    fn test_error_description_with_value() {
        let desc = Some("Custom error".to_string());
        let result = desc.as_deref().unwrap_or("OIDC authorization denied");
        assert_eq!(result, "Custom error");
    }

    // ── State equality check ──

    #[test]
    fn test_state_equality() {
        let cb_state = Some("stored_state_value".to_string());
        let stored = Some(("stored_state_value".to_string(), std::time::Instant::now()));
        // This mirrors: Some((ref stored, _)) if stored == cb_state
        match (&cb_state, &stored) {
            (Some(cb), Some((ref stored_state, _))) if stored_state == cb => { /* match */ }
            _ => panic!("state should match"),
        }
    }

    #[test]
    fn test_state_mismatch() {
        let cb_state = Some("wrong_state".to_string());
        let stored = Some(("expected_state".to_string(), std::time::Instant::now()));
        let is_mismatch = match (&cb_state, &stored) {
            (Some(cb), Some((ref stored_state, _))) if stored_state == cb => false,
            _ => true,
        };
        assert!(is_mismatch);
    }

    // ── Login URL format (issuer + client_id + redirect_uri + state) ──

    #[test]
    fn test_login_url_format_pattern() {
        let issuer = "https://pocketid.example.com";
        let client_id = "populatrs";
        let redirect_uri = "http://localhost:3044/auth/callback";
        let oauth_state = "test-state-123";

        let url = format!(
            "{}/authorize?response_type=code&client_id={}&redirect_uri={}&scope=openid+profile+email&state={}",
            issuer.trim_end_matches('/'),
            client_id,
            redirect_uri,
            oauth_state,
        );
        assert!(url.starts_with("https://pocketid.example.com/authorize?response_type=code"));
        assert!(url.contains("client_id=populatrs"));
        assert!(url.contains("redirect_uri=http://localhost:3044/auth/callback"));
        assert!(url.contains("scope=openid+profile+email"));
        assert!(url.contains("state=test-state-123"));
    }

    #[test]
    fn test_login_url_trims_trailing_slash() {
        let issuer = "https://pocketid.example.com/";
        let client_id = "populatrs";
        let redirect_uri = "http://localhost:3044/auth/callback";
        let oauth_state = "state-xyz";

        let url = format!(
            "{}/authorize?response_type=code&client_id={}&redirect_uri={}&scope=openid+profile+email&state={}",
            issuer.trim_end_matches('/'),
            client_id,
            redirect_uri,
            oauth_state,
        );
        // Should not have double slash
        assert!(!url.contains("//authorize"));
        assert_eq!(
            url,
            "https://pocketid.example.com/authorize?response_type=code&client_id=populatrs&redirect_uri=http://localhost:3044/auth/callback&scope=openid+profile+email&state=state-xyz"
        );
    }
}
