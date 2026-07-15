use anyhow::{Context, Result};
use minijinja::{Environment, Value};
use serde_json::json;

use crate::models::Post;

/// Context data for template rendering.
#[derive(Debug, Clone)]
pub struct TemplateContext {
    pub title: String,
    pub description: String,
    pub url: String,
}

impl From<&Post> for TemplateContext {
    fn from(post: &Post) -> Self {
        Self {
            title: post.title.clone(),
            description: post.description.clone().unwrap_or_default(),
            url: post.url.clone(),
        }
    }
}

/// Template renderer using minijinja.
pub struct TemplateRenderer {
    env: Environment<'static>,
}

impl TemplateRenderer {
    pub fn new() -> Self {
        let mut env = Environment::new();
        env.add_filter("truncate", truncate_function);
        env.add_filter("word_limit", word_limit_function);
        env.add_filter("strip_html", strip_html_function);
        Self { env }
    }

    pub fn render(&self, template: &str, context: &TemplateContext) -> Result<String> {
        let tmpl = self
            .env
            .template_from_str(template)
            .context("Failed to parse template")?;

        let template_vars = json!({
            "title": context.title,
            "description": context.description,
            "url": context.url
        });

        let rendered = tmpl
            .render(template_vars)
            .context("Failed to render template")?;

        Ok(rendered.trim().to_string())
    }

    pub fn get_default_template(publisher_type: &str) -> String {
        match publisher_type {
            "telegram" => "**{{ title }}**\n\n{{ description | truncate(480) }}\n\n🔗 [Leer más]({{ url }})".to_string(),
            "x" => "{{ title | truncate(240) }}\n\n{{ url }}".to_string(),
            "mastodon" => "{{ title }}\n\n{{ description | truncate(400) }}\n\n{{ url }}".to_string(),
            "linkedin" => "{{ title }}\n\n{{ description | truncate(700) }}\n\nLeer más: {{ url }}".to_string(),
            "matrix" => "<h3>{{ title }}</h3><p>{{ description | truncate(500) }}</p><p><a href=\"{{ url }}\">Leer más</a></p>".to_string(),
            "bluesky" => "{{ title | truncate(250) }}\n\n{{ url }}".to_string(),
            "threads" => "{{ title }}\n\n{{ description | truncate(450) }}\n\n{{ url }}".to_string(),
            "discord" => "**{{ title }}**\n\n{{ description | truncate(400) }}\n\n🔗 {{ url }}".to_string(),
            "openobserve" => "Feed: {{ title }}\nDescription: {{ description }}\nURL: {{ url }}".to_string(),
            _ => "{{ title }}\n\n{{ description }}\n\n{{ url }}".to_string(),
        }
    }
}

impl Default for TemplateRenderer {
    fn default() -> Self {
        Self::new()
    }
}

fn truncate_function(value: Value, length: Value) -> Result<Value, minijinja::Error> {
    let text = value.as_str().unwrap_or("");
    let max_len = length.as_i64().unwrap_or(100) as usize;

    if text.len() <= max_len {
        return Ok(Value::from(text));
    }

    let truncated = text.chars().take(max_len).collect::<String>();
    let result = if truncated.ends_with(' ') {
        truncated.trim_end().to_string() + "..."
    } else {
        if let Some(last_space) = truncated.rfind(' ') {
            truncated[..last_space].to_string() + "..."
        } else {
            truncated + "..."
        }
    };

    Ok(Value::from(result))
}

fn word_limit_function(value: Value, limit: Value) -> Result<Value, minijinja::Error> {
    let text = value.as_str().unwrap_or("");
    let max_words = limit.as_i64().unwrap_or(10) as usize;

    let words: Vec<&str> = text.split_whitespace().collect();

    if words.len() <= max_words {
        return Ok(Value::from(text));
    }

    let result = words[..max_words].join(" ") + "...";
    Ok(Value::from(result))
}

fn strip_html_function(value: Value) -> Result<Value, minijinja::Error> {
    let text = value.as_str().unwrap_or("");
    let mut result = text.to_string();

    result = result.replace("<br>", "\n");
    result = result.replace("<br/>", "\n");
    result = result.replace("<br />", "\n");
    result = result.replace("<p>", "\n");
    result = result.replace("</p>", "\n");

    while let Some(start) = result.find('<') {
        if let Some(end) = result[start..].find('>') {
            result.replace_range(start..start + end + 1, "");
        } else {
            break;
        }
    }

    result = result
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    Ok(Value::from(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_rendering() {
        let renderer = TemplateRenderer::new();
        let context = TemplateContext {
            title: "Test Title".to_string(),
            description: "Test Description".to_string(),
            url: "https://example.com".to_string(),
        };
        let result = renderer.render("{{ title }}: {{ description }}", &context).unwrap();
        assert_eq!(result, "Test Title: Test Description");
    }

    #[test]
    fn test_truncate_function() {
        let renderer = TemplateRenderer::new();
        let context = TemplateContext {
            title: "Very Long Title That Should Be Truncated".to_string(),
            description: "Description".to_string(),
            url: "https://example.com".to_string(),
        };
        let template = "{{ title | truncate(20) }}";
        let result = renderer.render(template, &context).unwrap();
        assert!(result.contains("..."));
    }

    #[test]
    fn test_render_from_post() {
        let post = Post::new(
            "g1".into(), "Test Post".into(), Some("A description".into()),
            "https://ex.com".into(), chrono::Utc::now(), "feed-1".into(),
        );
        let context = TemplateContext::from(&post);
        assert_eq!(context.title, "Test Post");
        assert_eq!(context.description, "A description");
        assert_eq!(context.url, "https://ex.com");
    }
}