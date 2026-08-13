use super::Publisher;
use crate::db::Database;
use crate::models::{Post, TemplateContext, TemplateRenderer};

use anyhow::Result;
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;
use uuid::Uuid;

use crate::models::PublisherConfig;

pub struct XPublisher {
    pub id: String,
    pub client_id: String,
    pub client_secret: String,
    pub access_token: Arc<Mutex<Option<String>>>,
    pub refresh_token: Arc<Mutex<Option<String>>>,
    pub redirect_uri: String,
    pub template: String,
    pub reply_template: String,
    client: Client,
    renderer: TemplateRenderer,
    pub config_file_path: Option<String>,
    pub db: Option<Arc<Database>>,
}

impl XPublisher {
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        client_id: String,
        client_secret: String,
        access_token: Option<String>,
        refresh_token: Option<String>,
        redirect_uri: Option<String>,
        template: String,
        reply_template: Option<String>,
        config_file_path: Option<String>,
        db: Option<Arc<Database>>,
    ) -> Self {
        let redirect_uri = redirect_uri.unwrap_or_else(|| "https://127.0.0.1".to_string());
        let reply_template =
            reply_template.unwrap_or_else(|| "Puedes verlo en {{ url }}".to_string());

        Self {
            id,
            client_id,
            client_secret,
            access_token: Arc::new(Mutex::new(access_token)),
            refresh_token: Arc::new(Mutex::new(refresh_token)),
            redirect_uri,
            template,
            reply_template,
            client: Client::new(),
            renderer: TemplateRenderer::new(),
            config_file_path,
            db,
        }
    }

    /// Genera la URL de autorización OAuth 2.0 PKCE para X/Twitter
    pub fn generate_auth_url(&self, state: Option<String>) -> (String, String) {
        let state = state.unwrap_or_else(|| Uuid::new_v4().to_string());
        let code_verifier = "challenge"; // En producción debería ser random
        let code_challenge = code_verifier; // Para method=plain
        let scope = "tweet.read tweet.write users.read offline.access";

        let mut url = Url::parse("https://twitter.com/i/oauth2/authorize").unwrap();
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("scope", scope)
            .append_pair("state", &state)
            .append_pair("code_challenge", code_challenge)
            .append_pair("code_challenge_method", "plain");

        (url.to_string(), code_verifier.to_string())
    }

    /// Intercambia el código de autorización por access_token y refresh_token
    pub async fn exchange_code_for_tokens(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<(String, Option<String>, u64)> {
        let auth_header = format!(
            "Basic {}",
            general_purpose::STANDARD.encode(format!("{}:{}", self.client_id, self.client_secret))
        );

        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &self.redirect_uri),
            ("code_verifier", code_verifier),
        ];

        tracing::info!("Exchanging authorization code for X tokens using OAuth 2.0 PKCE");

        let response = self
            .client
            .post("https://api.twitter.com/2/oauth2/token")
            .header("Authorization", auth_header)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await?;

        let status = response.status();
        tracing::info!("X token exchange response status: {}", status);

        if status.is_success() {
            let token_data: Value = response.json().await?;

            let access_token = token_data["access_token"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("No access_token in response"))?
                .to_string();

            let expires_in = token_data["expires_in"].as_u64().unwrap_or(7200);

            let refresh_token = token_data
                .get("refresh_token")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            // Actualizar tokens en memoria
            {
                let mut access_guard = self.access_token.lock().await;
                *access_guard = Some(access_token.clone());
            }

            if let Some(ref rt) = refresh_token {
                let mut refresh_guard = self.refresh_token.lock().await;
                *refresh_guard = Some(rt.clone());
            }

            tracing::info!(
                "Successfully obtained X tokens - expires in: {}s",
                expires_in
            );
            if refresh_token.is_some() {
                tracing::info!("Refresh token obtained");
            }

            Ok((access_token, refresh_token, expires_in))
        } else {
            let error_body = response.text().await.unwrap_or_default();
            tracing::error!("X token exchange failed: {}", error_body);
            Err(anyhow::anyhow!(
                "Failed to exchange code for X tokens: {} - {}",
                status,
                error_body
            ))
        }
    }

    /// Comando para setup interactivo de OAuth
    pub async fn oauth_setup(&self) -> Result<()> {
        println!("\n🐦 X/Twitter OAuth 2.0 PKCE Setup");
        println!("==================================");

        let (auth_url, code_verifier) = self.generate_auth_url(None);
        println!("\n1. Abre esta URL en tu navegador:");
        println!("{}", auth_url);

        println!("\n2. Autoriza la aplicación y copia el código de la URL de retorno");
        println!("3. Pega el código aquí:");

        let mut code = String::new();
        std::io::stdin().read_line(&mut code)?;
        let code = code.trim();

        if code.is_empty() {
            return Err(anyhow::anyhow!("Código no proporcionado"));
        }

        let (access_token, refresh_token, expires_in) =
            self.exchange_code_for_tokens(code, &code_verifier).await?;

        println!("\n✅ Tokens obtenidos exitosamente!");
        println!("Access Token: {}", access_token);
        println!("Expira en: {} segundos", expires_in);

        if let Some(rt) = &refresh_token {
            println!("Refresh Token: {}", rt);
        }

        // Guardar en configuración
        self.save_tokens_to_config(&access_token, refresh_token.as_deref())
            .await?;
        println!("\n💾 Tokens guardados en configuración");

        Ok(())
    }

    /// Obtener token de acceso usando refresh token (OAuth 2.0)
    async fn refresh_access_token(&self) -> Result<(String, String)> {
        let refresh_token = {
            let token_guard = self.refresh_token.lock().await;
            token_guard
                .clone()
                .ok_or_else(|| anyhow::anyhow!("No refresh token available"))?
        };

        let auth_header = format!(
            "Basic {}",
            general_purpose::STANDARD.encode(format!("{}:{}", self.client_id, self.client_secret))
        );

        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", &refresh_token),
        ];

        tracing::info!("Refreshing X access token using OAuth 2.0");

        let response = self
            .client
            .post("https://api.twitter.com/2/oauth2/token")
            .header("Authorization", auth_header)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await?;

        let status = response.status();
        tracing::info!("X OAuth 2.0 token refresh response status: {}", status);

        if status.is_success() {
            let result: Value = response.json().await?;

            let new_access_token = result["access_token"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("No access_token in response"))?
                .to_string();

            let new_refresh_token = result
                .get("refresh_token")
                .and_then(|v| v.as_str())
                .unwrap_or(&refresh_token) // Si no viene nuevo refresh_token, usar el actual
                .to_string();

            // Actualizar tokens en memoria
            {
                let mut access_guard = self.access_token.lock().await;
                *access_guard = Some(new_access_token.clone());
            }
            {
                let mut refresh_guard = self.refresh_token.lock().await;
                *refresh_guard = Some(new_refresh_token.clone());
            }

            tracing::info!("Successfully refreshed X access token");
            Ok((new_access_token, new_refresh_token))
        } else {
            let error_body = response.text().await.unwrap_or_default();
            tracing::error!("X OAuth 2.0 token refresh failed: {}", error_body);
            Err(anyhow::anyhow!(
                "Failed to refresh X token: {} - {}",
                status,
                error_body
            ))
        }
    }

    /// Obtener token de acceso válido, renovándolo si es necesario
    async fn get_valid_access_token(&self) -> Result<String> {
        {
            let access_guard = self.access_token.lock().await;
            if let Some(token) = access_guard.as_ref() {
                return Ok(token.clone());
            }
        }

        // Si no hay access token, intentar renovar
        let (new_access_token, new_refresh_token) = self.refresh_access_token().await?;

        // Guardar tokens actualizados en configuración si es posible
        if let Err(e) = self
            .save_tokens_to_config(&new_access_token, Some(&new_refresh_token))
            .await
        {
            tracing::warn!("Failed to save updated tokens to config: {}", e);
        }

        Ok(new_access_token)
    }

    /// Guardar tokens actualizados en la configuración (persiste a DB)
    pub async fn save_tokens_to_config(
        &self,
        access_token: &str,
        refresh_token: Option<&str>,
    ) -> Result<()> {
        tracing::info!("Saving X tokens to config for '{}'", self.id);

        if let Some(ref db) = self.db {
            let config = PublisherConfig::X {
                client_id: self.client_id.clone(),
                client_secret: self.client_secret.clone(),
                access_token: Some(access_token.to_string()),
                refresh_token: refresh_token.map(|s| s.to_string()),
                redirect_uri: Some(self.redirect_uri.clone()),
                template: self.template.clone(),
                reply_template: Some(self.reply_template.clone()),
            };
            db.upsert_publisher(&self.id, &config, true).await?;
            tracing::info!("Persisted X tokens to database for '{}'", self.id);
        } else {
            tracing::warn!(
                "No database reference — X tokens for '{}' only updated in memory",
                self.id
            );
        }

        Ok(())
    }

    /// Post a single tweet (text-only) and return the tweet ID.
    async fn post_tweet(&self, access_token: &str, text: &str) -> Result<String> {
        let url = "https://api.twitter.com/2/tweets";
        let tweet_data = json!({ "text": text });

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .json(&tweet_data)
            .send()
            .await?;

        let status = response.status();
        tracing::info!("X post tweet response status: {}", status);

        if status.is_success() {
            let result: Value = response.json().await?;
            let tweet_id = result["data"]["id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("No tweet ID in response"))?
                .to_string();
            Ok(tweet_id)
        } else {
            let error_body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Failed to post tweet: {} - {}",
                status,
                error_body
            ))
        }
    }

    /// Post a reply tweet (in reply to an existing tweet) and return the tweet ID.
    async fn post_reply(
        &self,
        access_token: &str,
        text: &str,
        in_reply_to_id: &str,
    ) -> Result<String> {
        let url = "https://api.twitter.com/2/tweets";
        let reply_data = json!({
            "text": text,
            "reply": {
                "in_reply_to_tweet_id": in_reply_to_id
            }
        });

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .json(&reply_data)
            .send()
            .await?;

        let status = response.status();
        tracing::info!("X post reply response status: {}", status);

        if status.is_success() {
            let result: Value = response.json().await?;
            let reply_id = result["data"]["id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("No reply ID in response"))?
                .to_string();
            Ok(reply_id)
        } else {
            let error_body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Failed to post reply: {} - {}",
                status,
                error_body
            ))
        }
    }

    /// Attempt to post a tweet with automatic token refresh on 401.
    async fn post_tweet_with_retry(&self, access_token: &str, text: &str) -> Result<String> {
        match self.post_tweet(access_token, text).await {
            Ok(tweet_id) => Ok(tweet_id),
            Err(e) => {
                // Check if the error is a 401 (unauthorized) — try refreshing token
                let err_msg = e.to_string();
                if err_msg.contains("401") {
                    tracing::info!("Access token expired, attempting to refresh...");
                    let (new_token, new_refresh) = self.refresh_access_token().await?;
                    if let Err(e) = self
                        .save_tokens_to_config(&new_token, Some(&new_refresh))
                        .await
                    {
                        tracing::warn!("Failed to save refreshed tokens: {}", e);
                    }
                    self.post_tweet(&new_token, text).await
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Attempt to post a reply with automatic token refresh on 401.
    async fn post_reply_with_retry(
        &self,
        access_token: &str,
        text: &str,
        in_reply_to_id: &str,
    ) -> Result<String> {
        match self.post_reply(access_token, text, in_reply_to_id).await {
            Ok(reply_id) => Ok(reply_id),
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("401") {
                    tracing::info!("Access token expired, attempting to refresh...");
                    let (new_token, new_refresh) = self.refresh_access_token().await?;
                    if let Err(e) = self
                        .save_tokens_to_config(&new_token, Some(&new_refresh))
                        .await
                    {
                        tracing::warn!("Failed to save refreshed tokens: {}", e);
                    }
                    self.post_reply(&new_token, text, in_reply_to_id).await
                } else {
                    Err(e)
                }
            }
        }
    }
}
#[async_trait]
impl Publisher for XPublisher {
    async fn publish(&self, post: &Post, feed_template: Option<&str>) -> Result<String> {
        // ── 1. Render main tweet text (without URL) ──
        let template_str = feed_template
            .filter(|t| !t.is_empty())
            .unwrap_or(&self.template);
        let context = TemplateContext {
            title: post.title.clone(),
            description: post.description.clone().unwrap_or_default(),
            url: post.url.clone(),
        };

        let main_text = self.renderer.render(template_str, &context)?;

        // Truncate to Twitter's character limit
        let main_text = if main_text.len() > 280 {
            format!("{}...", &main_text[..277])
        } else {
            main_text
        };

        tracing::info!("X two-step publish — main tweet: '{}'", main_text);

        // ── 2. Get valid access token ──
        let access_token = match self.get_valid_access_token().await {
            Ok(token) => token,
            Err(e) => {
                tracing::error!("Failed to get valid access token: {}", e);
                return Err(e);
            }
        };

        // ── 3. Post main tweet (text only) ──
        let main_tweet_id = match self.post_tweet_with_retry(&access_token, &main_text).await {
            Ok(id) => id,
            Err(e) => {
                tracing::error!(
                    publisher_id = %self.id,
                    text_sent = %main_text,
                    "Failed to publish main tweet to X"
                );
                return Err(e);
            }
        };

        tracing::info!("Main tweet published successfully — ID: {}", main_tweet_id);

        // ── 4. Render reply text (CTA + URL) ──
        let reply_context = TemplateContext {
            title: post.title.clone(),
            description: post.description.clone().unwrap_or_default(),
            url: post.url.clone(),
        };
        let reply_text = self.renderer.render(&self.reply_template, &reply_context)?;

        // Truncate reply to Twitter's character limit
        let reply_text = if reply_text.len() > 280 {
            format!("{}...", &reply_text[..277])
        } else {
            reply_text
        };

        tracing::info!(
            "X two-step publish — reply tweet: '{}' (in reply to: {})",
            reply_text,
            main_tweet_id
        );

        // ── 5. Small delay to ensure sequential execution ──
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // ── 6. Post reply with URL ──
        match self
            .post_reply_with_retry(&access_token, &reply_text, &main_tweet_id)
            .await
        {
            Ok(reply_id) => {
                tracing::info!("Reply tweet published successfully — ID: {}", reply_id);
                Ok(format!(
                    "Published to X: main={}, reply={}",
                    main_tweet_id, reply_id
                ))
            }
            Err(e) => {
                // Step 1 succeeded but Step 2 failed — log main_tweet_id for retry
                tracing::error!(
                    publisher_id = %self.id,
                    main_tweet_id = %main_tweet_id,
                    reply_text = %reply_text,
                    error = %e,
                    "Failed to publish reply tweet. Main tweet ID preserved for retry."
                );
                Ok(format!(
                    "Published to X (reply failed): main={}, error={}",
                    main_tweet_id, e
                ))
            }
        }
    }

    fn get_type(&self) -> &'static str {
        "x"
    }

    fn get_id(&self) -> &str {
        &self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
