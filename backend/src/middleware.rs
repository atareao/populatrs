use crate::auth::{AppState, AuthUser};
use crate::config::SessionTimeouts;
use crate::db::Database;
use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use chrono::{DateTime, Utc};
use std::sync::Arc;

/// Enforce the server-authoritative session timeouts for `user_id`.
///
/// Returns `Err(StatusCode::UNAUTHORIZED)` when no session exists or when the
/// stored session has passed either its idle or absolute deadline. An expired
/// session is revoked best-effort before returning; a revocation failure is
/// logged and never propagated.
pub async fn enforce_session_timeouts(
    db: &Database,
    user_id: &str,
    now: DateTime<Utc>,
    timeouts: &SessionTimeouts,
) -> Result<(), StatusCode> {
    let session = match db.get_session(user_id).await {
        Ok(Some(session)) => session,
        Ok(None) => return Err(StatusCode::UNAUTHORIZED),
        Err(e) => {
            // A storage failure is transient, not an expired session. Surface it
            // as a server error so the client retries instead of logging out.
            tracing::warn!(user_id = %user_id, error = %e, "failed to load session");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    if session.is_expired(now, timeouts.idle_seconds, timeouts.absolute_seconds) {
        if let Err(e) = db.delete_refresh_token(user_id).await {
            tracing::warn!(user_id = %user_id, error = %e, "failed to revoke expired session");
        }
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(())
}

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

    // Enforce server-authoritative session lifetimes (idle + absolute) after the
    // token is validated and before the user is admitted.
    enforce_session_timeouts(
        &state.db,
        &claims.sub,
        Utc::now(),
        &state.config.session_timeouts,
    )
    .await?;

    let user = AuthUser {
        user_id: claims.sub,
        email: claims.email,
        name: claims.name.or(claims.preferred_username),
    };

    req.extensions_mut().insert(user);
    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SessionTimeouts;
    use crate::db::Database;
    use chrono::Utc;

    async fn test_db() -> (Database, tempfile::TempDir) {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test.db");
        let db = Database::open(&path).await.unwrap();
        (db, dir)
    }

    #[tokio::test]
    async fn test_enforce_missing_session_returns_401() {
        let (db, _dir) = test_db().await;
        let result =
            enforce_session_timeouts(&db, "ghost-user", Utc::now(), &SessionTimeouts::default())
                .await;
        assert_eq!(result, Err(StatusCode::UNAUTHORIZED));
    }

    #[tokio::test]
    async fn test_enforce_idle_expired_revokes_and_returns_401() {
        let (db, _dir) = test_db().await;
        let now = Utc::now();
        db.create_session(
            "user-idle",
            "tok",
            3600,
            now - chrono::Duration::seconds(1801),
        )
        .await
        .unwrap();

        let result =
            enforce_session_timeouts(&db, "user-idle", now, &SessionTimeouts::default()).await;
        assert_eq!(result, Err(StatusCode::UNAUTHORIZED));
        assert!(db.get_session("user-idle").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_enforce_absolute_expired_revokes_even_with_recent_activity() {
        let (db, _dir) = test_db().await;
        let now = Utc::now();
        db.create_session(
            "user-abs",
            "tok",
            3600,
            now - chrono::Duration::seconds(14401),
        )
        .await
        .unwrap();
        db.touch_activity("user-abs", now).await.unwrap();

        let result =
            enforce_session_timeouts(&db, "user-abs", now, &SessionTimeouts::default()).await;
        assert_eq!(result, Err(StatusCode::UNAUTHORIZED));
        assert!(db.get_session("user-abs").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_enforce_valid_session_passes() {
        let (db, _dir) = test_db().await;
        let now = Utc::now();
        db.create_session("user-ok", "tok", 3600, now)
            .await
            .unwrap();

        let result =
            enforce_session_timeouts(&db, "user-ok", now, &SessionTimeouts::default()).await;
        assert_eq!(result, Ok(()));
    }

    #[tokio::test]
    async fn test_enforce_does_not_update_last_activity() {
        let (db, _dir) = test_db().await;
        let now = Utc::now();
        // Still valid, but only 5 seconds from its idle deadline.
        let t0 = now - chrono::Duration::seconds(1795);
        db.create_session("user-near", "tok", 3600, t0)
            .await
            .unwrap();

        let result =
            enforce_session_timeouts(&db, "user-near", now, &SessionTimeouts::default()).await;
        assert_eq!(result, Ok(()));

        // The ordinary request must not have extended the idle window.
        let session = db.get_session("user-near").await.unwrap().unwrap();
        assert_eq!(session.last_activity_at, t0.to_rfc3339());
    }
}
