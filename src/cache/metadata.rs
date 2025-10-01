use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::models::{MediaItemId, SourceId};

/// Media-specific cache key for file storage
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MediaCacheKey {
    pub source_id: SourceId,
    pub media_id: MediaItemId,
    pub quality: String, // e.g., "original", "1080p", "720p"
}

impl MediaCacheKey {
    pub fn new(source_id: SourceId, media_id: MediaItemId, quality: impl Into<String>) -> Self {
        Self {
            source_id,
            media_id,
            quality: quality.into(),
        }
    }

    /// Convert to string for filename-safe representation
    pub fn to_filename(&self) -> String {
        format!(
            "{}__{}__{}",
            self.source_id
                .as_str()
                .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_"),
            self.media_id
                .as_str()
                .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_"),
            self.quality
                .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")
        )
    }

    /// Convert from filename back to MediaCacheKey
    pub fn from_filename(filename: &str) -> Result<Self> {
        let parts: Vec<&str> = filename.split("__").collect();
        if parts.len() != 3 {
            return Err(anyhow::anyhow!("Invalid cache key filename format"));
        }

        Ok(Self {
            source_id: SourceId::from(parts[0]),
            media_id: MediaItemId::from(parts[1]),
            quality: parts[2].to_string(),
        })
    }
}

/// Metadata for a cached media file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetadata {
    /// Unique cache key for this entry
    pub cache_key: MediaCacheKey,

    /// Original media URL
    pub original_url: String,

    /// File size in bytes (actual size on disk)
    pub file_size: u64,

    /// Expected total file size (from server Content-Length)
    pub expected_total_size: u64,

    /// Number of bytes actually downloaded
    pub downloaded_bytes: u64,

    /// Whether the download is complete
    pub is_complete: bool,

    /// When this entry was created
    pub created_at: DateTime<Utc>,

    /// When this entry was last accessed
    pub last_accessed: DateTime<Utc>,

    /// When this entry was last modified
    pub last_modified: DateTime<Utc>,

    /// Number of times this entry has been accessed
    pub access_count: u64,

    /// MIME type of the cached file
    pub mime_type: Option<String>,

    /// Video codec information
    pub video_codec: Option<String>,

    /// Audio codec information
    pub audio_codec: Option<String>,

    /// Container format
    pub container: Option<String>,

    /// Video resolution
    pub resolution: Option<(u32, u32)>,

    /// Bitrate in bits per second
    pub bitrate: Option<u64>,

    /// Duration in seconds
    pub duration_secs: Option<f64>,

    /// Map of byte ranges that have been downloaded
    /// Key: start offset, Value: end offset
    pub downloaded_ranges: HashMap<u64, u64>,

    /// ETag or similar identifier for cache validation
    pub etag: Option<String>,

    /// HTTP headers from the original response
    pub headers: HashMap<String, String>,
}

impl CacheMetadata {
    pub fn new(cache_key: MediaCacheKey, original_url: String) -> Self {
        let now = Utc::now();
        Self {
            cache_key,
            original_url,
            file_size: 0,
            expected_total_size: 0,
            downloaded_bytes: 0,
            is_complete: false,
            created_at: now,
            last_accessed: now,
            last_modified: now,
            access_count: 0,
            mime_type: None,
            video_codec: None,
            audio_codec: None,
            container: None,
            resolution: None,
            bitrate: None,
            duration_secs: None,
            downloaded_ranges: HashMap::new(),
            etag: None,
            headers: HashMap::new(),
        }
    }

    /// Update access statistics
    pub fn mark_accessed(&mut self) {
        self.last_accessed = Utc::now();
        self.access_count += 1;
    }

    /// Check if a byte range has been downloaded
    pub fn has_range(&self, start: u64, end: u64) -> bool {
        for (&range_start, &range_end) in &self.downloaded_ranges {
            if start >= range_start && end <= range_end {
                return true;
            }
        }
        false
    }

    /// Add a downloaded byte range
    pub fn add_range(&mut self, start: u64, end: u64) {
        self.downloaded_ranges.insert(start, end);
        self.downloaded_bytes += end - start + 1;
        self.last_modified = Utc::now();

        // Merge overlapping ranges
        self.merge_ranges();

        // Check if download is complete
        // Must check against expected_total_size, not file_size
        // file_size is the current size on disk which grows as we download
        // expected_total_size is the final size from Content-Length header
        if let Some(&max_end) = self.downloaded_ranges.values().max()
            && self.expected_total_size > 0
            && max_end >= self.expected_total_size - 1
        {
            self.is_complete = true;
        }
    }

    /// Merge overlapping and adjacent ranges
    fn merge_ranges(&mut self) {
        let mut ranges: Vec<(u64, u64)> = self
            .downloaded_ranges
            .iter()
            .map(|(&start, &end)| (start, end))
            .collect();

        if ranges.is_empty() {
            return;
        }

        ranges.sort_by_key(|&(start, _)| start);

        let mut merged = Vec::new();
        let mut current = ranges[0];

        for &(start, end) in &ranges[1..] {
            if start <= current.1 + 1 {
                // Overlapping or adjacent ranges - merge
                current.1 = std::cmp::max(current.1, end);
            } else {
                // Non-overlapping range - save current and start new
                merged.push(current);
                current = (start, end);
            }
        }
        merged.push(current);

        // Rebuild the map
        self.downloaded_ranges.clear();
        for (start, end) in merged {
            self.downloaded_ranges.insert(start, end);
        }

        // Recalculate downloaded bytes
        self.downloaded_bytes = self
            .downloaded_ranges
            .iter()
            .map(|(&start, &end)| end - start + 1)
            .sum();
    }

    /// Get download progress as a percentage (0.0 to 1.0)
    pub fn progress(&self) -> f64 {
        if self.file_size == 0 {
            return 0.0;
        }
        self.downloaded_bytes as f64 / self.file_size as f64
    }

    /// Calculate priority score for LRU eviction (higher = keep longer)
    pub fn priority_score(&self) -> f64 {
        let now = Utc::now();
        let age_hours = now.signed_duration_since(self.last_accessed).num_hours() as f64;
        let access_weight = (self.access_count as f64).ln().max(1.0);
        let completion_weight = if self.is_complete { 2.0 } else { 1.0 };

        // Higher score = higher priority to keep
        // Recent access and high access count increase priority
        // Complete files get bonus
        (access_weight * completion_weight) / (age_hours + 1.0)
    }
}

/// Global cache metadata manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalCacheMetadata {
    /// Map of cache key to metadata
    /// Using String keys for serialization compatibility
    #[serde(
        serialize_with = "serialize_entries",
        deserialize_with = "deserialize_entries"
    )]
    pub entries: HashMap<MediaCacheKey, CacheMetadata>,

    /// Total cache size in bytes
    pub total_size: u64,

    /// Number of files in cache
    pub file_count: u32,

    /// When the metadata was last updated
    pub last_updated: DateTime<Utc>,
}

// Custom serialization for HashMap with MediaCacheKey
fn serialize_entries<S>(
    entries: &HashMap<MediaCacheKey, CacheMetadata>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeMap;
    let mut map = serializer.serialize_map(Some(entries.len()))?;
    for (k, v) in entries {
        // Convert MediaCacheKey to string for serialization
        let key_str = format!(
            "{}__{}__{}",
            k.source_id.as_str(),
            k.media_id.as_str(),
            k.quality
        );
        map.serialize_entry(&key_str, v)?;
    }
    map.end()
}

// Custom deserialization for HashMap with MediaCacheKey
fn deserialize_entries<'de, D>(
    deserializer: D,
) -> Result<HashMap<MediaCacheKey, CacheMetadata>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Deserialize, Error};
    let string_map: HashMap<String, CacheMetadata> = HashMap::deserialize(deserializer)?;
    let mut result = HashMap::new();

    for (key_str, value) in string_map {
        // Parse the string back to MediaCacheKey
        let parts: Vec<&str> = key_str.split("__").collect();
        if parts.len() != 3 {
            return Err(D::Error::custom(format!(
                "Invalid cache key format: {}",
                key_str
            )));
        }

        let key = MediaCacheKey {
            source_id: SourceId::from(parts[0]),
            media_id: MediaItemId::from(parts[1]),
            quality: parts[2].to_string(),
        };
        result.insert(key, value);
    }

    Ok(result)
}

impl Default for GlobalCacheMetadata {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            total_size: 0,
            file_count: 0,
            last_updated: Utc::now(),
        }
    }
}

impl GlobalCacheMetadata {
    /// Add or update cache entry metadata
    pub fn insert(&mut self, metadata: CacheMetadata) {
        let key = metadata.cache_key.clone();

        // Remove old size if updating
        if let Some(old_metadata) = self.entries.get(&key) {
            self.total_size = self.total_size.saturating_sub(old_metadata.file_size);
        } else {
            self.file_count += 1;
        }

        // Add new size
        self.total_size += metadata.file_size;
        self.entries.insert(key, metadata);
        self.last_updated = Utc::now();
    }

    /// Remove cache entry metadata
    pub fn remove(&mut self, key: &MediaCacheKey) -> Option<CacheMetadata> {
        if let Some(metadata) = self.entries.remove(key) {
            self.total_size = self.total_size.saturating_sub(metadata.file_size);
            self.file_count = self.file_count.saturating_sub(1);
            self.last_updated = Utc::now();
            Some(metadata)
        } else {
            None
        }
    }

    /// Get entries sorted by priority for LRU eviction
    pub fn entries_by_priority(&self) -> Vec<(&MediaCacheKey, &CacheMetadata)> {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by(|a, b| {
            a.1.priority_score()
                .partial_cmp(&b.1.priority_score())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        entries
    }

    /// Update access time for an entry
    pub fn mark_accessed(&mut self, key: &MediaCacheKey) {
        if let Some(metadata) = self.entries.get_mut(key) {
            metadata.mark_accessed();
            self.last_updated = Utc::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_cache_key_filename() {
        let key = MediaCacheKey::new(
            SourceId::from("plex-server"),
            MediaItemId::from("movie-123"),
            "1080p",
        );

        let filename = key.to_filename();
        assert_eq!(filename, "plex-server__movie-123__1080p");

        let parsed = MediaCacheKey::from_filename(&filename).unwrap();
        assert_eq!(parsed, key);
    }

    #[test]
    fn test_media_cache_key_sanitization() {
        let key = MediaCacheKey::new(
            SourceId::from("plex/server:8080"),
            MediaItemId::from("movie*123?"),
            "1080p<>",
        );

        let filename = key.to_filename();
        assert!(!filename.contains('/'));
        assert!(!filename.contains(':'));
        assert!(!filename.contains('*'));
        assert!(!filename.contains('?'));
        assert!(!filename.contains('<'));
        assert!(!filename.contains('>'));
    }

    #[test]
    fn test_cache_metadata_ranges() {
        let mut metadata = CacheMetadata::new(
            MediaCacheKey::new(SourceId::from("test"), MediaItemId::from("test"), "test"),
            "http://test.com".to_string(),
        );

        metadata.file_size = 1000;

        // Add non-overlapping ranges
        metadata.add_range(0, 99);
        metadata.add_range(200, 299);
        assert_eq!(metadata.downloaded_ranges.len(), 2);
        assert_eq!(metadata.downloaded_bytes, 200);

        // Add overlapping range
        metadata.add_range(90, 210);
        assert_eq!(metadata.downloaded_ranges.len(), 1);
        assert_eq!(metadata.downloaded_bytes, 300);

        assert!(metadata.has_range(50, 60));
        assert!(metadata.has_range(250, 280));
        assert!(!metadata.has_range(350, 400));
    }

    #[test]
    fn test_cache_metadata_completion() {
        let mut metadata = CacheMetadata::new(
            MediaCacheKey::new(SourceId::from("test"), MediaItemId::from("test"), "test"),
            "http://test.com".to_string(),
        );

        metadata.file_size = 1000;
        assert!(!metadata.is_complete);

        // Add complete range
        metadata.add_range(0, 999);
        assert!(metadata.is_complete);
        assert_eq!(metadata.progress(), 1.0);
    }

    #[test]
    fn test_priority_score() {
        let mut metadata1 = CacheMetadata::new(
            MediaCacheKey::new(SourceId::from("test"), MediaItemId::from("test1"), "test"),
            "http://test.com".to_string(),
        );

        let mut metadata2 = CacheMetadata::new(
            MediaCacheKey::new(SourceId::from("test"), MediaItemId::from("test2"), "test"),
            "http://test.com".to_string(),
        );

        metadata1.access_count = 10;
        metadata2.access_count = 1;

        // Higher access count should give higher priority
        assert!(metadata1.priority_score() > metadata2.priority_score());

        metadata2.is_complete = true;
        metadata2.access_count = 5;

        // Complete files get bonus
        assert!(metadata2.priority_score() > 0.0);
    }
}
