use std::path::PathBuf;

/// Server-authoritative session lifetimes, in seconds.
///
/// * `idle_seconds` — reject a request once this long has passed since the
///   last explicit activity signal.
/// * `absolute_seconds` — maximum age of a session, anchored at login and
///   never extended by refresh or activity.
/// * `activity_throttle_seconds` — minimum interval the client should use
///   between activity signals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionTimeouts {
    pub idle_seconds: u64,
    pub absolute_seconds: u64,
    pub activity_throttle_seconds: u64,
}

impl Default for SessionTimeouts {
    fn default() -> Self {
        Self {
            idle_seconds: 1800,
            absolute_seconds: 14400,
            activity_throttle_seconds: 60,
        }
    }
}

impl SessionTimeouts {
    /// Load the timeouts from the process environment, falling back to the
    /// defaults on an absent or unparseable value.
    pub fn from_env() -> Self {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Parse the three timeout settings through an arbitrary lookup function.
    /// Kept generic so tests can supply deterministic values without mutating
    /// the process-wide environment.
    fn from_lookup<F>(lookup: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        let defaults = Self::default();
        Self {
            idle_seconds: parse_seconds(
                lookup("SESSION_IDLE_TIMEOUT_SECONDS"),
                defaults.idle_seconds,
            ),
            absolute_seconds: parse_seconds(
                lookup("SESSION_ABSOLUTE_TIMEOUT_SECONDS"),
                defaults.absolute_seconds,
            ),
            activity_throttle_seconds: parse_seconds(
                lookup("SESSION_ACTIVITY_THROTTLE_SECONDS"),
                defaults.activity_throttle_seconds,
            ),
        }
    }
}

/// Parse an optional integer, falling back to `default` when the value is
/// absent, not a valid non-negative number, or zero. A zero-second idle or
/// absolute timeout would immediately expire every session, so it is treated
/// as misconfiguration and replaced with the default.
fn parse_seconds(value: Option<String>, default: u64) -> u64 {
    value
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(default)
}

/// Application configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub data_dir: PathBuf,
    pub database_url: PathBuf,
    pub timezone: String,
    pub log_level: String,
    pub log_format: String,
    pub oidc_issuer_url: Option<String>,
    pub oidc_client_id: Option<String>,
    pub oidc_client_secret: Option<String>,
    pub oidc_redirect_url: Option<String>,
    pub session_timeouts: SessionTimeouts,
}

impl Config {
    /// Load configuration from environment variables with sensible defaults.
    pub fn load() -> Self {
        Self {
            host: env_or("HOST", "0.0.0.0"),
            port: env_or_parsed("PORT", 3044),
            data_dir: PathBuf::from(env_or("DATA_DIR", "./data")),
            database_url: PathBuf::from(env_or("DATABASE_URL", "./data/populatrs.db")),
            timezone: env_or("TIMEZONE", "UTC"),
            log_level: env_or("RUST_LOG", "info"),
            log_format: env_or("LOG_FORMAT", "pretty"),
            oidc_issuer_url: std::env::var("OIDC_ISSUER_URL").ok(),
            oidc_client_id: std::env::var("OIDC_CLIENT_ID").ok(),
            oidc_client_secret: env_opt("OIDC_CLIENT_SECRET"),
            oidc_redirect_url: env_opt("OIDC_REDIRECT_URL")
                .or_else(|| Some("http://localhost:3044/auth/callback".to_string())),
            session_timeouts: SessionTimeouts::from_env(),
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
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
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
        assert_eq!(env_or_parsed::<u16>("UNSET_VAR_XYZ", 3044), 3044);
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

    // ── Session timeouts ──

    #[test]
    fn test_session_timeouts_defaults() {
        let timeouts = SessionTimeouts::default();
        assert_eq!(timeouts.idle_seconds, 1800);
        assert_eq!(timeouts.absolute_seconds, 14400);
        assert_eq!(timeouts.activity_throttle_seconds, 60);
    }

    #[test]
    fn test_session_timeouts_defaults_when_env_unset() {
        let timeouts = SessionTimeouts::from_lookup(|_| None);
        assert_eq!(timeouts.idle_seconds, 1800);
        assert_eq!(timeouts.absolute_seconds, 14400);
        assert_eq!(timeouts.activity_throttle_seconds, 60);
    }

    #[test]
    fn test_session_timeouts_env_overrides_defaults() {
        let timeouts = SessionTimeouts::from_lookup(|key| match key {
            "SESSION_IDLE_TIMEOUT_SECONDS" => Some("120".to_string()),
            "SESSION_ABSOLUTE_TIMEOUT_SECONDS" => Some("600".to_string()),
            "SESSION_ACTIVITY_THROTTLE_SECONDS" => Some("15".to_string()),
            _ => None,
        });
        assert_eq!(timeouts.idle_seconds, 120);
        assert_eq!(timeouts.absolute_seconds, 600);
        assert_eq!(timeouts.activity_throttle_seconds, 15);
    }

    #[test]
    fn test_session_timeouts_unparseable_falls_back_to_default() {
        let timeouts = SessionTimeouts::from_lookup(|key| match key {
            "SESSION_IDLE_TIMEOUT_SECONDS" => Some("not-a-number".to_string()),
            "SESSION_ABSOLUTE_TIMEOUT_SECONDS" => Some(String::new()),
            "SESSION_ACTIVITY_THROTTLE_SECONDS" => Some("-5".to_string()),
            _ => None,
        });
        assert_eq!(timeouts.idle_seconds, 1800);
        assert_eq!(timeouts.absolute_seconds, 14400);
        assert_eq!(timeouts.activity_throttle_seconds, 60);
    }

    #[test]
    fn test_session_timeouts_zero_falls_back_to_default() {
        let timeouts = SessionTimeouts::from_lookup(|key| match key {
            "SESSION_IDLE_TIMEOUT_SECONDS" => Some("0".to_string()),
            "SESSION_ABSOLUTE_TIMEOUT_SECONDS" => Some("0".to_string()),
            "SESSION_ACTIVITY_THROTTLE_SECONDS" => Some("0".to_string()),
            _ => None,
        });
        assert_eq!(timeouts.idle_seconds, 1800);
        assert_eq!(timeouts.absolute_seconds, 14400);
        assert_eq!(timeouts.activity_throttle_seconds, 60);
    }

    #[test]
    fn test_config_load_wires_session_timeouts() {
        // Config::load must include the session timeouts (defaults when unset).
        let config = Config::load();
        assert!(config.session_timeouts.idle_seconds > 0);
        assert!(config.session_timeouts.absolute_seconds > 0);
        assert!(config.session_timeouts.activity_throttle_seconds > 0);
    }
}
