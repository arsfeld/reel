use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tokio::sync::Semaphore;

/// Maximum concurrent artwork downloads to avoid flooding the server.
const MAX_CONCURRENT_DOWNLOADS: usize = 8;

#[derive(Debug, thiserror::Error)]
pub enum ArtworkError {
    #[error("Download error: {0}")]
    Download(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Downloads and caches artwork files to disk.
pub struct ArtworkCache {
    cache_dir: PathBuf,
    http: reqwest::Client,
    /// Limits concurrent downloads to avoid overwhelming the server.
    download_semaphore: Semaphore,
}

impl ArtworkCache {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            http: reqwest::Client::new(),
            download_semaphore: Semaphore::new(MAX_CONCURRENT_DOWNLOADS),
        }
    }

    /// Get or download artwork. Returns the cached file path.
    /// Downloads are limited to [`MAX_CONCURRENT_DOWNLOADS`] at a time.
    pub async fn get_or_download(&self, url: &str) -> Result<PathBuf, ArtworkError> {
        let path = self.path_for_url(url);

        if path.exists() {
            tracing::debug!("Artwork cache hit (disk): {}", path.display());
            return Ok(path);
        }

        // Acquire semaphore permit before downloading
        let wait_start = std::time::Instant::now();
        // Semaphore is owned by ArtworkCache, so acquire only fails if the
        // cache is dropped while a download is in progress (programming error).
        let _permit = self
            .download_semaphore
            .acquire()
            .await
            .expect("semaphore closed");
        let wait_time = wait_start.elapsed();

        // Re-check after acquiring permit (another task may have downloaded it)
        if path.exists() {
            tracing::debug!("Artwork cache hit (after wait): {}", path.display());
            return Ok(path);
        }

        // Ensure cache directory exists
        std::fs::create_dir_all(&self.cache_dir)?;

        // Download
        let dl_start = std::time::Instant::now();
        let bytes = self.http.get(url).send().await?.bytes().await?;
        let dl_time = dl_start.elapsed();
        std::fs::write(&path, &bytes)?;

        tracing::info!(
            "Artwork downloaded: {} bytes, waited {:?} for slot, download {:?}",
            bytes.len(),
            wait_time,
            dl_time,
        );

        Ok(path)
    }

    /// Check if artwork is already cached. Returns the path if it exists.
    pub fn cached_path(&self, url: &str) -> Option<PathBuf> {
        let path = self.path_for_url(url);
        if path.exists() { Some(path) } else { None }
    }

    /// Compute the cache file path for a URL (deterministic, based on SHA256).
    pub fn path_for_url(&self, url: &str) -> PathBuf {
        let hash = sha256_hex(url);
        // Take first 16 hex chars to avoid very long filenames
        let short_hash = &hash[..16];

        // Preserve file extension if present
        let ext = url
            .rsplit('/')
            .next()
            .and_then(|segment| {
                // Strip query params
                let name = segment.split('?').next().unwrap_or(segment);
                let dot_pos = name.rfind('.');
                dot_pos.map(|pos| &name[pos..])
            })
            .unwrap_or("");

        self.cache_dir.join(format!("{short_hash}{ext}"))
    }

    /// Clear all cached artwork.
    pub fn clear(&self) -> Result<(), ArtworkError> {
        if self.cache_dir.exists() {
            std::fs::remove_dir_all(&self.cache_dir)?;
        }
        Ok(())
    }
}

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_for_url_is_deterministic() {
        let cache = ArtworkCache::new(PathBuf::from("/tmp/test-artwork"));
        let p1 = cache.path_for_url("http://server/thumb/123");
        let p2 = cache.path_for_url("http://server/thumb/123");
        assert_eq!(p1, p2);
    }

    #[test]
    fn different_urls_give_different_paths() {
        let cache = ArtworkCache::new(PathBuf::from("/tmp/test-artwork"));
        let p1 = cache.path_for_url("http://server/thumb/123");
        let p2 = cache.path_for_url("http://server/thumb/456");
        assert_ne!(p1, p2);
    }

    #[test]
    fn path_preserves_extension() {
        let cache = ArtworkCache::new(PathBuf::from("/tmp/test-artwork"));
        let p = cache.path_for_url("http://server/image/poster.jpg");
        assert!(p.to_string_lossy().ends_with(".jpg"));
    }

    #[test]
    fn path_handles_url_with_query_params() {
        let cache = ArtworkCache::new(PathBuf::from("/tmp/test-artwork"));
        let p = cache.path_for_url("http://server/photo.jpg?width=300&height=450");
        assert!(p.to_string_lossy().ends_with(".jpg"));
    }

    #[test]
    fn path_handles_no_extension() {
        let cache = ArtworkCache::new(PathBuf::from("/tmp/test-artwork"));
        let p = cache.path_for_url("http://server/thumb/123");
        // Should still produce a valid path without extension
        assert!(!p.to_string_lossy().is_empty());
    }

    #[test]
    fn cached_path_returns_none_for_uncached() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ArtworkCache::new(dir.path().to_path_buf());
        assert!(cache.cached_path("http://server/thumb/123").is_none());
    }

    #[tokio::test]
    async fn get_or_download_downloads_and_caches() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/poster.jpg"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_bytes(vec![0xFF, 0xD8, 0xFF, 0xE0]), // fake JPEG header
            )
            .expect(1) // Only called once
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let cache = ArtworkCache::new(dir.path().to_path_buf());
        let url = format!("{}/poster.jpg", server.uri());

        // First call downloads
        let path = cache.get_or_download(&url).await.unwrap();
        assert!(path.exists());
        assert_eq!(std::fs::read(&path).unwrap(), vec![0xFF, 0xD8, 0xFF, 0xE0]);

        // Second call uses cache (wiremock expect(1) verifies no second request)
        let path2 = cache.get_or_download(&url).await.unwrap();
        assert_eq!(path, path2);
    }

    #[tokio::test]
    async fn get_or_download_creates_cache_dir() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/img.png"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_bytes(vec![0x89, 0x50, 0x4E, 0x47]),
            )
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let cache_dir = dir.path().join("artwork");
        assert!(!cache_dir.exists());

        let cache = ArtworkCache::new(cache_dir.clone());
        let url = format!("{}/img.png", server.uri());
        cache.get_or_download(&url).await.unwrap();

        assert!(cache_dir.exists());
    }

    #[test]
    fn clear_removes_cache_dir() {
        let dir = tempfile::tempdir().unwrap();
        let cache_dir = dir.path().join("artwork");
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(cache_dir.join("test.jpg"), b"data").unwrap();

        let cache = ArtworkCache::new(cache_dir.clone());
        cache.clear().unwrap();
        assert!(!cache_dir.exists());
    }

    #[test]
    fn clear_on_nonexistent_dir_is_ok() {
        let cache = ArtworkCache::new(PathBuf::from("/tmp/nonexistent-artwork-cache-test"));
        assert!(cache.clear().is_ok());
    }
}
