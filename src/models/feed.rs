use crate::models::{
    FeedConfig, FeedTypeConfig, Post, YouTubeClient, YouTubeConfig, YouTubeGlobalConfig,
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeedCacheMetadata {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub last_content_hash: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Feed {
    pub config: FeedConfig,
    pub last_checked: Option<DateTime<Utc>>,
    pub last_post_date: Option<DateTime<Utc>>,
    pub cache_metadata: FeedCacheMetadata,
    pub youtube_config: Option<YouTubeGlobalConfig>,
    client: Client,
}

impl Feed {
    pub fn new(config: FeedConfig, youtube_config: Option<YouTubeGlobalConfig>) -> Self {
        Self {
            config,
            last_checked: None,
            last_post_date: None,
            cache_metadata: FeedCacheMetadata::default(),
            youtube_config,
            client: Client::new(),
        }
    }

    pub fn new_with_cache(
        config: FeedConfig,
        cache_metadata: FeedCacheMetadata,
        youtube_config: Option<YouTubeGlobalConfig>,
    ) -> Self {
        Self {
            config,
            last_checked: None,
            last_post_date: None,
            cache_metadata,
            youtube_config,
            client: Client::new(),
        }
    }

    pub async fn fetch_posts(&mut self) -> Result<Vec<Post>> {
        let max_retries = self.config.max_retries.unwrap_or(3);
        let base_delay = self.config.retry_delay_seconds.unwrap_or(2);

        for attempt in 0..=max_retries {
            match self.fetch_posts_attempt().await {
                Ok(posts) => {
                    if attempt > 0 {
                        log::info!(
                            "Successfully fetched feed {} on attempt {}/{}",
                            self.config.name,
                            attempt + 1,
                            max_retries + 1
                        );
                    }
                    return Ok(posts);
                }
                Err(err) => {
                    if attempt == max_retries {
                        log::error!(
                            "Failed to fetch feed {} after {} attempts: {}",
                            self.config.name,
                            max_retries + 1,
                            err
                        );
                        return Err(err);
                    }

                    // Calculate exponential backoff: base_delay * 2^attempt
                    let delay_secs = base_delay * (2_u64.pow(attempt));
                    log::warn!(
                        "Failed to fetch feed {} (attempt {}/{}): {}. Retrying in {} seconds...",
                        self.config.name,
                        attempt + 1,
                        max_retries + 1,
                        err,
                        delay_secs
                    );

                    tokio::time::sleep(tokio::time::Duration::from_secs(delay_secs)).await;
                }
            }
        }

        // This should never be reached due to the return in the loop
        unreachable!()
    }

    async fn fetch_posts_attempt(&mut self) -> Result<Vec<Post>> {
        log::info!("Fetching posts from feed: {}", self.config.name);

        match self.config.feed_type.as_str() {
            "Rss" => {
                if let FeedTypeConfig::Rss { url } = &self.config.config {
                    let url = url.clone();
                    self.fetch_rss_posts(&url).await
                } else {
                    Err(anyhow::anyhow!("Invalid RSS feed configuration"))
                }
            }
            "Youtube" => {
                if let FeedTypeConfig::Youtube {
                    channel_id,
                    playlist_id,
                    username,
                    max_results,
                } = &self.config.config
                {
                    let youtube_global = self
                        .youtube_config
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("YouTube global configuration not found"))?;

                    let effective_max_results = max_results
                        .or(youtube_global.default_max_results)
                        .unwrap_or(10);

                    let youtube_config = YouTubeConfig {
                        api_key: youtube_global.api_key.clone(),
                        channel_id: channel_id.clone(),
                        playlist_id: playlist_id.clone(),
                        username: username.clone(),
                        max_results: Some(effective_max_results),
                    };
                    let youtube_client = YouTubeClient::new(youtube_global.api_key.clone());
                    let posts = youtube_client.fetch_channel_videos(&youtube_config).await?;
                    self.process_youtube_posts(posts)
                } else {
                    Err(anyhow::anyhow!("Invalid YouTube feed configuration"))
                }
            }
            _ => Err(anyhow::anyhow!(
                "Unknown feed type: {}",
                self.config.feed_type
            )),
        }
    }

    async fn fetch_rss_posts(&mut self, url: &str) -> Result<Vec<Post>> {
        // Build request with conditional headers
        let mut request_builder = self
            .client
            .get(url)
            .header("User-Agent", "Populatrs RSS Reader 1.0");

        // Add If-None-Match header if we have an ETag
        if let Some(etag) = &self.cache_metadata.etag {
            request_builder = request_builder.header("If-None-Match", etag);
            log::debug!("Using If-None-Match: {}", etag);
        }

        // Add If-Modified-Since header if we have Last-Modified
        if let Some(last_modified) = &self.cache_metadata.last_modified {
            request_builder = request_builder.header("If-Modified-Since", last_modified);
            log::debug!("Using If-Modified-Since: {}", last_modified);
        }

        let response = request_builder.send().await?;

        // Handle 304 Not Modified
        if response.status() == StatusCode::NOT_MODIFIED {
            log::info!(
                "Feed {} not modified (304), skipping download",
                self.config.name
            );
            self.last_checked = Some(Utc::now());
            return Ok(Vec::new()); // Return empty vec, content hasn't changed
        }

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to fetch feed {}: HTTP {}",
                url,
                response.status()
            ));
        }

        // Update cache metadata from response headers
        if let Some(etag) = response.headers().get("etag") {
            if let Ok(etag_str) = etag.to_str() {
                self.cache_metadata.etag = Some(etag_str.to_string());
                log::debug!("Updated ETag for {}: {}", self.config.name, etag_str);
            }
        }

        if let Some(last_modified) = response.headers().get("last-modified") {
            if let Ok(last_modified_str) = last_modified.to_str() {
                self.cache_metadata.last_modified = Some(last_modified_str.to_string());
                log::debug!(
                    "Updated Last-Modified for {}: {}",
                    self.config.name,
                    last_modified_str
                );
            }
        }

        let content = response.text().await?;

        // Calculate content hash for additional change detection
        let content_hash = format!("{:x}", md5::compute(&content));

        // Check if content actually changed based on hash
        if let Some(last_hash) = &self.cache_metadata.last_content_hash {
            if *last_hash == content_hash {
                log::info!(
                    "Feed {} content unchanged (same hash), skipping parse",
                    self.config.name
                );
                self.last_checked = Some(Utc::now());
                return Ok(Vec::new());
            }
        }

        self.cache_metadata.last_content_hash = Some(content_hash);
        self.last_checked = Some(Utc::now());

        // Try to parse as RSS first
        if let Ok(channel) = content.parse::<rss::Channel>() {
            return self.parse_rss_posts(channel);
        }

        // Try to parse as Atom/RSS using feed-rs
        if let Ok(feed) = feed_rs::parser::parse(content.as_bytes()) {
            return self.parse_feed_rs_posts(feed);
        }

        Err(anyhow::anyhow!("Unable to parse feed"))
    }

    fn process_youtube_posts(&mut self, mut posts: Vec<Post>) -> Result<Vec<Post>> {
        log::info!("Processing {} YouTube videos", posts.len());

        if posts.is_empty() {
            return Ok(posts);
        }

        // Sort by publication date (newest first) to get the latest ones
        posts.sort_by(|a, b| b.published.cmp(&a.published));

        // Update our last post date to the most recent post
        if let Some(latest_post) = posts.first() {
            self.last_post_date = Some(latest_post.published);
        }

        // For YouTube, we want the latest posts regardless of previous processing
        // No date filtering - just take the most recent ones
        log::info!("Available videos after sorting:");
        for (i, post) in posts.iter().enumerate() {
            log::info!("  {}: '{}' ({})", i + 1, post.title, post.published);
        }

        let mut filtered_posts: Vec<Post> = posts
            .into_iter()
            .take(2) // Limit to 2 most recent
            .collect();

        log::info!(
            "Selected {} videos after taking 2 most recent",
            filtered_posts.len()
        );

        // Sort the filtered posts by publication date (oldest first) for correct publication order
        // This ensures that if there are 2 new videos, the older one is published first
        filtered_posts.sort_by(|a, b| a.published.cmp(&b.published));

        log::info!(
            "Selected {} YouTube videos for publishing from feed: {} (ordered for publication)",
            filtered_posts.len(),
            self.config.name
        );

        for (i, post) in filtered_posts.iter().enumerate() {
            log::info!(
                "Will publish #{}: '{}' ({})",
                i + 1,
                post.title,
                post.published
            );
        }

        Ok(filtered_posts)
    }

    fn parse_rss_posts(&mut self, channel: rss::Channel) -> Result<Vec<Post>> {
        let mut posts = Vec::new();
        let mut latest_date: Option<DateTime<Utc>> = None;

        for item in channel.items() {
            if let Some(post) = Post::from_rss_item(item, self.config.id.clone()) {
                // Update latest date
                if latest_date.is_none() || post.published > latest_date.unwrap() {
                    latest_date = Some(post.published);
                }

                // Only include posts newer than our last check (if we have one)
                if let Some(last_post_date) = self.last_post_date {
                    if post.published <= last_post_date {
                        continue;
                    }
                }

                posts.push(post);
            }
        }

        // Update last post date
        if let Some(latest) = latest_date {
            self.last_post_date = Some(latest);
        }

        // Sort posts by publication date (newest first)
        posts.sort_by(|a, b| b.published.cmp(&a.published));
        // Limit to only the 2 most recent posts
        posts.truncate(2);
        log::info!(
            "Found {} new posts in feed: {}",
            posts.len(),
            self.config.name
        );
        Ok(posts)
    }

    fn parse_feed_rs_posts(&mut self, feed: feed_rs::model::Feed) -> Result<Vec<Post>> {
        let mut posts = Vec::new();
        let mut latest_date: Option<DateTime<Utc>> = None;

        for entry in feed.entries {
            if let Some(post) = Post::from_feed_item(&entry, self.config.id.clone()) {
                // Update latest date
                if latest_date.is_none() || post.published > latest_date.unwrap() {
                    latest_date = Some(post.published);
                }

                // Only include posts newer than our last check (if we have one)
                if let Some(last_post_date) = self.last_post_date {
                    if post.published <= last_post_date {
                        continue;
                    }
                }

                posts.push(post);
            }
        }

        // Update last post date
        if let Some(latest) = latest_date {
            self.last_post_date = Some(latest);
        }

        // Sort posts by publication date (newest first)
        posts.sort_by(|a, b| b.published.cmp(&a.published));

        // Limit to only the 2 most recent posts
        posts.truncate(2);

        log::info!(
            "Found {} new posts in feed: {}",
            posts.len(),
            self.config.name
        );
        Ok(posts)
    }

    pub fn should_check(&self, default_interval_minutes: u64) -> bool {
        if !self.config.enabled {
            return false;
        }

        let interval_minutes = self
            .config
            .check_interval_minutes
            .unwrap_or(default_interval_minutes);

        match self.last_checked {
            None => true,
            Some(last_checked) => {
                let now = Utc::now();
                let duration_since_check = now.signed_duration_since(last_checked);
                duration_since_check.num_minutes() >= interval_minutes as i64
            }
        }
    }

    pub fn get_publishers(&self) -> &[String] {
        &self.config.publishers
    }
}

pub struct FeedManager {
    feeds: Vec<Feed>,
}

impl FeedManager {
    pub fn new() -> Self {
        Self { feeds: Vec::new() }
    }

    pub fn add_feed(&mut self, config: FeedConfig, youtube_config: Option<YouTubeGlobalConfig>) {
        let feed = Feed::new(config, youtube_config);
        self.feeds.push(feed);
    }

    pub fn load_feeds(
        &mut self,
        configs: Vec<FeedConfig>,
        youtube_config: Option<YouTubeGlobalConfig>,
    ) {
        self.feeds.clear();
        for config in configs {
            self.add_feed(config, youtube_config.clone());
        }
    }

    pub fn load_feeds_with_cache(
        &mut self,
        configs: Vec<FeedConfig>,
        youtube_config: Option<YouTubeGlobalConfig>,
        cache: &crate::storage::FeedCacheStorage,
    ) {
        self.feeds.clear();
        for config in configs {
            let cache_metadata = cache.feeds.get(&config.id).cloned().unwrap_or_default();
            let feed = Feed::new_with_cache(config, cache_metadata, youtube_config.clone());
            self.feeds.push(feed);
        }
    }

    pub fn get_cache_metadata(&self) -> crate::storage::FeedCacheStorage {
        let mut cache = crate::storage::FeedCacheStorage::default();
        for feed in &self.feeds {
            cache
                .feeds
                .insert(feed.config.id.clone(), feed.cache_metadata.clone());
        }
        cache
    }

    pub async fn check_all_feeds(
        &mut self,
        default_interval_minutes: u64,
    ) -> Vec<(String, Result<Vec<Post>>)> {
        let mut results = Vec::new();

        for feed in &mut self.feeds {
            if feed.should_check(default_interval_minutes) {
                log::info!("Checking feed: {}", feed.config.name);
                let result = feed.fetch_posts().await;
                results.push((feed.config.id.clone(), result));
            } else {
                log::debug!("Skipping feed (not due for check): {}", feed.config.name);
            }
        }

        results
    }

    pub fn get_feed(&self, id: &str) -> Option<&Feed> {
        self.feeds.iter().find(|f| f.config.id == id)
    }

    pub fn get_feed_mut(&mut self, id: &str) -> Option<&mut Feed> {
        self.feeds.iter_mut().find(|f| f.config.id == id)
    }

    pub fn list_feeds(&self) -> Vec<(&str, &str, bool)> {
        self.feeds
            .iter()
            .map(|f| {
                (
                    f.config.id.as_str(),
                    f.config.name.as_str(),
                    f.config.enabled,
                )
            })
            .collect()
    }

    pub fn get_enabled_feeds_count(&self) -> usize {
        self.feeds.iter().filter(|f| f.config.enabled).count()
    }
}
