use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use tracing::instrument;
use url::Url;

use crate::auth::AppState;
use crate::models::FeedConfig;

#[derive(Debug, Deserialize, Default)]
pub struct RunQuery {
    publish: Option<bool>,
    dry_run: Option<bool>,
}

/// List all feeds.
pub async fn list(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    match state.db.list_feeds().await {
        Ok(feeds) => Json(json!({
            "feeds": feeds,
            "total": feeds.len()
        })),
        Err(e) => Json(json!({
            "error": format!("Failed to list feeds: {e}"),
            "feeds": [],
            "total": 0
        })),
    }
}

/// Create a new feed.
pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(feed): Json<FeedConfig>,
) -> impl IntoResponse {
    // Check for duplicate ID
    match state.db.get_feed(&feed.id).await {
        Ok(Some(_)) => {
            return (
                StatusCode::CONFLICT,
                Json(json!({"error": format!("Feed '{}' already exists", feed.id)})),
            )
                .into_response();
        }
        Ok(None) => {}
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {e}")})),
            )
                .into_response();
        }
    }

    match state.db.create_feed(&feed).await {
        Ok(()) => {
            // Fetch initial items and mark them as seen (prevents publishing backlog)
            let feed_model = crate::models::Feed::new(feed.clone(), None);
            match feed_model.fetch_posts().await {
                Ok(initial_posts) => {
                    let count = initial_posts.len();
                    for post in &initial_posts {
                        state
                            .db
                            .mark_post_published(
                                &post.guid,
                                &feed.id,
                                &post.title,
                                &post.url,
                                None,
                                post.description.as_deref(),
                            )
                            .await
                            .ok();
                    }
                    tracing::info!(
                        feed_id = %feed.id,
                        count = count,
                        "Marked initial items as seen"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        feed_id = %feed.id,
                        error = %e,
                        "Could not fetch initial items for feed (non-critical)"
                    );
                }
            }
            (
                StatusCode::CREATED,
                Json(json!({"status": "ok", "message": "Feed created"})),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create feed: {e}")})),
        )
            .into_response(),
    }
}

/// Update an existing feed.
pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(feed): Json<FeedConfig>,
) -> impl IntoResponse {
    match state.db.update_feed(&id, &feed).await {
        Ok(true) => Json(json!({"status": "ok", "message": "Feed updated"})).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Feed not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to update feed: {e}")})),
        )
            .into_response(),
    }
}

/// Delete a feed.
pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.delete_feed(&id).await {
        Ok(true) => Json(json!({"status": "ok", "message": "Feed deleted"})).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Feed not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to delete feed: {e}")})),
        )
            .into_response(),
    }
}

/// Toggle feed enabled/disabled.
pub async fn toggle(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.toggle_feed(&id).await {
        Ok(Some(enabled)) => Json(json!({
            "status": "ok",
            "enabled": enabled,
            "message": if enabled { "Feed enabled" } else { "Feed disabled" }
        }))
        .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Feed not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to toggle feed: {e}")})),
        )
            .into_response(),
    }
}

/// Run a feed manually: fetch posts, optionally publish.
/// `?publish=true` — fetch + publish to publishers + mark as published
/// `?publish=false` (default) — only mark as published, no publish
/// `?dry_run=true` — fetch + preview only, no marking or publishing
#[instrument(skip(state))]
pub async fn run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<RunQuery>,
) -> impl IntoResponse {
    let do_publish = query.publish.unwrap_or(false);
    let is_dry_run = query.dry_run.unwrap_or(false);
    let feed_config = match state.db.get_feed(&id).await {
        Ok(Some(f)) => f,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Feed not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {e}")})),
            )
                .into_response();
        }
    };

    let youtube_config = state.db.get_youtube_config().await.unwrap_or(None);
    let feed = crate::models::Feed::new(feed_config.clone(), youtube_config);
    match feed.fetch_posts().await {
        Ok(posts) => {
            let posts: Vec<_> = posts
                .into_iter()
                .map(|mut p| {
                    p.feed_id = id.clone();
                    p
                })
                .collect();

            // Filter out already-published posts
            let mut new_posts = Vec::new();
            for post in posts {
                let already_published = state
                    .db
                    .is_post_published(&post.guid, &post.feed_id)
                    .await
                    .unwrap_or(false);
                if !already_published {
                    new_posts.push(post);
                }
            }

            // Apply MIN_DATE filter from global settings
            let min_date = state.db.get_min_date().await.unwrap_or(None);
            if let Some(ref min_dt) = min_date {
                new_posts.retain(|p| p.published_date >= *min_dt);
            }

            // Apply MAX_POSTS limit from global settings
            // Take the newest N posts (last N in oldest-first sorted vec)
            let max_posts = state.db.get_max_posts().await.unwrap_or(1);
            if max_posts > 0 && new_posts.len() > max_posts as usize {
                let split_at = new_posts.len() - max_posts as usize;
                new_posts = new_posts.split_off(split_at);
            }

            if is_dry_run {
                // Dry run: just return what would be published, no side effects
                tracing::info!(count = new_posts.len(), feed_id = %id, "Dry run completed");
            } else if do_publish {
                // Full pipeline: publish to publishers + mark + record results
                let pub_ids: Vec<String> = feed_config
                    .publishers
                    .iter()
                    .map(|b| b.publisher_id.clone())
                    .collect();

                let feed_templates: HashMap<String, String> = feed_config
                    .publishers
                    .iter()
                    .filter_map(|b| {
                        b.template
                            .clone()
                            .filter(|t| !t.is_empty())
                            .map(|t| (b.publisher_id.clone(), t))
                    })
                    .collect();

                for post in &new_posts {
                    // Mark first so scheduler doesn't pick it up
                    state
                        .db
                        .mark_post_published(
                            &post.guid,
                            &post.feed_id,
                            &post.title,
                            &post.url,
                            None,
                            post.description.as_deref(),
                        )
                        .await
                        .ok();

                    if !pub_ids.is_empty() {
                        let results = state
                            .publisher_manager
                            .publish_to_all(post, &pub_ids, &feed_templates)
                            .await;

                        for (publisher_id, result) in pub_ids.iter().zip(results.iter()) {
                            let msg = result.as_ref().map(|s| s.as_str()).unwrap_or("error");
                            state
                                .db
                                .record_publish_result(
                                    &post.guid,
                                    &post.feed_id,
                                    publisher_id,
                                    result.is_ok(),
                                    Some(msg),
                                )
                                .await
                                .ok();
                        }
                    }
                }

                tracing::info!(count = new_posts.len(), feed_id = %id, "Manual feed run with publish");
            } else if !is_dry_run {
                // Just mark as published (no publishers recorded)
                for post in &new_posts {
                    state
                        .db
                        .mark_post_published(
                            &post.guid,
                            &post.feed_id,
                            &post.title,
                            &post.url,
                            None,
                            post.description.as_deref(),
                        )
                        .await
                        .ok();
                }

                tracing::info!(count = new_posts.len(), feed_id = %id, "Manual feed run (mark only)");
            }

            let posts_json: Vec<serde_json::Value> = new_posts
                .iter()
                .map(|p| {
                    json!({
                        "guid": p.guid,
                        "title": p.title,
                        "url": p.url,
                    })
                })
                .collect();

            let mut response = json!({
                "status": "ok",
                "feed_id": id,
                "posts_count": new_posts.len(),
                "posts": posts_json,
            });

            if is_dry_run {
                response["dry_run"] = json!(true);
                response["message"] = json!(format!("Dry run: {} post(s) would be published", new_posts.len()));
            }

            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            tracing::error!(feed_id = %id, error = %e, "Manual feed run failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to fetch feed: {e}")})),
            )
                .into_response()
        }
    }
}

/// Publish previously-previewed posts to all assigned publishers.
/// Body: { "posts": [{ "guid": "...", "title": "...", "url": "..." }] }
#[instrument(skip(state))]
pub async fn publish(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let feed_config = match state.db.get_feed(&id).await {
        Ok(Some(f)) => f,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Feed not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {e}")})),
            )
                .into_response();
        }
    };

    let pub_ids: Vec<String> = feed_config
        .publishers
        .iter()
        .map(|b| b.publisher_id.clone())
        .collect();

    if pub_ids.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "No publishers configured for this feed"})),
        )
            .into_response();
    }

    let feed_templates: HashMap<String, String> = feed_config
        .publishers
        .iter()
        .filter_map(|b| {
            b.template
                .clone()
                .filter(|t| !t.is_empty())
                .map(|t| (b.publisher_id.clone(), t))
        })
        .collect();

    let posts = match body.get("posts").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Missing 'posts' array"})),
            )
                .into_response();
        }
    };

    let mut published = 0u64;

    for post_val in posts {
        let guid = post_val.get("guid").and_then(|v| v.as_str()).unwrap_or("");
        let title = post_val
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled");
        let url = post_val.get("url").and_then(|v| v.as_str()).unwrap_or("");

        if guid.is_empty() {
            continue;
        }

        let post = crate::models::Post::new(
            guid.to_string(),
            title.to_string(),
            None,
            url.to_string(),
            chrono::Utc::now(),
            id.clone(),
        );

        let results = state
            .publisher_manager
            .publish_to_all(&post, &pub_ids, &feed_templates)
            .await;

        for (publisher_id, result) in pub_ids.iter().zip(results.iter()) {
            let msg = result.as_ref().map(|s| s.as_str()).unwrap_or("error");
            let _ = state
                .db
                .record_publish_result(guid, &id, publisher_id, result.is_ok(), Some(msg))
                .await;
        }

        // Mark as published (idempotent, already done in preview run)
        let _ = state
            .db
            .mark_post_published(guid, &id, title, url, None, None)
            .await;

        published += 1;
    }

    tracing::info!(count = published, feed_id = %id, "Published via manual publish");

    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "published": published,
        })),
    )
        .into_response()
}

/// Resolve a YouTube URL to a channel ID.
/// Accepts formats like:
/// - https://youtube.com/@handle
/// - https://youtube.com/channel/UCxxx
/// - https://www.youtube.com/@handle
/// - @handle
#[instrument(skip(_state))]
pub async fn resolve_youtube(
    State(_state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let url_str = match body.get("url").and_then(|v| v.as_str()) {
        Some(u) => u.trim(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Missing 'url' field"})),
            )
                .into_response();
        }
    };

    let input = url_str.trim().to_lowercase();

    // Try to extract from common URL patterns
    let handle_or_id = if input.starts_with("http") {
        let parsed = match Url::parse(url_str) {
            Ok(u) => u,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Invalid URL format"})),
                )
                    .into_response();
            }
        };

        let path = parsed.path().trim_end_matches('/');

        if let Some(ch) = path.strip_prefix("/channel/") {
            ch.to_string()
        } else if let Some(handle) = path.strip_prefix("/@") {
            handle.to_string()
        } else if let Some(custom) = path.strip_prefix("/c/") {
            custom.to_string()
        } else {
            path.trim_start_matches('/').to_string()
        }
    } else {
        input.trim_start_matches('@').to_string()
    };

    if handle_or_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Could not extract handle or channel ID from URL"})),
        )
            .into_response();
    }

    // If it starts with UC, it's already a channel ID
    if handle_or_id.starts_with("uc") && handle_or_id.len() > 2 {
        let channel_id = format!("UC{}", &handle_or_id[2..]);
        return Json(json!({"channel_id": channel_id})).into_response();
    }

    // It's a handle — resolve by fetching the channel page and scraping the channel ID
    // No API key needed — YouTube embeds the channel ID in the HTML
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (compatible; Populatrs/1.0)")
        .build()
        .unwrap_or_default();

    // Try multiple URL patterns to find the channel ID
    let urls_to_try = vec![
        format!("https://www.youtube.com/@{}/about", handle_or_id),
        format!("https://www.youtube.com/@{}", handle_or_id),
        format!("https://m.youtube.com/@{}", handle_or_id),
    ];

    let mut channel_id: Option<String> = None;
    let mut last_error = String::new();

    for url in &urls_to_try {
        match client.get(url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    last_error = format!("HTTP {status} for {url}");
                    continue;
                }
                match resp.text().await {
                    Ok(html) => {
                        // Look for /channel/UCxxxxx in the page source
                        if let Some(start) = html.find("/channel/UC") {
                            let end = start + 9 + 24; // "/channel/UC" = 10 + 24 chars of ID
                            if end <= html.len() {
                                let candidate = &html[start + 9..end];
                                // Validate: 24 chars, alphanumeric + underscore + dash
                                if candidate.len() == 24
                                    && candidate
                                        .chars()
                                        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                                {
                                    channel_id = Some(format!("UC{}", &candidate[2..]));
                                    break;
                                }
                            }
                        }
                        // Alternative: look for "channelId":"UCxxxxx"
                        if channel_id.is_none() {
                            for pattern in &[r#""channelId":"UC"#, r#"externalId":"UC"#] {
                                if let Some(start) = html.find(pattern) {
                                    let id_start = start + pattern.len();
                                    let id_end = id_start + 22; // UC = 2, so 24-2 = 22
                                    if id_end <= html.len() {
                                        let candidate = &html[id_start..id_end];
                                        if candidate
                                            .chars()
                                            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                                        {
                                            channel_id = Some(format!("UC{candidate}"));
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        if channel_id.is_none() {
                            last_error = "Could not find channel ID in page".to_string();
                        }
                    }
                    Err(e) => {
                        last_error = format!("Failed to read response: {e}");
                    }
                }
            }
            Err(e) => {
                last_error = format!("Failed to fetch {url}: {e}");
            }
        }
        if channel_id.is_some() {
            break;
        }
    }

    match channel_id {
        Some(id) => {
            tracing::info!(handle = %handle_or_id, channel_id = %id, "Resolved YouTube handle via page scrape");
            Json(json!({"channel_id": id})).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("Could not resolve '{}': {}", handle_or_id, last_error)})),
        )
            .into_response(),
    }
}
