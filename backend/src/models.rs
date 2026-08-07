use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// Re-export template types used by publishers
pub use crate::template::{TemplateContext, TemplateRenderer};

// ───── AppConfig (legacy, for compatibility) ─────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub feeds: Vec<FeedConfig>,
    pub publishers: HashMap<String, PublisherConfig>,
    pub youtube: Option<YouTubeGlobalConfig>,
    pub schedule: ScheduleConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YouTubeGlobalConfig {
    pub api_key: String,
    pub default_max_results: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleConfig {
    pub cron_expression: String,
    pub timezone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub data_dir: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            feeds: vec![],
            publishers: HashMap::new(),
            youtube: None,
            schedule: ScheduleConfig {
                cron_expression: "0 * * * *".to_string(),
                timezone: "UTC".to_string(),
            },
            storage: StorageConfig {
                data_dir: "./data".to_string(),
            },
        }
    }
}

// ───── Feed-Publisher binding ─────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedPublisherBinding {
    pub publisher_id: String,
    /// Optional per-feed template override. If empty/None, the publisher's
    /// default template is used instead.
    pub template: Option<String>,
}

// ───── Feeds ─────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedConfig {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub feed_type: String,
    pub config: FeedTypeConfig,
    pub enabled: bool,
    #[serde(default)]
    pub publishers: Vec<FeedPublisherBinding>,
    pub max_retries: Option<u32>,
    pub retry_delay_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FeedTypeConfig {
    Rss {
        url: String,
    },
    Youtube {
        channel_id: Option<String>,
        playlist_id: Option<String>,
        username: Option<String>,
        max_results: Option<u64>,
    },
}

// ───── Publishers ─────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum PublisherConfig {
    Telegram {
        bot_token: String,
        chat_id: String,
        parse_mode: Option<String>,
        message_thread_id: Option<String>,
        template: String,
    },
    X {
        client_id: String,
        client_secret: String,
        access_token: Option<String>,
        refresh_token: Option<String>,
        redirect_uri: Option<String>,
        template: String,
    },
    Mastodon {
        server_url: String,
        client_id: Option<String>,
        client_secret: Option<String>,
        access_token: Option<String>,
        redirect_uri: Option<String>,
        template: String,
    },
    LinkedIn {
        client_id: String,
        client_secret: String,
        access_token: Option<String>,
        refresh_token: Option<String>,
        user_id: Option<String>,
        redirect_uri: Option<String>,
        template: String,
    },
    OpenObserve {
        url: String,
        organization: String,
        stream_name: String,
        access_token: String,
        template: String,
    },
    Matrix {
        homeserver_url: String,
        access_token: String,
        room_id: String,
        template: String,
    },
    Bluesky {
        handle: String,
        password: String,
        pds_url: Option<String>,
        template: String,
    },
    Threads {
        client_id: String,
        client_secret: String,
        access_token: Option<String>,
        user_id: Option<String>,
        redirect_uri: Option<String>,
        template: String,
        token_expires_at: Option<i64>,
    },
    Discord {
        webhook_url: String,
        template: String,
    },
}

impl PublisherConfig {
    /// Return the type name as a lowercase string (used for SQL storage).
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Telegram { .. } => "telegram",
            Self::X { .. } => "x",
            Self::Mastodon { .. } => "mastodon",
            Self::LinkedIn { .. } => "linkedin",
            Self::OpenObserve { .. } => "openobserve",
            Self::Matrix { .. } => "matrix",
            Self::Bluesky { .. } => "bluesky",
            Self::Threads { .. } => "threads",
            Self::Discord { .. } => "discord",
        }
    }
}

// ───── Published Posts ─────

/// Minimal info about a published post, used to republish it to a single
/// publisher from the Publication History view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishedPostInfo {
    pub title: String,
    pub url: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishedPost {
    pub post_guid: String,
    pub feed_id: String,
    pub title: String,
    pub url: String,
    pub published_at: chrono::DateTime<chrono::Utc>,
    pub publisher_results: Vec<PublisherResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublisherResult {
    pub publisher_id: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishedPostsStorage {
    pub posts: Vec<PublishedPost>,
}

impl PublishedPostsStorage {
    pub fn new() -> Self {
        Self { posts: vec![] }
    }

    pub fn is_published(&self, post: &Post) -> bool {
        self.posts.iter().any(|p| p.post_guid == post.guid)
    }

    pub fn mark_published(&mut self, post: &Post, results: Vec<(String, bool, String)>) {
        let publisher_results = results
            .into_iter()
            .map(|(publisher_id, success, message)| PublisherResult {
                publisher_id,
                success,
                message,
            })
            .collect();

        self.posts.push(PublishedPost {
            post_guid: post.guid.clone(),
            feed_id: post.feed_id.clone(),
            title: post.title.clone(),
            url: post.url.clone(),
            published_at: chrono::Utc::now(),
            publisher_results,
        });

        // Keep only last 1000 posts
        if self.posts.len() > 1000 {
            self.posts.remove(0);
        }
    }

    pub fn cleanup_old_posts(&mut self, days: u64) {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        self.posts.retain(|p| p.published_at > cutoff);
    }
}

impl Default for PublishedPostsStorage {
    fn default() -> Self {
        Self::new()
    }
}

// ───── Feed-related types ─────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub guid: String,
    pub title: String,
    pub description: Option<String>,
    pub url: String,
    pub published_date: chrono::DateTime<chrono::Utc>,
    pub feed_id: String,
}

impl Post {
    pub fn new(
        guid: String,
        title: String,
        description: Option<String>,
        url: String,
        published_date: chrono::DateTime<chrono::Utc>,
        feed_id: String,
    ) -> Self {
        Self {
            guid,
            title,
            description,
            url,
            published_date,
            feed_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedCacheMetadata {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub last_content_hash: Option<String>,
}

#[derive(Debug)]
pub struct Feed {
    pub config: FeedConfig,
    pub youtube_config: Option<YouTubeGlobalConfig>,
}

impl Feed {
    pub fn new(config: FeedConfig, youtube_config: Option<YouTubeGlobalConfig>) -> Self {
        Self {
            config,
            youtube_config,
        }
    }

    pub async fn fetch_posts(&self) -> anyhow::Result<Vec<Post>> {
        let mut posts = match self.config.feed_type.to_lowercase().as_str() {
            "rss" => crate::feed::fetch_rss_posts(&self.config).await?,
            "youtube" => {
                crate::feed::fetch_youtube_posts(&self.config, self.youtube_config.as_ref()).await?
            }
            other => return Err(anyhow::anyhow!("Unknown feed type: {}", other)),
        };
        // Sort oldest-first so posts are published in chronological order
        posts.sort_by_key(|p| p.published_date);
        Ok(posts)
    }
}

// ───── FeedManager ─────

pub struct FeedManager {
    feeds: Vec<FeedConfig>,
    cache: HashMap<String, FeedCacheMetadata>,
    youtube_config: Option<YouTubeGlobalConfig>,
}

impl FeedManager {
    pub fn new() -> Self {
        Self {
            feeds: vec![],
            cache: HashMap::new(),
            youtube_config: None,
        }
    }

    pub fn load_feeds_with_cache(
        &mut self,
        feeds: Vec<FeedConfig>,
        youtube_config: Option<YouTubeGlobalConfig>,
        cache: &HashMap<String, FeedCacheMetadata>,
    ) {
        self.feeds = feeds;
        self.youtube_config = youtube_config;
        self.cache = cache.clone();
    }

    pub fn get_feed(&self, id: &str) -> Option<&FeedConfig> {
        self.feeds.iter().find(|f| f.id == id)
    }

    pub fn get_cache_metadata(&self) -> HashMap<String, FeedCacheMetadata> {
        self.cache.clone()
    }

    pub async fn check_all_feeds(&mut self) -> Vec<(String, anyhow::Result<Vec<Post>>)> {
        let mut results = Vec::new();
        for feed_config in &self.feeds {
            if !feed_config.enabled {
                continue;
            }
            let feed = Feed::new(feed_config.clone(), self.youtube_config.clone());
            let result = feed.fetch_posts().await.map(|posts| {
                posts
                    .into_iter()
                    .map(|mut p| {
                        p.feed_id = feed_config.id.clone();
                        p
                    })
                    .collect()
            });
            results.push((feed_config.id.clone(), result));
        }
        results
    }
}

impl Default for FeedManager {
    fn default() -> Self {
        Self::new()
    }
}

// ───── PublisherManager (re-export) ─────

pub use crate::publisher::PublisherManager;

// ───── Scheduler timing (shared between scheduler and status route) ─────

#[derive(Debug, Clone, Default)]
pub struct SchedulerStatus {
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
}

pub type SharedSchedulerStatus = Arc<Mutex<SchedulerStatus>>;

// ───── Feed Logs (history for LogsPage) ─────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedLogEntry {
    pub guid: String,
    pub feed_id: String,
    pub title: String,
    pub url: String,
    pub published_at: String,
    pub publisher_results: Vec<FeedLogPublisherResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedLogPublisherResult {
    pub publisher_id: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedLogResponse {
    pub entries: Vec<FeedLogEntry>,
    pub total: u64,
    pub retention_days: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publisher_type_name() {
        assert_eq!(
            PublisherConfig::Telegram {
                bot_token: String::new(),
                chat_id: String::new(),
                parse_mode: None,
                message_thread_id: None,
                template: String::new(),
            }
            .type_name(),
            "telegram"
        );
        assert_eq!(
            PublisherConfig::X {
                client_id: String::new(),
                client_secret: String::new(),
                access_token: None,
                refresh_token: None,
                redirect_uri: None,
                template: String::new(),
            }
            .type_name(),
            "x"
        );
        assert_eq!(
            PublisherConfig::Bluesky {
                handle: String::new(),
                password: String::new(),
                pds_url: None,
                template: String::new(),
            }
            .type_name(),
            "bluesky"
        );
    }

    #[test]
    fn test_published_posts_storage() {
        let mut storage = PublishedPostsStorage::new();
        assert!(storage.posts.is_empty());

        let post = Post::new(
            "guid-1".to_string(),
            "Test".to_string(),
            None,
            "https://ex.com".to_string(),
            chrono::Utc::now(),
            "feed-1".to_string(),
        );

        assert!(!storage.is_published(&post));
        storage.mark_published(
            &post,
            vec![("telegram".to_string(), true, "OK".to_string())],
        );
        assert!(storage.is_published(&post));
        assert_eq!(storage.posts.len(), 1);
    }

    #[test]
    fn test_published_posts_cleanup() {
        let mut storage = PublishedPostsStorage::new();
        let post = Post::new(
            "guid-1".to_string(),
            "Test".to_string(),
            None,
            "https://ex.com".to_string(),
            chrono::Utc::now(),
            "feed-1".to_string(),
        );
        storage.mark_published(&post, vec![]);
        assert_eq!(storage.posts.len(), 1);
        storage.cleanup_old_posts(0);
        assert!(storage.posts.is_empty());
    }
}
