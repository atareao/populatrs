use anyhow::{Context, Result};
use minijinja::{Environment, Value};
use serde_json::json;

/// Estructura que contiene los datos para el renderizado de templates
#[derive(Debug, Clone)]
pub struct TemplateContext {
    pub title: String,
    pub description: String,
    pub url: String,
}

/// Renderizador de templates que utiliza MiniliJinja
pub struct TemplateRenderer {
    env: Environment<'static>,
}

impl TemplateRenderer {
    /// Crea una nueva instancia del renderizador
    pub fn new() -> Self {
        let mut env = Environment::new();

        // Añadir filtros útiles para templates
        env.add_filter("truncate", truncate_function);
        env.add_filter("word_limit", word_limit_function);
        env.add_filter("strip_html", strip_html_function);

        Self { env }
    }

    /// Renderiza un template con el contexto proporcionado
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

    /// Obtiene el template por defecto para un tipo de publisher específico
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

/// Función para truncar texto a un número específico de caracteres
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
        // Encontrar el último espacio para no cortar palabras
        if let Some(last_space) = truncated.rfind(' ') {
            truncated[..last_space].to_string() + "..."
        } else {
            truncated + "..."
        }
    };

    Ok(Value::from(result))
}

/// Función para limitar el número de palabras
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

/// Función básica para eliminar tags HTML
fn strip_html_function(value: Value, _args: Value) -> Result<Value, minijinja::Error> {
    let text = value.as_str().unwrap_or("");

    // Básica eliminación de tags HTML
    let mut result = text.to_string();

    // Reemplazar tags comunes con su equivalente en texto plano
    result = result.replace("<br>", "\n");
    result = result.replace("<br/>", "\n");
    result = result.replace("<br />", "\n");
    result = result.replace("<p>", "\n");
    result = result.replace("</p>", "\n");

    // Eliminar todos los tags HTML restantes
    while let Some(start) = result.find('<') {
        if let Some(end) = result[start..].find('>') {
            result.replace_range(start..start + end + 1, "");
        } else {
            break;
        }
    }

    // Limpiar espacios múltiples y saltos de línea excesivos
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

        let template = "{{ title }}: {{ description }}";
        let result = renderer.render(template, &context).unwrap();
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
        assert!(result.len() <= 24); // 20 + "..." = 23, but accounting for word boundaries
        assert!(result.contains("..."));
    }
}
