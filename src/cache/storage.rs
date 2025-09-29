use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use tokio::fs as tokio_fs;
use tracing::{debug, error, info, warn};

use super::config::FileCacheConfig;
use super::metadata::{CacheMetadata, GlobalCacheMetadata, MediaCacheKey};

/// Entry representing a cached file
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub metadata: CacheMetadata,
    pub file_path: PathBuf,
}

impl CacheEntry {
    /// Check if the file exists on disk
    pub fn exists(&self) -> bool {
        self.file_path.exists()
    }

    /// Get file size from disk
    pub fn file_size(&self) -> Result<u64> {
        let metadata = std::fs::metadata(&self.file_path)
            .with_context(|| format!("Failed to get file metadata for {:?}", self.file_path))?;
        Ok(metadata.len())
    }

    /// Read a specific byte range from the cached file
    pub fn read_range(&self, start: u64, length: u64) -> Result<Vec<u8>> {
        let mut file = File::open(&self.file_path)
            .with_context(|| format!("Failed to open cache file {:?}", self.file_path))?;

        file.seek(SeekFrom::Start(start)).with_context(|| {
            format!(
                "Failed to seek to position {} in {:?}",
                start, self.file_path
            )
        })?;

        let mut buffer = vec![0u8; length as usize];
        file.read_exact(&mut buffer).with_context(|| {
            format!("Failed to read {} bytes from {:?}", length, self.file_path)
        })?;

        Ok(buffer)
    }

    /// Check if a byte range is available in the cached file
    pub fn has_range(&self, start: u64, end: u64) -> bool {
        self.metadata.has_range(start, end)
    }
}

/// Additional metadata for cache entries
#[derive(Debug, Clone)]
pub struct CacheEntryMetadata {
    pub key: MediaCacheKey,
    pub file_path: PathBuf,
    pub size: u64,
    pub is_complete: bool,
    pub last_accessed: chrono::DateTime<chrono::Utc>,
}

/// Storage manager for the file cache
#[derive(Debug)]
pub struct CacheStorage {
    config: FileCacheConfig,
    cache_dir: PathBuf,
    metadata_file: PathBuf,
    global_metadata: GlobalCacheMetadata,
}

impl CacheStorage {
    /// Create a new cache storage instance
    pub async fn new(config: FileCacheConfig) -> Result<Self> {
        let cache_dir = config.cache_directory()?;
        let metadata_file = cache_dir.join("metadata.json");

        // Create cache directory if it doesn't exist
        tokio_fs::create_dir_all(&cache_dir)
            .await
            .with_context(|| format!("Failed to create cache directory {:?}", cache_dir))?;

        // Load existing metadata or create new
        let global_metadata = if metadata_file.exists() {
            Self::load_metadata(&metadata_file)
                .await
                .unwrap_or_else(|e| {
                    warn!(
                        "Failed to load cache metadata: {}, starting with empty cache",
                        e
                    );
                    GlobalCacheMetadata::default()
                })
        } else {
            GlobalCacheMetadata::default()
        };

        let mut storage = Self {
            config,
            cache_dir,
            metadata_file,
            global_metadata,
        };

        // Validate and clean up cache on startup
        storage.validate_cache().await?;

        info!(
            "Cache storage initialized at {:?} with {} entries ({} MB)",
            storage.cache_dir,
            storage.global_metadata.file_count,
            storage.global_metadata.total_size / 1024 / 1024
        );

        Ok(storage)
    }

    /// Load metadata from disk
    async fn load_metadata(metadata_file: &Path) -> Result<GlobalCacheMetadata> {
        let contents = tokio_fs::read_to_string(metadata_file)
            .await
            .with_context(|| format!("Failed to read metadata file {:?}", metadata_file))?;

        serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse metadata file {:?}", metadata_file))
    }

    /// Save metadata to disk
    async fn save_metadata(&self) -> Result<()> {
        let contents = serde_json::to_string_pretty(&self.global_metadata)
            .context("Failed to serialize cache metadata")?;

        tokio_fs::write(&self.metadata_file, contents)
            .await
            .with_context(|| format!("Failed to write metadata file {:?}", self.metadata_file))
    }

    /// Validate cache and remove invalid entries
    async fn validate_cache(&mut self) -> Result<()> {
        let mut invalid_keys = Vec::new();

        for (key, metadata) in &self.global_metadata.entries {
            let file_path = self.get_file_path(key);

            // Check if file exists
            if !file_path.exists() {
                warn!("Cache file missing for key {:?}: {:?}", key, file_path);
                invalid_keys.push(key.clone());
                continue;
            }

            // Check file size matches metadata
            if let Ok(actual_size) = tokio_fs::metadata(&file_path).await {
                if actual_size.len() != metadata.file_size {
                    warn!(
                        "Cache file size mismatch for key {:?}: expected {}, actual {}",
                        key,
                        metadata.file_size,
                        actual_size.len()
                    );
                    invalid_keys.push(key.clone());
                }
            } else {
                warn!(
                    "Failed to read file metadata for key {:?}: {:?}",
                    key, file_path
                );
                invalid_keys.push(key.clone());
            }
        }

        // Remove invalid entries
        for key in invalid_keys {
            self.global_metadata.remove(&key);
        }

        if !self.global_metadata.entries.is_empty() {
            self.save_metadata().await?;
        }

        Ok(())
    }

    /// Get file path for a cache key
    fn get_file_path(&self, key: &MediaCacheKey) -> PathBuf {
        let filename = format!("{}.cache", key.to_filename());
        self.cache_dir.join(filename)
    }

    /// Get cache entry by key
    pub fn get_entry(&mut self, key: &MediaCacheKey) -> Option<CacheEntry> {
        if let Some(metadata) = self.global_metadata.entries.get_mut(key) {
            metadata.mark_accessed();

            let entry = CacheEntry {
                metadata: metadata.clone(),
                file_path: self.get_file_path(key),
            };

            // Only return if file actually exists
            if entry.exists() {
                // Update global metadata access time
                self.global_metadata.last_updated = chrono::Utc::now();
                Some(entry)
            } else {
                warn!(
                    "Cache entry exists in metadata but file is missing: {:?}",
                    key
                );
                self.global_metadata.remove(key);
                None
            }
        } else {
            None
        }
    }

    /// Create a new cache entry
    pub async fn create_entry(
        &mut self,
        key: MediaCacheKey,
        original_url: String,
    ) -> Result<CacheEntry> {
        let file_path = self.get_file_path(&key);

        info!(
            "Creating cache entry for key: {:?}, file_path: {:?}",
            key, file_path
        );

        // Check if entry already exists
        if self.global_metadata.entries.contains_key(&key) {
            info!(
                "Cache entry already exists for key: {:?}, reusing existing entry",
                key
            );
            return Ok(CacheEntry {
                metadata: self.global_metadata.entries.get(&key).unwrap().clone(),
                file_path,
            });
        }

        // Create parent directory if it doesn't exist
        if let Some(parent) = file_path.parent() {
            debug!("Creating parent directory: {:?}", parent);
            tokio_fs::create_dir_all(parent)
                .await
                .map_err(|e| {
                    error!("Failed to create parent directory {:?}: {}", parent, e);
                    e
                })
                .with_context(|| {
                    format!("Failed to create parent directory for {:?}", file_path)
                })?;
            debug!("Parent directory created successfully");
        }

        // Create the file using tokio async I/O
        debug!("Creating cache file: {:?}", file_path);
        tokio_fs::File::create(&file_path)
            .await
            .map_err(|e| {
                error!("Failed to create cache file {:?}: {}", file_path, e);
                e
            })
            .with_context(|| format!("Failed to create cache file {:?}", file_path))?;
        debug!("Cache file created successfully");

        let metadata = CacheMetadata::new(key.clone(), original_url.clone());
        debug!("Created metadata for key: {:?}, URL: {}", key, original_url);

        self.global_metadata.insert(metadata.clone());
        debug!("Inserted metadata into global cache");

        // Save metadata
        debug!("Saving metadata to disk");
        self.save_metadata().await.map_err(|e| {
            error!("Failed to save metadata: {}", e);
            e
        })?;
        info!("Cache entry created successfully for key: {:?}", key);

        Ok(CacheEntry {
            metadata,
            file_path,
        })
    }

    /// Write data to a cache entry at a specific offset
    pub async fn write_to_entry(
        &mut self,
        key: &MediaCacheKey,
        offset: u64,
        data: &[u8],
    ) -> Result<()> {
        let file_path = self.get_file_path(key);

        // Write data to file
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&file_path)
            .with_context(|| format!("Failed to open cache file for writing: {:?}", file_path))?;

        file.seek(SeekFrom::Start(offset))
            .with_context(|| format!("Failed to seek to offset {} in {:?}", offset, file_path))?;

        file.write_all(data)
            .with_context(|| format!("Failed to write data to cache file {:?}", file_path))?;

        file.flush()
            .with_context(|| format!("Failed to flush cache file {:?}", file_path))?;

        // Update metadata
        if let Some(metadata) = self.global_metadata.entries.get_mut(key) {
            let end_offset = offset + data.len() as u64 - 1;
            metadata.add_range(offset, end_offset);

            // Update file size if necessary
            if let Ok(file_metadata) = file.metadata() {
                let old_size = metadata.file_size;
                metadata.file_size = file_metadata.len();

                // Update global size tracking
                self.global_metadata.total_size =
                    self.global_metadata.total_size.saturating_sub(old_size) + metadata.file_size;
            }

            self.global_metadata.last_updated = chrono::Utc::now();
        }

        // Save metadata periodically (not on every write for performance)
        if offset % (1024 * 1024) == 0 {
            // Save every 1MB written
            self.save_metadata().await?;
        }

        Ok(())
    }

    /// Remove a cache entry
    pub async fn remove_entry(&mut self, key: &MediaCacheKey) -> Result<()> {
        let file_path = self.get_file_path(key);

        // Remove file
        if file_path.exists() {
            tokio_fs::remove_file(&file_path)
                .await
                .with_context(|| format!("Failed to remove cache file {:?}", file_path))?;
        }

        // Remove from metadata
        self.global_metadata.remove(key);

        // Save metadata
        self.save_metadata().await?;

        debug!("Removed cache entry for key: {:?}", key);
        Ok(())
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> CacheStats {
        CacheStats {
            total_size_bytes: self.global_metadata.total_size,
            file_count: self.global_metadata.file_count,
            max_size_bytes: self.config.max_size_mb * 1024 * 1024,
            cache_dir: self.cache_dir.clone(),
        }
    }

    /// List all cache entries
    pub fn list_entries(&self) -> Vec<CacheEntryMetadata> {
        self.global_metadata
            .entries
            .iter()
            .map(|(key, metadata)| CacheEntryMetadata {
                key: key.clone(),
                file_path: self.get_file_path(key),
                size: metadata.file_size,
                is_complete: metadata.is_complete,
                last_accessed: metadata.last_accessed,
            })
            .collect()
    }

    /// Cleanup cache to fit within size limits
    pub async fn cleanup_cache(&mut self) -> Result<()> {
        let max_size = self.config.max_size_mb * 1024 * 1024;
        let max_files = self.config.max_files_count;

        if self.global_metadata.total_size <= max_size
            && self.global_metadata.file_count <= max_files
        {
            return Ok(()); // No cleanup needed
        }

        info!(
            "Starting cache cleanup - current size: {} MB, max: {} MB",
            self.global_metadata.total_size / 1024 / 1024,
            max_size / 1024 / 1024
        );

        // Get entries sorted by priority (lowest priority first for removal)
        let entries_to_remove: Vec<_> = {
            let entries_by_priority = self.global_metadata.entries_by_priority();
            let mut to_remove = Vec::new();

            for (key, metadata) in entries_by_priority {
                // Stop if we're within limits
                if self.global_metadata.total_size <= max_size
                    && self.global_metadata.file_count <= max_files
                {
                    break;
                }

                to_remove.push((key.clone(), metadata.file_size));

                // Update our running totals to see if we need more removals
                if self
                    .global_metadata
                    .total_size
                    .saturating_sub(metadata.file_size)
                    <= max_size
                    && self.global_metadata.file_count.saturating_sub(1) <= max_files
                {
                    break;
                }
            }

            to_remove
        };

        let mut removed_count = 0;
        let mut freed_bytes = 0;

        for (key, file_size) in entries_to_remove {
            // Remove this entry
            freed_bytes += file_size;
            removed_count += 1;

            self.remove_entry(&key).await.unwrap_or_else(|e| {
                error!("Failed to remove cache entry {:?}: {}", key, e);
            });
        }

        if removed_count > 0 {
            info!(
                "Cache cleanup completed - removed {} files, freed {} MB",
                removed_count,
                freed_bytes / 1024 / 1024
            );
        }

        Ok(())
    }

    /// Check if an entry exists and is complete
    pub fn is_complete(&self, key: &MediaCacheKey) -> bool {
        self.global_metadata
            .entries
            .get(key)
            .map(|metadata| metadata.is_complete)
            .unwrap_or(false)
    }

    /// Get available disk space for cache directory
    pub async fn get_available_space(&self) -> Result<u64> {
        let metadata = tokio_fs::metadata(&self.cache_dir).await.with_context(|| {
            format!(
                "Failed to get metadata for cache directory {:?}",
                self.cache_dir
            )
        })?;

        // This is a simplified approach - in a real implementation,
        // you'd want to use platform-specific APIs to get actual free space
        // For now, we'll return a large value
        Ok(u64::MAX)
    }

    /// Force save metadata to disk
    pub async fn flush_metadata(&self) -> Result<()> {
        self.save_metadata().await
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_size_bytes: u64,
    pub file_count: u32,
    pub max_size_bytes: u64,
    pub cache_dir: PathBuf,
}

impl CacheStats {
    pub fn usage_percentage(&self) -> f64 {
        if self.max_size_bytes == 0 {
            return 0.0;
        }
        (self.total_size_bytes as f64 / self.max_size_bytes as f64) * 100.0
    }

    pub fn total_size_mb(&self) -> f64 {
        self.total_size_bytes as f64 / 1024.0 / 1024.0
    }

    pub fn max_size_mb(&self) -> f64 {
        self.max_size_bytes as f64 / 1024.0 / 1024.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MediaItemId, SourceId};
    use tempfile::TempDir;

    async fn create_test_storage() -> (CacheStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let mut config = FileCacheConfig::default();
        config.cache_directory = Some(temp_dir.path().to_path_buf());
        config.max_size_mb = 100; // 100MB for testing

        let storage = CacheStorage::new(config).await.unwrap();
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_cache_storage_creation() {
        let (_storage, _temp_dir) = create_test_storage().await;
        // Test passes if no panic
    }

    #[tokio::test]
    async fn test_create_and_get_entry() {
        let (mut storage, _temp_dir) = create_test_storage().await;

        let key = MediaCacheKey::new(
            SourceId::from("test-source"),
            MediaItemId::from("test-media"),
            "1080p",
        );

        // Create entry
        let entry = storage
            .create_entry(key.clone(), "http://test.com/video.mp4".to_string())
            .await
            .unwrap();
        assert!(entry.exists());

        // Get entry
        let retrieved = storage.get_entry(&key).unwrap();
        assert_eq!(retrieved.metadata.cache_key, key);
        assert_eq!(retrieved.metadata.original_url, "http://test.com/video.mp4");
    }

    #[tokio::test]
    async fn test_write_and_read_data() {
        let (mut storage, _temp_dir) = create_test_storage().await;

        let key = MediaCacheKey::new(
            SourceId::from("test-source"),
            MediaItemId::from("test-media"),
            "1080p",
        );

        // Create entry
        storage
            .create_entry(key.clone(), "http://test.com/video.mp4".to_string())
            .await
            .unwrap();

        // Write data
        let test_data = b"Hello, World!";
        storage.write_to_entry(&key, 0, test_data).await.unwrap();

        // Read data back
        let entry = storage.get_entry(&key).unwrap();
        let read_data = entry.read_range(0, test_data.len() as u64).unwrap();
        assert_eq!(read_data, test_data);

        // Check metadata
        assert!(entry.has_range(0, test_data.len() as u64 - 1));
    }

    #[tokio::test]
    async fn test_cache_cleanup() {
        let (mut storage, _temp_dir) = create_test_storage().await;

        // Create multiple entries to exceed limits
        for i in 0..5 {
            let key = MediaCacheKey::new(
                SourceId::from("test-source"),
                MediaItemId::from(&format!("test-media-{}", i)),
                "1080p",
            );

            storage
                .create_entry(key.clone(), format!("http://test.com/video{}.mp4", i))
                .await
                .unwrap();

            // Write some data to each entry
            let data = vec![0u8; 1024 * 1024]; // 1MB
            storage.write_to_entry(&key, 0, &data).await.unwrap();
        }

        // Set low limits to trigger cleanup
        storage.config.max_size_mb = 3; // 3MB limit
        storage.config.max_files_count = 3;

        // Run cleanup
        storage.cleanup_cache().await.unwrap();

        let stats = storage.get_stats();
        assert!(stats.total_size_bytes <= 3 * 1024 * 1024);
        assert!(stats.file_count <= 3);
    }

    #[tokio::test]
    async fn test_remove_entry() {
        let (mut storage, _temp_dir) = create_test_storage().await;

        let key = MediaCacheKey::new(
            SourceId::from("test-source"),
            MediaItemId::from("test-media"),
            "1080p",
        );

        // Create and verify entry
        storage
            .create_entry(key.clone(), "http://test.com/video.mp4".to_string())
            .await
            .unwrap();
        assert!(storage.get_entry(&key).is_some());

        // Remove entry
        storage.remove_entry(&key).await.unwrap();
        assert!(storage.get_entry(&key).is_none());
    }
}
