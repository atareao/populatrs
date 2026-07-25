use std::sync::Arc;

use base64::Engine;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};

use crate::config::Config;
use crate::db::Database;
use crate::models::PublisherManager;

// ───── Log Entry (broadcast via SSE) ─────

/// A single log entry sent via SSE to the LogsPage.
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
    pub target: String,
}

// ───── OIDC Discovery ─────

#[derive(Debug, Clone, Deserialize)]
pub struct OidcMetadata {
    pub issuer: String,
    pub authorization_endpoint: Option<String>,
    pub token_endpoint: Option<String>,
    pub userinfo_endpoint: Option<String>,
    pub jwks_uri: Option<String>,
}

// ───── JWKS ─────

#[derive(Clone, Debug, Deserialize)]
pub struct Jwk {
    pub kty: String,
    pub kid: Option<String>,
    pub n: Option<String>,
    pub e: Option<String>,
    pub alg: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct JwksResponse {
    pub keys: Vec<Jwk>,
}

// ───── OIDC State ─────

pub type OidcStates =
    Arc<tokio::sync::Mutex<std::collections::HashMap<String, (String, std::time::Instant)>>>;

// ───── JWT Validator ─────

pub struct JwtValidator {
    jwks: Arc<RwLock<Vec<DecodingKey>>>,
    issuer: String,
    client_id: String,
    is_dev: bool,
}

impl JwtValidator {
    pub fn new(issuer: &str, client_id: &str) -> Self {
        Self {
            jwks: Arc::new(RwLock::new(Vec::new())),
            issuer: issuer.to_string(),
            client_id: client_id.to_string(),
            is_dev: false,
        }
    }

    pub fn dev() -> Self {
        Self {
            jwks: Arc::new(RwLock::new(Vec::new())),
            issuer: "http://localhost:8765".into(),
            client_id: "populatrs".into(),
            is_dev: true,
        }
    }

    pub fn is_dev(&self) -> bool {
        self.is_dev
    }

    pub async fn fetch_jwks(&self, issuer: &str) -> Result<(), String> {
        let jwks_url = format!("{}/.well-known/jwks.json", issuer.trim_end_matches('/'));
        let resp: JwksResponse = reqwest::get(&jwks_url)
            .await
            .map_err(|e| format!("failed to fetch JWKS: {e}"))?
            .json()
            .await
            .map_err(|e| format!("failed to parse JWKS: {e}"))?;

        let mut keys = Vec::new();
        for jwk in &resp.keys {
            if let (Some(n), Some(e)) = (&jwk.n, &jwk.e) {
                let n_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .decode(n)
                    .map_err(|e| format!("invalid JWK n: {e}"))?;
                let e_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .decode(e)
                    .map_err(|e| format!("invalid JWK e: {e}"))?;
                let key = DecodingKey::from_rsa_raw_components(&n_bytes, &e_bytes);
                keys.push(key);
            }
        }
        tracing::info!(count = keys.len(), "JWKS fetched");
        *self.jwks.write().await = keys;
        Ok(())
    }

    pub async fn validate_token(&self, token: &str) -> Result<JwtClaims, String> {
        let keys = {
            let jwks = self.jwks.read().await;
            if jwks.is_empty() {
                drop(jwks);
                self.fetch_jwks(&self.issuer).await?;
                return Box::pin(self.validate_token(token)).await;
            }
            jwks.clone()
        };

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.client_id]);

        for key in &keys {
            if let Ok(claims) = jsonwebtoken::decode::<JwtClaims>(token, key, &validation) {
                return Ok(claims.claims);
            }
        }
        Err("no matching JWK found for token".to_string())
    }
}

// ───── JWT Claims ─────

#[derive(Clone, Debug, Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub preferred_username: Option<String>,
}

// ───── AppState (shared state) ─────

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: Database,
    pub oidc_metadata: Option<OidcMetadata>,
    pub jwt_validator: Arc<JwtValidator>,
    pub oidc_states: OidcStates,
    pub oauth_states: OidcStates,
    /// Broadcast sender for log entries (SSE to LogsPage).
    pub log_tx: broadcast::Sender<LogEntry>,
    pub publisher_manager: Arc<PublisherManager>,
}

impl AppState {
    pub fn new(
        config: Config,
        db: Database,
        log_tx: broadcast::Sender<LogEntry>,
        publisher_manager: Arc<PublisherManager>,
    ) -> Self {
        let jwt_validator = if config.oidc_configured() {
            JwtValidator::new(
                config.oidc_issuer_url.as_deref().unwrap_or(""),
                config.oidc_client_id.as_deref().unwrap_or(""),
            )
        } else {
            JwtValidator::dev()
        };

        Self {
            config,
            db,
            oidc_metadata: None,
            jwt_validator: Arc::new(jwt_validator),
            oidc_states: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            oauth_states: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            log_tx,
            publisher_manager,
        }
    }
}

// Ensure Sync + Send for shared state
fn _assert_send_sync()
where
    AppState: Send + Sync,
{
}

// ───── AuthUser (extracted from validated token) ─────

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user_id: String,
    pub email: Option<String>,
    pub name: Option<String>,
}
