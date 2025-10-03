use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tracing::{debug, error, info};

use crate::db::entities::CacheEntryModel;

/// Check if an I/O error is due to disk space exhaustion (ENOSPC)
fn is_disk_full_error(err: &std::io::Error) -> bool {
    // Check for StorageFull error kind (available in Rust 1.80+)
    #[cfg(feature = "storage_full_error")]
    if matches!(err.kind(), std::io::ErrorKind::StorageFull) {
        return true;
    }

    // Check raw OS error code (28 = ENOSPC on Unix, ERROR_DISK_FULL on Windows)
    #[cfg(unix)]
    if err.raw_os_error() == Some(28) {
        return true;
    }

    #[cfg(windows)]
    if err.raw_os_error() == Some(112) {
        // ERROR_DISK_FULL
        return true;
    }

    // Fallback: check error message for common disk space error strings
    let error_msg = err.to_string().to_lowercase();
    error_msg.contains("no space left")
        || error_msg.contains("disk full")
        || error_msg.contains("out of space")
        || error_msg.contains("enospc")
}

/// Manages physical storage of chunks on disk with sparse file support
pub struct ChunkStore {
    cache_dir: PathBuf,
}

impl ChunkStore {
    /// Create a new ChunkStore with the specified cache directory
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// Get the file path for a cache entry
    pub fn get_file_path(&self, entry_id: i32) -> PathBuf {
        self.cache_dir.join(format!("{}.cache", entry_id))
    }

    /// Write a chunk to disk at the appropriate offset
    /// Uses sparse file writes - seeks to offset and writes data
    ///
    /// # Errors
    /// Returns an error with "DISK_FULL" prefix if the write fails due to insufficient disk space (ENOSPC)
    pub async fn write_chunk(
        &self,
        entry_id: i32,
        chunk_index: u64,
        chunk_size: u64,
        data: &[u8],
    ) -> Result<()> {
        let file_path = self.get_file_path(entry_id);
        let offset = chunk_index * chunk_size;

        // Open file for writing (create if doesn't exist)
        let mut file = match OpenOptions::new()
            .write(true)
            .create(true)
            .open(&file_path)
            .await
        {
            Ok(f) => f,
            Err(e) if is_disk_full_error(&e) => {
                // AC #4: Handle disk full errors gracefully
                error!(
                    "DISK_FULL: Cannot open cache file {:?} - disk space exhausted",
                    file_path
                );
                return Err(anyhow!(
                    "DISK_FULL: No space left on device to create cache file"
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                // AC #5: Handle permission errors gracefully
                error!(
                    "PERMISSION_DENIED: Cannot open cache file {:?} - permission denied",
                    file_path
                );
                return Err(anyhow!(
                    "PERMISSION_DENIED: Permission denied for cache file {:?}",
                    file_path
                ));
            }
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("Failed to open cache file {:?}", file_path));
            }
        };

        // Seek to the correct offset
        if let Err(e) = file.seek(SeekFrom::Start(offset)).await {
            if is_disk_full_error(&e) {
                error!(
                    "DISK_FULL: Cannot seek in cache file {:?} - disk space exhausted",
                    file_path
                );
                return Err(anyhow!("DISK_FULL: No space left on device"));
            }
            return Err(e).with_context(|| {
                format!("Failed to seek to offset {} in {:?}", offset, file_path)
            });
        }

        // Write the data
        if let Err(e) = file.write_all(data).await {
            if is_disk_full_error(&e) {
                // AC #4: Handle disk full errors gracefully
                error!(
                    "DISK_FULL: Cannot write {} bytes to cache file {:?} - disk space exhausted",
                    data.len(),
                    file_path
                );
                return Err(anyhow!("DISK_FULL: No space left on device during write"));
            }
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                // AC #5: Handle permission errors gracefully
                error!(
                    "PERMISSION_DENIED: Cannot write {} bytes to cache file {:?} - permission denied",
                    data.len(),
                    file_path
                );
                return Err(anyhow!(
                    "PERMISSION_DENIED: Permission denied writing to cache file {:?}",
                    file_path
                ));
            }
            return Err(e).with_context(|| {
                format!("Failed to write {} bytes to {:?}", data.len(), file_path)
            });
        }

        // Flush to ensure data is written
        if let Err(e) = file.flush().await {
            if is_disk_full_error(&e) {
                error!(
                    "DISK_FULL: Cannot flush cache file {:?} - disk space exhausted",
                    file_path
                );
                return Err(anyhow!("DISK_FULL: No space left on device during flush"));
            }
            return Err(e).with_context(|| format!("Failed to flush data to {:?}", file_path));
        }

        Ok(())
    }

    /// Read a chunk from disk
    pub async fn read_chunk(
        &self,
        entry_id: i32,
        chunk_index: u64,
        chunk_size: u64,
    ) -> Result<Vec<u8>> {
        let file_path = self.get_file_path(entry_id);
        let offset = chunk_index * chunk_size;

        let mut file = File::open(&file_path)
            .await
            .with_context(|| format!("Failed to open cache file {:?}", file_path))?;

        file.seek(SeekFrom::Start(offset))
            .await
            .with_context(|| format!("Failed to seek to offset {} in {:?}", offset, file_path))?;

        let mut buffer = vec![0u8; chunk_size as usize];
        let bytes_read = file
            .read(&mut buffer)
            .await
            .with_context(|| format!("Failed to read chunk from {:?}", file_path))?;

        buffer.truncate(bytes_read);

        Ok(buffer)
    }

    /// Read a byte range from disk (may span multiple chunks)
    pub async fn read_range(&self, entry_id: i32, start: u64, length: usize) -> Result<Vec<u8>> {
        let file_path = self.get_file_path(entry_id);

        let mut file = File::open(&file_path)
            .await
            .with_context(|| format!("Failed to open cache file {:?}", file_path))?;

        file.seek(SeekFrom::Start(start))
            .await
            .with_context(|| format!("Failed to seek to offset {} in {:?}", start, file_path))?;

        let mut buffer = vec![0u8; length];
        file.read_exact(&mut buffer).await.with_context(|| {
            format!(
                "Failed to read {} bytes at offset {} from {:?}",
                length, start, file_path
            )
        })?;

        Ok(buffer)
    }

    /// Create a sparse file with the expected size
    /// This pre-allocates the file on disk to avoid fragmentation
    pub async fn create_file(&self, entry_id: i32, expected_size: u64) -> Result<()> {
        let file_path = self.get_file_path(entry_id);

        info!(
            "Creating sparse cache file {:?} with expected size {} MB",
            file_path,
            expected_size / 1024 / 1024
        );

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&file_path)
            .await
            .with_context(|| format!("Failed to create cache file {:?}", file_path))?;

        // Set file length (creates sparse file on supported filesystems)
        file.set_len(expected_size)
            .await
            .with_context(|| format!("Failed to set file length for {:?}", file_path))?;

        info!("Successfully created cache file {:?}", file_path);

        Ok(())
    }

    /// Delete cache file
    pub async fn delete_file(&self, entry_id: i32) -> Result<()> {
        let file_path = self.get_file_path(entry_id);

        if file_path.exists() {
            tokio::fs::remove_file(&file_path)
                .await
                .with_context(|| format!("Failed to delete cache file {:?}", file_path))?;

            info!("Deleted cache file {:?}", file_path);
        } else {
            debug!(
                "Cache file {:?} does not exist, skipping deletion",
                file_path
            );
        }

        Ok(())
    }

    /// Check if a cache file exists
    pub fn file_exists(&self, entry_id: i32) -> bool {
        self.get_file_path(entry_id).exists()
    }

    /// Get the file size on disk
    pub async fn file_size(&self, entry_id: i32) -> Result<u64> {
        let file_path = self.get_file_path(entry_id);
        let metadata = tokio::fs::metadata(&file_path)
            .await
            .with_context(|| format!("Failed to get metadata for {:?}", file_path))?;

        Ok(metadata.len())
    }

    /// Get the cache directory path
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }
}

/// Helper function to calculate chunk boundaries for an entry
pub fn calculate_chunk_range(
    chunk_index: u64,
    chunk_size: u64,
    entry: &CacheEntryModel,
) -> (u64, u64) {
    let start_byte = chunk_index * chunk_size;
    let end_byte = std::cmp::min(
        start_byte + chunk_size - 1,
        entry.expected_total_size.unwrap_or(0) as u64 - 1,
    );

    (start_byte, end_byte)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_store() -> (ChunkStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let store = ChunkStore::new(temp_dir.path().to_path_buf());
        (store, temp_dir)
    }

    #[tokio::test]
    async fn test_write_and_read_chunk() {
        let (store, _temp_dir) = create_test_store().await;
        let entry_id = 1;
        let chunk_index = 0;
        let chunk_size = 1024;
        let data = b"Hello, World!";

        // Write chunk
        store
            .write_chunk(entry_id, chunk_index, chunk_size, data)
            .await
            .unwrap();

        // Read chunk
        let read_data = store
            .read_chunk(entry_id, chunk_index, chunk_size)
            .await
            .unwrap();

        // Compare
        assert_eq!(&read_data[..data.len()], data);
    }

    #[tokio::test]
    async fn test_write_chunk_at_offset() {
        let (store, _temp_dir) = create_test_store().await;
        let entry_id = 1;
        let chunk_size = 1024;

        // Write chunk 0
        let data0 = b"Chunk 0";
        store
            .write_chunk(entry_id, 0, chunk_size, data0)
            .await
            .unwrap();

        // Write chunk 2 (skip chunk 1 - sparse file test)
        let data2 = b"Chunk 2";
        store
            .write_chunk(entry_id, 2, chunk_size, data2)
            .await
            .unwrap();

        // Read chunk 0
        let read_data0 = store.read_chunk(entry_id, 0, chunk_size).await.unwrap();
        assert_eq!(&read_data0[..data0.len()], data0);

        // Read chunk 2
        let read_data2 = store.read_chunk(entry_id, 2, chunk_size).await.unwrap();
        assert_eq!(&read_data2[..data2.len()], data2);
    }

    #[tokio::test]
    async fn test_read_range() {
        let (store, _temp_dir) = create_test_store().await;
        let entry_id = 1;
        let chunk_size = 10;

        // Write some data
        let data = b"0123456789ABCDEFGHIJ";
        store
            .write_chunk(entry_id, 0, chunk_size, data)
            .await
            .unwrap();

        // Read a range spanning bytes 5-14
        let read_data = store.read_range(entry_id, 5, 10).await.unwrap();
        assert_eq!(&read_data, b"56789ABCDE");
    }

    #[tokio::test]
    async fn test_create_and_delete_file() {
        let (store, _temp_dir) = create_test_store().await;
        let entry_id = 1;

        // Create file
        store.create_file(entry_id, 1024).await.unwrap();
        assert!(store.file_exists(entry_id));

        // Check size
        let size = store.file_size(entry_id).await.unwrap();
        assert_eq!(size, 1024);

        // Delete file
        store.delete_file(entry_id).await.unwrap();
        assert!(!store.file_exists(entry_id));
    }

    #[tokio::test]
    async fn test_calculate_chunk_range() {
        use crate::db::entities::CacheEntryModel;
        use chrono::Utc;

        let entry = CacheEntryModel {
            id: 1,
            source_id: "test".to_string(),
            media_id: "test".to_string(),
            quality: "1080p".to_string(),
            original_url: "http://test".to_string(),
            file_path: "/tmp/test".to_string(),
            file_size: 0,
            expected_total_size: Some(10000),
            downloaded_bytes: 0,
            is_complete: false,
            priority: 0,
            created_at: Utc::now().naive_utc(),
            last_accessed: Utc::now().naive_utc(),
            last_modified: Utc::now().naive_utc(),
            access_count: 0,
            mime_type: None,
            video_codec: None,
            audio_codec: None,
            container: None,
            resolution_width: None,
            resolution_height: None,
            bitrate: None,
            duration_secs: None,
            etag: None,
            expires_at: None,
        };

        let chunk_size = 2048;

        // First chunk
        let (start, end) = calculate_chunk_range(0, chunk_size, &entry);
        assert_eq!(start, 0);
        assert_eq!(end, 2047);

        // Second chunk
        let (start, end) = calculate_chunk_range(1, chunk_size, &entry);
        assert_eq!(start, 2048);
        assert_eq!(end, 4095);

        // Last chunk (should be limited by file size)
        let (start, end) = calculate_chunk_range(4, chunk_size, &entry);
        assert_eq!(start, 8192);
        assert_eq!(end, 9999); // Limited by file size
    }
}
