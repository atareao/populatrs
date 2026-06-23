use super::Publisher;
use crate::models::{Post, TemplateContext, TemplateRenderer};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

pub struct MastodonPublisher {
    pub id: String,
    pub server_url: String,
    pub access_token: String,
    pub template: String,
    client: Client,
    renderer: TemplateRenderer,
}

impl MastodonPublisher {
    pub fn new(id: String, server_url: String, access_token: String, template: String) -> Self {
        Self {
            id,
            server_url,
            access_token,
            template,
            client: Client::new(),
            renderer: TemplateRenderer::new(),
        }
    }
}

#[async_trait]
impl Publisher for MastodonPublisher {
    async fn publish(&self, post: &Post) -> Result<String> {
        let url = format!("{}/api/v1/statuses", self.server_url);

        let context = TemplateContext {
            title: post.title.clone(),
            description: post.description.clone().unwrap_or_default(),
            url: post.link.clone(),
        };

        let status = self.renderer.render(&self.template, &context)?;

        let payload = json!({
            "status": status,
            "visibility": "public"
        });

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.access_token)
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
