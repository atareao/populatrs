use super::Publisher;
use crate::models::{Post, TemplateContext, TemplateRenderer};
use crate::storage::StorageManager;
use anyhow::Result;
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;
use uuid::Uuid;

pub struct XPublisher {
    pub id: String,
    pub client_id: String,
    pub client_secret: String,
    pub access_token: Arc<Mutex<Option<String>>>,
    pub refresh_token: Arc<Mutex<Option<String>>>,
    pub redirect_uri: String,
    pub template: String,
    client: Client,
    renderer: TemplateRenderer,
    pub config_file_path: Option<String>,
}

impl XPublisher {
    pub fn new(
        id: String,
        client_id: String,
        client_secret: String,
        access_token: Option<String>,
        refresh_token: Option<String>,
        redirect_uri: Option<String>,
        template: String,
        config_file_path: Option<String>,
    ) -> Self {
        let redirect_uri = redirect_uri.unwrap_or_else(|| "https://127.0.0.1".to_string());

        Self {
            id,
            client_id,
            client_secret,
            access_token: Arc::new(Mutex::new(access_token)),
            refresh_token: Arc::new(Mutex::new(refresh_token)),
            redirect_uri,
            template,
            client: Client::new(),
            renderer: TemplateRenderer::new(),
            config_file_path,
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

        log::info!("Exchanging authorization code for X tokens using OAuth 2.0 PKCE");

        let response = self
            .client
            .post("https://api.twitter.com/2/oauth2/token")
            .header("Authorization", auth_header)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await?;

        let status = response.status();
        log::info!("X token exchange response status: {}", status);

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

            log::info!(
                "Successfully obtained X tokens - expires in: {}s",
                expires_in
            );
            if refresh_token.is_some() {
                log::info!("Refresh token obtained");
            }

            Ok((access_token, refresh_token, expires_in))
        } else {
            let error_body = response.text().await.unwrap_or_default();
            log::error!("X token exchange failed: {}", error_body);
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

        log::info!("Refreshing X access token using OAuth 2.0");

        let response = self
            .client
            .post("https://api.twitter.com/2/oauth2/token")
            .header("Authorization", auth_header)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await?;

        let status = response.status();
        log::info!("X OAuth 2.0 token refresh response status: {}", status);

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

            log::info!("Successfully refreshed X access token");
            Ok((new_access_token, new_refresh_token))
        } else {
            let error_body = response.text().await.unwrap_or_default();
            log::error!("X OAuth 2.0 token refresh failed: {}", error_body);
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
            log::warn!("Failed to save updated tokens to config: {}", e);
        }

        Ok(new_access_token)
    }

    /// Guardar tokens actualizados en la configuración
    async fn save_tokens_to_config(
        &self,
        access_token: &str,
        refresh_token: Option<&str>,
    ) -> Result<()> {
        if let Some(config_path) = &self.config_file_path {
            // Cargar configuración actual
            let mut config = StorageManager::load_config_from_file(config_path)?;

            // Actualizar el publisher X específico
            if let Some(publisher_config) = config.publishers.get_mut(&self.id) {
                if let crate::models::config::PublisherConfig::X {
                    access_token: ref mut at,
                    refresh_token: ref mut rt,
                    ..
                } = publisher_config
                {
                    *at = Some(access_token.to_string());
                    *rt = refresh_token.map(|t| t.to_string());
                }
            }

            // Guardar configuración actualizada
            StorageManager::save_config_to_file(&config, config_path)?;
            log::info!("Updated X tokens in configuration file");
        }
        Ok(())
    }
}
#[async_trait]
impl Publisher for XPublisher {
    async fn publish(&self, post: &Post) -> Result<String> {
        let context = TemplateContext {
            title: post.title.clone(),
            description: post.description.clone().unwrap_or_default(),
            url: post.link.clone(),
        };

        let tweet_text = self.renderer.render(&self.template, &context)?;

        // Truncate to Twitter's character limit
        let tweet_text = if tweet_text.len() > 280 {
            format!("{}...", &tweet_text[..277])
        } else {
            tweet_text
        };

        log::info!(
            "Attempting to publish to X with OAuth 2.0: '{}'",
            tweet_text
        );

        // Obtener token de acceso válido
        let access_token = match self.get_valid_access_token().await {
            Ok(token) => token,
            Err(e) => {
                log::error!("Failed to get valid access token: {}", e);
                return Err(e);
            }
        };

        // Usar X API v2 con OAuth 2.0
        let url = "https://api.twitter.com/2/tweets";

        let tweet_data = json!({
            "text": tweet_text
        });

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .json(&tweet_data)
            .send()
            .await?;

        let status = response.status();
        log::info!("X API v2 OAuth 2.0 response status: {}", status);

        if status.is_success() {
            let result: Value = response.json().await?;
            let tweet_id = result["data"]["id"].as_str().unwrap_or("unknown");
            Ok(format!("Published to X: {}", tweet_id))
        } else {
            let error_body = response.text().await.unwrap_or_default();
            log::error!("X API v2 OAuth 2.0 Error Response: {}", error_body);

            // Si el error es de autenticación, intentar renovar token
            if status.as_u16() == 401 {
                log::info!("Access token expired, attempting to refresh...");

                match self.refresh_access_token().await {
                    Ok((new_access_token, new_refresh_token)) => {
                        // Guardar tokens actualizados
                        if let Err(e) = self
                            .save_tokens_to_config(&new_access_token, Some(&new_refresh_token))
                            .await
                        {
                            log::warn!("Failed to save refreshed tokens: {}", e);
                        }

                        // Reintentar publicación con nuevo token
                        let retry_response = self
                            .client
                            .post(url)
                            .header("Authorization", format!("Bearer {}", new_access_token))
                            .header("Content-Type", "application/json")
                            .json(&tweet_data)
                            .send()
                            .await?;

                        let retry_status = retry_response.status();
                        log::info!("X API v2 retry response status: {}", retry_status);

                        if retry_status.is_success() {
                            let result: Value = retry_response.json().await?;
                            let tweet_id = result["data"]["id"].as_str().unwrap_or("unknown");
                            Ok(format!(
                                "Published to X (after token refresh): {}",
                                tweet_id
                            ))
                        } else {
                            let error_body = retry_response.text().await.unwrap_or_default();
                            log::error!("X API v2 retry failed: {}", error_body);
                            Err(anyhow::anyhow!(
                                "Failed to publish to X after token refresh: {} - {}",
                                retry_status,
                                error_body
                            ))
                        }
                    }
                    Err(refresh_error) => {
                        log::error!("Failed to refresh X token: {}", refresh_error);
                        Err(anyhow::anyhow!(
                            "Failed to publish to X - token refresh failed: {}",
                            refresh_error
                        ))
                    }
                }
            } else {
                // Parse error para mejor diagnóstico
                if let Ok(error_json) = serde_json::from_str::<Value>(&error_body) {
                    if let Some(errors) = error_json.get("errors") {
                        log::error!("X API Errors: {:#}", errors);
                    }
                    if let Some(detail) = error_json.get("detail") {
                        log::error!("X API Detail: {}", detail);
                    }
                    if let Some(title) = error_json.get("title") {
                        log::error!("X API Title: {}", title);
                    }
                }

                Err(anyhow::anyhow!(
                    "Failed to publish to X: {} - {}",
                    status,
                    error_body
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
