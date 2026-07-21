use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Redirect},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::auth::{AppState, AuthUser};

#[derive(Debug, Deserialize)]
pub struct AuthCallbackQuery {
    pub code: String,
    pub state: Option<String>,
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
pub async fn login(State(state): State<Arc<AppState>>) -> Redirect {
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
        let url = format!(
            "{}/authorize?response_type=code&client_id={}&redirect_uri={}&scope=openid+profile+email",
            issuer.trim_end_matches('/'),
            client_id,
            redirect_uri,
        );
        tracing::info!("redirecting to OIDC provider: {}", url);
        Redirect::to(&url)
    } else {
        Redirect::to("/auth/dev-login")
    }
}

#[instrument(skip(state))]
pub async fn callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuthCallbackQuery>,
) -> impl IntoResponse {
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
        ("code", &query.code),
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
