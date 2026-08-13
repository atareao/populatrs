use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use tokio::sync::Mutex;

use crate::models::{
    FeedConfig, FeedLogEntry, FeedLogPublisherResult, FeedLogResponse, FeedPublisherBinding,
    FeedTypeConfig, PublishedPostInfo, PublisherConfig, RetryPolicy, ScheduleConfig,
    YouTubeGlobalConfig,
};

/// Database handle wrapping a SQLite connection.
#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Open (or create) the SQLite database at `path` and run migrations.
    pub async fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create data directory")?;
        }

        let conn = Connection::open(path).context("Failed to open SQLite database")?;

        // Enable WAL mode for better concurrent access
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .context("Failed to set PRAGMAs")?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.run_migrations().await?;
        Ok(db)
    }

    /// Create tables if they don't exist.
    async fn run_migrations(&self) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS feeds (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                feed_type TEXT NOT NULL,
                config_json TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                max_retries INTEGER,
                retry_delay_seconds INTEGER,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS publishers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                publisher_type TEXT NOT NULL,
                config_json TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS feed_publishers (
                feed_id TEXT NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
                publisher_id TEXT NOT NULL REFERENCES publishers(id) ON DELETE CASCADE,
                template TEXT,
                PRIMARY KEY (feed_id, publisher_id)
            );

            CREATE TABLE IF NOT EXISTS published_posts (
                guid TEXT NOT NULL,
                feed_id TEXT NOT NULL,
                title TEXT NOT NULL,
                url TEXT NOT NULL,
                content_hash TEXT,
                published_at TEXT NOT NULL,
                PRIMARY KEY (guid, feed_id)
            );

            CREATE TABLE IF NOT EXISTS publish_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                guid TEXT NOT NULL,
                feed_id TEXT NOT NULL,
                publisher_id TEXT NOT NULL,
                success INTEGER NOT NULL,
                message TEXT,
                published_at TEXT NOT NULL,
                FOREIGN KEY (guid, feed_id) REFERENCES published_posts(guid, feed_id)
            );

            CREATE INDEX IF NOT EXISTS idx_publish_results_published_at
                ON publish_results(published_at);

            CREATE TABLE IF NOT EXISTS feed_cache (
                feed_id TEXT PRIMARY KEY REFERENCES feeds(id) ON DELETE CASCADE,
                etag TEXT,
                last_modified TEXT,
                last_content_hash TEXT
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )
        .context("Failed to run database migrations")?;

        // Migration: add template column to feed_publishers for existing databases
        let _ = conn.execute_batch("ALTER TABLE feed_publishers ADD COLUMN template TEXT;");

        // Migration: add description column to published_posts for existing databases
        let _ = conn.execute_batch("ALTER TABLE published_posts ADD COLUMN description TEXT;");

        Ok(())
    }

    // ───── Feeds ─────

    /// List all feeds.
    pub async fn list_feeds(&self) -> Result<Vec<FeedConfig>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, feed_type, config_json, enabled, \
                 max_retries, retry_delay_seconds, created_at, updated_at FROM feeds ORDER BY name",
            )
            .context("Failed to prepare list_feeds")?;

        let feeds = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let feed_type: String = row.get(2)?;
                let config_json: String = row.get(3)?;
                let enabled: bool = row.get::<_, i32>(4)? != 0;
                let max_retries: Option<i32> = row.get(5)?;
                let retry_delay: Option<i64> = row.get(6)?;

                let config: FeedTypeConfig = serde_json::from_str(&config_json)
                    .unwrap_or(FeedTypeConfig::Rss { url: String::new() });

                Ok(FeedConfig {
                    id,
                    name,
                    feed_type,
                    config,
                    enabled,
                    publishers: Vec::new(), // filled below
                    max_retries: max_retries.map(|v| v as u32),
                    retry_delay_seconds: retry_delay.map(|v| v as u64),
                })
            })
            .context("Failed to query feeds")?;

        let mut result = Vec::new();
        for feed in feeds {
            let mut feed = feed?;
            // Load linked publishers with optional template override
            let mut pstmt = conn
                .prepare("SELECT publisher_id, template FROM feed_publishers WHERE feed_id = ?1")
                .context("Failed to prepare feed_publishers query")?;
            let bindings: Vec<FeedPublisherBinding> = pstmt
                .query_map(params![&feed.id], |row| {
                    let publisher_id: String = row.get(0)?;
                    let template: Option<String> = row.get(1)?;
                    Ok(FeedPublisherBinding {
                        publisher_id,
                        template,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            feed.publishers = bindings;
            result.push(feed);
        }
        Ok(result)
    }

    /// Get a single feed by ID.
    pub async fn get_feed(&self, id: &str) -> Result<Option<FeedConfig>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, feed_type, config_json, enabled, \
                 max_retries, retry_delay_seconds FROM feeds WHERE id = ?1",
            )
            .context("Failed to prepare get_feed")?;

        let feed = stmt
            .query_row(params![id], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let feed_type: String = row.get(2)?;
                let config_json: String = row.get(3)?;
                let enabled: bool = row.get::<_, i32>(4)? != 0;
                let max_retries: Option<i32> = row.get(5)?;
                let retry_delay: Option<i64> = row.get(6)?;

                let config: FeedTypeConfig = serde_json::from_str(&config_json)
                    .unwrap_or(FeedTypeConfig::Rss { url: String::new() });

                Ok(FeedConfig {
                    id,
                    name,
                    feed_type,
                    config,
                    enabled,
                    publishers: Vec::new(),
                    max_retries: max_retries.map(|v| v as u32),
                    retry_delay_seconds: retry_delay.map(|v| v as u64),
                })
            })
            .optional()
            .context("Failed to query feed")?;

        if let Some(mut feed) = feed {
            let mut pstmt = conn
                .prepare("SELECT publisher_id, template FROM feed_publishers WHERE feed_id = ?1")?;
            let bindings: Vec<FeedPublisherBinding> = pstmt
                .query_map(params![id], |row| {
                    let publisher_id: String = row.get(0)?;
                    let template: Option<String> = row.get(1)?;
                    Ok(FeedPublisherBinding {
                        publisher_id,
                        template,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            feed.publishers = bindings;
            Ok(Some(feed))
        } else {
            Ok(None)
        }
    }

    /// Create a new feed.
    pub async fn create_feed(&self, feed: &FeedConfig) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let config_json =
            serde_json::to_string(&feed.config).context("Failed to serialize feed config")?;

        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO feeds (id, name, feed_type, config_json, enabled, \
             max_retries, retry_delay_seconds, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                feed.id,
                feed.name,
                feed.feed_type,
                config_json,
                feed.enabled as i32,
                feed.max_retries.map(|v| v as i32),
                feed.retry_delay_seconds.map(|v| v as i64),
                now,
                now,
            ],
        )
        .context("Failed to insert feed")?;

        // Link publishers
        for binding in &feed.publishers {
            conn.execute(
                "INSERT OR IGNORE INTO feed_publishers (feed_id, publisher_id, template) VALUES (?1, ?2, ?3)",
                params![feed.id, binding.publisher_id, binding.template],
            )?;
        }

        Ok(())
    }

    /// Update an existing feed.
    pub async fn update_feed(&self, id: &str, feed: &FeedConfig) -> Result<bool> {
        let now = Utc::now().to_rfc3339();
        let config_json =
            serde_json::to_string(&feed.config).context("Failed to serialize feed config")?;

        let conn = self.conn.lock().await;
        let rows = conn
            .execute(
                "UPDATE feeds SET name = ?1, feed_type = ?2, config_json = ?3, enabled = ?4, \
                 max_retries = ?5, retry_delay_seconds = ?6, updated_at = ?7 \
                 WHERE id = ?8",
                params![
                    feed.name,
                    feed.feed_type,
                    config_json,
                    feed.enabled as i32,
                    feed.max_retries.map(|v| v as i32),
                    feed.retry_delay_seconds.map(|v| v as i64),
                    now,
                    id,
                ],
            )
            .context("Failed to update feed")?;

        if rows == 0 {
            return Ok(false);
        }

        // Update publisher links
        conn.execute(
            "DELETE FROM feed_publishers WHERE feed_id = ?1",
            params![id],
        )?;
        for binding in &feed.publishers {
            conn.execute(
                "INSERT OR IGNORE INTO feed_publishers (feed_id, publisher_id, template) VALUES (?1, ?2, ?3)",
                params![id, binding.publisher_id, binding.template],
            )?;
        }

        Ok(true)
    }

    /// Delete a feed by ID.
    pub async fn delete_feed(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().await;
        let rows = conn
            .execute("DELETE FROM feeds WHERE id = ?1", params![id])
            .context("Failed to delete feed")?;
        Ok(rows > 0)
    }

    /// Toggle feed enabled/disabled.
    pub async fn toggle_feed(&self, id: &str) -> Result<Option<bool>> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();
        let rows = conn
            .execute(
                "UPDATE feeds SET enabled = CASE WHEN enabled = 1 THEN 0 ELSE 1 END, updated_at = ?1 WHERE id = ?2",
                params![now, id],
            )
            .context("Failed to toggle feed")?;
        if rows == 0 {
            return Ok(None);
        }
        let enabled: bool = conn
            .query_row(
                "SELECT enabled FROM feeds WHERE id = ?1",
                params![id],
                |row| row.get::<_, i32>(0).map(|v| v != 0),
            )
            .optional()
            .context("Failed to read feed after toggle")?
            .unwrap_or(false);
        Ok(Some(enabled))
    }

    // ───── Publishers ─────

    /// List all publishers.
    pub async fn list_publishers(&self) -> Result<HashMap<String, (PublisherConfig, bool)>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare("SELECT id, config_json, enabled FROM publishers ORDER BY name")
            .context("Failed to prepare list_publishers")?;

        let publishers = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let config_json: String = row.get(1)?;
                let enabled: bool = row.get::<_, i32>(2)? != 0;
                Ok((id, config_json, enabled))
            })
            .context("Failed to query publishers")?;

        let mut result = HashMap::new();
        for entry in publishers {
            let (id, config_json, enabled) = entry?;
            if let Ok(config) = serde_json::from_str::<PublisherConfig>(&config_json) {
                result.insert(id, (config, enabled));
            }
        }
        Ok(result)
    }

    /// Get a single publisher config by ID.
    pub async fn get_publisher(&self, id: &str) -> Result<Option<PublisherConfig>> {
        let publishers = self.list_publishers().await?;
        Ok(publishers
            .into_iter()
            .find(|(k, _)| k == id)
            .map(|(_, (v, _))| v))
    }

    /// Get the enabled status of a publisher.
    pub async fn get_publisher_enabled(&self, id: &str) -> Result<Option<bool>> {
        let publishers = self.list_publishers().await?;
        Ok(publishers
            .into_iter()
            .find(|(k, _)| k == id)
            .map(|(_, (_, enabled))| enabled))
    }

    /// Create or update a publisher.
    pub async fn upsert_publisher(
        &self,
        id: &str,
        config: &PublisherConfig,
        enabled: bool,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let ptype = config.type_name().to_lowercase();
        let config_json =
            serde_json::to_string(config).context("Failed to serialize publisher config")?;
        let name = id.to_string();

        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO publishers (id, name, publisher_type, config_json, enabled, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(id) DO UPDATE SET \
             name = excluded.name, publisher_type = excluded.publisher_type, \
             config_json = excluded.config_json, enabled = excluded.enabled, updated_at = excluded.updated_at",
            params![id, name, ptype, config_json, enabled as i32, now, now],
        )
        .context("Failed to upsert publisher")?;
        Ok(())
    }

    /// Set the enabled status of a publisher.
    pub async fn set_publisher_enabled(&self, id: &str, enabled: bool) -> Result<bool> {
        let conn = self.conn.lock().await;
        let rows = conn
            .execute(
                "UPDATE publishers SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
                params![enabled as i32, Utc::now().to_rfc3339(), id],
            )
            .context("Failed to set publisher enabled")?;
        Ok(rows > 0)
    }

    /// Delete a publisher by ID.
    pub async fn delete_publisher(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().await;
        let rows = conn
            .execute("DELETE FROM publishers WHERE id = ?1", params![id])
            .context("Failed to delete publisher")?;
        Ok(rows > 0)
    }

    // ───── Published Posts ─────

    /// Check if a post has been published.
    pub async fn is_post_published(&self, guid: &str, feed_id: &str) -> Result<bool> {
        let conn = self.conn.lock().await;
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM published_posts WHERE guid = ?1 AND feed_id = ?2",
                params![guid, feed_id],
                |_| Ok(true),
            )
            .optional()
            .context("Failed to check published post")?
            .unwrap_or(false);
        Ok(exists)
    }

    /// Mark a post as published.
    pub async fn mark_post_published(
        &self,
        guid: &str,
        feed_id: &str,
        title: &str,
        url: &str,
        content_hash: Option<&str>,
        description: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT OR IGNORE INTO published_posts (guid, feed_id, title, url, content_hash, description, published_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![guid, feed_id, title, url, content_hash, description, now],
        )
        .context("Failed to mark post as published")?;
        Ok(())
    }

    /// Get a published post's title, url and description by guid + feed_id.
    pub async fn get_published_post(
        &self,
        guid: &str,
        feed_id: &str,
    ) -> Result<Option<PublishedPostInfo>> {
        let conn = self.conn.lock().await;
        let post = conn
            .query_row(
                "SELECT title, url, description FROM published_posts WHERE guid = ?1 AND feed_id = ?2",
                params![guid, feed_id],
                |row| {
                    Ok(PublishedPostInfo {
                        title: row.get(0)?,
                        url: row.get(1)?,
                        description: row.get(2)?,
                    })
                },
            )
            .optional()
            .context("Failed to query published post")?;
        Ok(post)
    }

    /// Replace the publish result for a (guid, feed_id, publisher_id) triple.
    /// Deletes any existing result and inserts the new one, so a republish
    /// attempt updates the tag shown in the UI.
    pub async fn replace_publish_result(
        &self,
        guid: &str,
        feed_id: &str,
        publisher_id: &str,
        success: bool,
        message: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().await;
        // Wrap the delete+insert in an explicit transaction so a failure in the
        // INSERT cannot leave the previous result lost.
        conn.execute("BEGIN", [])
            .context("Failed to begin transaction")?;
        let result = (|| -> Result<()> {
            conn.execute(
                "DELETE FROM publish_results WHERE guid = ?1 AND feed_id = ?2 AND publisher_id = ?3",
                params![guid, feed_id, publisher_id],
            )
            .context("Failed to delete existing publish result")?;
            conn.execute(
                "INSERT INTO publish_results (guid, feed_id, publisher_id, success, message, published_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![guid, feed_id, publisher_id, success as i32, message, now],
            )
            .context("Failed to insert publish result")?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])
                    .context("Failed to commit transaction")?;
                Ok(())
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    /// Record a publish result for a specific publisher.
    /// Skips if a result already exists for this (guid, feed_id, publisher_id).
    pub async fn record_publish_result(
        &self,
        guid: &str,
        feed_id: &str,
        publisher_id: &str,
        success: bool,
        message: Option<&str>,
    ) -> Result<()> {
        // Check if already recorded — prevents duplicates
        if self
            .is_publish_result_recorded(guid, feed_id, publisher_id)
            .await
            .unwrap_or(false)
        {
            return Ok(());
        }
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO publish_results (guid, feed_id, publisher_id, success, message, published_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![guid, feed_id, publisher_id, success as i32, message, now],
        )
        .context("Failed to record publish result")?;
        Ok(())
    }

    /// Check if a publish result already exists for this (guid, feed_id, publisher_id).
    pub async fn is_publish_result_recorded(
        &self,
        guid: &str,
        feed_id: &str,
        publisher_id: &str,
    ) -> Result<bool> {
        let conn = self.conn.lock().await;
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM publish_results WHERE guid = ?1 AND feed_id = ?2 AND publisher_id = ?3",
                params![guid, feed_id, publisher_id],
                |_| Ok(true),
            )
            .optional()
            .context("Failed to check publish result")?
            .unwrap_or(false);
        Ok(exists)
    }

    /// Clean up old published posts (older than `days`).
    pub async fn cleanup_old_posts(&self, days: i64) -> Result<u64> {
        let conn = self.conn.lock().await;
        let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        let deleted = conn
            .execute(
                "DELETE FROM published_posts WHERE published_at < ?1",
                params![cutoff],
            )
            .context("Failed to cleanup old posts")?;
        Ok(deleted as u64)
    }

    /// Clean up old publish results (older than `days`).
    pub async fn cleanup_old_publish_results(&self, days: i64) -> Result<u64> {
        let conn = self.conn.lock().await;
        let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        let deleted = conn
            .execute(
                "DELETE FROM publish_results WHERE published_at < ?1",
                params![cutoff],
            )
            .context("Failed to cleanup old publish results")?;
        Ok(deleted as u64)
    }

    /// List feed log entries with their publisher results.
    pub async fn list_feed_logs(&self, limit: i64, offset: i64) -> Result<FeedLogResponse> {
        let conn = self.conn.lock().await;

        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM published_posts", [], |row| row.get(0))
            .context("Failed to count feed logs")?;

        let retention_days: u64 = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'log_retention_days'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        let mut stmt = conn
            .prepare(
                "SELECT pp.guid, pp.feed_id, pp.title, pp.url, pp.published_at,
                        pr.publisher_id, pr.success, pr.message
                 FROM published_posts pp
                 LEFT JOIN publish_results pr ON pp.guid = pr.guid AND pp.feed_id = pr.feed_id
                 ORDER BY pp.published_at DESC
                 LIMIT ?1 OFFSET ?2",
            )
            .context("Failed to prepare list_feed_logs")?;

        let rows = stmt
            .query_map(params![limit, offset], |row| {
                let guid: String = row.get(0)?;
                let feed_id: String = row.get(1)?;
                let title: String = row.get(2)?;
                let url: String = row.get(3)?;
                let published_at: String = row.get(4)?;
                let publisher_id: Option<String> = row.get(5)?;
                let success: Option<i32> = row.get(6)?;
                let message: Option<String> = row.get(7)?;
                Ok((
                    guid,
                    feed_id,
                    title,
                    url,
                    published_at,
                    publisher_id,
                    success,
                    message,
                ))
            })
            .context("Failed to query feed logs")?;

        // Group by post, preserving ORDER BY
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        let mut entries: Vec<FeedLogEntry> = Vec::new();

        for row in rows {
            let (guid, feed_id, title, url, published_at, publisher_id, success, message) = row?;

            let key = (guid.clone(), feed_id.clone());
            if seen.insert(key) {
                entries.push(FeedLogEntry {
                    guid: guid.clone(),
                    feed_id: feed_id.clone(),
                    title: title.clone(),
                    url: url.clone(),
                    published_at: published_at.clone(),
                    publisher_results: Vec::new(),
                });
            }

            if let Some(pid) = publisher_id {
                if let Some(entry) = entries.last_mut() {
                    entry.publisher_results.push(FeedLogPublisherResult {
                        publisher_id: pid,
                        success: success.unwrap_or(0) != 0,
                        message: message.unwrap_or_default(),
                    });
                }
            }
        }

        Ok(FeedLogResponse {
            entries,
            total: total as u64,
            retention_days,
        })
    }

    /// Get the log retention days setting.
    pub async fn get_log_retention(&self) -> Result<u64> {
        Ok(self
            .get_setting("log_retention_days")
            .await
            .ok()
            .flatten()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(30))
    }

    /// Set the log retention days setting.
    pub async fn set_log_retention(&self, days: u64) -> Result<()> {
        self.set_setting("log_retention_days", &days.to_string())
            .await
    }

    /// Get the retry policy from settings. Returns default if not set.
    pub async fn get_retry_policy(&self) -> Result<RetryPolicy> {
        match self.get_setting("retry_policy").await? {
            Some(json_str) => serde_json::from_str(&json_str)
                .map_err(|e| anyhow::anyhow!("Failed to parse retry policy: {e}")),
            None => Ok(RetryPolicy::default()),
        }
    }

    /// Set the retry policy in settings.
    pub async fn set_retry_policy(&self, policy: &RetryPolicy) -> Result<()> {
        let json_str = serde_json::to_string(policy).context("Failed to serialize retry policy")?;
        self.set_setting("retry_policy", &json_str).await
    }

    // ───── Feed Cache ─────

    /// Get cached ETag for a feed.
    pub async fn get_feed_cache(
        &self,
        feed_id: &str,
    ) -> Result<Option<(Option<String>, Option<String>, Option<String>)>> {
        let conn = self.conn.lock().await;
        let result = conn
            .query_row(
                "SELECT etag, last_modified, last_content_hash FROM feed_cache WHERE feed_id = ?1",
                params![feed_id],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()
            .context("Failed to query feed cache")?;
        Ok(result)
    }

    /// Update feed cache.
    pub async fn upsert_feed_cache(
        &self,
        feed_id: &str,
        etag: Option<&str>,
        last_modified: Option<&str>,
        last_content_hash: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO feed_cache (feed_id, etag, last_modified, last_content_hash) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(feed_id) DO UPDATE SET \
             etag = excluded.etag, last_modified = excluded.last_modified, \
             last_content_hash = excluded.last_content_hash",
            params![feed_id, etag, last_modified, last_content_hash],
        )
        .context("Failed to upsert feed cache")?;
        Ok(())
    }

    // ───── Settings ─────

    /// Get a setting value.
    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().await;
        let value = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .context("Failed to query setting")?;
        Ok(value)
    }

    /// Set a setting value.
    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )
        .context("Failed to set setting")?;
        Ok(())
    }

    /// Get schedule configuration (from settings table).
    pub async fn get_schedule(&self) -> Result<ScheduleConfig> {
        // Try new key first
        let cron = self.get_setting("schedule_cron").await?;
        match cron {
            Some(expr) => Ok(ScheduleConfig {
                cron_expression: expr,
                timezone: self
                    .get_setting("schedule_timezone")
                    .await?
                    .unwrap_or_else(|| "UTC".to_string()),
            }),
            None => {
                // Migration from old interval-based schedule
                let old_interval = self
                    .get_setting("schedule_interval")
                    .await?
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(60);
                let cron_expr = format!("*/{} * * * *", old_interval.min(59));
                let timezone = self
                    .get_setting("schedule_timezone")
                    .await?
                    .unwrap_or_else(|| "UTC".to_string());
                // Save new format
                let config = ScheduleConfig {
                    cron_expression: cron_expr,
                    timezone,
                };
                self.set_schedule(&config).await?;
                Ok(config)
            }
        }
    }

    /// Update schedule configuration.
    pub async fn set_schedule(&self, schedule: &ScheduleConfig) -> Result<()> {
        self.set_setting("schedule_cron", &schedule.cron_expression)
            .await?;
        self.set_setting("schedule_timezone", &schedule.timezone)
            .await?;
        Ok(())
    }

    // ───── YouTube Config ─────

    /// Load YouTube global configuration from settings table.
    pub async fn get_youtube_config(&self) -> Result<Option<YouTubeGlobalConfig>> {
        let api_key = self.get_setting("youtube_api_key").await?;
        match api_key {
            Some(key) => {
                let default_max_results = self
                    .get_setting("youtube_default_max_results")
                    .await?
                    .and_then(|v| v.parse::<u64>().ok());
                Ok(Some(YouTubeGlobalConfig {
                    api_key: key,
                    default_max_results,
                }))
            }
            None => Ok(None),
        }
    }

    /// Set YouTube global configuration in settings table.
    pub async fn set_youtube_config(&self, config: &YouTubeGlobalConfig) -> Result<()> {
        self.set_setting("youtube_api_key", &config.api_key).await?;
        self.set_setting(
            "youtube_default_max_results",
            &config
                .default_max_results
                .map(|v| v.to_string())
                .unwrap_or_default(),
        )
        .await?;
        Ok(())
    }

    /// Set only the YouTube API key (used by the Settings UI).
    pub async fn set_youtube_api_key(&self, api_key: &str) -> Result<()> {
        self.set_setting("youtube_api_key", api_key).await?;
        Ok(())
    }

    // ───── Stats ─────

    /// Get dashboard stats.
    pub async fn get_stats(&self) -> Result<Stats> {
        let conn = self.conn.lock().await;

        let total_feeds: i64 =
            conn.query_row("SELECT COUNT(*) FROM feeds", [], |row| row.get(0))?;
        let enabled_feeds: i64 =
            conn.query_row("SELECT COUNT(*) FROM feeds WHERE enabled = 1", [], |row| {
                row.get(0)
            })?;
        let total_publishers: i64 =
            conn.query_row("SELECT COUNT(*) FROM publishers", [], |row| row.get(0))?;
        let total_published: i64 =
            conn.query_row("SELECT COUNT(*) FROM published_posts", [], |row| row.get(0))?;

        // ⚠️ Inline schedule queries while holding the lock to avoid deadlock.
        //    Do NOT call self.get_schedule() here — it would try to re-acquire
        //    the same tokio::sync::Mutex, which is not reentrant.
        let cron_expression: String = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'schedule_cron'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("Failed to query schedule_cron")?
            .unwrap_or_else(|| "0 * * * *".to_string());
        let timezone: String = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'schedule_timezone'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("Failed to query schedule_timezone")?
            .unwrap_or_else(|| "UTC".to_string());

        Ok(Stats {
            total_feeds: total_feeds as u64,
            enabled_feeds: enabled_feeds as u64,
            total_publishers: total_publishers as u64,
            total_published: total_published as u64,
            schedule: ScheduleConfig {
                cron_expression,
                timezone,
            },
        })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Stats {
    pub total_feeds: u64,
    pub enabled_feeds: u64,
    pub total_publishers: u64,
    pub total_published: u64,
    pub schedule: ScheduleConfig,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn test_db() -> (Database, TempDir) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.db");
        let db = Database::open(&path).await.unwrap();
        (db, dir)
    }

    #[tokio::test]
    async fn test_create_and_list_feeds() {
        let (db, _dir) = test_db().await;
        let feed = FeedConfig {
            id: "test-feed".into(),
            name: "Test Feed".into(),
            feed_type: "rss".into(),
            config: FeedTypeConfig::Rss {
                url: "https://example.com/feed.xml".into(),
            },
            enabled: true,
            publishers: vec![],
            max_retries: Some(3),
            retry_delay_seconds: Some(5),
        };
        db.create_feed(&feed).await.unwrap();
        let feeds = db.list_feeds().await.unwrap();
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].id, "test-feed");
        assert_eq!(feeds[0].name, "Test Feed");
    }

    #[tokio::test]
    async fn test_get_feed_not_found() {
        let (db, _dir) = test_db().await;
        let feed = db.get_feed("nonexistent").await.unwrap();
        assert!(feed.is_none());
    }

    #[tokio::test]
    async fn test_update_feed() {
        let (db, _dir) = test_db().await;
        let feed = FeedConfig {
            id: "feed-1".into(),
            name: "Original".into(),
            feed_type: "rss".into(),
            config: FeedTypeConfig::Rss {
                url: "https://ex.com/feed.xml".into(),
            },
            enabled: true,
            publishers: vec![],
            max_retries: None,
            retry_delay_seconds: None,
        };
        db.create_feed(&feed).await.unwrap();

        let updated = FeedConfig {
            name: "Updated".into(),
            ..feed
        };
        db.update_feed("feed-1", &updated).await.unwrap();
        let fetched = db.get_feed("feed-1").await.unwrap().unwrap();
        assert_eq!(fetched.name, "Updated");
    }

    #[tokio::test]
    async fn test_delete_feed() {
        let (db, _dir) = test_db().await;
        let feed = FeedConfig {
            id: "to-delete".into(),
            name: "Delete Me".into(),
            feed_type: "rss".into(),
            config: FeedTypeConfig::Rss {
                url: "https://ex.com/feed.xml".into(),
            },
            enabled: true,
            publishers: vec![],
            max_retries: None,
            retry_delay_seconds: None,
        };
        db.create_feed(&feed).await.unwrap();
        assert!(db.delete_feed("to-delete").await.unwrap());
        assert!(db.get_feed("to-delete").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_toggle_feed() {
        let (db, _dir) = test_db().await;
        let feed = FeedConfig {
            id: "toggle-me".into(),
            name: "Toggle".into(),
            feed_type: "rss".into(),
            config: FeedTypeConfig::Rss {
                url: "https://ex.com/feed.xml".into(),
            },
            enabled: true,
            publishers: vec![],
            max_retries: None,
            retry_delay_seconds: None,
        };
        db.create_feed(&feed).await.unwrap();
        let enabled = db.toggle_feed("toggle-me").await.unwrap();
        assert_eq!(enabled, Some(false));
        let enabled = db.toggle_feed("toggle-me").await.unwrap();
        assert_eq!(enabled, Some(true));
    }

    #[tokio::test]
    async fn test_publishers_roundtrip() {
        let (db, _dir) = test_db().await;
        let config = PublisherConfig::Telegram {
            bot_token: "123:abc".into(),
            chat_id: "-100".into(),
            parse_mode: Some("HTML".into()),
            message_thread_id: None,
            template: "📰 *{{ title }}*".into(),
        };
        db.upsert_publisher("telegram-1", &config, true)
            .await
            .unwrap();
        let publishers = db.list_publishers().await.unwrap();
        assert_eq!(publishers.len(), 1);
        assert!(publishers.contains_key("telegram-1"));
    }

    #[tokio::test]
    async fn test_published_posts() {
        let (db, _dir) = test_db().await;
        assert!(!db.is_post_published("guid-1", "feed-1").await.unwrap());
        db.mark_post_published(
            "guid-1",
            "feed-1",
            "Test Post",
            "https://ex.com",
            None,
            None,
        )
        .await
        .unwrap();
        assert!(db.is_post_published("guid-1", "feed-1").await.unwrap());
    }

    #[tokio::test]
    async fn test_settings() {
        let (db, _dir) = test_db().await;
        db.set_setting("test_key", "test_value").await.unwrap();
        let val = db.get_setting("test_key").await.unwrap();
        assert_eq!(val, Some("test_value".to_string()));
    }

    #[tokio::test]
    async fn test_schedule() {
        let (db, _dir) = test_db().await;
        let sched = ScheduleConfig {
            cron_expression: "0 */2 * * *".to_string(),
            timezone: "Europe/Madrid".into(),
        };
        db.set_schedule(&sched).await.unwrap();
        let loaded = db.get_schedule().await.unwrap();
        assert_eq!(loaded.cron_expression, "0 */2 * * *");
        assert_eq!(loaded.timezone, "Europe/Madrid");
    }
}
