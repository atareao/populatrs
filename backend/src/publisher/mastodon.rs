use super::Publisher;
use crate::models::{Post, TemplateContext, TemplateRenderer};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use url::Url;
use uuid::Uuid;

pub struct MastodonPublisher {
    pub id: String,
    pub server_url: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub access_token: Option<String>,
    pub redirect_uri: String,
    pub template: String,
    client: Client,
    renderer: TemplateRenderer,
}

impl MastodonPublisher {
    pub fn new(
        id: String,
        server_url: String,
        client_id: Option<String>,
        client_secret: Option<String>,
        access_token: Option<String>,
        redirect_uri: Option<String>,
        template: String,
    ) -> Self {
        let redirect_uri = redirect_uri
            .unwrap_or_else(|| "http://localhost:3044/oauth/callback".to_string())
            // Auto-append /oauth/callback path if the user only provided a base URL
            .trim_end_matches('/')
            .to_string();
        let redirect_uri = if redirect_uri.ends_with("/oauth/callback") {
            redirect_uri
        } else {
            format!("{}/oauth/callback", redirect_uri)
        };

        Self {
            id,
            server_url,
            client_id,
            client_secret,
            access_token,
            redirect_uri,
            template,
            client: Client::new(),
            renderer: TemplateRenderer::new(),
        }
    }

    /// Register this application on the Mastodon server.
    ///
    /// POST `{server_url}/api/v1/apps` with:
    /// - client_name = "populatrs"
    /// - redirect_uris = self.redirect_uri
    /// - scopes = "read write push" (compatible con todas las versiones de Mastodon)
    ///
    /// Returns `(client_id, client_secret)` on success.
    pub async fn register_app(&self) -> Result<(String, String)> {
        let url = format!("{}/api/v1/apps", self.server_url.trim_end_matches('/'));

        let payload = json!({
            "client_name": "populatrs",
            "redirect_uris": self.redirect_uri,
            "scopes": "read write push"
        });

        tracing::info!("Registering Mastodon app at {}", self.server_url);

        let response = self.client.post(&url).json(&payload).send().await?;

        let status = response.status();
        if status.is_success() {
            let result: Value = response.json().await?;
            let client_id = result["client_id"]
                .as_str()
                .ok_or_else(|| {
                    anyhow::anyhow!("No client_id in Mastodon app registration response")
                })?
                .to_string();
            let client_secret = result["client_secret"]
                .as_str()
                .ok_or_else(|| {
                    anyhow::anyhow!("No client_secret in Mastodon app registration response")
                })?
                .to_string();
            tracing::info!(
                "Successfully registered Mastodon app: client_id={}",
                client_id
            );
            Ok((client_id, client_secret))
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Failed to register Mastodon app: {} - {}",
                status,
                body
            ))
        }
    }

    /// Generate the OAuth authorization URL for the Mastodon server.
    ///
    /// GET `{server_url}/oauth/authorize` with:
    /// - response_type = code
    /// - client_id
    /// - redirect_uri
    /// - scope = "write read" (compatible con todas las versiones de Mastodon)
    /// - state (optional, auto-generated if not provided)
    pub fn generate_auth_url(&self, state: Option<String>) -> String {
        let state = state.unwrap_or_else(|| Uuid::new_v4().to_string());
        let scope = "read write push";

        let mut url = Url::parse(&format!(
            "{}/oauth/authorize",
            self.server_url.trim_end_matches('/')
        ))
        .expect("Invalid Mastodon server URL");

        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", self.client_id.as_deref().unwrap_or_default())
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("scope", scope)
            .append_pair("state", &state);

        url.to_string()
    }

    /// Exchange the authorization code for an access token.
    ///
    /// POST `{server_url}/oauth/token` with form-encoded params:
    /// - grant_type = authorization_code
    /// - code
    /// - client_id
    /// - client_secret
    /// - redirect_uri
    ///
    /// Mastodon returns a long-lived `access_token` with no expiration.
    /// Returns `(access_token, None, 0)` where None = no refresh token.
    pub async fn exchange_code_for_tokens(
        &self,
        code: &str,
    ) -> Result<(String, Option<String>, u64)> {
        let url = format!("{}/oauth/token", self.server_url.trim_end_matches('/'));

        let client_id = self
            .client_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("No client_id configured"))?;
        let client_secret = self
            .client_secret
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("No client_secret configured"))?;

        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("redirect_uri", &self.redirect_uri),
        ];

        tracing::info!("Exchanging authorization code for Mastodon tokens");

        let response = self.client.post(&url).form(&params).send().await?;

        let status = response.status();
        if status.is_success() {
            let token_data: Value = response.json().await?;
            let access_token = token_data["access_token"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("No access_token in Mastodon token response"))?
                .to_string();

            tracing::info!("Successfully obtained Mastodon access token");
            // Mastodon access tokens are long-lived with no refresh token
            Ok((access_token, None, 0))
        } else {
            let error_body = response.text().await.unwrap_or_default();
            tracing::error!("Mastodon token exchange failed: {}", error_body);
            Err(anyhow::anyhow!(
                "Failed to exchange code for Mastodon tokens: {} - {}",
                status,
                error_body
            ))
        }
    }
}

#[async_trait]
impl Publisher for MastodonPublisher {
    async fn publish(&self, post: &Post, feed_template: Option<&str>) -> Result<String> {
        let template_str = feed_template
            .filter(|t| !t.is_empty())
            .unwrap_or(&self.template);
        let url = format!("{}/api/v1/statuses", self.server_url.trim_end_matches('/'));

        let token = self
            .access_token
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No access token"))?;

        let context = TemplateContext {
            title: post.title.clone(),
            description: post.description.clone().unwrap_or_default(),
            url: post.url.clone(),
        };

        let status = self.renderer.render(template_str, &context)?;

        let payload = json!({
            "status": status,
            "visibility": "public"
        });

        let response = self
            .client
            .post(&url)
            .bearer_auth(token)
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let result: Value = response.json().await?;
            Ok(format!("Published to Mastodon: {}", result["id"]))
        } else {
            Err(anyhow::anyhow!(
                "Failed to publish to Mastodon: {}",
                response.status()
            ))
        }
    }

    fn get_type(&self) -> &'static str {
        "mastodon"
    }

    fn get_id(&self) -> &str {
        &self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
