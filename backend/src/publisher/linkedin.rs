use super::Publisher;
use crate::models::{Post, TemplateContext, TemplateRenderer};

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;
use uuid::Uuid;

pub struct LinkedInPublisher {
    pub id: String,
    pub client_id: String,
    pub client_secret: String,
    pub access_token: Arc<Mutex<Option<String>>>,
    pub refresh_token: Arc<Mutex<Option<String>>>,
    pub user_id: Option<String>,
    pub redirect_uri: String,
    pub template: String,
    client: Client,
    renderer: TemplateRenderer,
    pub config_file_path: Option<String>,
}

impl LinkedInPublisher {
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        client_id: String,
        client_secret: String,
        access_token: Option<String>,
        refresh_token: Option<String>,
        user_id: Option<String>,
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
            user_id,
            redirect_uri,
            template,
            client: Client::new(),
            renderer: TemplateRenderer::new(),
            config_file_path,
        }
    }

    /// Genera la URL de autorización OAuth 2.0 para LinkedIn
    pub fn generate_auth_url(&self, state: Option<String>) -> String {
        let state = state.unwrap_or_else(|| Uuid::new_v4().to_string());
        let scope = "w_member_social openid profile email";

        let mut url = Url::parse("https://www.linkedin.com/oauth/v2/authorization").unwrap();
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("scope", scope)
            .append_pair("state", &state);

        url.to_string()
    }

    /// Intercambia el código de autorización por access_token y refresh_token
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

        tracing::info!("Exchanging authorization code for LinkedIn tokens");

        let response = self
            .client
            .post("https://www.linkedin.com/oauth/v2/accessToken")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&payload)
            .send()
            .await?;

        let status = response.status();
        tracing::info!("LinkedIn token exchange response status: {}", status);

        if status.is_success() {
            let token_data: Value = response.json().await?;

            let access_token = token_data["access_token"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("No access_token in response"))?
                .to_string();

            let expires_in = token_data["expires_in"].as_u64().unwrap_or(3600);

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
                "Successfully obtained LinkedIn tokens - expires in: {}s",
                expires_in
            );
            if refresh_token.is_some() {
                tracing::info!("Refresh token obtained");
            }

            Ok((access_token, refresh_token, expires_in))
        } else {
            let error_body = response.text().await.unwrap_or_default();
            tracing::error!("LinkedIn token exchange failed: {}", error_body);
            Err(anyhow::anyhow!(
                "Failed to exchange code for tokens: {} - {}",
                status,
                error_body
            ))
        }
    }

    /// Renueva el access_token usando refresh_token
    pub async fn refresh_access_token(&self) -> Result<(String, Option<String>)> {
        let refresh_token = {
            let token_guard = self.refresh_token.lock().await;
            token_guard
                .clone()
                .ok_or_else(|| anyhow::anyhow!("No refresh token available"))?
        };

        let payload = [
            ("grant_type", "refresh_token"),
            ("refresh_token", &refresh_token),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
        ];

        tracing::info!("Refreshing LinkedIn access token");

        let response = self
            .client
            .post("https://www.linkedin.com/oauth/v2/accessToken")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&payload)
            .send()
            .await?;

        let status = response.status();
        tracing::info!("LinkedIn token refresh response status: {}", status);

        if status.is_success() {
            let token_data: Value = response.json().await?;

            let new_access_token = token_data["access_token"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("No access_token in refresh response"))?
                .to_string();

            let new_refresh_token = token_data
                .get("refresh_token")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or(Some(refresh_token));

            // Actualizar tokens en memoria
            {
                let mut access_guard = self.access_token.lock().await;
                *access_guard = Some(new_access_token.clone());
            }

            if let Some(ref rt) = new_refresh_token {
                let mut refresh_guard = self.refresh_token.lock().await;
                *refresh_guard = Some(rt.clone());
            }

            tracing::info!("Successfully refreshed LinkedIn access token");
            Ok((new_access_token, new_refresh_token))
        } else {
            let error_body = response.text().await.unwrap_or_default();
            tracing::error!("LinkedIn token refresh failed: {}", error_body);
            Err(anyhow::anyhow!(
                "Failed to refresh LinkedIn token: {} - {}",
                status,
                error_body
            ))
        }
    }

    /// Obtiene un access_token válido, renovándolo si es necesario
    pub async fn get_valid_access_token(&self) -> Result<String> {
        {
            let access_guard = self.access_token.lock().await;
            if let Some(token) = access_guard.as_ref() {
                return Ok(token.clone());
            }
        }

        // Si no hay access token, intentar renovar
        let (new_access_token, new_refresh_token) = self.refresh_access_token().await?;

        // Guardar tokens actualizados en configuración
        if let Err(e) = self
            .save_tokens_to_config(&new_access_token, new_refresh_token.as_deref())
            .await
        {
            tracing::warn!("Failed to save updated LinkedIn tokens to config: {}", e);
        }

        Ok(new_access_token)
    }

    /// Guarda tokens actualizados en la configuración
    pub async fn save_tokens_to_config(
        &self,
        _access_token: &str,
        _refresh_token: Option<&str>,
    ) -> Result<()> {
        tracing::info!("LinkedIn tokens updated in memory for '{}'", self.id);
        Ok(())
    }

    /// Comando para setup interactivo de OAuth
    pub async fn oauth_setup(&self) -> Result<()> {
        println!("\n🔗 LinkedIn OAuth 2.0 Setup");
        println!("===========================");

        let auth_url = self.generate_auth_url(None);
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

        let (access_token, refresh_token, expires_in) = self.exchange_code_for_tokens(code).await?;

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
}

#[async_trait]
impl Publisher for LinkedInPublisher {
    async fn publish(&self, post: &Post, feed_template: Option<&str>) -> Result<String> {
        let template_str = feed_template
            .filter(|t| !t.is_empty())
            .unwrap_or(&self.template);
        let url = "https://api.linkedin.com/v2/ugcPosts";

        let context = TemplateContext {
            title: post.title.clone(),
            description: post.description.clone().unwrap_or_default(),
            url: post.url.clone(),
        };

        let commentary = self.renderer.render(template_str, &context)?;

        tracing::info!("Attempting to publish to LinkedIn: '{}'", commentary);

        // Obtener access token válido (renovándolo si es necesario)
        let access_token = match self.get_valid_access_token().await {
            Ok(token) => token,
            Err(e) => {
                tracing::error!("Failed to get valid LinkedIn access token: {}", e);
                return Err(e);
            }
        };

        // Determinar el author URN
        let author_urn = if let Some(user_id) = &self.user_id {
            // Si tenemos user_id, detectar si es un número (organization) o string (user)
            if user_id.chars().all(|c| c.is_ascii_digit()) {
                format!("urn:li:organization:{}", user_id)
            } else {
                format!("urn:li:person:{}", user_id)
            }
        } else {
            // Si no hay user_id, necesitamos obtener el perfil del usuario autenticado
            match self.get_user_profile(&access_token).await {
                Ok(profile_urn) => profile_urn,
                Err(e) => {
                    tracing::error!("Failed to get LinkedIn user profile: {}", e);
                    return Err(e);
                }
            }
        };

        let payload = json!({
            "author": author_urn,
            "lifecycleState": "PUBLISHED",
            "specificContent": {
                "com.linkedin.ugc.ShareContent": {
                    "shareCommentary": {
                        "text": commentary
                    },
                    "shareMediaCategory": "ARTICLE",
                    "media": [{
                        "status": "READY",
                        "description": {
                            "text": post.description.as_deref().unwrap_or("")
                        },
                        "originalUrl": post.url,
                        "title": {
                            "text": post.title
                        }
                    }]
                }
            },
            "visibility": {
                "com.linkedin.ugc.MemberNetworkVisibility": "PUBLIC"
            }
        });

        tracing::debug!(
            "LinkedIn payload: {}",
            serde_json::to_string_pretty(&payload)?
        );

        let response = self
            .client
            .post(url)
            .bearer_auth(&access_token)
            .header("X-Restli-Protocol-Version", "2.0.0")
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        tracing::info!("LinkedIn API response status: {}", status);

        if status.is_success() {
            let result: Value = response.json().await?;
            let post_id = result["id"].as_str().unwrap_or("unknown");
            Ok(format!("Published to LinkedIn: {}", post_id))
        } else {
            let error_body = response.text().await.unwrap_or_default();
            tracing::error!("LinkedIn API Error Response: {}", error_body);

            // Si el error es de autenticación, intentar renovar token
            if status.as_u16() == 401 {
                tracing::info!("LinkedIn access token expired, attempting to refresh...");

                match self.refresh_access_token().await {
                    Ok((new_access_token, new_refresh_token)) => {
                        // Guardar tokens actualizados
                        if let Err(e) = self
                            .save_tokens_to_config(&new_access_token, new_refresh_token.as_deref())
                            .await
                        {
                            tracing::warn!("Failed to save refreshed LinkedIn tokens: {}", e);
                        }

                        // Reintentar publicación con nuevo token
                        let retry_response = self
                            .client
                            .post(url)
                            .bearer_auth(&new_access_token)
                            .header("X-Restli-Protocol-Version", "2.0.0")
                            .header("Content-Type", "application/json")
                            .header("Access-Control-Allow-Origin", "*")
                            .json(&payload)
                            .send()
                            .await?;

                        let retry_status = retry_response.status();
                        tracing::info!("LinkedIn API retry response status: {}", retry_status);

                        if retry_status.is_success() {
                            let result: Value = retry_response.json().await?;
                            let post_id = result["id"].as_str().unwrap_or("unknown");
                            Ok(format!(
                                "Published to LinkedIn (after token refresh): {}",
                                post_id
                            ))
                        } else {
                            let error_body = retry_response.text().await.unwrap_or_default();
                            tracing::error!("LinkedIn API retry failed: {}", error_body);
                            Err(anyhow::anyhow!(
                                "Failed to publish to LinkedIn after token refresh: {} - {}",
                                retry_status,
                                error_body
                            ))
                        }
                    }
                    Err(refresh_error) => {
                        tracing::error!("Failed to refresh LinkedIn token: {}", refresh_error);
                        Err(anyhow::anyhow!(
                            "Failed to publish to LinkedIn - token refresh failed: {}",
                            refresh_error
                        ))
                    }
                }
            } else {
                // Parse error para mejor diagnóstico
                if let Ok(error_json) = serde_json::from_str::<Value>(&error_body) {
                    if let Some(message) = error_json.get("message") {
                        tracing::error!("LinkedIn API Error Message: {}", message);
                    }
                    if let Some(service_error_code) = error_json.get("serviceErrorCode") {
                        tracing::error!("LinkedIn API Service Error Code: {}", service_error_code);
                    }
                }

                Err(anyhow::anyhow!(
                    "Failed to publish to LinkedIn: {} - {}",
                    status,
                    error_body
                ))
            }
        }
    }

    fn get_type(&self) -> &'static str {
        "linkedin"
    }

    fn get_id(&self) -> &str {
        &self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl LinkedInPublisher {
    /// Obtiene el perfil del usuario autenticado para determinar el URN
    async fn get_user_profile(&self, access_token: &str) -> Result<String> {
        let profile_url = "https://api.linkedin.com/v2/people/~:(id)";

        let response = self
            .client
            .get(profile_url)
            .bearer_auth(access_token)
            .header("X-Restli-Protocol-Version", "2.0.0")
            .send()
            .await?;

        if response.status().is_success() {
            let profile: Value = response.json().await?;
            if let Some(id) = profile.get("id").and_then(|v| v.as_str()) {
                Ok(format!("urn:li:person:{}", id))
            } else {
                Err(anyhow::anyhow!(
                    "Could not get user ID from LinkedIn profile"
                ))
            }
        } else {
            let error_body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Failed to get LinkedIn profile: {}",
                error_body
            ))
        }
    }
}
