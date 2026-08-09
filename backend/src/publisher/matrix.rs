use super::Publisher;
use crate::models::{Post, TemplateContext, TemplateRenderer};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

pub struct MatrixPublisher {
    #[allow(dead_code)]
    pub id: String,
    pub homeserver_url: String,
    pub access_token: String,
    pub room_id: String,
    pub template: String,
    client: Client,
    renderer: TemplateRenderer,
}

impl MatrixPublisher {
    pub fn new(
        id: String,
        homeserver_url: String,
        access_token: String,
        room_id: String,
        template: String,
    ) -> Self {
        Self {
            id,
            homeserver_url,
            access_token,
            room_id,
            template,
            client: Client::new(),
            renderer: TemplateRenderer::new(),
        }
    }
}

#[async_trait]
impl Publisher for MatrixPublisher {
    async fn publish(&self, post: &Post, feed_template: Option<&str>) -> Result<String> {
        let template_str = feed_template
            .filter(|t| !t.is_empty())
            .unwrap_or(&self.template);
        let txn_id = uuid::Uuid::new_v4().to_string();
        let url = format!(
            "{}/_matrix/client/v3/rooms/{}/send/m.room.message/{}",
            self.homeserver_url.trim_end_matches('/'),
            self.room_id,
            txn_id
        );

        let context = TemplateContext {
            title: post.title.clone(),
            description: post.description.clone().unwrap_or_default(),
            url: post.url.clone(),
        };

        let message = self.renderer.render(template_str, &context)?;

        let payload = json!({
            "msgtype": "m.text",
            "body": format!("{}\n\n{}\n\n{}", post.title, post.description.as_deref().unwrap_or(""), post.url),
            "format": "org.matrix.custom.html",
            "formatted_body": message
        });

        let response = self
            .client
            .put(&url)
            .bearer_auth(&self.access_token)
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let result: Value = response.json().await?;
            Ok(format!("Published to Matrix: {}", result["event_id"]))
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!(
                publisher_id = %self.id,
                request_body = %serde_json::to_string(&payload).unwrap_or_default(),
                response_status = %status,
                response_body = %body,
                "Failed to publish to Matrix"
            );
            Err(anyhow::anyhow!(
                "Failed to publish to Matrix: {} — {}",
                status,
                body
            ))
        }
    }

    fn get_type(&self) -> &'static str {
        "matrix"
    }

    fn get_id(&self) -> &str {
        &self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
