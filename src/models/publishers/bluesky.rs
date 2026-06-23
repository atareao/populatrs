use super::Publisher;
use crate::models::{Post, TemplateContext, TemplateRenderer};
use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use reqwest::Client;
use serde_json::{json, Value};

pub struct BlueskyPublisher {
    pub id: String,
    pub handle: String,
    pub password: String,
    pub pds_url: String,
    pub template: String,
    client: Client,
    renderer: TemplateRenderer,
}

impl BlueskyPublisher {
    pub fn new(
        id: String,
        handle: String,
        password: String,
        pds_url: Option<String>,
        template: String,
    ) -> Self {
        Self {
            id,
            handle,
            password,
            pds_url: pds_url.unwrap_or_else(|| "https://bsky.social".to_string()),
            template,
            client: Client::new(),
            renderer: TemplateRenderer::new(),
        }
    }

    fn extract_url_facets(&self, text: &str) -> Vec<Value> {
        let url_regex = Regex::new(r"https?://[^\s]+").unwrap();
        let mut facets = Vec::new();

        for mat in url_regex.find_iter(text) {
            let facet = json!({
                "$type": "app.bsky.richtext.facet",
                "index": {
                    "byteStart": mat.start(),
                    "byteEnd": mat.end()
                },
                "features": [
                    {
                        "$type": "app.bsky.richtext.facet#link",
                        "uri": mat.as_str()
                    }
                ]
            });
            facets.push(facet);
        }

        facets
    }

    async fn authenticate(&self) -> Result<(String, String)> {
        let auth_url = format!("{}/xrpc/com.atproto.server.createSession", self.pds_url);

        let payload = json!({
            "identifier": self.handle,
            "password": self.password
        });

        let response = self.client.post(&auth_url).json(&payload).send().await?;

        if response.status().is_success() {
            let result: Value = response.json().await?;
            if let (Some(access_token), Some(did)) =
                (result["accessJwt"].as_str(), result["did"].as_str())
            {
                Ok((access_token.to_string(), did.to_string()))
            } else {
                Err(anyhow::anyhow!(
                    "Missing access token or DID in Bluesky auth response"
                ))
            }
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Bluesky authentication failed: {} - {}",
                status,
                error_text
            ))
        }
    }
}

#[async_trait]
impl Publisher for BlueskyPublisher {
    async fn publish(&self, post: &Post) -> Result<String> {
        // Authenticate first and get both access token and DID
        let (access_token, did) = self.authenticate().await?;

        let create_url = format!("{}/xrpc/com.atproto.repo.createRecord", self.pds_url);

        let context = TemplateContext {
            title: post.title.clone(),
            description: post.description.clone().unwrap_or_default(),
            url: post.link.clone(),
        };

        let text = self.renderer.render(&self.template, &context)?;

        // Bluesky has a character limit of 300
        let text = if text.len() > 300 {
            format!("{}...", &text[..297])
        } else {
            text
        };

        let now = chrono::Utc::now().to_rfc3339();

        // Extract URL facets for automatic link detection
        let facets = self.extract_url_facets(&text);

        let mut record = json!({
            "text": text,
            "createdAt": now,
            "$type": "app.bsky.feed.post"
        });

        // Add facets if any URLs were found
        if !facets.is_empty() {
            record["facets"] = json!(facets);
        }

        let payload = json!({
            "repo": did,  // Use DID instead of handle
            "collection": "app.bsky.feed.post",
            "record": record
        });

        let response = self
            .client
            .post(&create_url)
            .bearer_auth(&access_token)
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let result: Value = response.json().await?;
            Ok(format!(
                "Published to Bluesky: {}",
                result["uri"].as_str().unwrap_or("unknown")
            ))
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Failed to publish to Bluesky: {} - {}",
                status,
                error_text
            ))
        }
    }

    fn get_type(&self) -> &'static str {
        "bluesky"
    }

    fn get_id(&self) -> &str {
        &self.id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
