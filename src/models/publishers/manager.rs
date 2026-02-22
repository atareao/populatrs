use super::{
    BlueskyPublisher, LinkedInPublisher, MastodonPublisher, MatrixPublisher, OpenObservePublisher,
    Publisher, TelegramPublisher, ThreadsPublisher, XPublisher,
};
use crate::models::{Post, PublisherConfig, TemplateRenderer};
use anyhow::Result;
use std::collections::HashMap;

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
            let template_str = template
                .clone()
                .unwrap_or_else(|| TemplateRenderer::get_default_template("telegram"));
            Ok(Box::new(TelegramPublisher::new(
                id,
                bot_token.clone(),
                chat_id.clone(),
                parse_mode.clone(),
                message_thread_id.clone(),
                template_str,
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
            let template_str = template
                .clone()
                .unwrap_or_else(|| TemplateRenderer::get_default_template("x"));
            Ok(Box::new(XPublisher::new(
                id,
                client_id.clone(),
                client_secret.clone(),
                access_token.clone(),
                refresh_token.clone(),
                redirect_uri.clone(),
                template_str,
                config_path,
            )))
        }
        PublisherConfig::Mastodon {
            server_url,
            access_token,
            template,
        } => {
            let template_str = template
                .clone()
                .unwrap_or_else(|| TemplateRenderer::get_default_template("mastodon"));
            Ok(Box::new(MastodonPublisher::new(
                id,
                server_url.clone(),
                access_token.clone(),
                template_str,
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
            let template_str = template
                .clone()
                .unwrap_or_else(|| TemplateRenderer::get_default_template("linkedin"));
            Ok(Box::new(LinkedInPublisher::new(
                id,
                client_id.clone(),
                client_secret.clone(),
                access_token.clone(),
                refresh_token.clone(),
                user_id.clone(),
                redirect_uri.clone(),
                template_str,
                config_path,
            )))
        }
        PublisherConfig::OpenObserve {
            url,
            organization,
            stream_name,
            access_token,
            template,
        } => {
            let template_str = template
                .clone()
                .unwrap_or_else(|| TemplateRenderer::get_default_template("openobserve"));
            Ok(Box::new(OpenObservePublisher::new(
                id,
                url.clone(),
                organization.clone(),
                stream_name.clone(),
                access_token.clone(),
                template_str,
            )))
        }
        PublisherConfig::Matrix {
            homeserver_url,
            access_token,
            room_id,
            template,
        } => {
            let template_str = template
                .clone()
                .unwrap_or_else(|| TemplateRenderer::get_default_template("matrix"));
            Ok(Box::new(MatrixPublisher::new(
                id,
                homeserver_url.clone(),
                access_token.clone(),
                room_id.clone(),
                template_str,
            )))
        }
        PublisherConfig::Bluesky {
            handle,
            password,
            pds_url,
            template,
        } => {
            let template_str = template
                .clone()
                .unwrap_or_else(|| TemplateRenderer::get_default_template("bluesky"));
            Ok(Box::new(BlueskyPublisher::new(
                id,
                handle.clone(),
                password.clone(),
                pds_url.clone(),
                template_str,
            )))
        }
        PublisherConfig::Threads {
            access_token,
            user_id,
            template,
        } => {
            let template_str = template
                .clone()
                .unwrap_or_else(|| TemplateRenderer::get_default_template("threads"));
            Ok(Box::new(ThreadsPublisher::new(
                id,
                access_token.clone(),
                user_id.clone(),
                template_str,
            )))
        }
    }
}

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
        let publisher =
            create_publisher_with_config_path(id.clone(), config, self.config_path.clone())?;
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
                let result = publisher.publish(post).await;
                results.push(result);
            } else {
                results.push(Err(anyhow::anyhow!("Publisher not found: {}", id)));
            }
        }

        results
    }

    pub fn get_publisher(&self, id: &str) -> Option<&Box<dyn Publisher>> {
        self.publishers.get(id)
    }

    pub fn list_publishers(&self) -> Vec<(&str, &str)> {
        self.publishers
            .iter()
            .map(|(id, publisher)| (id.as_str(), publisher.get_type()))
            .collect()
    }
}
