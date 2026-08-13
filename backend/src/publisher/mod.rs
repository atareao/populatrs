use anyhow::Result;
use async_trait::async_trait;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use crate::db::Database;
use crate::models::{Post, PublisherConfig};

pub mod bluesky;
pub mod discord;
pub mod linkedin;
pub mod mastodon;
pub mod matrix;
pub mod openobserve;
pub mod telegram;
pub mod threads;
pub mod x;

pub use bluesky::BlueskyPublisher;
pub use discord::DiscordPublisher;
pub use linkedin::LinkedInPublisher;
pub use mastodon::MastodonPublisher;
pub use matrix::MatrixPublisher;
pub use openobserve::OpenObservePublisher;
pub use telegram::TelegramPublisher;
pub use threads::ThreadsPublisher;
pub use x::XPublisher;

#[async_trait]
pub trait Publisher: Send + Sync {
    /// Publish a post. If `feed_template` is provided and non-empty, it
    /// overrides the publisher's default template for this publication.
    async fn publish(&self, post: &Post, feed_template: Option<&str>) -> Result<String>;
    fn get_type(&self) -> &'static str;
    fn get_id(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
}

pub fn create_publisher(id: String, config: &PublisherConfig) -> Result<Box<dyn Publisher>> {
    create_publisher_with_config_path(id, config, None, None)
}

pub fn create_publisher_with_config_path(
    id: String,
    config: &PublisherConfig,
    config_path: Option<String>,
    db: Option<Arc<Database>>,
) -> Result<Box<dyn Publisher>> {
    match config {
        PublisherConfig::Telegram {
            bot_token,
            chat_id,
            parse_mode,
            message_thread_id,
            template,
        } => Ok(Box::new(TelegramPublisher::new(
            id,
            bot_token.clone(),
            chat_id.clone(),
            parse_mode.clone(),
            message_thread_id.clone(),
            template.clone(),
        ))),
        PublisherConfig::X {
            client_id,
            client_secret,
            access_token,
            refresh_token,
            redirect_uri,
            template,
            reply_template,
        } => Ok(Box::new(XPublisher::new(
            id,
            client_id.clone(),
            client_secret.clone(),
            access_token.clone(),
            refresh_token.clone(),
            redirect_uri.clone(),
            template.clone(),
            reply_template.clone(),
            config_path,
            db,
        ))),
        PublisherConfig::Mastodon {
            server_url,
            client_id,
            client_secret,
            access_token,
            redirect_uri,
            template,
        } => Ok(Box::new(MastodonPublisher::new(
            id,
            server_url.clone(),
            client_id.clone(),
            client_secret.clone(),
            access_token.clone(),
            redirect_uri.clone(),
            template.clone(),
        ))),
        PublisherConfig::LinkedIn {
            client_id,
            client_secret,
            access_token,
            refresh_token,
            user_id,
            redirect_uri,
            template,
        } => Ok(Box::new(LinkedInPublisher::new(
            id,
            client_id.clone(),
            client_secret.clone(),
            access_token.clone(),
            refresh_token.clone(),
            user_id.clone(),
            redirect_uri.clone(),
            template.clone(),
            config_path,
        ))),
        PublisherConfig::OpenObserve {
            url,
            organization,
            stream_name,
            access_token,
            template,
        } => Ok(Box::new(OpenObservePublisher::new(
            id,
            url.clone(),
            organization.clone(),
            stream_name.clone(),
            access_token.clone(),
            template.clone(),
        ))),
        PublisherConfig::Matrix {
            homeserver_url,
            access_token,
            room_id,
            template,
        } => Ok(Box::new(MatrixPublisher::new(
            id,
            homeserver_url.clone(),
            access_token.clone(),
            room_id.clone(),
            template.clone(),
        ))),
        PublisherConfig::Bluesky {
            handle,
            password,
            pds_url,
            template,
        } => Ok(Box::new(BlueskyPublisher::new(
            id,
            handle.clone(),
            password.clone(),
            pds_url.clone(),
            template.clone(),
        ))),
        PublisherConfig::Threads {
            client_id,
            client_secret,
            access_token,
            user_id,
            redirect_uri,
            template,
            token_expires_at,
        } => Ok(Box::new(ThreadsPublisher::new(
            id,
            client_id.clone(),
            client_secret.clone(),
            access_token.clone(),
            user_id.clone(),
            redirect_uri.clone(),
            template.clone(),
            db,
            *token_expires_at,
        ))),
        PublisherConfig::Discord {
            webhook_url,
            template,
        } => Ok(Box::new(DiscordPublisher::new(
            id,
            webhook_url.clone(),
            template.clone(),
        ))),
    }
}

/// Get the default template string for a given publisher type.
pub fn get_default_template(publisher_type: &str) -> String {
    match publisher_type {
        "telegram" => "📰 *{{ title }}*\n\n{{ description }}\n\n🔗 {{ url }}".to_string(),
        "x" => "{{ title }}".to_string(),
        "mastodon" => "{{ title }}\n\n{{ url }}\n\n{{ description }}".to_string(),
        "linkedin" => "{{ title }}\n\n{{ description }}\n\n🔗 {{ url }}".to_string(),
        "bluesky" => "{{ title }}\n\n{{ url }}".to_string(),
        "threads" => "{{ title }}\n\n{{ url }}".to_string(),
        "discord" => "**{{ title }}**\n\n{{ description }}\n\n{{ url }}".to_string(),
        "matrix" => "📰 {{ title }}\n\n{{ description }}\n\n🔗 {{ url }}".to_string(),
        "openobserve" => {
            r#"{"title": "{{ title }}", "url": "{{ url }}", "description": "{{ description }}"}"#
                .to_string()
        }
        _ => "{{ title }} - {{ url }}".to_string(),
    }
}

/// The central PublisherManager holds all active publishers.
pub struct PublisherManager {
    publishers: HashMap<String, Box<dyn Publisher>>,
    config_path: Option<String>,
    db: Option<Arc<Database>>,
}

impl PublisherManager {
    pub fn new() -> Self {
        Self {
            publishers: HashMap::new(),
            config_path: None,
            db: None,
        }
    }

    pub fn new_with_config_path(config_path: String) -> Self {
        Self {
            publishers: HashMap::new(),
            config_path: Some(config_path),
            db: None,
        }
    }

    pub fn new_with_db(config_path: Option<String>, db: Option<Arc<Database>>) -> Self {
        Self {
            publishers: HashMap::new(),
            config_path,
            db,
        }
    }

    pub fn add_publisher(&mut self, id: String, config: &PublisherConfig) -> Result<()> {
        let publisher = create_publisher_with_config_path(
            id.clone(),
            config,
            self.config_path.clone(),
            self.db.clone(),
        )?;
        self.publishers.insert(id, publisher);
        Ok(())
    }

    /// Publish a post to all given publisher IDs, optionally using
    /// feed-specific templates (publisher_id → template override).
    pub async fn publish_to_all(
        &self,
        post: &Post,
        publisher_ids: &[String],
        feed_templates: &HashMap<String, String>,
    ) -> Vec<Result<String>> {
        let mut results = Vec::new();
        for id in publisher_ids {
            if let Some(publisher) = self.publishers.get(id) {
                let feed_template = feed_templates.get(id).map(|s| s.as_str());
                results.push(publisher.publish(post, feed_template).await);
            } else {
                results.push(Err(anyhow::anyhow!("Publisher not found: {}", id)));
            }
        }
        results
    }

    #[allow(dead_code)]
    pub fn get_publisher(&self, id: &str) -> Option<&dyn Publisher> {
        self.publishers.get(id).map(|p| p.as_ref())
    }

    #[allow(dead_code)]
    pub fn list_publishers(&self) -> Vec<(&str, &str)> {
        self.publishers
            .iter()
            .map(|(id, publisher)| (id.as_str(), publisher.get_type()))
            .collect()
    }
}

impl Default for PublisherManager {
    fn default() -> Self {
        Self::new()
    }
}
