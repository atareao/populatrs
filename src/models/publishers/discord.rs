use super::Publisher;
use crate::models::{Post, TemplateContext, TemplateRenderer};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;

pub struct DiscordPublisher {
    pub id: String,
    pub webhook_url: String,
    pub template: String,
    client: Client,
    renderer: TemplateRenderer,
}

impl DiscordPublisher {
    pub fn new(id: String, webhook_url: String, template: String) -> Self {
        Self {
            id,
            webhook_url,
            template,
            client: Client::new(),
            renderer: TemplateRenderer::new(),
        }
    }
}

#[async_trait]
impl Publisher for DiscordPublisher {
    async fn publish(&self, post: &Post) -> Result<String> {
        let context = TemplateContext {
            title: post.title.clone(),
            description: post.description.clone().unwrap_or_default(),
            url: post.link.clone(),
        };

        let message = self.renderer.render(&self.template, &context)?;

        // Validate message length (Discord max is 2000 characters per message)
        if message.len() > 2000 {
            return Err(anyhow::anyhow!(
                "Message too long for Discord: {} characters (max 2000)",
                message.len()
            ));
        }

        // Validate webhook URL format
        if self.webhook_url.is_empty() {
            return Err(anyhow::anyhow!("Discord webhook URL is empty"));
        }

        if !self
            .webhook_url
            .starts_with("https://discord.com/api/webhooks/")
        {
            return Err(anyhow::anyhow!(
                "Invalid Discord webhook URL format. Expected: https://discord.com/api/webhooks/WEBHOOK_ID/WEBHOOK_TOKEN"
            ));
        }

        let payload = json!({
            "content": message,
            "username": "RSS Bot"
        });

        log::debug!(
            "Discord webhook URL (truncated): {}...",
            &self.webhook_url.chars().take(50).collect::<String>()
        );

        let response = self
            .client
            .post(&self.webhook_url)
            .header("Content-Type", "application/json")
            .header("User-Agent", "Populatrs/1.0")
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            Ok("Published to Discord via webhook".to_string())
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            // Discord rate limiting
            if status.as_u16() == 429 {
                return Err(anyhow::anyhow!(
                    "Discord rate limit exceeded. Please reduce posting frequency."
                ));
            }

            // Handle 400 Bad Request (invalid webhook)
            if status.as_u16() == 400 {
                return Err(anyhow::anyhow!(
                    "Discord webhook error (400). Check:\n\
                     1. Webhook URL is valid and complete\n\
                     2. Webhook hasn't been deleted\n\
                     3. Message content is valid\n\
                     Error: {}",
                    error_text
                ));
            }

            // Handle 404 Not Found (webhook deleted or invalid)
            if status.as_u16() == 404 {
                return Err(anyhow::anyhow!(
                    "Discord webhook not found (404). The webhook may have been deleted or the URL is incorrect.\n\
                     Error: {}",
                    error_text
                ));
            }

            Err(anyhow::anyhow!(
                "Failed to publish to Discord: {} - {}",
                status,
                error_text
            ))
        }
    }

    fn get_type(&self) -> &'static str {
        "Discord"
    }

    fn get_id(&self) -> &str {
        &self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
