//! Cache configuration with TTL settings per content type
//!
//! This module provides centralized TTL (Time-To-Live) configuration for cached metadata.
//! Different content types have different refresh intervals based on how frequently they change.

use chrono::{DateTime, Utc};
use std::time::Duration;

/// Type of content for TTL lookup
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    /// Library list (changes rarely)
    Libraries,
    /// Media items like movies and shows (changes occasionally)
    MediaItems,
    /// Episodes (changes occasionally)
    Episodes,
    /// Full metadata including cast/crew (changes rarely)
    FullMetadata,
    /// Home page sections like "Continue Watching" (changes frequently)
    HomeSections,
}

/// Configuration for cache TTL durations per content type.
///
/// These values control how long cached metadata is considered "fresh" before
/// being eligible for background refresh. The stale-while-revalidate pattern
/// means stale data is served immediately while fresh data loads in background.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// TTL for library list (default: 1 hour)
    pub libraries_ttl: Duration,
    /// TTL for media items like movies/shows (default: 4 hours)
    pub media_items_ttl: Duration,
    /// TTL for episodes (default: 12 hours)
    pub episodes_ttl: Duration,
    /// TTL for full metadata including cast/crew (default: 24 hours)
    pub full_metadata_ttl: Duration,
    /// TTL for home page sections (default: 30 minutes)
    pub home_sections_ttl: Duration,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            libraries_ttl: Duration::from_secs(3600),          // 1 hour
            media_items_ttl: Duration::from_secs(4 * 3600),    // 4 hours
            episodes_ttl: Duration::from_secs(12 * 3600),      // 12 hours
            full_metadata_ttl: Duration::from_secs(24 * 3600), // 24 hours
            home_sections_ttl: Duration::from_secs(1800),      // 30 minutes
        }
    }
}

impl CacheConfig {
    /// Create a new CacheConfig with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the TTL duration for a specific content type
    pub fn ttl_for(&self, content_type: ContentType) -> Duration {
        match content_type {
            ContentType::Libraries => self.libraries_ttl,
            ContentType::MediaItems => self.media_items_ttl,
            ContentType::Episodes => self.episodes_ttl,
            ContentType::FullMetadata => self.full_metadata_ttl,
            ContentType::HomeSections => self.home_sections_ttl,
        }
    }

    /// Check if content is stale based on its fetched_at timestamp and content type
    ///
    /// Returns true if the content should be refreshed (either because it's older
    /// than the TTL or because the fetched_at timestamp is missing/null).
    pub fn is_stale(&self, fetched_at: Option<DateTime<Utc>>, content_type: ContentType) -> bool {
        match fetched_at {
            Some(timestamp) => {
                let ttl = self.ttl_for(content_type);
                let age = Utc::now().signed_duration_since(timestamp);
                age > chrono::Duration::from_std(ttl).unwrap_or(chrono::Duration::MAX)
            }
            None => {
                // If no fetched_at timestamp, content is considered stale
                true
            }
        }
    }

    /// Check if a naive datetime (from DB) is stale
    pub fn is_stale_naive(
        &self,
        fetched_at: Option<chrono::NaiveDateTime>,
        content_type: ContentType,
    ) -> bool {
        self.is_stale(fetched_at.map(|dt| dt.and_utc()), content_type)
    }

    /// Get the age of content in seconds, or None if no timestamp
    pub fn age_secs(&self, fetched_at: Option<DateTime<Utc>>) -> Option<i64> {
        fetched_at.map(|timestamp| Utc::now().signed_duration_since(timestamp).num_seconds())
    }
}

/// Global static instance for convenient access
static CACHE_CONFIG: std::sync::OnceLock<CacheConfig> = std::sync::OnceLock::new();

/// Get the global cache configuration
pub fn cache_config() -> &'static CacheConfig {
    CACHE_CONFIG.get_or_init(CacheConfig::default)
}

/// Initialize the global cache configuration with custom values
/// This should be called early in app initialization if custom values are needed
pub fn init_cache_config(config: CacheConfig) {
    let _ = CACHE_CONFIG.set(config);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_ttl_values() {
        let config = CacheConfig::default();

        assert_eq!(config.libraries_ttl.as_secs(), 3600);
        assert_eq!(config.media_items_ttl.as_secs(), 4 * 3600);
        assert_eq!(config.episodes_ttl.as_secs(), 12 * 3600);
        assert_eq!(config.full_metadata_ttl.as_secs(), 24 * 3600);
        assert_eq!(config.home_sections_ttl.as_secs(), 1800);
    }

    #[test]
    fn test_ttl_for_content_type() {
        let config = CacheConfig::default();

        assert_eq!(
            config.ttl_for(ContentType::Libraries).as_secs(),
            config.libraries_ttl.as_secs()
        );
        assert_eq!(
            config.ttl_for(ContentType::MediaItems).as_secs(),
            config.media_items_ttl.as_secs()
        );
        assert_eq!(
            config.ttl_for(ContentType::Episodes).as_secs(),
            config.episodes_ttl.as_secs()
        );
        assert_eq!(
            config.ttl_for(ContentType::FullMetadata).as_secs(),
            config.full_metadata_ttl.as_secs()
        );
        assert_eq!(
            config.ttl_for(ContentType::HomeSections).as_secs(),
            config.home_sections_ttl.as_secs()
        );
    }

    #[test]
    fn test_is_stale_with_none_timestamp() {
        let config = CacheConfig::default();

        // None fetched_at should always be stale
        assert!(config.is_stale(None, ContentType::Libraries));
        assert!(config.is_stale(None, ContentType::MediaItems));
        assert!(config.is_stale(None, ContentType::HomeSections));
    }

    #[test]
    fn test_is_stale_with_fresh_content() {
        let config = CacheConfig::default();

        // Just fetched - should not be stale
        let now = Utc::now();
        assert!(!config.is_stale(Some(now), ContentType::Libraries));
        assert!(!config.is_stale(Some(now), ContentType::MediaItems));
        assert!(!config.is_stale(Some(now), ContentType::HomeSections));
    }

    #[test]
    fn test_is_stale_with_old_content() {
        let config = CacheConfig::default();

        // 2 hours ago - should be stale for libraries (1h TTL) but not for media_items (4h TTL)
        let two_hours_ago = Utc::now() - chrono::Duration::hours(2);
        assert!(config.is_stale(Some(two_hours_ago), ContentType::Libraries));
        assert!(!config.is_stale(Some(two_hours_ago), ContentType::MediaItems));

        // 6 hours ago - should be stale for both libraries and media_items
        let six_hours_ago = Utc::now() - chrono::Duration::hours(6);
        assert!(config.is_stale(Some(six_hours_ago), ContentType::Libraries));
        assert!(config.is_stale(Some(six_hours_ago), ContentType::MediaItems));
    }

    #[test]
    fn test_age_secs() {
        let config = CacheConfig::default();

        assert!(config.age_secs(None).is_none());

        let now = Utc::now();
        let age = config.age_secs(Some(now)).unwrap();
        // Should be very close to 0 (within 1 second)
        assert!(age >= 0 && age < 2);
    }
}
