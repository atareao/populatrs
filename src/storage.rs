use crate::models::{AppConfig, FeedCacheMetadata, PublishedPostsStorage};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeedCacheStorage {
    pub feeds: HashMap<String, FeedCacheMetadata>,
}

#[derive(Clone)]
pub struct StorageManager {
    data_dir: String,
    published_posts_file: String,
    feed_cache_file: String,
}

impl StorageManager {
    pub fn new(data_dir: String, published_posts_file: String) -> Self {
        Self {
            data_dir,
            published_posts_file,
            feed_cache_file: "feed_cache.json".to_string(),
        }
    }

    pub fn init(&self) -> Result<()> {
        // Create data directory if it doesn't exist
        fs::create_dir_all(&self.data_dir)?;
        log::info!("Storage initialized in directory: {}", self.data_dir);
        Ok(())
    }

    pub fn load_published_posts(&self) -> Result<PublishedPostsStorage> {
        let file_path = Path::new(&self.data_dir).join(&self.published_posts_file);

        if !file_path.exists() {
            log::info!("Published posts file doesn't exist, creating new storage");
            return Ok(PublishedPostsStorage::new());
        }

        let content = fs::read_to_string(&file_path)?;
        let storage: PublishedPostsStorage = serde_json::from_str(&content).unwrap_or_else(|_| {
            log::warn!("Failed to parse published posts file, creating new storage");
            PublishedPostsStorage::new()
        });

        log::info!(
            "Loaded {} published posts from storage",
            storage.posts.len()
        );
        Ok(storage)
    }

    pub fn save_published_posts(&self, storage: &PublishedPostsStorage) -> Result<()> {
        let file_path = Path::new(&self.data_dir).join(&self.published_posts_file);
        let content = serde_json::to_string_pretty(storage)?;
        fs::write(&file_path, content)?;
        log::debug!("Saved {} published posts to storage", storage.posts.len());
        Ok(())
    }

    pub fn load_feed_cache(&self) -> Result<FeedCacheStorage> {
        let file_path = Path::new(&self.data_dir).join(&self.feed_cache_file);

        if !file_path.exists() {
            log::info!("Feed cache file doesn't exist, creating new cache storage");
            return Ok(FeedCacheStorage::default());
        }

        let content = fs::read_to_string(&file_path)?;
        let cache: FeedCacheStorage = serde_json::from_str(&content).unwrap_or_else(|_| {
            log::warn!("Failed to parse feed cache file, creating new cache storage");
            FeedCacheStorage::default()
        });

        log::info!("Loaded cache for {} feeds from storage", cache.feeds.len());
        Ok(cache)
    }

    pub fn save_feed_cache(&self, cache: &FeedCacheStorage) -> Result<()> {
        let file_path = Path::new(&self.data_dir).join(&self.feed_cache_file);
        let content = serde_json::to_string_pretty(cache)?;
        fs::write(&file_path, content)?;
        log::debug!("Saved cache for {} feeds to storage", cache.feeds.len());
        Ok(())
    }

    pub fn load_config_from_file(file_path: &str) -> Result<AppConfig> {
        if !Path::new(file_path).exists() {
            log::warn!(
                "Config file {} doesn't exist, creating default config",
                file_path
            );
            let default_config = AppConfig::default();
            let content = serde_json::to_string_pretty(&default_config)?;
            fs::write(file_path, content)?;
            return Ok(default_config);
        }

        let content = fs::read_to_string(file_path)?;
        let config: AppConfig = serde_json::from_str(&content)?;
        log::info!("Loaded configuration from: {}", file_path);
        Ok(config)
    }

    pub fn save_config_to_file(config: &AppConfig, file_path: &str) -> Result<()> {
        let content = serde_json::to_string_pretty(config)?;
        fs::write(file_path, content)?;
        log::info!("Saved configuration to: {}", file_path);
        Ok(())
    }

    pub fn backup_published_posts(&self) -> Result<()> {
        let source_path = Path::new(&self.data_dir).join(&self.published_posts_file);
        if !source_path.exists() {
            return Ok(());
        }

        let backup_filename = format!(
            "{}.backup.{}",
            self.published_posts_file,
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        );
        let backup_path = Path::new(&self.data_dir).join(backup_filename);

        fs::copy(&source_path, &backup_path)?;
        log::info!("Created backup: {}", backup_path.display());
        Ok(())
    }

    pub fn cleanup_old_backups(&self, days_to_keep: u64) -> Result<()> {
        let dir = fs::read_dir(&self.data_dir)?;
        let cutoff_date = std::time::SystemTime::now()
            - std::time::Duration::from_secs(days_to_keep * 24 * 60 * 60);

        for entry in dir {
            let entry = entry?;
            let path = entry.path();

            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.contains(".backup.") {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(created) = metadata.created() {
                            if created < cutoff_date {
                                if let Err(e) = fs::remove_file(&path) {
                                    log::warn!(
                                        "Failed to remove old backup {}: {}",
                                        path.display(),
                                        e
                                    );
                                } else {
                                    log::info!("Removed old backup: {}", path.display());
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
