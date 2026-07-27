use super::Publisher;
use crate::db::Database;
use crate::models::{Post, PublisherConfig, TemplateContext, TemplateRenderer};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::time;
use url::Url;
use uuid::Uuid;

pub struct ThreadsPublisher {
    #[allow(dead_code)]
    pub id: String,
    pub client_id: String,
    pub client_secret: String,
    pub access_token: Arc<tokio::sync::Mutex<Option<String>>>,
    pub user_id: Option<String>,
    pub redirect_uri: String,
    pub template: String,
    client: Client,
    renderer: TemplateRenderer,
    pub db: Option<Arc<Database>>,
    pub token_expires_at: Arc<tokio::sync::Mutex<Option<i64>>>,
}

impl ThreadsPublisher {
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        client_id: String,
        client_secret: String,
        access_token: Option<String>,
        user_id: Option<String>,
        redirect_uri: Option<String>,
        template: String,
        db: Option<Arc<Database>>,
        token_expires_at: Option<i64>,
    ) -> Self {
        let redirect_uri = redirect_uri.unwrap_or_else(|| "https://127.0.0.1".to_string());

        Self {
            id,
            client_id,
            client_secret,
            access_token: Arc::new(tokio::sync::Mutex::new(access_token)),
            user_id,
            redirect_uri,
            template,
            client: Client::new(),
            renderer: TemplateRenderer::new(),
            db,
            token_expires_at: Arc::new(tokio::sync::Mutex::new(token_expires_at)),
        }
    }

    /// Generates a Meta Threads OAuth 2.0 authorization URL.
    ///
    /// Scopes: `threads_basic,threads_content_publish`
    pub fn generate_auth_url(&self, state: Option<String>) -> String {
        let state = state.unwrap_or_else(|| Uuid::new_v4().to_string());
        let scope = "threads_basic,threads_content_publish";

        let mut url = Url::parse("https://www.threads.net/oauth/authorize").unwrap();
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("scope", scope)
            .append_pair("state", &state);

        url.to_string()
    }

    /// Exchanges the authorization code for access_token and user_id.
    ///
    /// Meta's Threads API returns: access_token, user_id
    pub async fn exchange_code_for_tokens(
        &self,
        code: &str,
    ) -> Result<(String, Option<String>, u64)> {
        let payload = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &self.redirect_uri),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
        ];

        tracing::info!("Exchanging authorization code for Threads tokens");

        let response = self
            .client
            .post("https://graph.threads.net/oauth/access_token")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&payload)
            .send()
            .await?;

        let status = response.status();
        tracing::info!("Threads token exchange response status: {}", status);

        if status.is_success() {
            let token_data: Value = response.json().await?;

            let access_token = token_data["access_token"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("No access_token in response"))?
                .to_string();

            let user_id = token_data.get("user_id").and_then(|v| {
                v.as_str()
                    .map(|s| s.to_string())
                    .or_else(|| v.as_u64().map(|n| n.to_string()))
            });

            let expires_in = token_data["expires_in"].as_u64().unwrap_or(3600);

            tracing::info!(
                "Successfully obtained Threads tokens - expires in: {}s, user_id: {:?}",
                expires_in,
                user_id
            );

            Ok((access_token, user_id, expires_in))
        } else {
            let error_body = response.text().await.unwrap_or_default();
            tracing::error!("Threads token exchange failed: {}", error_body);
            Err(anyhow::anyhow!(
                "Failed to exchange code for tokens: {} - {}",
                status,
                error_body
            ))
        }
    }

    /// Exchange a short-lived Threads token for a long-lived one (60 days).
    /// Meta endpoint: GET /access_token?grant_type=th_exchange_token
    pub(crate) async fn exchange_for_long_lived_token(
        &self,
        short_lived: &str,
    ) -> Result<(String, u64)> {
        let url = "https://graph.threads.net/access_token";
        let response = self
            .client
            .get(url)
            .query(&[
                ("grant_type", "th_exchange_token"),
                ("client_secret", &self.client_secret),
                ("access_token", short_lived),
            ])
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to exchange Threads token for long-lived: {} - {}",
                status,
                body
            ));
        }

        let data: serde_json::Value = response.json().await?;
        let long_lived = data["access_token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No access_token in long-lived exchange response"))?
            .to_string();
        let expires_in = data["expires_in"].as_u64().unwrap_or(5184000);

        tracing::info!(
            "Exchanged Threads token for long-lived — expires in {}s",
            expires_in
        );

        Ok((long_lived, expires_in))
    }

    /// Persist the current access_token and expiry to the database.
    async fn save_tokens_to_config(&self) -> Result<()> {
        let Some(ref db) = self.db else {
            tracing::warn!("No database reference — Threads tokens not persisted");
            return Ok(());
        };

        let access_token = {
            let guard = self.access_token.lock().await;
            guard
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Cannot save Threads config: no access token"))?
        };

        let expires_at = {
            let guard = self.token_expires_at.lock().await;
            *guard
        };

        let config = PublisherConfig::Threads {
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            access_token: Some(access_token),
            user_id: self.user_id.clone(),
            redirect_uri: Some(self.redirect_uri.clone()),
            template: self.template.clone(),
            token_expires_at: expires_at,
        };
        db.upsert_publisher(&self.id, &config, true).await?;
        tracing::info!("Persisted Threads tokens to database for '{}'", self.id);
        Ok(())
    }

    /// Return a valid access token, exchanging for long-lived if expired/missing.
    async fn get_valid_access_token(&self) -> Result<String> {
        let now = chrono::Utc::now().timestamp();

        // Check current state
        {
            let token_guard = self.access_token.lock().await;
            let expires_guard = self.token_expires_at.lock().await;

            if let Some(ref token) = *token_guard {
                match *expires_guard {
                    Some(expires_at) if now < expires_at - 3600 => {
                        // Still valid with at least 1 hour buffer
                        return Ok(token.clone());
                    }
                    Some(expires_at) => {
                        // Token exists but is expired or near-expiry, try exchange
                        tracing::info!(
                            "Threads token expired at {}, attempting long-lived exchange",
                            expires_at
                        );
                    }
                    None => {
                        // No expiry info — token was stored before the fix.
                        // It's already long-lived (OAuth callback always exchanges),
                        // so assume it's still valid and use it directly.
                        // We return Ok here instead of attempting exchange to avoid
                        // the 452 error (exchanging a long-lived token as short-lived).
                        tracing::debug!(
                            "Threads token has no expiry info, assuming long-lived and using as-is"
                        );
                        return Ok(token.clone());
                    }
                }
            }
        }

        // Need to exchange (token missing or expired)
        let current_token = {
            let guard = self.access_token.lock().await;
            guard
                .clone()
                .ok_or_else(|| anyhow::anyhow!("No Threads access token available"))?
        };

        let (new_token, expires_in) = self.exchange_for_long_lived_token(&current_token).await?;

        // Update in-memory state
        {
            let mut token_guard = self.access_token.lock().await;
            *token_guard = Some(new_token.clone());
        }
        {
            let mut expires_guard = self.token_expires_at.lock().await;
            *expires_guard = Some(now + expires_in as i64);
        }

        // Persist to DB
        if let Err(e) = self.save_tokens_to_config().await {
            tracing::warn!("Failed to persist Threads tokens: {}", e);
        }

        Ok(new_token)
    }
}

#[async_trait]
impl Publisher for ThreadsPublisher {
    async fn publish(&self, post: &Post, feed_template: Option<&str>) -> Result<String> {
        let template_str = feed_template
            .filter(|t| !t.is_empty())
            .unwrap_or(&self.template);

        // Get a valid (long-lived) access token
        let access_token = self.get_valid_access_token().await?;

        let user_id = self
            .user_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No user id"))?;

        // Threads API uses a two-step process: create container, then publish

        // Step 1: Create media container
        let container_url = format!("https://graph.threads.net/v1.0/{}/threads", user_id);

        let context = TemplateContext {
            title: post.title.clone(),
            description: post.description.clone().unwrap_or_default(),
            url: post.url.clone(),
        };

        let text = self.renderer.render(template_str, &context)?;

        // Threads has a character limit of 500
        let text = if text.chars().count() > 500 {
            format!("{}...", text.chars().take(497).collect::<String>())
        } else {
            text
        };

        let container_payload = json!({
            "media_type": "TEXT",
            "text": text,
        });

        let container_response = self
            .client
            .post(&container_url)
            .query(&[("access_token", access_token.as_str())])
            .json(&container_payload)
            .send()
            .await?;

        if !container_response.status().is_success() {
            let status = container_response.status();
            let error_text = container_response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to create Threads container: {} - {}",
                status,
                error_text
            ));
        }

        let container_result: Value = container_response.json().await?;
        let container_id = container_result["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No container ID in Threads response"))?;

        tracing::info!("Created Threads container: {}", container_id);

        // Small delay to ensure the container is ready for publishing
        // Threads API sometimes needs time to process the container
        time::sleep(std::time::Duration::from_millis(2000)).await;

        // Step 2: Publish the container
        let publish_url = format!("https://graph.threads.net/v1.0/{}/threads_publish", user_id);

        let publish_payload = json!({
            "creation_id": container_id,
        });

        tracing::info!("Publishing Threads container: {}", container_id);

        let publish_response = self
            .client
            .post(&publish_url)
            .query(&[("access_token", access_token.as_str())])
            .json(&publish_payload)
            .send()
            .await?;

        if publish_response.status().is_success() {
            let result: Value = publish_response.json().await?;
            Ok(format!(
                "Published to Threads: {}",
                result["id"].as_str().unwrap_or("unknown")
            ))
        } else {
            let status = publish_response.status();
            let error_text = publish_response.text().await.unwrap_or_default();

            // If the container doesn't exist, maybe we need to wait longer
            if error_text.contains("does not exist") || error_text.contains("No se encuentra") {
                tracing::warn!(
                    "Container {} not found, trying again after delay...",
                    container_id
                );

                // Wait a bit more and try once more
                time::sleep(std::time::Duration::from_millis(3000)).await;

                let retry_response = self
                    .client
                    .post(&publish_url)
                    .query(&[("access_token", access_token.as_str())])
                    .json(&publish_payload)
                    .send()
                    .await?;

                if retry_response.status().is_success() {
                    let result: Value = retry_response.json().await?;
                    Ok(format!(
                        "Published to Threads (retry): {}",
                        result["id"].as_str().unwrap_or("unknown")
                    ))
                } else {
                    let retry_status = retry_response.status();
                    let retry_error = retry_response.text().await.unwrap_or_default();
                    Err(anyhow::anyhow!(
                        "Failed to publish to Threads after retry: {} - {}",
                        retry_status,
                        retry_error
                    ))
                }
            } else {
                Err(anyhow::anyhow!(
                    "Failed to publish to Threads: {} - {}",
                    status,
                    error_text
                ))
            }
        }
    }

    fn get_type(&self) -> &'static str {
        "threads"
    }

    fn get_id(&self) -> &str {
        &self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
