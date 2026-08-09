use super::Publisher;
use crate::models::{Post, TemplateContext, TemplateRenderer};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;

pub struct OpenObservePublisher {
    #[allow(dead_code)]
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
    async fn publish(&self, post: &Post, feed_template: Option<&str>) -> Result<String> {
        let template_str = feed_template
            .filter(|t| !t.is_empty())
            .unwrap_or(&self.template);
        let url = format!(
            "{}/api/{}/{}/_json",
            self.url, self.organization, self.stream_name
        );

        let context = TemplateContext {
            title: post.title.clone(),
            description: post.description.clone().unwrap_or_default(),
            url: post.url.clone(),
        };

        let formatted_message = self.renderer.render(template_str, &context)?;

        let log_entry = json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "level": "INFO",
            "source": "populatrs",
            "feed_id": post.feed_id,
            "title": post.title,
            "description": post.description,
            "link": post.url,
            "published": post.published_date,
            "guid": post.guid,
            "formatted_message": formatted_message
        });

        let payload_body = vec![log_entry];

        tracing::debug!(
            publisher_id = %self.id,
            payload = %serde_json::to_string(&payload_body).unwrap_or_default(),
            "Sending to OpenObserve"
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Basic {}", self.access_token))
            .json(&payload_body)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(format!("Published to OpenObserve: {}", post.guid))
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!(
                publisher_id = %self.id,
                request_body = %serde_json::to_string(&payload_body).unwrap_or_default(),
                response_status = %status,
                response_body = %body,
                "Failed to publish to OpenObserve"
            );
            Err(anyhow::anyhow!(
                "Failed to publish to OpenObserve: {} - {}",
                status,
                body
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
