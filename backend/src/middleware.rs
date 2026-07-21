use crate::auth::{AppState, AuthUser};
use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use std::sync::Arc;

/// Dev mode auth: accepts any token, uses default dev user.
/// Production mode: validates Bearer token via OIDC/JWKS.
pub async fn require_auth(mut req: Request, next: Next) -> Result<Response, StatusCode> {
    // Check if we're in dev mode — access state from the request
    // Since we can't extract State directly in `from_fn`, we use an extension
    let is_dev = req
        .extensions()
        .get::<Arc<AppState>>()
        .map(|state| state.jwt_validator.is_dev())
        .unwrap_or(true);

    if is_dev {
        let user = AuthUser {
            user_id: "dev-user".to_string(),
            email: Some("dev@populatrs.app".to_string()),
            name: Some("Developer".to_string()),
        };
        req.extensions_mut().insert(user);
        return Ok(next.run(req).await);
    }

    // Production mode: validate Bearer token
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Get state from extension (inserted by a wrapper in main.rs)
    let state = req
        .extensions()
        .get::<Arc<AppState>>()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let claims = state
        .jwt_validator
        .validate_token(token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let user = AuthUser {
        user_id: claims.sub,
        email: claims.email,
        name: claims.name.or(claims.preferred_username),
    };

    req.extensions_mut().insert(user);
    Ok(next.run(req).await)
}
