use crate::models::Post;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YouTubeConfig {
    pub api_key: String,
    pub channel_id: Option<String>,
    pub playlist_id: Option<String>,
    pub username: Option<String>,
    pub max_results: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YouTubeResponse {
    pub items: Vec<YouTubeVideo>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YouTubeVideo {
    pub id: VideoId,
    pub snippet: VideoSnippet,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum VideoId {
    Simple(String),
    Complex {
        #[serde(rename = "videoId")]
        video_id: String,
    },
}

impl VideoId {
    pub fn get_id(&self) -> &str {
        match self {
            VideoId::Simple(id) => id,
            VideoId::Complex { video_id } => video_id,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct VideoSnippet {
    #[serde(rename = "publishedAt")]
    pub published_at: String,
    pub title: String,
    pub description: String,
    pub thumbnails: HashMap<String, Thumbnail>,
    #[serde(rename = "channelTitle")]
    pub channel_title: String,
    #[serde(rename = "resourceId")]
    pub resource_id: Option<ResourceId>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResourceId {
    #[serde(rename = "videoId")]
    pub video_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Thumbnail {
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelResponse {
    pub items: Vec<ChannelItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelItem {
    #[serde(rename = "contentDetails")]
    pub content_details: ChannelContentDetails,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelContentDetails {
    #[serde(rename = "relatedPlaylists")]
    pub related_playlists: RelatedPlaylists,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RelatedPlaylists {
    pub uploads: String,
}

pub struct YouTubeClient {
    client: Client,
    api_key: String,
}

impl YouTubeClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }

    pub async fn fetch_channel_videos(&self, config: &YouTubeConfig) -> Result<Vec<Post>> {
        log::info!("Starting YouTube video fetch with config: channel_id={:?}, playlist_id={:?}, username={:?}", 
                   config.channel_id, config.playlist_id, config.username);

        let playlist_id = if let Some(playlist_id) = &config.playlist_id {
            log::info!("Using direct playlist_id: {}", playlist_id);
            playlist_id.clone()
        } else if let Some(channel_id) = &config.channel_id {
            log::info!("Getting uploads playlist for channel: {}", channel_id);
            self.get_uploads_playlist_id(channel_id).await?
        } else if let Some(username) = &config.username {
            log::info!("Getting channel ID for username: {}", username);
            let channel_id = self.get_channel_id_by_username(username).await?;
            self.get_uploads_playlist_id(&channel_id).await?
        } else {
            return Err(anyhow!("Must specify channel_id, playlist_id, or username"));
        };

        self.fetch_playlist_videos(&playlist_id, config.max_results.unwrap_or(10))
            .await
    }

    async fn get_channel_id_by_username(&self, username: &str) -> Result<String> {
        let url = format!(
            "https://www.googleapis.com/youtube/v3/channels?part=id&forUsername={}&key={}",
            username, self.api_key
        );

        let response: ChannelResponse = self.client.get(&url).send().await?.json().await?;

        response
            .items
            .first()
            .map(|item| item.content_details.related_playlists.uploads.clone())
            .ok_or_else(|| anyhow!("Channel not found for username: {}", username))
    }

    async fn get_uploads_playlist_id(&self, channel_id: &str) -> Result<String> {
        let url = format!(
            "https://www.googleapis.com/youtube/v3/channels?part=contentDetails&id={}&key={}",
            channel_id, self.api_key
        );

        let response: ChannelResponse = self.client.get(&url).send().await?.json().await?;

        response
            .items
            .first()
            .map(|item| item.content_details.related_playlists.uploads.clone())
            .ok_or_else(|| anyhow!("Channel not found: {}", channel_id))
    }

    async fn fetch_playlist_videos(
        &self,
        playlist_id: &str,
        max_results: u64,
    ) -> Result<Vec<Post>> {
        // Always fetch at least 50 videos to ensure we get the most recent ones
        let fetch_count = std::cmp::max(max_results, 50);
        let url = format!(
            "https://www.googleapis.com/youtube/v3/playlistItems?part=snippet&playlistId={}&maxResults={}&key={}",
            playlist_id, fetch_count, self.api_key
        );

        log::info!("Fetching playlist videos from: {}", playlist_id);
        log::debug!("YouTube API URL: {}", url.replace(&self.api_key, "***"));

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("YouTube API error {}: {}", status, error_text));
        }

        let youtube_response: YouTubeResponse = response.json().await?;

        let total_videos = youtube_response.items.len();
        log::info!("Found {} videos in playlist {}", total_videos, playlist_id);

        let mut posts = Vec::new();
        for (index, video) in youtube_response.items.iter().enumerate() {
            // Get video ID from resourceId for playlist items, fallback to id
            let video_id = if let Some(resource_id) = &video.snippet.resource_id {
                &resource_id.video_id
            } else {
                video.id.get_id()
            };

            log::debug!(
                "Video {}: title='{}', video_id='{}', resource_id={:?}",
                index + 1,
                video.snippet.title,
                video_id,
                video.snippet.resource_id.as_ref().map(|r| &r.video_id)
            );

            // Skip private or deleted videos
            if video.snippet.title == "Private video"
                || video.snippet.title == "Deleted video"
                || video.snippet.title.is_empty()
                || video_id.is_empty()
            {
                log::info!(
                    "Skipping video '{}' (video_id='{}'): reason={}",
                    video.snippet.title,
                    video_id,
                    if video.snippet.title == "Private video" {
                        "private"
                    } else if video.snippet.title == "Deleted video" {
                        "deleted"
                    } else if video.snippet.title.is_empty() {
                        "empty_title"
                    } else {
                        "empty_video_id"
                    }
                );
                continue;
            }

            let video_url = format!("https://www.youtube.com/watch?v={}", video_id);

            // Parse published date
            let pub_date =
                DateTime::parse_from_rfc3339(&video.snippet.published_at)?.with_timezone(&Utc);

            let post = Post::new(
                video_id.to_string(),
                video.snippet.title.clone(),
                Some(video.snippet.description.clone()),
                video_url,
                pub_date,
                "youtube".to_string(),
            );

            log::info!("Adding video: '{}'", video.snippet.title);
            posts.push(post);
        }

        // Sort by publication date (newest first)
        posts.sort_by(|a, b| b.published.cmp(&a.published));

        log::info!(
            "Filtered {} valid videos from {} total",
            posts.len(),
            total_videos
        );

        Ok(posts)
    }
}
