use std::sync::Arc;

use populatrs::auth::{AppState, JwtValidator, OidcMetadata};
use populatrs::config::Config;
use populatrs::db::Database;
use populatrs::embed::serve_embedded;
use populatrs::middleware;
use populatrs::models::{FeedManager, PublisherManager};
use populatrs::routes;

use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let config = Config::load();

    // ───── Log broadcast for SSE LogsPage ─────
    let (log_tx, log_layer) = populatrs::routes::logs::log_layer();
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    // Initialize tracing: SSE broadcast + env filter + fmt output
    if config.log_format == "json" {
        tracing_subscriber::registry()
            .with(log_layer)
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(log_layer)
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .pretty()
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true),
            )
            .init();
    }

    tracing::info!("🚀 Populatrs starting...");

    // Ensure data directory exists
    if let Err(e) = tokio::fs::create_dir_all(&config.data_dir).await {
        tracing::warn!("Could not create data dir: {}", e);
    }

    // Initialize database
    let db = match Database::open(&config.database_url).await {
        Ok(db) => {
            tracing::info!("📦 Database opened: {}", config.database_url.display());
            db
        }
        Err(e) => {
            tracing::error!("❌ Failed to open database: {}", e);
            std::process::exit(1);
        }
    };

    // ═══════════════════════════════════════════════════════
    // OIDC Setup
    // ═══════════════════════════════════════════════════════
    let oidc_metadata: Option<OidcMetadata> = if config.oidc_configured() {
        let issuer = config.oidc_issuer_url.as_deref().unwrap();
        let well_known = format!(
            "{}/.well-known/openid-configuration",
            issuer.trim_end_matches('/')
        );
        let client = reqwest::Client::new();
        match client.get(&well_known).send().await {
            Ok(resp) => match resp.json::<OidcMetadata>().await {
                Ok(m) => {
                    tracing::info!("✅ OIDC discovery: {}", m.issuer);
                    Some(m)
                }
                Err(e) => {
                    tracing::error!("❌ OIDC discovery parse failed: {}", e);
                    std::process::exit(1);
                }
            },
            Err(e) => {
                tracing::error!("❌ OIDC discovery request failed: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        tracing::warn!("🔓 OIDC not configured — running in DEV MODE (no authentication)");
        None
    };

    // In production, fetch JWKS for token validation
    let jwt_validator = if let Some(ref _oidc) = oidc_metadata {
        let validator = JwtValidator::new(
            config.oidc_issuer_url.as_deref().unwrap(),
            config.oidc_client_id.as_deref().unwrap(),
        );
        if let Err(e) = validator
            .fetch_jwks(config.oidc_issuer_url.as_deref().unwrap())
            .await
        {
            tracing::error!("❌ JWKS fetch failed: {}. OIDC will not work.", e);
            std::process::exit(1);
        }
        Arc::new(validator)
    } else {
        tracing::info!("🔓 Using dev JWT validator (accepts any token)");
        Arc::new(JwtValidator::dev())
    };

    // ───── Publisher Manager (shared) ─────
    let publishers = db.list_publishers().await.unwrap_or_default();
    let mut publisher_manager = PublisherManager::new();
    for (id, (pub_config, enabled)) in &publishers {
        if !enabled {
            continue;
        }
        if let Err(e) = publisher_manager.add_publisher(id.clone(), pub_config) {
            tracing::error!("Failed to initialize publisher {}: {}", id, e);
        }
    }
    let publisher_manager = Arc::new(publisher_manager);

    let app_state = Arc::new(AppState {
        config: config.clone(),
        db: db.clone(),
        oidc_metadata,
        jwt_validator: jwt_validator.clone(),
        oidc_states: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        oauth_states: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        log_tx,
        publisher_manager: publisher_manager.clone(),
    });

    // ───── Scheduler ─────
    let db_for_scheduler = db.clone();
    let config_for_scheduler = config.clone();
    tokio::spawn(async move {
        feed_scheduler_loop(db_for_scheduler, config_for_scheduler).await;
    });

    // ───── Build router ─────
    let state_for_middleware = app_state.clone();
    let app = routes::api_routes()
        .layer(CorsLayer::permissive())
        .layer(axum::middleware::from_fn(
            move |mut req: axum::extract::Request, next: axum::middleware::Next| {
                let state = state_for_middleware.clone();
                async move {
                    req.extensions_mut().insert(state);
                    middleware::require_auth(req, next).await
                }
            },
        ))
        .fallback(|req: axum::extract::Request| async move {
            let path = req.uri().path().to_string();
            serve_embedded(&path).await
        })
        .with_state(app_state.clone());

    let addr = if config.host == "0.0.0.0" {
        format!("[::]:{}", config.port)
    } else {
        format!("{}:{}", config.host, config.port)
    };

    tracing::info!("🌐 Populatrs en http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");
}

/// Periodic feed scheduler loop.
async fn feed_scheduler_loop(db: Database, config: Config) {
    // Initial delay before first check
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    loop {
        let feeds = match db.list_feeds().await {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("Failed to load feeds for scheduler: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                continue;
            }
        };

        let enabled_feeds: Vec<_> = feeds.iter().filter(|f| f.enabled).collect();
        tracing::info!(
            "⏰ Scheduler: {} feeds enabled out of {}",
            enabled_feeds.len(),
            feeds.len()
        );

        let publishers = match db.list_publishers().await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to load publishers for scheduler: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                continue;
            }
        };

        let mut publisher_manager = PublisherManager::new();
        for (id, (pub_config, enabled)) in &publishers {
            if !enabled {
                tracing::debug!("Skipping disabled publisher: {}", id);
                continue;
            }
            if let Err(e) = publisher_manager.add_publisher(id.clone(), pub_config) {
                tracing::error!("Failed to initialize publisher {}: {}", id, e);
            }
        }
        let publisher_manager = Arc::new(publisher_manager);

        let mut feed_manager = FeedManager::new();
        let youtube_config = db.get_youtube_config().await.unwrap_or(None);
        feed_manager.load_feeds_with_cache(
            feeds.clone(),
            youtube_config,
            &std::collections::HashMap::new(),
        );
        let feed_manager = Arc::new(Mutex::new(feed_manager));

        let interval = config.default_interval_minutes;
        if let Err(e) =
            populatrs::run_feed_check(feed_manager, publisher_manager, &db, interval, false).await
        {
            tracing::error!("Scheduler feed check error: {}", e);
        }

        // Clean up old logs based on retention setting
        let retention_days = db.get_log_retention().await.unwrap_or(30) as i64;
        if let Err(e) = db.cleanup_old_posts(retention_days).await {
            tracing::error!("Failed to cleanup old posts: {}", e);
        }
        if let Err(e) = db.cleanup_old_publish_results(retention_days).await {
            tracing::error!("Failed to cleanup old publish results: {}", e);
        }

        tokio::time::sleep(std::time::Duration::from_secs(interval * 60)).await;
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("ctrl_c handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("signal handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => { tracing::info!("🛑 SIGINT received, shutting down..."); }
        _ = terminate => { tracing::info!("🛑 SIGTERM received, shutting down..."); }
    }
}
