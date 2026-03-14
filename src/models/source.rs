use serde::{Deserialize, Serialize};

use super::media::SourceType;

/// Configuration for a media source connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceConfig {
    pub url: String,
    pub token: String,
}

/// A configured media source (e.g., a Plex server).
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub id: String,
    pub source_type: SourceType,
    pub name: String,
    pub config: SourceConfig,
    pub enabled: bool,
    pub last_synced_at: Option<String>,
}

impl Source {
    /// Generate a source ID from URL.
    pub fn make_id(url: &str) -> String {
        // Normalize: strip trailing slash, lowercase scheme+host
        let normalized = url.trim_end_matches('/');
        format!("plex:{normalized}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_config_serde_roundtrip() {
        let config = SourceConfig {
            url: "http://192.168.1.100:32400".to_string(),
            token: "abc123".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: SourceConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn source_config_deserialize_from_json() {
        let json = r#"{"url":"http://localhost:32400","token":"my-token"}"#;
        let config: SourceConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.url, "http://localhost:32400");
        assert_eq!(config.token, "my-token");
    }

    #[test]
    fn make_id_strips_trailing_slash() {
        let id1 = Source::make_id("http://localhost:32400/");
        let id2 = Source::make_id("http://localhost:32400");
        assert_eq!(id1, id2);
    }

    #[test]
    fn make_id_format() {
        let id = Source::make_id("http://192.168.1.100:32400");
        assert_eq!(id, "plex:http://192.168.1.100:32400");
    }
}
