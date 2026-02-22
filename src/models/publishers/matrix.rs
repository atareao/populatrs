use super::Publisher;
use crate::models::{Post, TemplateContext, TemplateRenderer};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

pub struct MatrixPublisher {
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
    async fn publish(&self, post: &Post) -> Result<String> {
        let txn_id = uuid::Uuid::new_v4().to_string();
        let url = format!(
            "{}/_matrix/client/r0/rooms/{}/send/m.room.message/{}",
            self.homeserver_url, self.room_id, txn_id
        );

        let context = TemplateContext {
            title: post.title.clone(),
            description: post.description.clone().unwrap_or_default(),
            url: post.link.clone(),
        };

        let message = self.renderer.render(&self.template, &context)?;

        let payload = json!({
            "msgtype": "m.text",
            "body": format!("{}\n\n{}\n\n{}", post.title, post.description.as_deref().unwrap_or(""), post.link),
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
            Err(anyhow::anyhow!(
                "Failed to publish to Matrix: {}",
                response.status()
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
