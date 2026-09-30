use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect},
    Json,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::instrument;

use crate::auth::{AppState, AuthUser, OidcMetadata};
use crate::db::Database;

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
    expires_in: u64,
    id_token: Option<String>,
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserInfoResponse {
    sub: String,
    email: Option<String>,
    name: Option<String>,
}

/// Genera un code_verifier de 48 bytes aleatorios → 64 chars base64url sin padding.
fn generate_code_verifier() -> String {
    let mut bytes = [0u8; 48];
    getrandom::fill(&mut bytes).expect("RNG failure");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Calcula el code_challenge S256: SHA256(verifier) → base64url sin padding.
fn compute_code_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash)
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
        let code_verifier = generate_code_verifier();
        let code_challenge = compute_code_challenge(&code_verifier);
        {
            let mut states = state.oidc_states.lock().await;
            states.insert(
                "oidc:login".to_string(),
                (oauth_state.clone(), code_verifier, Instant::now()),
            );
        }

        let url = format!(
            "{}/authorize?response_type=code&client_id={}&redirect_uri={}&scope=openid+profile+email&state={}&code_challenge_method=S256&code_challenge={}",
            issuer.trim_end_matches('/'),
            client_id,
            redirect_uri,
            oauth_state,
            code_challenge,
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
    let code_verifier: Option<String> = if let Some(ref cb_state) = query.state {
        let stored_state = state.oidc_states.lock().await.remove("oidc:login");
        match stored_state {
            Some((ref stored, ref verifier, _)) if stored == cb_state => Some(verifier.clone()),
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
                None
            }
        }
    } else {
        tracing::warn!("OIDC callback without state parameter");
        None
    };

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
    let mut params = vec![
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
        ("client_secret", client_secret),
    ];
    if let Some(ref cv) = code_verifier {
        params.push(("code_verifier", cv));
    }

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
    let jwt = token_data
        .id_token
        .clone()
        .unwrap_or(token_data.access_token);

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

    // Guardar refresh_token si está presente
    if let Some(ref refresh_token) = token_data.refresh_token {
        let user_id = user_info
            .as_ref()
            .map(|u| u.sub.clone())
            .unwrap_or_else(|| {
                // Fallback: extraer sub del id_token (sin async aquí)
                token_data
                    .id_token
                    .as_deref()
                    .and_then(extract_sub_from_jwt)
                    .unwrap_or_default()
            });
        if !user_id.is_empty() {
            state
                .db
                .save_refresh_token(&user_id, refresh_token, token_data.expires_in)
                .await
                .map_err(|e| tracing::warn!("Failed to save refresh token: {}", e))
                .ok();
        }
    }

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

#[derive(Debug, Serialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub expires_in: u64,
}

/// Refresca el access_token usando el refresh_token almacenado en SQLite.
///
/// 1. Extrae el user_id del token JWT actual (aunque esté expirado, se decodifica
///    sin verificar firma para obtener el `sub`)
/// 2. Lee el refresh_token de SQLite
/// 3. Lo canjea en PocketID por un nuevo access_token + refresh_token (rotación)
/// 4. Guarda el nuevo refresh_token
/// 5. Devuelve el nuevo access_token
///
/// En modo dev (sin OIDC), devuelve un token de desarrollo.
#[instrument(skip(state, headers))]
pub async fn refresh_token(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<RefreshResponse>, StatusCode> {
    // Modo dev: devolver token de desarrollo
    if state.jwt_validator.is_dev() {
        return Ok(Json(RefreshResponse {
            access_token: "dev-token".to_string(),
            expires_in: 3600,
        }));
    }

    // Extraer user_id del token JWT (puede estar expirado — validamos firma pero no expiración)
    let auth_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate the JWT (signature + issuer + audience) but allow expired tokens
    let claims = state
        .jwt_validator
        .validate_token_ignore_expiry(token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let user_id = claims.sub;

    // 1. Obtener refresh_token de SQLite
    let refresh_token = state
        .db
        .get_refresh_token(&user_id)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // 2. Canjear en PocketID
    let issuer = state
        .config
        .oidc_issuer_url
        .as_deref()
        .ok_or(StatusCode::BAD_GATEWAY)?;
    let token_url = format!("{}/api/oidc/token", issuer.trim_end_matches('/'));
    let client_id = state.config.oidc_client_id.as_deref().unwrap_or("");
    let client_secret = state.config.oidc_client_secret.as_deref().unwrap_or("");

    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", &refresh_token),
        ("client_id", client_id),
        ("client_secret", client_secret),
    ];

    let client = reqwest::Client::new();
    let resp = client
        .post(&token_url)
        .form(&params)
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    if !resp.status().is_success() {
        // Do NOT delete the stored token here. The compare-and-swap below is the
        // only owner of rotation, and logout deletes explicitly. Deleting on a
        // failed exchange could kill a concurrent refresh that already succeeded:
        // with two concurrent refreshes A and B reading R, if A exchanges
        // successfully and B fails, B deleting R would make A's CAS fail and
        // return 401, ending a session that was just renewed. An invalid token is
        // overwritten on the next login and expires on its own.
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token_data: TokenResponse = resp.json().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    // 3. Persist the rotated refresh token (compare-and-swap). This runs on every
    //    successful exchange, even when the provider returns no new refresh token,
    //    so a concurrent logout that removed the row always rejects the request.
    //    If logout removed the row or a concurrent refresh rotated it, the CAS
    //    returns false and we reject with 401.
    let persisted = persist_rotated_token(
        &state.db,
        &user_id,
        &refresh_token,
        token_data.refresh_token.as_deref(),
        token_data.expires_in,
    )
    .await
    .map_err(|e| {
        // A database failure (lock, disk) is transient, not an expired session.
        // Map it to 500 so the client retries instead of clearing the token and
        // forcing a logout. Only a failed CAS (`persisted == false`) means 401.
        tracing::warn!("Failed to save refresh token: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    if !persisted {
        tracing::warn!(
            user_id = %user_id,
            "refresh rotation rejected: stored token changed or was revoked"
        );
        return Err(StatusCode::UNAUTHORIZED);
    }

    // 4. Devolver nuevo access_token
    Ok(Json(RefreshResponse {
        access_token: token_data.access_token,
        expires_in: token_data.expires_in,
    }))
}

/// Persist the refresh token after a successful exchange (rotation).
///
/// Always performs a compare-and-swap against the token that was read before
/// the exchange, so a concurrent logout (which deletes the row) or a concurrent
/// refresh (which rotates it) wins over this request.
///
/// When the provider returns a new refresh token it is stored; otherwise the
/// same token is re-saved with a renewed expiry, which still verifies that the
/// row was not removed concurrently.
///
/// Returns `Ok(true)` when the row was updated, `Ok(false)` when the stored
/// token changed or was removed, and `Err` on a database failure.
async fn persist_rotated_token(
    db: &Database,
    user_id: &str,
    old_token: &str,
    new_token: Option<&str>,
    expires_in: u64,
) -> anyhow::Result<bool> {
    // When the provider does not issue a new refresh token, re-save the same
    // one with a renewed expiry. The CAS still verifies the row is current, so
    // a concurrent logout or rotation is not resurrected.
    let token_to_store = new_token.unwrap_or(old_token);
    db.save_refresh_token_if_current(user_id, old_token, token_to_store, expires_in)
        .await
}

/// Extrae el `sub` de un JWT sin verificar la firma.
/// Solo usado en el callback OIDC como fallback cuando userinfo falla;
/// el token acaba de ser emitido por el proveedor, no hay riesgo de suplantación.
fn extract_sub_from_jwt(token: &str) -> Option<String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    use base64::Engine;
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1])
        .ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&payload).ok()?;
    claims.get("sub")?.as_str().map(|s| s.to_string())
}

#[derive(Debug, Serialize)]
pub struct LogoutResponse {
    pub end_session_url: Option<String>,
}

/// Build the RP-initiated logout URL by appending `client_id` and, when
/// available, `post_logout_redirect_uri` to the provider's
/// `end_session_endpoint`.
///
/// Fails closed: returns `None` when the endpoint is not a valid absolute URL
/// or its scheme is not `http`/`https`. Any query parameters already present on
/// the endpoint are preserved.
pub fn build_end_session_url(
    endpoint: &str,
    client_id: &str,
    post_logout_redirect_uri: Option<&str>,
) -> Option<String> {
    let mut url = url::Url::parse(endpoint).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("client_id", client_id);
        if let Some(redirect) = post_logout_redirect_uri {
            pairs.append_pair("post_logout_redirect_uri", redirect);
        }
    }
    Some(url.to_string())
}

/// Derive the post-logout redirect URI from the OIDC redirect URL by keeping
/// its origin and pointing at `/login`.
///
/// Returns `None` when `redirect_url` is not a valid absolute URL.
pub fn derive_post_logout_redirect_uri(redirect_url: &str) -> Option<String> {
    let url = url::Url::parse(redirect_url).ok()?;
    let origin = url.origin().ascii_serialization();
    if origin == "null" {
        return None;
    }
    Some(format!("{}/login", origin.trim_end_matches('/')))
}

/// Revoke the user's server-side refresh token and build the logout response.
///
/// Extracted from the `logout` handler so it can be tested without constructing
/// a full `AppState` / real JWT validator. A storage error is logged but never
/// fails the logout.
async fn revoke_and_build_logout(
    db: &Database,
    sub: &str,
    metadata: Option<&OidcMetadata>,
    client_id: &str,
    redirect_url: Option<&str>,
) -> LogoutResponse {
    if let Err(e) = db.delete_refresh_token(sub).await {
        tracing::warn!("Failed to delete refresh token on logout: {}", e);
    }

    let end_session_url = metadata
        .and_then(|m| m.end_session_endpoint.as_deref())
        .and_then(|endpoint| {
            let post_logout_redirect_uri = redirect_url.and_then(derive_post_logout_redirect_uri);
            build_end_session_url(endpoint, client_id, post_logout_redirect_uri.as_deref())
        });

    LogoutResponse { end_session_url }
}

/// Terminate the session: revoke the caller's server-side refresh token and,
/// when the provider advertises an `end_session_endpoint`, return the
/// RP-initiated logout URL for the client to redirect to.
///
/// The bearer token is validated ignoring expiration so logout still works with
/// an expired access token (the refresh token must not survive).
#[instrument(skip(state, headers))]
pub async fn logout(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<LogoutResponse>, StatusCode> {
    // Dev mode has no server-side session to revoke.
    if state.jwt_validator.is_dev() {
        return Ok(Json(LogoutResponse {
            end_session_url: None,
        }));
    }

    let auth_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let claims = state
        .jwt_validator
        .validate_token_ignore_expiry(token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let client_id = state.config.oidc_client_id.as_deref().unwrap_or("");
    let response = revoke_and_build_logout(
        &state.db,
        &claims.sub,
        state.oidc_metadata.as_ref(),
        client_id,
        state.config.oidc_redirect_url.as_deref(),
    )
    .await;

    Ok(Json(response))
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
        let stored = Some((
            "stored_state_value".to_string(),
            "verifier123".to_string(),
            std::time::Instant::now(),
        ));
        // This mirrors: Some((ref stored, _, _)) if stored == cb_state
        match (&cb_state, &stored) {
            (Some(cb), Some((ref stored_state, _, _))) if stored_state == cb => { /* match */ }
            _ => panic!("state should match"),
        }
    }

    #[test]
    fn test_state_mismatch() {
        let cb_state = Some("wrong_state".to_string());
        let stored = Some((
            "expected_state".to_string(),
            "verifier456".to_string(),
            std::time::Instant::now(),
        ));
        let is_mismatch = match (&cb_state, &stored) {
            (Some(cb), Some((ref stored_state, _, _))) if stored_state == cb => false,
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

    // ── PKCE helpers ──

    #[test]
    fn test_generate_code_verifier_length() {
        let verifier = generate_code_verifier();
        // 48 bytes → base64url sin padding = 64 caracteres
        assert_eq!(verifier.len(), 64);
        // Solo caracteres base64url (letras, dígitos, -, _)
        assert!(verifier
            .chars()
            .all(|c| { c.is_ascii_alphanumeric() || c == '-' || c == '_' }));
        // Dos llamadas deberían producir valores distintos (aleatoriedad)
        assert_ne!(verifier, generate_code_verifier());
    }

    #[test]
    fn test_compute_code_challenge() {
        let verifier = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        let challenge = compute_code_challenge(verifier);
        assert!(!challenge.is_empty());
        assert!(challenge
            .chars()
            .all(|c| { c.is_ascii_alphanumeric() || c == '-' || c == '_' }));
        // Mismo verifier → mismo challenge (determinismo)
        assert_eq!(
            compute_code_challenge(verifier),
            compute_code_challenge(verifier)
        );
    }

    #[test]
    fn test_login_url_includes_pkce_params() {
        let issuer = "https://pocketid.example.com";
        let client_id = "populatrs";
        let redirect_uri = "http://localhost:3044/auth/callback";
        let oauth_state = "test-state-123";
        let code_challenge = "test-challenge-value";

        let url = format!(
            "{}/authorize?response_type=code&client_id={}&redirect_uri={}&scope=openid+profile+email&state={}&code_challenge_method=S256&code_challenge={}",
            issuer.trim_end_matches('/'),
            client_id,
            redirect_uri,
            oauth_state,
            code_challenge,
        );
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("code_challenge=test-challenge-value"));
        assert!(url.starts_with("https://pocketid.example.com/authorize?"));
    }

    // ── LogoutResponse serialization ──

    #[test]
    fn test_logout_response_with_end_session_url() {
        let resp = LogoutResponse {
            end_session_url: Some("https://idp.example.com/logout".into()),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["end_session_url"], "https://idp.example.com/logout");
    }

    #[test]
    fn test_logout_response_without_end_session_url() {
        let resp = LogoutResponse {
            end_session_url: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["end_session_url"].is_null());
    }

    // ── build_end_session_url ──

    #[test]
    fn test_build_end_session_url_appends_params() {
        let url = build_end_session_url(
            "https://idp.example.com/logout",
            "populatrs",
            Some("https://app.example.com/login"),
        )
        .expect("valid https endpoint should build");
        assert!(url.starts_with("https://idp.example.com/logout?"));
        assert!(url.contains("client_id=populatrs"));
        // post_logout_redirect_uri must be URL-encoded
        assert!(url.contains("post_logout_redirect_uri=https%3A%2F%2Fapp.example.com%2Flogin"));
    }

    #[test]
    fn test_build_end_session_url_preserves_existing_query() {
        let url = build_end_session_url(
            "https://idp.example.com/logout?foo=bar",
            "populatrs",
            Some("https://app.example.com/login"),
        )
        .expect("valid https endpoint should build");
        assert!(url.contains("foo=bar"));
        assert!(url.contains("client_id=populatrs"));
        assert!(url.contains("post_logout_redirect_uri=https%3A%2F%2Fapp.example.com%2Flogin"));
    }

    #[test]
    fn test_build_end_session_url_omits_redirect_when_none() {
        let url = build_end_session_url("https://idp.example.com/logout", "populatrs", None)
            .expect("valid https endpoint should build");
        assert!(url.contains("client_id=populatrs"));
        assert!(!url.contains("post_logout_redirect_uri"));
    }

    #[test]
    fn test_build_end_session_url_rejects_protocol_relative() {
        assert!(build_end_session_url("//evil.com/logout", "populatrs", None).is_none());
    }

    #[test]
    fn test_build_end_session_url_rejects_javascript_scheme() {
        assert!(build_end_session_url("javascript:alert(1)", "populatrs", None).is_none());
    }

    #[test]
    fn test_build_end_session_url_rejects_unparseable() {
        assert!(build_end_session_url("not a url", "populatrs", None).is_none());
    }

    // ── derive_post_logout_redirect_uri ──

    #[test]
    fn test_derive_post_logout_redirect_uri_from_callback() {
        let derived = derive_post_logout_redirect_uri("https://app.example.com/auth/callback");
        assert_eq!(derived.as_deref(), Some("https://app.example.com/login"));
    }

    #[test]
    fn test_derive_post_logout_redirect_uri_preserves_port() {
        let derived = derive_post_logout_redirect_uri("http://localhost:3044/auth/callback");
        assert_eq!(derived.as_deref(), Some("http://localhost:3044/login"));
    }

    #[test]
    fn test_derive_post_logout_redirect_uri_invalid_returns_none() {
        assert!(derive_post_logout_redirect_uri("not a url").is_none());
        assert!(derive_post_logout_redirect_uri("").is_none());
    }

    // ── revoke_and_build_logout (logout handler core) ──

    async fn test_db() -> (Database, tempfile::TempDir) {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test.db");
        let db = Database::open(&path).await.unwrap();
        (db, dir)
    }

    fn metadata_with_endpoint(endpoint: Option<&str>) -> OidcMetadata {
        OidcMetadata {
            issuer: "https://idp.example.com".into(),
            authorization_endpoint: Some("https://idp.example.com/authorize".into()),
            token_endpoint: Some("https://idp.example.com/token".into()),
            userinfo_endpoint: Some("https://idp.example.com/userinfo".into()),
            jwks_uri: Some("https://idp.example.com/jwks".into()),
            end_session_endpoint: endpoint.map(|s| s.to_string()),
        }
    }

    #[tokio::test]
    async fn test_revoke_and_build_logout_deletes_token_and_returns_url() {
        let (db, _dir) = test_db().await;
        db.save_refresh_token("user-1", "refresh-tok", 3600)
            .await
            .unwrap();
        let meta = metadata_with_endpoint(Some("https://idp.example.com/logout"));

        let resp = revoke_and_build_logout(
            &db,
            "user-1",
            Some(&meta),
            "populatrs",
            Some("https://app.example.com/auth/callback"),
        )
        .await;

        // Row revoked from storage.
        assert!(db.get_refresh_token("user-1").await.unwrap().is_none());
        // URL built with encoded post-logout redirect.
        let url = resp
            .end_session_url
            .expect("end_session_url should be present");
        assert!(url.contains("client_id=populatrs"));
        assert!(url.contains("post_logout_redirect_uri=https%3A%2F%2Fapp.example.com%2Flogin"));
    }

    #[tokio::test]
    async fn test_revoke_and_build_logout_without_endpoint_returns_none() {
        let (db, _dir) = test_db().await;
        db.save_refresh_token("user-2", "refresh-tok", 3600)
            .await
            .unwrap();
        let meta = metadata_with_endpoint(None);

        let resp = revoke_and_build_logout(
            &db,
            "user-2",
            Some(&meta),
            "populatrs",
            Some("https://app.example.com/auth/callback"),
        )
        .await;

        assert!(resp.end_session_url.is_none());
        // Token is still revoked even without a provider endpoint.
        assert!(db.get_refresh_token("user-2").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_revoke_and_build_logout_no_stored_token_is_idempotent() {
        let (db, _dir) = test_db().await;
        let meta = metadata_with_endpoint(Some("https://idp.example.com/logout"));

        let resp = revoke_and_build_logout(
            &db,
            "ghost-user",
            Some(&meta),
            "populatrs",
            Some("https://app.example.com/auth/callback"),
        )
        .await;

        // No stored token: must not error, and the URL is still built.
        assert!(resp.end_session_url.is_some());
    }

    #[tokio::test]
    async fn test_revoke_and_build_logout_omits_underivable_redirect() {
        let (db, _dir) = test_db().await;
        let meta = metadata_with_endpoint(Some("https://idp.example.com/logout"));

        let resp =
            revoke_and_build_logout(&db, "user-3", Some(&meta), "populatrs", Some("not a url"))
                .await;

        let url = resp
            .end_session_url
            .expect("end_session_url should be present");
        assert!(url.contains("client_id=populatrs"));
        // Underivable redirect must be omitted, not passed through raw.
        assert!(!url.contains("post_logout_redirect_uri"));
    }

    #[tokio::test]
    async fn test_revoke_and_build_logout_malformed_endpoint_returns_none() {
        let (db, _dir) = test_db().await;
        let meta = metadata_with_endpoint(Some("javascript:alert(1)"));

        let resp = revoke_and_build_logout(
            &db,
            "user-4",
            Some(&meta),
            "populatrs",
            Some("https://app.example.com/auth/callback"),
        )
        .await;

        assert!(resp.end_session_url.is_none());
    }

    // ── Logout-during-refresh race (storage level) ──

    #[tokio::test]
    async fn test_refresh_rotation_not_persisted_when_logout_removes_token() {
        let (db, _dir) = test_db().await;
        // A refresh token is stored for the user.
        db.save_refresh_token("user-race", "original-token", 3600)
            .await
            .unwrap();

        // The refresh exchange reads the stored token...
        let read_token = db
            .get_refresh_token("user-race")
            .await
            .unwrap()
            .expect("token should be stored");

        // ...then logout removes it before the rotation is persisted.
        db.delete_refresh_token("user-race").await.unwrap();

        // The compare-and-swap rotation must not resurrect the revoked session.
        let persisted = db
            .save_refresh_token_if_current("user-race", &read_token, "rotated-token", 3600)
            .await
            .unwrap();

        assert!(!persisted);
        assert!(db.get_refresh_token("user-race").await.unwrap().is_none());
    }

    // ── persist_rotated_token (post-exchange persistence decision) ──

    #[tokio::test]
    async fn test_persist_rotated_token_with_new_token_returns_true() {
        let (db, _dir) = test_db().await;
        db.save_refresh_token("user-persist-1", "old-token", 3600)
            .await
            .unwrap();

        let persisted =
            persist_rotated_token(&db, "user-persist-1", "old-token", Some("new-token"), 3600)
                .await
                .unwrap();

        assert!(persisted);
        assert_eq!(
            db.get_refresh_token("user-persist-1")
                .await
                .unwrap()
                .as_deref(),
            Some("new-token")
        );
    }

    #[tokio::test]
    async fn test_persist_rotated_token_without_new_token_resaves_same() {
        let (db, _dir) = test_db().await;
        db.save_refresh_token("user-persist-2", "same-token", 3600)
            .await
            .unwrap();

        let persisted = persist_rotated_token(&db, "user-persist-2", "same-token", None, 3600)
            .await
            .unwrap();

        assert!(persisted);
        assert_eq!(
            db.get_refresh_token("user-persist-2")
                .await
                .unwrap()
                .as_deref(),
            Some("same-token")
        );
    }

    #[tokio::test]
    async fn test_persist_rotated_token_returns_false_when_row_removed() {
        let (db, _dir) = test_db().await;
        db.save_refresh_token("user-persist-3", "old-token", 3600)
            .await
            .unwrap();
        db.delete_refresh_token("user-persist-3").await.unwrap();

        let persisted =
            persist_rotated_token(&db, "user-persist-3", "old-token", Some("new-token"), 3600)
                .await
                .unwrap();

        assert!(!persisted);
        assert!(db
            .get_refresh_token("user-persist-3")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn test_persist_rotated_token_without_new_token_returns_false_when_row_removed() {
        let (db, _dir) = test_db().await;
        db.save_refresh_token("user-persist-4", "old-token", 3600)
            .await
            .unwrap();
        db.delete_refresh_token("user-persist-4").await.unwrap();

        // The provider issued no new refresh token, so the same token is re-saved.
        // The CAS must still reject because the row was revoked concurrently.
        let persisted = persist_rotated_token(&db, "user-persist-4", "old-token", None, 3600)
            .await
            .unwrap();

        assert!(!persisted);
        assert!(db
            .get_refresh_token("user-persist-4")
            .await
            .unwrap()
            .is_none());
    }
}
