use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest::Client;

use crate::models::{FeedConfig, FeedTypeConfig, Post, YouTubeGlobalConfig};

/// Fetch posts from an RSS feed URL.
pub async fn fetch_rss_posts(config: &FeedConfig) -> Result<Vec<Post>> {
    let url = match &config.config {
        FeedTypeConfig::Rss { url } => url.clone(),
        _ => anyhow::bail!("Not an RSS feed config"),
    };

    let client = Client::builder()
        .user_agent("Populatrs RSS Reader 1.0")
        .build()
        .context("Failed to create HTTP client")?;

    let max_retries = config.max_retries.unwrap_or(3);
    let base_delay = config.retry_delay_seconds.unwrap_or(2);

    for attempt in 0..=max_retries {
        match fetch_rss_with_retry(&client, &url).await {
            Ok(posts) => {
                if attempt > 0 {
                    tracing::info!(
                        "Successfully fetched feed {} on attempt {}/{}",
                        config.name,
                        attempt + 1,
                        max_retries + 1
                    );
                }
                return Ok(posts);
            }
            Err(err) => {
                if attempt == max_retries {
                    return Err(err.context(format!(
                        "Failed to fetch feed {} after {} attempts",
                        config.name,
                        max_retries + 1
                    )));
                }
                let delay_secs = base_delay * (2_u64.pow(attempt));
                tracing::warn!(
                    "Failed to fetch feed {} (attempt {}/{}): {}. Retrying in {}s...",
                    config.name,
                    attempt + 1,
                    max_retries + 1,
                    err,
                    delay_secs
                );
                tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
            }
        }
    }
    unreachable!()
}

async fn fetch_rss_with_retry(client: &Client, url: &str) -> Result<Vec<Post>> {
    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to send HTTP request")?;

    if !response.status().is_success() {
        anyhow::bail!("HTTP {}", response.status());
    }

    let content = response.text().await.context("Failed to read response body")?;

    // Try RSS first
    if let Ok(channel) = content.parse::<rss::Channel>() {
        return Ok(parse_rss_channel(channel));
    }

    // Try Atom via feed-rs
    if let Ok(feed) = feed_rs::parser::parse(content.as_bytes()) {
        return Ok(parse_feed_rs(feed));
    }

    anyhow::bail!("Unable to parse feed as RSS or Atom")
}

fn parse_rss_channel(channel: rss::Channel) -> Vec<Post> {
    channel
        .items()
        .iter()
        .filter_map(|item| {
            let guid = item
                .guid()
                .map(|g| g.value().to_string())
                .or_else(|| item.link().map(String::from))
                .unwrap_or_default();

            if guid.is_empty() {
                return None;
            }

            let title = item.title().unwrap_or("Untitled").to_string();
            let description = item.description().map(String::from);
            let url = item.link().unwrap_or("").to_string();
            let pub_date = item
                .pub_date()
                .and_then(|d| DateTime::parse_from_rfc2822(d).ok())
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);

            Some(Post::new(guid, title, description, url, pub_date, String::new()))
        })
        .collect()
}

fn parse_feed_rs(feed: feed_rs::model::Feed) -> Vec<Post> {
    feed.entries
        .into_iter()
        .filter_map(|entry| {
            let guid = entry.id;
            if guid.is_empty() {
                return None;
            }
            let title = entry.title.map(|t| t.content).unwrap_or_else(|| "Untitled".to_string());
            let description = entry.summary.map(|s| s.content);
            let url = entry
                .links
                .iter()
                .find(|l| l.rel.as_deref() == Some("alternate") || l.rel.is_none())
                .map(|l| l.href.clone())
                .unwrap_or_default();
            let pub_date = entry
                .published
                .or(entry.updated)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);

            Some(Post::new(guid, title, description, url, pub_date, String::new()))
        })
        .collect()
}

/// Fetch posts from a YouTube channel.
pub async fn fetch_youtube_posts(
    config: &FeedConfig,
    youtube_config: Option<&YouTubeGlobalConfig>,
) -> Result<Vec<Post>> {
    let youtube_global = youtube_config
        .ok_or_else(|| anyhow::anyhow!("YouTube global configuration not found"))?;

    let (channel_id, playlist_id, username, max_results) = match &config.config {
        FeedTypeConfig::Youtube {
            channel_id,
            playlist_id,
            username,
            max_results,
        } => (
            channel_id.clone(),
            playlist_id.clone(),
            username.clone(),
            max_results,
        ),
        _ => anyhow::bail!("Not a YouTube feed config"),
    };

    let effective_max_results = max_results
        .or(youtube_global.default_max_results)
        .unwrap_or(10);

    let client = Client::new();
    let api_key = &youtube_global.api_key;

    // If playlist_id is provided, fetch from playlist
    if let Some(pid) = &playlist_id {
        return fetch_youtube_playlist(&client, api_key, pid, effective_max_results).await;
    }

    // If channel_id is provided, fetch from channel
    if let Some(cid) = &channel_id {
        return fetch_youtube_channel(&client, api_key, cid, effective_max_results).await;
    }

    // If username is provided, resolve and fetch
    if let Some(uname) = &username {
        // Try to resolve username to channel ID via search
        let cid = resolve_youtube_username(&client, api_key, uname).await?;
        return fetch_youtube_channel(&client, api_key, &cid, effective_max_results).await;
    }

    anyhow::bail!("No channel_id, playlist_id, or username provided for YouTube feed")
}

async fn fetch_youtube_channel(
    client: &Client,
    api_key: &str,
    channel_id: &str,
    max_results: u64,
) -> Result<Vec<Post>> {
    let url = format!(
        "https://www.googleapis.com/youtube/v3/search?part=snippet&channelId={}&order=date&maxResults={}&type=video&key={}",
        channel_id, max_results, api_key
    );

    let resp: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .context("Failed to fetch YouTube channel")?
        .json()
        .await
        .context("Failed to parse YouTube response")?;

    parse_youtube_response(resp)
}

async fn fetch_youtube_playlist(
    client: &Client,
    api_key: &str,
    playlist_id: &str,
    max_results: u64,
) -> Result<Vec<Post>> {
    let url = format!(
        "https://www.googleapis.com/youtube/v3/playlistItems?part=snippet&playlistId={}&maxResults={}&key={}",
        playlist_id, max_results, api_key
    );

    let resp: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .context("Failed to fetch YouTube playlist")?
        .json()
        .await
        .context("Failed to parse YouTube response")?;

    parse_youtube_response(resp)
}

async fn resolve_youtube_username(
    client: &Client,
    api_key: &str,
    username: &str,
) -> Result<String> {
    let url = format!(
        "https://www.googleapis.com/youtube/v3/channels?part=id&forHandle={}&key={}",
        username, api_key
    );

    let resp: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .context("Failed to resolve YouTube username")?
        .json()
        .await
        .context("Failed to parse YouTube channel response")?;

    let channel_id = resp["items"][0]["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("YouTube user '{}' not found", username))?;

    Ok(channel_id.to_string())
}

fn parse_youtube_response(resp: serde_json::Value) -> Result<Vec<Post>> {
    let items = resp["items"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("No items in YouTube response"))?;

    let posts = items
        .iter()
        .filter_map(|item| {
            let snippet = &item["snippet"];
            let resource = &item["snippet"];

            let video_id = item.get("id")
                .and_then(|id| {
                    id.get("videoId")
                        .and_then(|v| v.as_str().map(String::from))
                        .or_else(|| id.as_str().map(String::from))
                })
                .or_else(|| {
                    resource.get("resourceId")
                        .and_then(|r| r.get("videoId")?.as_str().map(String::from))
                })?;

            let title = snippet["title"].as_str()?;
            let description = snippet["description"].as_str().map(String::from);
            let url = format!("https://www.youtube.com/watch?v={}", video_id);
            let published_at = snippet["publishedAt"]
                .as_str()
                .and_then(|d| DateTime::parse_from_rfc3339(d).ok())
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);

            Some(Post::new(
                format!("yt:{}", video_id),
                title.to_string(),
                description,
                url,
                published_at,
                String::new(),
            ))
        })
        .collect();

    Ok(posts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rss_channel() {
        let rss_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
<channel>
    <title>Test Feed</title>
    <item>
        <guid>123</guid>
        <title>Post 1</title>
        <link>https://example.com/1</link>
        <description>Desc 1</description>
        <pubDate>Mon, 01 Jan 2024 00:00:00 GMT</pubDate>
    </item>
    <item>
        <guid>456</guid>
        <title>Post 2</title>
        <link>https://example.com/2</link>
        <pubDate>Mon, 02 Jan 2024 00:00:00 GMT</pubDate>
    </item>
</channel>
</rss>"#;

        let channel: rss::Channel = rss_xml.parse().unwrap();
        let posts = parse_rss_channel(channel);
        assert_eq!(posts.len(), 2);
        assert_eq!(posts[0].guid, "123");
        assert_eq!(posts[0].title, "Post 1");
        assert_eq!(posts[1].guid, "456");
        assert_eq!(posts[1].title, "Post 2");
        assert!(posts[1].description.is_none());
    }

    #[test]
    fn test_parse_feed_rs_atom() {
        let atom_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
    <title>Test Atom</title>
    <entry>
        <id>atom-1</id>
        <title>Atom Post</title>
        <link href="https://example.com/atom"/>
        <summary>Atom summary</summary>
        <published>2024-01-01T00:00:00Z</published>
    </entry>
</feed>"#;

        let feed = feed_rs::parser::parse(atom_xml.as_bytes()).unwrap();
        let posts = parse_feed_rs(feed);
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].guid, "atom-1");
        assert_eq!(posts[0].title, "Atom Post");
        assert_eq!(posts[0].description, Some("Atom summary".to_string()));
    }

    #[test]
    fn test_parse_youtube_response() {
        let json = serde_json::json!({
            "items": [
                {
                    "id": { "videoId": "abc123" },
                    "snippet": {
                        "title": "My Video",
                        "description": "A great video",
                        "publishedAt": "2024-01-15T10:00:00Z"
                    }
                }
            ]
        });

        let posts = parse_youtube_response(json).unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].guid, "yt:abc123");
        assert_eq!(posts[0].title, "My Video");
        assert_eq!(posts[0].url, "https://www.youtube.com/watch?v=abc123");
    }

    #[test]
    fn test_parse_youtube_response_with_resource_id() {
        let json = serde_json::json!({
            "items": [
                {
                    "snippet": {
                        "resourceId": { "videoId": "def456" },
                        "title": "Playlist Video",
                        "description": "From a playlist",
                        "publishedAt": "2024-02-01T00:00:00Z"
                    }
                }
            ]
        });

        let posts = parse_youtube_response(json).unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].guid, "yt:def456");
    }
}