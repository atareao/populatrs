use anyhow::Result;
use async_trait::async_trait;
use std::any::Any;
use std::collections::HashMap;

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
    async fn publish(&self, post: &Post) -> Result<String>;
    fn get_type(&self) -> &'static str;
    fn get_id(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
}

pub fn create_publisher(id: String, config: &PublisherConfig) -> Result<Box<dyn Publisher>> {
    create_publisher_with_config_path(id, config, None)
}

pub fn create_publisher_with_config_path(
    id: String,
    config: &PublisherConfig,
    config_path: Option<String>,
) -> Result<Box<dyn Publisher>> {
    match config {
        PublisherConfig::Telegram {
            bot_token,
            chat_id,
            parse_mode,
            message_thread_id,
            template,
        } => {
            let template_str = template.clone().unwrap_or_else(|| get_default_template("telegram"));
            Ok(Box::new(TelegramPublisher::new(
                id, bot_token.clone(), chat_id.clone(),
                parse_mode.clone(), message_thread_id.clone(), template_str,
            )))
        }
        PublisherConfig::X {
            client_id,
            client_secret,
            access_token,
            refresh_token,
            redirect_uri,
            template,
        } => {
            let template_str = template.clone().unwrap_or_else(|| get_default_template("x"));
            Ok(Box::new(XPublisher::new(
                id, client_id.clone(), client_secret.clone(),
                access_token.clone(), refresh_token.clone(), redirect_uri.clone(),
                template_str, config_path,
            )))
        }
        PublisherConfig::Mastodon {
            server_url,
            access_token,
            template,
        } => {
            let template_str = template.clone().unwrap_or_else(|| get_default_template("mastodon"));
            Ok(Box::new(MastodonPublisher::new(
                id, server_url.clone(), access_token.clone(), template_str,
            )))
        }
        PublisherConfig::LinkedIn {
            client_id,
            client_secret,
            access_token,
            refresh_token,
            user_id,
            redirect_uri,
            template,
        } => {
            let template_str = template.clone().unwrap_or_else(|| get_default_template("linkedin"));
            Ok(Box::new(LinkedInPublisher::new(
                id, client_id.clone(), client_secret.clone(),
                access_token.clone(), refresh_token.clone(), user_id.clone(),
                redirect_uri.clone(), template_str, config_path,
            )))
        }
        PublisherConfig::OpenObserve {
            url,
            organization,
            stream_name,
            access_token,
            template,
        } => {
            let template_str = template.clone().unwrap_or_else(|| get_default_template("openobserve"));
            Ok(Box::new(OpenObservePublisher::new(
                id, url.clone(), organization.clone(), stream_name.clone(),
                access_token.clone(), template_str,
            )))
        }
        PublisherConfig::Matrix {
            homeserver_url,
            access_token,
            room_id,
            template,
        } => {
            let template_str = template.clone().unwrap_or_else(|| get_default_template("matrix"));
            Ok(Box::new(MatrixPublisher::new(
                id, homeserver_url.clone(), access_token.clone(), room_id.clone(), template_str,
            )))
        }
        PublisherConfig::Bluesky {
            handle,
            password,
            pds_url,
            template,
        } => {
            let template_str = template.clone().unwrap_or_else(|| get_default_template("bluesky"));
            Ok(Box::new(BlueskyPublisher::new(
                id, handle.clone(), password.clone(), pds_url.clone(), template_str,
            )))
        }
        PublisherConfig::Threads {
            access_token,
            user_id,
            template,
        } => {
            let template_str = template.clone().unwrap_or_else(|| get_default_template("threads"));
            Ok(Box::new(ThreadsPublisher::new(
                id, access_token.clone(), user_id.clone(), template_str,
            )))
        }
        PublisherConfig::Discord {
            webhook_url,
            template,
        } => {
            let template_str = template.clone().unwrap_or_else(|| get_default_template("discord"));
            Ok(Box::new(DiscordPublisher::new(
                id, webhook_url.clone(), template_str,
            )))
        }
    }
}

/// Get the default template string for a given publisher type.
pub fn get_default_template(publisher_type: &str) -> String {
    match publisher_type {
        "telegram" => {
            "📰 *{{ title }}*\n\n{{ description }}\n\n🔗 {{ url }}".to_string()
        }
        "x" => {
            "{{ title }}\n\n{{ url }}".to_string()
        }
        "mastodon" => {
            "{{ title }}\n\n{{ url }}\n\n{{ description }}".to_string()
        }
        "linkedin" => {
            "{{ title }}\n\n{{ description }}\n\n🔗 {{ url }}".to_string()
        }
        "bluesky" => {
            "{{ title }}\n\n{{ url }}".to_string()
        }
        "threads" => {
            "{{ title }}\n\n{{ url }}".to_string()
        }
        "discord" => {
            "**{{ title }}**\n\n{{ description }}\n\n{{ url }}".to_string()
        }
        "matrix" => {
            "📰 {{ title }}\n\n{{ description }}\n\n🔗 {{ url }}".to_string()
        }
        "openobserve" => {
            r#"{"title": "{{ title }}", "url": "{{ url }}", "description": "{{ description }}"}"#.to_string()
        }
        _ => "{{ title }} - {{ url }}".to_string(),
    }
}

/// The central PublisherManager holds all active publishers.
pub struct PublisherManager {
    publishers: HashMap<String, Box<dyn Publisher>>,
    config_path: Option<String>,
}

impl PublisherManager {
    pub fn new() -> Self {
        Self {
            publishers: HashMap::new(),
            config_path: None,
        }
    }

    pub fn new_with_config_path(config_path: String) -> Self {
        Self {
            publishers: HashMap::new(),
            config_path: Some(config_path),
        }
    }

    pub fn add_publisher(&mut self, id: String, config: &PublisherConfig) -> Result<()> {
        let publisher = create_publisher_with_config_path(id.clone(), config, self.config_path.clone())?;
        self.publishers.insert(id, publisher);
        Ok(())
    }

    pub async fn publish_to_all(
        &self,
        post: &Post,
        publisher_ids: &[String],
    ) -> Vec<Result<String>> {
        let mut results = Vec::new();
        for id in publisher_ids {
            if let Some(publisher) = self.publishers.get(id) {
                results.push(publisher.publish(post).await);
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