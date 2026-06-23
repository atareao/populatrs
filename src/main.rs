mod models;
mod storage;

use models::*;
use storage::StorageManager;

use anyhow::Result;
use clap::{Arg, Command};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let matches = Command::new("Populatrs")
        .version("1.0")
        .about(
            "RSS Feed Publisher - Automatically publishes RSS feed updates to multiple platforms",
        )
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .help("Configuration file path")
                .default_value("config.json"),
        )
        .arg(
            Arg::new("once")
                .long("once")
                .help("Run once and exit (don't start scheduler)")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("dry-run")
                .long("dry-run")
                .help("Only check feeds, don't publish anything")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("linkedin-oauth")
                .long("linkedin-oauth")
                .help("Setup LinkedIn OAuth 2.0 authentication")
                .requires("linkedin-publisher")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("linkedin-publisher")
                .long("linkedin-publisher")
                .help("LinkedIn publisher ID for OAuth setup")
                .value_name("PUBLISHER_ID"),
        )
        .arg(
            Arg::new("x-oauth")
                .long("x-oauth")
                .help("Setup X (Twitter) OAuth 2.0 authentication with PKCE")
                .requires("x-publisher")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("x-publisher")
                .long("x-publisher")
                .help("X (Twitter) publisher ID for OAuth setup")
                .value_name("PUBLISHER_ID"),
        )
        .get_matches();

    let config_file = matches.get_one::<String>("config").unwrap();
    let run_once = matches.get_flag("once");
    let dry_run = matches.get_flag("dry-run");
    let linkedin_oauth = matches.get_flag("linkedin-oauth");
    let linkedin_publisher_id = matches.get_one::<String>("linkedin-publisher");
    let x_oauth = matches.get_flag("x-oauth");
    let x_publisher_id = matches.get_one::<String>("x-publisher");

    log::info!("Starting Populatrs RSS Publisher");
    log::info!("Config file: {}", config_file);

    // Load configuration
    let config = StorageManager::load_config_from_file(config_file)?;
    validate_config(&config)?;

    // Handle LinkedIn OAuth setup command
    if linkedin_oauth {
        if let Some(publisher_id) = linkedin_publisher_id {
            return handle_linkedin_oauth_setup(&config, publisher_id, config_file).await;
        }
    }

    // Handle X OAuth setup command
    if x_oauth {
        if let Some(publisher_id) = x_publisher_id {
            return handle_x_oauth_setup(&config, publisher_id, config_file).await;
        }
    }

    // Initialize storage
    let storage_manager = StorageManager::new(
        config.storage.data_dir.clone(),
        config.storage.published_posts_file.clone(),
    );
    storage_manager.init()?;

    // Load published posts tracking
    let published_posts = Arc::new(Mutex::new(storage_manager.load_published_posts()?));

    // Load feed cache for ETag/conditional requests
    let feed_cache = storage_manager.load_feed_cache()?;

    // Initialize publishers
    let mut publisher_manager = PublisherManager::new_with_config_path(config_file.to_string());
    for (id, publisher_config) in &config.publishers {
        if let Err(e) = publisher_manager.add_publisher(id.clone(), publisher_config) {
            log::error!("Failed to initialize publisher {}: {}", id, e);
        } else {
            log::info!(
                "Initialized publisher: {} ({})",
                id,
                get_publisher_type_name(publisher_config)
            );
        }
    }
    let publisher_manager = Arc::new(publisher_manager);

    // Initialize feeds with cache metadata
    let mut feed_manager = FeedManager::new();
    feed_manager.load_feeds_with_cache(config.feeds.clone(), config.youtube.clone(), &feed_cache);
    let feed_manager = Arc::new(Mutex::new(feed_manager));

    log::info!(
        "Loaded {} feeds and {} publishers",
        config.feeds.len(),
        config.publishers.len()
    );

    if run_once {
        // Run once and exit
        log::info!("Running in one-shot mode");
        run_feed_check(
            feed_manager.clone(),
            publisher_manager.clone(),
            published_posts.clone(),
            &storage_manager,
            config.schedule.default_interval_minutes,
            dry_run,
        )
        .await?;
        log::info!("One-shot mode completed");
        return Ok(());
    }

    // Create scheduler
    let mut scheduler = JobScheduler::new().await?;

    // Create main job
    let job_feed_manager = feed_manager.clone();
    let job_publisher_manager = publisher_manager.clone();
    let job_published_posts = published_posts.clone();
    let job_storage_manager = storage_manager.clone();
    let job_interval = config.schedule.default_interval_minutes;

    let job = Job::new_async(
        format!("0 */{} * * * *", job_interval).as_str(), // Every N minutes
        move |_uuid, _l| {
            let feed_manager = job_feed_manager.clone();
            let publisher_manager = job_publisher_manager.clone();
            let published_posts = job_published_posts.clone();
            let storage_manager = job_storage_manager.clone();

            Box::pin(async move {
                if let Err(e) = run_feed_check(
                    feed_manager,
                    publisher_manager,
                    published_posts,
                    &storage_manager,
                    job_interval,
                    dry_run,
                )
                .await
                {
                    log::error!("Error in scheduled feed check: {}", e);
                }
            })
        },
    )?;

    scheduler.add(job).await?;

    // Create cleanup job (daily)
    let cleanup_published_posts = published_posts.clone();
    let cleanup_storage_manager = storage_manager.clone();

    let cleanup_job = Job::new_async(
        "0 0 2 * * *", // Daily at 2 AM
        move |_uuid, _l| {
            let published_posts = cleanup_published_posts.clone();
            let storage_manager = cleanup_storage_manager.clone();

            Box::pin(async move {
                log::info!("Running daily cleanup");

                // Cleanup old published posts (keep 30 days)
                {
                    let mut storage = published_posts.lock().await;
                    storage.cleanup_old_posts(30);
                }

                // Save updated storage
                {
                    let storage = published_posts.lock().await.clone();
                    if let Err(e) = storage_manager.save_published_posts(&storage) {
                        log::error!("Failed to save cleaned up storage: {}", e);
                    }
                }

                // Cleanup old backups (keep 7 days)
                if let Err(e) = storage_manager.cleanup_old_backups(7) {
                    log::error!("Failed to cleanup old backups: {}", e);
                }

                log::info!("Daily cleanup completed");
            })
        },
    )?;

    scheduler.add(cleanup_job).await?;

    // Start scheduler
    log::info!("Starting scheduler with {}min intervals", job_interval);
    scheduler.start().await?;

    // Run initial check
    log::info!("Running initial feed check");
    run_feed_check(
        feed_manager.clone(),
        publisher_manager.clone(),
        published_posts.clone(),
        &storage_manager,
        config.schedule.default_interval_minutes,
        dry_run,
    )
    .await?;

    // Keep running
    log::info!("Scheduler started. Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await?;
    log::info!("Shutdown signal received, stopping scheduler");

    scheduler.shutdown().await?;
    log::info!("Populatrs stopped");

    Ok(())
}

async fn run_feed_check(
    feed_manager: Arc<Mutex<FeedManager>>,
    publisher_manager: Arc<PublisherManager>,
    published_posts: Arc<Mutex<PublishedPostsStorage>>,
    storage_manager: &StorageManager,
    default_interval_minutes: u64,
    dry_run: bool,
) -> Result<()> {
    log::info!("Starting feed check cycle");

    // Check all feeds for new posts
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
                    log::debug!("No new posts in feed: {}", feed_id);
                    continue;
                }

                total_new_posts += new_posts.len();
                log::info!("Found {} new posts in feed: {}", new_posts.len(), feed_id);

                // Get publisher IDs for this feed
                let publisher_ids = {
                    let manager = feed_manager.lock().await;
                    if let Some(feed) = manager.get_feed(&feed_id) {
                        feed.get_publishers().to_vec()
                    } else {
                        log::error!("Feed not found: {}", feed_id);
                        continue;
                    }
                };

                if publisher_ids.is_empty() {
                    log::warn!("No publishers configured for feed: {}", feed_id);
                    continue;
                }

                for post in new_posts {
                    // Check if already published
                    let already_published = {
                        let storage = published_posts.lock().await;
                        storage.is_published(&post)
                    };

                    if already_published {
                        log::debug!("Post already published: {}", post.title);
                        continue;
                    }

                    log::info!("Publishing new post: {}", post.title);

                    if dry_run {
                        log::info!(
                            "[DRY RUN] Would publish to {} publishers: {:?}",
                            publisher_ids.len(),
                            publisher_ids
                        );
                        continue;
                    }

                    // Publish to all configured publishers
                    let results = publisher_manager
                        .publish_to_all(&post, &publisher_ids)
                        .await;

                    let mut publish_results = Vec::new();
                    let mut successful_publishes = 0;

                    for (i, result) in results.into_iter().enumerate() {
                        let publisher_id = &publisher_ids[i];
                        match result {
                            Ok(message) => {
                                log::info!("✓ Published to {}: {}", publisher_id, message);
                                publish_results.push((publisher_id.clone(), true, message));
                                successful_publishes += 1;
                            }
                            Err(e) => {
                                log::error!("✗ Failed to publish to {}: {}", publisher_id, e);
                                publish_results.push((publisher_id.clone(), false, e.to_string()));
                            }
                        }
                    }

                    // Mark as published (even if some publishers failed)
                    {
                        let mut storage = published_posts.lock().await;
                        storage.mark_published(&post, publish_results);
                    }

                    if successful_publishes > 0 {
                        total_published += 1;
                        log::info!(
                            "Successfully published \"{}\" to {}/{} publishers",
                            post.title,
                            successful_publishes,
                            publisher_ids.len()
                        );
                    }

                    // Small delay between posts to avoid rate limiting
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
            }
            Err(e) => {
                log::error!("Failed to fetch feed {}: {}", feed_id, e);
            }
        }
    }

    // Save updated published posts storage
    if !dry_run {
        let storage = published_posts.lock().await.clone();
        if let Err(e) = storage_manager.save_published_posts(&storage) {
            log::error!("Failed to save published posts: {}", e);
        }
    }

    // Save updated feed cache (ETags, Last-Modified, etc.)
    {
        let manager = feed_manager.lock().await;
        let cache = manager.get_cache_metadata();
        if let Err(e) = storage_manager.save_feed_cache(&cache) {
            log::error!("Failed to save feed cache: {}", e);
        }
    }

    log::info!(
        "Feed check cycle completed: {} new posts found, {} published",
        total_new_posts,
        total_published
    );

    Ok(())
}

fn validate_config(config: &AppConfig) -> Result<()> {
    if config.feeds.is_empty() {
        return Err(anyhow::anyhow!("No feeds configured"));
    }

    if config.publishers.is_empty() {
        return Err(anyhow::anyhow!("No publishers configured"));
    }

    // Check that all feed publisher references exist
    for feed in &config.feeds {
        for publisher_id in &feed.publishers {
            if !config.publishers.contains_key(publisher_id) {
                return Err(anyhow::anyhow!(
                    "Feed '{}' references non-existent publisher '{}'",
                    feed.id,
                    publisher_id
                ));
            }
        }
    }

    log::info!("Configuration validation passed");
    Ok(())
}

fn get_publisher_type_name(config: &PublisherConfig) -> &'static str {
    match config {
        PublisherConfig::Telegram { .. } => "Telegram",
        PublisherConfig::X { .. } => "X/Twitter",
        PublisherConfig::Mastodon { .. } => "Mastodon",
        PublisherConfig::LinkedIn { .. } => "LinkedIn",
        PublisherConfig::OpenObserve { .. } => "OpenObserve",
        PublisherConfig::Matrix { .. } => "Matrix",
        PublisherConfig::Bluesky { .. } => "Bluesky",
        PublisherConfig::Threads { .. } => "Threads",
        PublisherConfig::Discord { .. } => "Discord",
    }
}

/// Handle LinkedIn OAuth 2.0 setup command
async fn handle_linkedin_oauth_setup(
    config: &AppConfig,
    publisher_id: &str,
    config_file: &str,
) -> Result<()> {
    use crate::models::publishers::manager::create_publisher_with_config_path;
    use crate::models::publishers::LinkedInPublisher;

    // Find the LinkedIn publisher in config
    let publisher_config = config.publishers.get(publisher_id).ok_or_else(|| {
        anyhow::anyhow!("LinkedIn publisher '{}' not found in config", publisher_id)
    })?;

    // Verify it's a LinkedIn publisher
    if let PublisherConfig::LinkedIn { .. } = publisher_config {
        // Create the publisher instance
        let publisher = create_publisher_with_config_path(
            publisher_id.to_string(),
            publisher_config,
            Some(config_file.to_string()),
        )?;

        // Downcast to LinkedInPublisher to access OAuth methods
        let linkedin_publisher = publisher
            .as_ref()
            .as_any()
            .downcast_ref::<LinkedInPublisher>()
            .ok_or_else(|| anyhow::anyhow!("Failed to cast to LinkedInPublisher"))?;

        // Run OAuth setup
        linkedin_publisher.oauth_setup().await?;

        println!("\n🎉 LinkedIn OAuth setup completed successfully!");
        println!("You can now use the LinkedIn publisher for automated posting.");
    } else {
        return Err(anyhow::anyhow!(
            "Publisher '{}' is not a LinkedIn publisher",
            publisher_id
        ));
    }

    Ok(())
}

async fn handle_x_oauth_setup(
    config: &AppConfig,
    publisher_id: &str,
    config_file: &str,
) -> Result<()> {
    use crate::models::publishers::manager::create_publisher_with_config_path;
    use crate::models::publishers::XPublisher;

    // Find the X publisher in config
    let publisher_config = config
        .publishers
        .get(publisher_id)
        .ok_or_else(|| anyhow::anyhow!("X publisher '{}' not found in config", publisher_id))?;

    // Verify it's an X publisher
    if let PublisherConfig::X { .. } = publisher_config {
        // Create the publisher instance
        let publisher = create_publisher_with_config_path(
            publisher_id.to_string(),
            publisher_config,
            Some(config_file.to_string()),
        )?;

        // Downcast to XPublisher to access OAuth methods
        let x_publisher = publisher
            .as_ref()
            .as_any()
            .downcast_ref::<XPublisher>()
            .ok_or_else(|| anyhow::anyhow!("Failed to cast to XPublisher"))?;

        // Run OAuth setup
        x_publisher.oauth_setup().await?;

        println!("\n🎉 X (Twitter) OAuth setup completed successfully!");
        println!("You can now use the X publisher for automated posting.");
    } else {
        return Err(anyhow::anyhow!(
            "Publisher '{}' is not an X publisher",
            publisher_id
        ));
    }

    Ok(())
}
