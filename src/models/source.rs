use serde::{Deserialize, Serialize};

use super::media::SourceType;

/// Configuration for a media source connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceConfig {
    pub url: String,
    pub token: String,
    /// Server-side user id. Jellyfin needs this alongside the token to scope
    /// per-user routes; Plex does not. Optional and defaulted so existing
    /// serialized Plex configs (`{url, token}`) deserialize unchanged.
    #[serde(default)]
    pub user_id: Option<String>,
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
    /// Generate a source ID from source type + URL.
    ///
    /// The id is `{source_type}:{normalized_url}` (trailing slash stripped).
    /// **Invariant:** `make_id(SourceType::Plex, url)` must byte-equal the legacy
    /// `plex:{url}` string — existing DB rows, `MediaItem.id` composite keys, and
    /// persisted visibility keys all embed that exact prefix, so any drift would
    /// orphan a user's synced Plex media on upgrade.
    pub fn make_id(source_type: SourceType, url: &str) -> String {
        // Normalize: strip trailing slash, lowercase scheme+host
        let normalized = url.trim_end_matches('/');
        format!("{}:{normalized}", source_type.as_str())
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
            user_id: None,
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
    fn source_config_without_user_id_deserializes() {
        // Legacy serialized Plex configs carry no `user_id`; it must default to None.
        let json = r#"{"url":"http://localhost:32400","token":"my-token"}"#;
        let config: SourceConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.user_id, None);
    }

    #[test]
    fn source_config_with_user_id_round_trips() {
        let config = SourceConfig {
            url: "https://jelly.example".to_string(),
            token: "jf-token".to_string(),
            user_id: Some("user-42".to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: SourceConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.user_id, Some("user-42".to_string()));
        assert_eq!(config, parsed);
    }

    #[test]
    fn make_id_strips_trailing_slash() {
        let id1 = Source::make_id(SourceType::Plex, "http://localhost:32400/");
        let id2 = Source::make_id(SourceType::Plex, "http://localhost:32400");
        assert_eq!(id1, id2);
    }

    #[test]
    fn make_id_format() {
        let id = Source::make_id(SourceType::Plex, "http://192.168.1.100:32400");
        assert_eq!(id, "plex:http://192.168.1.100:32400");
    }

    #[test]
    fn make_id_plex_unchanged() {
        // Upgrade-safety invariant: the Plex id must byte-equal the legacy form,
        // or existing media/visibility keys stop resolving.
        let id = Source::make_id(SourceType::Plex, "http://192.168.1.100:32400/");
        assert_eq!(id, "plex:http://192.168.1.100:32400");
    }

    #[test]
    fn make_id_jellyfin_prefix() {
        let id = Source::make_id(SourceType::Jellyfin, "https://jelly.example/");
        assert_eq!(id, "jellyfin:https://jelly.example");
    }
}
