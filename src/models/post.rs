use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Post {
    pub guid: String,
    pub title: String,
    pub description: Option<String>,
    pub link: String,
    pub published: DateTime<Utc>,
    pub feed_id: String,
}

impl Post {
    pub fn new(
        guid: String,
        title: String,
        description: Option<String>,
        link: String,
        published: DateTime<Utc>,
        feed_id: String,
    ) -> Self {
        Self {
            guid,
            title,
            description,
            link,
            published,
            feed_id,
        }
    }

    pub fn from_rss_item(item: &rss::Item, feed_id: String) -> Option<Self> {
        let title = item.title().unwrap_or("Untitled").to_string();
        let link = item.link().unwrap_or("").to_string();
        let guid = item.guid().map(|g| g.value()).unwrap_or(&link).to_string();

        // Try to extract description from multiple RSS sources
        let description = Self::extract_rss_description(item);

        // Parse published date
        let published = if let Some(pub_date) = item.pub_date() {
            DateTime::parse_from_rfc2822(pub_date)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now())
        } else {
            Utc::now()
        };

        if !guid.is_empty() && !title.is_empty() && !link.is_empty() {
            Some(Self::new(
                guid,
                title,
                description,
                link,
                published,
                feed_id,
            ))
        } else {
            None
        }
    }

    fn extract_rss_description(item: &rss::Item) -> Option<String> {
        // Try description field first (standard RSS description)
        if let Some(desc) = item.description() {
            if !desc.is_empty() {
                return Some(desc.to_string());
            }
        }

        // For now, just use the standard description
        // TODO: Add extension support for content:encoded and media:description later
        None
    }

    pub fn from_feed_item(item: &feed_rs::model::Entry, feed_id: String) -> Option<Self> {
        let title = item.title.as_ref()?.content.clone();
        let link = item.links.first()?.href.clone();
        let guid = item.id.clone();

        // Try to extract description from multiple sources (YouTube uses media:description)
        let description = Self::extract_description(item);

        let published = item.published.or(item.updated).unwrap_or_else(Utc::now);

        if !guid.is_empty() && !title.is_empty() && !link.is_empty() {
            Some(Self::new(
                guid,
                title,
                description,
                link,
                published,
                feed_id,
            ))
        } else {
            None
        }
    }

    fn extract_description(item: &feed_rs::model::Entry) -> Option<String> {
        // Priority order for description extraction:
        // 1. Content (usually the full content - better for YouTube)
        // 2. Summary (Atom summary field)

        // Try content first (usually more complete for YouTube)
        if let Some(content) = item.content.as_ref() {
            if let Some(body) = &content.body {
                if !body.is_empty() {
                    return Some(body.clone());
                }
            }
        }

        // Try summary field
        if let Some(summary) = item.summary.as_ref() {
            if !summary.content.is_empty() {
                return Some(summary.content.clone());
            }
        }

        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishedPost {
    pub post_guid: String,
    pub feed_id: String,
    pub published_at: DateTime<Utc>,
    pub publisher_results: Vec<PublisherResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublisherResult {
    pub publisher_id: String,
    pub success: bool,
    pub message: String,
    pub published_at: DateTime<Utc>,
}

impl PublishedPost {
    pub fn new(post: &Post) -> Self {
        Self {
            post_guid: post.guid.clone(),
            feed_id: post.feed_id.clone(),
            published_at: Utc::now(),
            publisher_results: Vec::new(),
        }
    }

    pub fn add_result(&mut self, publisher_id: String, success: bool, message: String) {
        self.publisher_results.push(PublisherResult {
            publisher_id,
            success,
            message,
            published_at: Utc::now(),
        });
    }
}

/// Storage for tracking which posts have been published
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PublishedPostsStorage {
    pub posts: Vec<PublishedPost>,
}

impl PublishedPostsStorage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_published(&self, post: &Post) -> bool {
        self.posts
            .iter()
            .any(|p| p.post_guid == post.guid && p.feed_id == post.feed_id)
    }

    pub fn mark_published(&mut self, post: &Post, results: Vec<(String, bool, String)>) {
        let mut published_post = PublishedPost::new(post);

        for (publisher_id, success, message) in results {
            published_post.add_result(publisher_id, success, message);
        }

        self.posts.push(published_post);
    }

    pub fn get_published_count(&self, feed_id: &str) -> usize {
        self.posts.iter().filter(|p| p.feed_id == feed_id).count()
    }

    pub fn cleanup_old_posts(&mut self, days_to_keep: u64) {
        let cutoff_date = Utc::now() - chrono::Duration::days(days_to_keep as i64);
        self.posts.retain(|p| p.published_at > cutoff_date);
    }
}
