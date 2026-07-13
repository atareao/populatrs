use std::path::PathBuf;

/// Application configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub data_dir: PathBuf,
    pub database_url: PathBuf,
    pub default_interval_minutes: u64,
    pub timezone: String,
    pub log_level: String,
    pub log_format: String,
    pub oidc_issuer_url: Option<String>,
    pub oidc_client_id: Option<String>,
    pub oidc_client_secret: Option<String>,
    pub oidc_redirect_url: Option<String>,
}

impl Config {
    /// Load configuration from environment variables with sensible defaults.
    pub fn load() -> Self {
        Self {
            host: env_or("HOST", "0.0.0.0"),
            port: env_or_parsed("PORT", 8080),
            data_dir: PathBuf::from(env_or("DATA_DIR", "./data")),
            database_url: PathBuf::from(env_or("DATABASE_URL", "./data/populatrs.db")),
            default_interval_minutes: env_or_parsed("CHECK_INTERVAL", 60),
            timezone: env_or("TIMEZONE", "UTC"),
            log_level: env_or("RUST_LOG", "info"),
            log_format: env_or("LOG_FORMAT", "pretty"),
            oidc_issuer_url: std::env::var("OIDC_ISSUER_URL").ok(),
            oidc_client_id: std::env::var("OIDC_CLIENT_ID").ok(),
            oidc_client_secret: env_opt("OIDC_CLIENT_SECRET"),
            oidc_redirect_url: env_opt("OIDC_REDIRECT_URL")
                .or_else(|| Some("http://localhost:8080/auth/callback".to_string())),
        }
    }

    /// Whether OIDC is fully configured (required for production).
    pub fn oidc_configured(&self) -> bool {
        self.oidc_issuer_url.is_some() && self.oidc_client_id.is_some()
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_or_parsed<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_opt(key: &str) -> Option<String> {
    let v = std::env::var(key).ok()?;
    if v.is_empty() { None } else { Some(v) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        Config::load();
        // Just ensure it doesn't panic
    }

    #[test]
    fn test_env_or() {
        assert_eq!(env_or("UNSET_VAR_XYZ", "default"), "default");
    }

    #[test]
    fn test_env_or_parsed() {
        assert_eq!(env_or_parsed::<u16>("UNSET_VAR_XYZ", 8080), 8080);
    }

    #[test]
    fn test_oidc_configured() {
        let config = Config {
            oidc_issuer_url: Some("http://localhost:8765".into()),
            oidc_client_id: Some("populatrs".into()),
            ..Config::load()
        };
        assert!(config.oidc_configured());
    }

    #[test]
    fn test_oidc_not_configured() {
        let config = Config {
            oidc_issuer_url: None,
            oidc_client_id: Some("populatrs".into()),
            ..Config::load()
        };
        assert!(!config.oidc_configured());
    }
}