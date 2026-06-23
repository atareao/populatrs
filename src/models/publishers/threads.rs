use super::Publisher;
use crate::models::{Post, TemplateContext, TemplateRenderer};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::time;

pub struct ThreadsPublisher {
    pub id: String,
    pub access_token: String,
    pub user_id: String,
    pub template: String,
    client: Client,
    renderer: TemplateRenderer,
}

impl ThreadsPublisher {
    pub fn new(id: String, access_token: String, user_id: String, template: String) -> Self {
        Self {
            id,
            access_token,
            user_id,
            template,
            client: Client::new(),
            renderer: TemplateRenderer::new(),
        }
    }
}

#[async_trait]
impl Publisher for ThreadsPublisher {
    async fn publish(&self, post: &Post) -> Result<String> {
        // Threads API uses a two-step process: create container, then publish

        // Step 1: Create media container
        let container_url = format!("https://graph.threads.net/v1.0/{}/threads", self.user_id);

        let context = TemplateContext {
            title: post.title.clone(),
            description: post.description.clone().unwrap_or_default(),
            url: post.link.clone(),
        };

        let text = self.renderer.render(&self.template, &context)?;

        // Threads has a character limit of 500
        let text = if text.len() > 500 {
            format!("{}...", &text[..497])
        } else {
            text
        };

        let container_payload = json!({
            "media_type": "TEXT",
            "text": text,
            "access_token": self.access_token
        });

        let container_response = self
            .client
            .post(&container_url)
            .json(&container_payload)
            .send()
            .await?;

        if !container_response.status().is_success() {
            let status = container_response.status();
            let error_text = container_response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to create Threads container: {} - {}",
                status,
                error_text
            ));
        }

        let container_result: Value = container_response.json().await?;
        let container_id = container_result["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No container ID in Threads response"))?;

        log::info!("Created Threads container: {}", container_id);

        // Small delay to ensure the container is ready for publishing
        // Threads API sometimes needs time to process the container
        time::sleep(std::time::Duration::from_millis(2000)).await;

        // Step 2: Publish the container
        let publish_url = format!(
            "https://graph.threads.net/v1.0/{}/threads_publish",
            self.user_id
        );

        let publish_payload = json!({
            "creation_id": container_id,
            "access_token": self.access_token
        });

        log::info!("Publishing Threads container: {}", container_id);

        let publish_response = self
            .client
            .post(&publish_url)
            .json(&publish_payload)
            .send()
            .await?;

        if publish_response.status().is_success() {
            let result: Value = publish_response.json().await?;
            Ok(format!(
                "Published to Threads: {}",
                result["id"].as_str().unwrap_or("unknown")
            ))
        } else {
            let status = publish_response.status();
            let error_text = publish_response.text().await.unwrap_or_default();

            // If the container doesn't exist, maybe we need to wait longer
            if error_text.contains("does not exist") || error_text.contains("No se encuentra") {
                log::warn!(
                    "Container {} not found, trying again after delay...",
                    container_id
                );

                // Wait a bit more and try once more
                time::sleep(std::time::Duration::from_millis(3000)).await;

                let retry_response = self
                    .client
                    .post(&publish_url)
                    .json(&publish_payload)
                    .send()
                    .await?;

                if retry_response.status().is_success() {
                    let result: Value = retry_response.json().await?;
                    Ok(format!(
                        "Published to Threads (retry): {}",
                        result["id"].as_str().unwrap_or("unknown")
                    ))
                } else {
                    let retry_status = retry_response.status();
                    let retry_error = retry_response.text().await.unwrap_or_default();
                    Err(anyhow::anyhow!(
                        "Failed to publish to Threads after retry: {} - {}",
                        retry_status,
                        retry_error
                    ))
                }
            } else {
                Err(anyhow::anyhow!(
                    "Failed to publish to Threads: {} - {}",
                    status,
                    error_text
                ))
            }
        }
    }

    fn get_type(&self) -> &'static str {
        "threads"
    }

    fn get_id(&self) -> &str {
        &self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
