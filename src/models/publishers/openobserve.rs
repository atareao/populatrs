use super::Publisher;
use crate::models::{Post, TemplateContext, TemplateRenderer};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;

pub struct OpenObservePublisher {
    pub id: String,
    pub url: String,
    pub organization: String,
    pub stream_name: String,
    pub access_token: String,
    pub template: String,
    client: Client,
    renderer: TemplateRenderer,
}

impl OpenObservePublisher {
    pub fn new(
        id: String,
        url: String,
        organization: String,
        stream_name: String,
        access_token: String,
        template: String,
    ) -> Self {
        Self {
            id,
            url,
            organization,
            stream_name,
            access_token,
            template,
            client: Client::new(),
            renderer: TemplateRenderer::new(),
        }
    }
}

#[async_trait]
impl Publisher for OpenObservePublisher {
    async fn publish(&self, post: &Post) -> Result<String> {
        let url = format!(
            "{}/api/{}/{}/_json",
            self.url, self.organization, self.stream_name
        );

        let context = TemplateContext {
            title: post.title.clone(),
            description: post.description.clone().unwrap_or_default(),
            url: post.link.clone(),
        };

        let formatted_message = self.renderer.render(&self.template, &context)?;

        let log_entry = json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "level": "INFO",
            "source": "populatrs",
            "feed_id": post.feed_id,
            "title": post.title,
            "description": post.description,
            "link": post.link,
            "published": post.published,
            "guid": post.guid,
            "formatted_message": formatted_message
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Basic {}", &self.access_token))
            .json(&vec![log_entry])
            .send()
            .await?;

        if response.status().is_success() {
            Ok(format!("Published to OpenObserve: {}", post.guid))
        } else {
            Err(anyhow::anyhow!(
                "Failed to publish to OpenObserve: {}",
                response.status()
            ))
        }
    }

    fn get_type(&self) -> &'static str {
        "openobserve"
    }

    fn get_id(&self) -> &str {
        &self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
