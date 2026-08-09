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

/// Publicador para LinkedIn usando 3-legged OAuth (Authorization Code Flow).
///
/// El usuario autoriza la aplicación una vez mediante un popup OAuth.
/// Los tokens se almacenan en la base de datos y se renuevan automáticamente.
///
/// Scopes usados:
/// - `w_member_social` — Publicar posts en nombre del usuario
/// - `openid profile email` — Obtener perfil via OpenID Connect (/v2/userinfo)
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
    ///
    /// Scopes: `w_member_social` para publicar, `openid profile email` para perfil
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
    ///
    /// Sigue el paso 3 de la documentación de Microsoft:
    /// POST /oauth/v2/accessToken con grant_type=authorization_code
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

    /// Obtiene el perfil del usuario autenticado usando OpenID Connect (/v2/userinfo)
    ///
    /// El campo `sub` puede venir en dos formatos:
    /// - ID simple: `"ACoAABBsdfg"` → se devuelve `"urn:li:person:ACoAABBsdfg"`
    /// - URN completo: `"urn:li:person:ACoAABBsdfg"` → se devuelve tal cual
    pub async fn get_user_profile(&self, access_token: &str) -> Result<String> {
        let profile_url = "https://api.linkedin.com/v2/userinfo";

        let response = self
            .client
            .get(profile_url)
            .bearer_auth(access_token)
            .send()
            .await?;

        if response.status().is_success() {
            let profile: Value = response.json().await?;
            // OpenID Connect: el campo 'sub' es el identificador único
            if let Some(sub) = profile.get("sub").and_then(|v| v.as_str()) {
                tracing::info!("LinkedIn userinfo sub: {}", sub);
                // Si ya viene como URN completo, usarlo directamente
                if sub.starts_with("urn:li:") {
                    Ok(sub.to_string())
                } else {
                    Ok(format!("urn:li:person:{}", sub))
                }
            } else {
                Err(anyhow::anyhow!(
                    "Could not get user sub from LinkedIn userinfo endpoint. Response: {}",
                    profile
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

    /// Publica un post usando la nueva Posts API de LinkedIn (/rest/posts)
    ///
    /// La API legacy `/v2/ugcPosts` está deprecada. La nueva API usa:
    /// - URL: `https://api.linkedin.com/rest/posts`
    /// - Header: `LinkedIn-Version: YYYYMM`
    /// - Formato de payload diferente
    async fn publish_rest_posts_api(
        &self,
        access_token: &str,
        author_urn: &str,
        commentary: &str,
        post: &Post,
    ) -> Result<String> {
        let posts_url = "https://api.linkedin.com/rest/posts";

        let payload = json!({
            "author": author_urn,
            "commentary": commentary,
            "visibility": "PUBLIC",
            "distribution": {
                "feedDistribution": "MAIN_FEED",
                "targetEntities": [],
                "thirdPartyDistributionChannels": []
            },
            "content": {
                "article": {
                    "source": post.url,
                    "title": post.title,
                    "description": post.description.as_deref().unwrap_or("")
                }
            },
            "lifecycleState": "PUBLISHED",
            "isReshareDisabledByAuthor": false
        });

        tracing::debug!(
            "LinkedIn Posts API payload: {}",
            serde_json::to_string_pretty(&payload)?
        );

        let response = self
            .client
            .post(posts_url)
            .bearer_auth(access_token)
            .header("LinkedIn-Version", "202501")
            .header("X-Restli-Protocol-Version", "2.0.0")
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        let status = response.status();

        if status.is_success() {
            let result: Value = response.json().await?;
            let post_id = result
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            tracing::info!(
                "Successfully published to LinkedIn (Posts API): {}",
                post_id
            );
            Ok(format!("Published to LinkedIn: {}", post_id))
        } else {
            let error_body = response.text().await.unwrap_or_default();
            tracing::error!(
                publisher_id = %self.id,
                commentary_sent = %commentary,
                request_body = %serde_json::to_string(&payload).unwrap_or_default(),
                response_status = %status,
                response_body = %error_body,
                "Failed to publish to LinkedIn (Posts API)"
            );
            Err(anyhow::anyhow!(
                "Failed to publish to LinkedIn (Posts API): {} - {}",
                status,
                error_body
            ))
        }
    }

    /// Publica un post usando la API legacy UGC (/v2/ugcPosts)
    ///
    /// Esta API está deprecada pero sigue funcionando para apps existentes.
    async fn publish_ugc_api(
        &self,
        access_token: &str,
        author_urn: &str,
        commentary: &str,
        post: &Post,
    ) -> Result<String> {
        let ugc_url = "https://api.linkedin.com/v2/ugcPosts";

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
            "LinkedIn UGC API payload: {}",
            serde_json::to_string_pretty(&payload)?
        );

        let response = self
            .client
            .post(ugc_url)
            .bearer_auth(access_token)
            .header("X-Restli-Protocol-Version", "2.0.0")
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        let status = response.status();

        if status.is_success() {
            let result: Value = response.json().await?;
            let post_id = result["id"].as_str().unwrap_or("unknown");
            tracing::info!("Successfully published to LinkedIn (UGC API): {}", post_id);
            Ok(format!("Published to LinkedIn: {}", post_id))
        } else {
            let error_body = response.text().await.unwrap_or_default();
            tracing::error!(
                publisher_id = %self.id,
                commentary_sent = %commentary,
                request_body = %serde_json::to_string(&payload).unwrap_or_default(),
                response_status = %status,
                response_body = %error_body,
                "Failed to publish to LinkedIn (UGC API)"
            );

            // Si el error es 403 con ACCESS_DENIED en /author, probablemente
            // el sub de OpenID Connect no es el person_id legacy. Intentar
            // con la nueva Posts API.
            if status.as_u16() == 403 && error_body.contains("/author") {
                tracing::info!("UGC API failed on /author field, trying Posts API as fallback...");
                return Err(anyhow::anyhow!("UGC_API_AUTHOR_ERROR:{}", error_body));
            }

            // Si el error es de autenticación, devolver error específico
            if status.as_u16() == 401 {
                return Err(anyhow::anyhow!("TOKEN_EXPIRED:{}", error_body));
            }

            Err(anyhow::anyhow!(
                "Failed to publish to LinkedIn: {} - {}",
                status,
                error_body
            ))
        }
    }
}

#[async_trait]
impl Publisher for LinkedInPublisher {
    async fn publish(&self, post: &Post, feed_template: Option<&str>) -> Result<String> {
        let template_str = feed_template
            .filter(|t| !t.is_empty())
            .unwrap_or(&self.template);

        let context = TemplateContext {
            title: post.title.clone(),
            description: post.description.clone().unwrap_or_default(),
            url: post.url.clone(),
        };

        let commentary = self.renderer.render(template_str, &context)?;

        // Normalizar: si la plantilla tiene \n literales (barra+ene),
        // convertirlos a saltos de línea reales. Esto pasa cuando el
        // template se guarda desde el formulario web y los \n se almacenan
        // como texto literal en la BD.
        let commentary = commentary.replace("\\n", "\n");

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
            // Si el user_id ya es un URN completo, usarlo directamente
            if user_id.starts_with("urn:li:") {
                user_id.clone()
            } else {
                format!("urn:li:person:{}", user_id)
            }
        } else {
            // Si no hay user_id, obtener el perfil del usuario autenticado
            match self.get_user_profile(&access_token).await {
                Ok(profile_urn) => profile_urn,
                Err(e) => {
                    tracing::error!("Failed to get LinkedIn user profile: {}", e);
                    return Err(e);
                }
            }
        };

        tracing::info!("LinkedIn author URN: {}", author_urn);

        // ── Estrategia de publicación ──
        // 1. Intentar primero con la API legacy UGC (/v2/ugcPosts)
        // 2. Si falla con error de /author, probar con la nueva Posts API (/rest/posts)
        // 3. Si falla con 401, refrescar el token y reintentar

        let ugc_result = self
            .publish_ugc_api(&access_token, &author_urn, &commentary, post)
            .await;

        match ugc_result {
            Ok(msg) => return Ok(msg),
            Err(e) => {
                let err_msg = e.to_string();

                // Si el error de UGC API es por /author, probar Posts API
                if err_msg.starts_with("UGC_API_AUTHOR_ERROR:") {
                    tracing::info!("UGC API author field failed, falling back to Posts API...");

                    // Guardar el error_body para diagnóstico
                    let error_body = err_msg.strip_prefix("UGC_API_AUTHOR_ERROR:").unwrap_or("");

                    // Intentar con la nueva Posts API
                    let posts_result = self
                        .publish_rest_posts_api(&access_token, &author_urn, &commentary, post)
                        .await;

                    match posts_result {
                        Ok(msg) => return Ok(msg),
                        Err(posts_e) => {
                            // Si ambos fallan, dar un mensaje claro con diagnóstico
                            return Err(anyhow::anyhow!(
                                "LinkedIn publication failed. UGC API error: {} | Posts API error: {}. \
                                 This usually means the LinkedIn app needs the 'Share on LinkedIn' product \
                                 or the app is in Developer mode (needs to be Live). \
                                 Check: https://www.linkedin.com/developers/apps",
                                error_body,
                                posts_e
                            ));
                        }
                    }
                }

                // Si el error es de token expirado, refrescar y reintentar
                if err_msg.starts_with("TOKEN_EXPIRED:") {
                    tracing::info!("LinkedIn access token expired, attempting to refresh...");

                    match self.refresh_access_token().await {
                        Ok((new_access_token, new_refresh_token)) => {
                            // Guardar tokens actualizados
                            if let Err(save_err) = self
                                .save_tokens_to_config(
                                    &new_access_token,
                                    new_refresh_token.as_deref(),
                                )
                                .await
                            {
                                tracing::warn!(
                                    "Failed to save refreshed LinkedIn tokens: {}",
                                    save_err
                                );
                            }

                            // Reintentar publicación con nuevo token
                            // Primero UGC API
                            let retry_ugc = self
                                .publish_ugc_api(&new_access_token, &author_urn, &commentary, post)
                                .await;

                            match retry_ugc {
                                Ok(msg) => return Ok(format!("{} (after token refresh)", msg)),
                                Err(retry_e) => {
                                    let retry_msg = retry_e.to_string();
                                    if retry_msg.starts_with("UGC_API_AUTHOR_ERROR:") {
                                        // Fallback a Posts API
                                        return self
                                            .publish_rest_posts_api(
                                                &new_access_token,
                                                &author_urn,
                                                &commentary,
                                                post,
                                            )
                                            .await
                                            .map(|msg| format!("{} (after token refresh)", msg));
                                    }
                                    return Err(anyhow::anyhow!(
                                        "Failed to publish to LinkedIn after token refresh: {}",
                                        retry_msg
                                    ));
                                }
                            }
                        }
                        Err(refresh_error) => {
                            tracing::error!("Failed to refresh LinkedIn token: {}", refresh_error);
                            return Err(anyhow::anyhow!(
                                "Failed to publish to LinkedIn - token refresh failed: {}",
                                refresh_error
                            ));
                        }
                    }
                }

                // Otros errores
                return Err(e);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Post;
    use chrono::Utc;

    fn make_publisher() -> LinkedInPublisher {
        LinkedInPublisher::new(
            "test-linkedin".to_string(),
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
            None,
            None,
            None,
            None,
            "{{ title }} - {{ url }}".to_string(),
            None,
        )
    }

    fn make_post() -> Post {
        Post::new(
            "test-guid".to_string(),
            "Test Title".to_string(),
            Some("Test description".to_string()),
            "https://example.com/test".to_string(),
            Utc::now(),
            "test-feed".to_string(),
        )
    }

    #[test]
    fn test_new_publisher() {
        let publisher = make_publisher();
        assert_eq!(publisher.get_id(), "test-linkedin");
        assert_eq!(publisher.get_type(), "linkedin");
        assert!(publisher.user_id.is_none());
        assert_eq!(publisher.redirect_uri, "https://127.0.0.1");
    }

    #[test]
    fn test_new_publisher_with_user_id() {
        let publisher = LinkedInPublisher::new(
            "test-user".to_string(),
            "client_id".to_string(),
            "client_secret".to_string(),
            None,
            None,
            Some("user-123".to_string()),
            Some("https://example.com/callback".to_string()),
            "{{ title }}".to_string(),
            None,
        );
        assert_eq!(publisher.user_id.as_deref(), Some("user-123"));
        assert_eq!(publisher.redirect_uri, "https://example.com/callback");
    }

    #[test]
    fn test_generate_auth_url() {
        let publisher = make_publisher();
        let url = publisher.generate_auth_url(Some("test-state".to_string()));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("client_id=test_client_id"));
        assert!(url.contains("state=test-state"));
        assert!(url.contains("scope=w_member_social+openid+profile+email"));
        assert!(url.contains("redirect_uri="));
    }

    #[test]
    fn test_template_rendering() {
        let publisher = make_publisher();
        let post = make_post();

        let context = TemplateContext {
            title: post.title.clone(),
            description: post.description.clone().unwrap_or_default(),
            url: post.url.clone(),
        };

        let result = publisher
            .renderer
            .render("{{ title }} - {{ url }}", &context);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Test Title - https://example.com/test");
    }

    #[test]
    fn test_get_type_and_id() {
        let publisher = make_publisher();
        assert_eq!(publisher.get_type(), "linkedin");
        assert_eq!(publisher.get_id(), "test-linkedin");
    }
}
