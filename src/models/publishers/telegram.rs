use super::Publisher;
use crate::models::{Post, TemplateContext, TemplateRenderer};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

pub struct TelegramPublisher {
    pub id: String,
    pub bot_token: String,
    pub chat_id: String,
    pub parse_mode: Option<String>,
    pub message_thread_id: Option<String>,
    pub template: String,
    client: Client,
    renderer: TemplateRenderer,
}

impl TelegramPublisher {
    pub fn new(
        id: String,
        bot_token: String,
        chat_id: String,
        parse_mode: Option<String>,
        message_thread_id: Option<String>,
        template: String,
    ) -> Self {
        Self {
            id,
            bot_token,
            chat_id,
            parse_mode,
            message_thread_id,
            template,
            client: Client::new(),
            renderer: TemplateRenderer::new(),
        }
    }
}

#[async_trait]
impl Publisher for TelegramPublisher {
    async fn publish(&self, post: &Post) -> Result<String> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);

        let context = TemplateContext {
            title: post.title.clone(),
            description: post.description.clone().unwrap_or_default(),
            url: post.link.clone(),
        };

        let message = self.renderer.render(&self.template, &context)?;

        // Validate message length (Telegram max is 4096 characters)
        if message.len() > 4096 {
            return Err(anyhow::anyhow!(
                "Message too long for Telegram: {} characters (max 4096)",
                message.len()
            ));
        }

        let mut payload = json!({
            "chat_id": self.chat_id,
            "text": message,
            "parse_mode": self.parse_mode,
            "disable_web_page_preview": false
        });

        // Add message_thread_id if specified (for posting to topics)
        if let Some(thread_id) = &self.message_thread_id {
            match thread_id.parse::<i64>() {
                Ok(id) if id > 0 => {
                    payload["message_thread_id"] = json!(id);
                    log::debug!("Using message_thread_id: {}", id);
                }
                _ => {
                    log::warn!("Invalid message_thread_id '{}', ignoring", thread_id);
                }
            }
        }

        let response = self.client.post(&url).json(&payload).send().await?;

        if response.status().is_success() {
            let result: Value = response.json().await?;
            Ok(format!(
                "Published to Telegram: {}",
                result["result"]["message_id"]
            ))
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Failed to publish to Telegram: {} - {}",
                status,
                error_text
            ))
        }
    }

    fn get_type(&self) -> &'static str {
        "telegram"
    }

    fn get_id(&self) -> &str {
        &self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
