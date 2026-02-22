use crate::models::Post;
use anyhow::Result;
use async_trait::async_trait;
use std::any::Any;

#[async_trait]
pub trait Publisher: Send + Sync {
    async fn publish(&self, post: &Post) -> Result<String>;
    fn get_type(&self) -> &'static str;
    fn get_id(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
}

pub mod bluesky;
pub mod linkedin;
pub mod manager;
pub mod mastodon;
pub mod matrix;
pub mod openobserve;
pub mod telegram;
pub mod threads;
pub mod x;

pub use bluesky::BlueskyPublisher;
pub use linkedin::LinkedInPublisher;
pub use manager::PublisherManager;
pub use mastodon::MastodonPublisher;
pub use matrix::MatrixPublisher;
pub use openobserve::OpenObservePublisher;
pub use telegram::TelegramPublisher;
pub use threads::ThreadsPublisher;
pub use x::XPublisher;
