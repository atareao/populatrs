pub mod auth;
pub mod config;
pub mod db;
pub mod embed;
pub mod feed;
pub mod middleware;
pub mod models;
pub mod publisher;
pub mod routes;
pub mod template;

use std::sync::Arc;
use anyhow::Result;
use tokio::sync::Mutex;

use crate::db::Database;
use crate::models::*;

/// Execute a feed check cycle: fetch all feeds, publish new posts.
pub async fn run_feed_check(
    feed_manager: Arc<Mutex<FeedManager>>,
    publisher_manager: Arc<PublisherManager>,
    published_posts: Arc<Mutex<PublishedPostsStorage>>,
    db: &Database,
    default_interval_minutes: u64,
    dry_run: bool,
) -> Result<()> {
    tracing::info!("Starting feed check cycle");

    let feed_results = {
        let mut manager = feed_manager.lock().await;
        manager.check_all_feeds(default_interval_minutes).await
    };

    let mut total_new_posts = 0;
    let mut total_published = 0;

    for (feed_id, result) in feed_results {
        match result {
            Ok(new_posts) => {
                if new_posts.is_empty() {
                    tracing::debug!("No new posts in feed: {}", feed_id);
                    continue;
                }

                total_new_posts += new_posts.len();
                tracing::info!("Found {} new posts in feed: {}", new_posts.len(), feed_id);

                let publisher_ids = {
                    let manager = feed_manager.lock().await;
                    if let Some(feed) = manager.get_feed(&feed_id) {
                        feed.publishers.clone()
                    } else {
                        tracing::error!("Feed not found: {}", feed_id);
                        continue;
                    }
                };

                if publisher_ids.is_empty() {
                    tracing::warn!("No publishers configured for feed: {}", feed_id);
                    continue;
                }

                for post in new_posts {
                    // Check if already published (via DB)
                    let already_published = db
                        .is_post_published(&post.guid, &post.feed_id)
                        .await
                        .unwrap_or(false);

                    if already_published {
                        tracing::debug!("Post already published: {}", post.title);
                        continue;
                    }

                    tracing::info!("Publishing new post: {}", post.title);

                    if dry_run {
                        tracing::info!(
                            "[DRY RUN] Would publish to {} publishers: {:?}",
                            publisher_ids.len(),
                            publisher_ids
                        );
                        continue;
                    }

                    let results = publisher_manager
                        .publish_to_all(&post, &publisher_ids)
                        .await;

                    let mut successful_publishes = 0;

                    for (i, result) in results.into_iter().enumerate() {
                        let publisher_id = &publisher_ids[i];
                        match result {
                            Ok(message) => {
                                tracing::info!("✓ Published to {}: {}", publisher_id, message);
                                db.record_publish_result(
                                    &post.guid,
                                    &post.feed_id,
                                    publisher_id,
                                    true,
                                    Some(&message),
                                )
                                .await
                                .ok();
                                successful_publishes += 1;
                            }
                            Err(e) => {
                                tracing::error!("✗ Failed to publish to {}: {}", publisher_id, e);
                                db.record_publish_result(
                                    &post.guid,
                                    &post.feed_id,
                                    publisher_id,
                                    false,
                                    Some(&e.to_string()),
                                )
                                .await
                                .ok();
                            }
                        }
                    }

                    // Mark as published in DB
                    db.mark_post_published(
                        &post.guid,
                        &post.feed_id,
                        &post.title,
                        &post.url,
                        None,
                    )
                    .await
                    .ok();

                    // Also keep in-memory for legacy compatibility
                    {
                        let mut storage = published_posts.lock().await;
                        storage.mark_published(&post, vec![]);
                    }

                    if successful_publishes > 0 {
                        total_published += 1;
                        tracing::info!(
                            "Successfully published \"{}\" to {}/{} publishers",
                            post.title,
                            successful_publishes,
                            publisher_ids.len()
                        );
                    }

                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
            }
            Err(e) => {
                tracing::error!("Failed to fetch feed {}: {}", feed_id, e);
            }
        }
    }

    tracing::info!(
        "Feed check cycle completed: {} new posts found, {} published",
        total_new_posts,
        total_published
    );

    Ok(())
}