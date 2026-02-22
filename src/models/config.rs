use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
pub struct FeedConfig {
    pub id: String,
    #[serde(rename = "type")]
    pub feed_type: String,
    pub config: FeedTypeConfig,
    pub name: String,
    pub enabled: bool,
    pub publishers: Vec<String>, // Publisher IDs to publish this feed to
    pub check_interval_minutes: Option<u64>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum PublisherConfig {
    Telegram {
        bot_token: String,
        chat_id: String,
        parse_mode: Option<String>,
        message_thread_id: Option<String>,
        template: Option<String>,
    },
    X {
        client_id: String,
        client_secret: String,
        access_token: Option<String>,
        refresh_token: Option<String>,
        redirect_uri: Option<String>,
        template: Option<String>,
    },
    Mastodon {
        server_url: String,
        access_token: String,
        template: Option<String>,
    },
    LinkedIn {
        client_id: String,
        client_secret: String,
        access_token: Option<String>,
        refresh_token: Option<String>,
        user_id: Option<String>,
        redirect_uri: Option<String>,
        template: Option<String>,
    },
    OpenObserve {
        url: String,
        organization: String,
        stream_name: String,
        access_token: String,
        template: Option<String>,
    },
    Matrix {
        homeserver_url: String,
        access_token: String,
        room_id: String,
        template: Option<String>,
    },
    Bluesky {
        handle: String,
        password: String,
        pds_url: Option<String>,
        template: Option<String>,
    },
    Threads {
        access_token: String,
        user_id: String,
        template: Option<String>,
    },
    Discord {
        webhook_url: String,
        template: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleConfig {
    pub default_interval_minutes: u64,
    pub timezone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub data_dir: String,
    pub published_posts_file: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            feeds: vec![],
            publishers: HashMap::new(),
            youtube: None,
            schedule: ScheduleConfig {
                default_interval_minutes: 60,
                timezone: "UTC".to_string(),
            },
            storage: StorageConfig {
                data_dir: "./data".to_string(),
                published_posts_file: "published_posts.json".to_string(),
            },
        }
    }
}
