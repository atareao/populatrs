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

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::db::Database;
use crate::models::*;

/// Calculate exponential backoff delay for a given attempt number.
/// attempt 1 → base_delay * multiplier^0 = base_delay
/// attempt 2 → base_delay * multiplier^1
/// attempt N → base_delay * multiplier^(N-1), capped at max_delay_seconds
pub fn calculate_backoff(attempt: u32, policy: &RetryPolicy) -> u64 {
    let exponent = attempt.saturating_sub(1);
    let delay = policy.base_delay_seconds as f64 * policy.backoff_multiplier.powi(exponent as i32);
    (delay as u64).min(policy.max_delay_seconds).max(1)
}

/// Publish a post to a single publisher with retry logic.
/// Uses `replace_publish_result` on each attempt so the UI tag updates.
pub async fn publish_with_retry(
    publisher: &dyn crate::publisher::Publisher,
    post: &Post,
    template: Option<&str>,
    policy: &RetryPolicy,
    db: &Database,
) -> Result<String> {
    let max_attempts = policy.max_retries + 1; // initial attempt + retries

    for attempt in 1..=max_attempts {
        let result = publisher.publish(post, template).await;

        match &result {
            Ok(message) => {
                db.replace_publish_result(
                    &post.guid,
                    &post.feed_id,
                    publisher.get_id(),
                    true,
                    Some(message),
                )
                .await
                .ok();
                return Ok(message.clone());
            }
            Err(e) => {
                db.replace_publish_result(
                    &post.guid,
                    &post.feed_id,
                    publisher.get_id(),
                    false,
                    Some(&e.to_string()),
                )
                .await
                .ok();

                if attempt < max_attempts {
                    let delay = calculate_backoff(attempt, policy);
                    tracing::warn!(
                        publisher_id = %publisher.get_id(),
                        attempt = attempt,
                        max_attempts = max_attempts,
                        delay_secs = delay,
                        error = %e,
                        "Publish failed, retrying..."
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                } else {
                    tracing::error!(
                        publisher_id = %publisher.get_id(),
                        attempt = attempt,
                        error = %e,
                        "All publish attempts exhausted"
                    );
                    return Err(anyhow::anyhow!("{}", e));
                }
            }
        }
    }

    unreachable!()
}

/// Execute a feed check cycle: fetch all feeds, publish new posts.
pub async fn run_feed_check(
    feed_manager: Arc<Mutex<FeedManager>>,
    publisher_manager: Arc<PublisherManager>,
    db: &Database,
    dry_run: bool,
) -> Result<()> {
    tracing::info!("Starting feed check cycle");

    let feed_results = {
        let mut manager = feed_manager.lock().await;
        manager.check_all_feeds().await
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

                let feed_templates: HashMap<String, String> = publisher_ids
                    .iter()
                    .filter_map(|b| {
                        b.template
                            .clone()
                            .filter(|t| !t.is_empty())
                            .map(|t| (b.publisher_id.clone(), t))
                    })
                    .collect();

                let pub_ids: Vec<String> = publisher_ids
                    .iter()
                    .map(|b| b.publisher_id.clone())
                    .collect();

                if pub_ids.is_empty() {
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
                            pub_ids.len(),
                            pub_ids
                        );
                        continue;
                    }

                    // Mark as published in DB first (FK reference for publish_results)
                    db.mark_post_published(
                        &post.guid,
                        &post.feed_id,
                        &post.title,
                        &post.url,
                        None,
                        post.description.as_deref(),
                    )
                    .await
                    .ok();

                    let policy = db.get_retry_policy().await.unwrap_or_default();
                    let mut successful_publishes = 0;

                    for pub_id in &pub_ids {
                        let feed_template = feed_templates.get(pub_id).map(|s| s.as_str());
                        let publisher = match publisher_manager.get_publisher(pub_id) {
                            Some(p) => p,
                            None => {
                                tracing::error!("Publisher not found: {}", pub_id);
                                continue;
                            }
                        };

                        match publish_with_retry(publisher, &post, feed_template, &policy, db).await
                        {
                            Ok(message) => {
                                tracing::info!("✓ Published to {}: {}", pub_id, message);
                                successful_publishes += 1;
                            }
                            Err(e) => {
                                tracing::error!("✗ Failed to publish to {}: {}", pub_id, e);
                            }
                        }
                    }

                    if successful_publishes > 0 {
                        total_published += 1;
                        tracing::info!(
                            "Successfully published \"{}\" to {}/{} publishers",
                            post.title,
                            successful_publishes,
                            pub_ids.len()
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
