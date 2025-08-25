use anyhow::{Result, anyhow};
use futures::future::join_all;
use gtk4::gdk_pixbuf::Pixbuf;
use gtk4::{gdk, gio, glib, prelude::*};
use lru::LruCache;
use reqwest::Client;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;
use tokio::fs;
use tokio::sync::{RwLock, Semaphore};
use tracing::{info, trace};

use crate::constants::*;

/// Image size variants for different UI contexts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageSize {
    /// Small thumbnail (120x180 for posters, 120x68 for landscape)
    Small,
    /// Medium size (180x270 for posters, 320x180 for landscape)
    Medium,
    /// Large/full size (360x540 for posters, 640x360 for landscape)
    Large,
    /// Original size
    Original,
}

impl ImageSize {
    // Keep dimensions for UI sizing hints (card dimensions)
    pub fn dimensions_for_poster(&self) -> (u32, u32) {
        match self {
            Self::Small => (120, 180),
            Self::Medium => (180, 270),
            Self::Large => (360, 540),
            Self::Original => (0, 0),
        }
    }

    pub fn dimensions_for_landscape(&self) -> (u32, u32) {
        match self {
            Self::Small => (120, 68),
            Self::Medium => (320, 180),
            Self::Large => (640, 360),
            Self::Original => (0, 0),
        }
    }
}

/// Cache entry with metadata
struct CacheEntry {
    texture: gdk::Texture,
    size_bytes: usize,
    last_accessed: Instant,
    access_count: u32,
}

/// Image loader with LRU cache
pub struct ImageLoader {
    client: Client,
    cache_dir: PathBuf,
    memory_cache: Arc<RwLock<LruCache<String, CacheEntry>>>,
    cache_size: Arc<AtomicU64>,
    max_cache_size: u64,
    download_semaphore: Arc<Semaphore>,
    active_downloads: Arc<AtomicUsize>,
}

impl ImageLoader {
    pub fn new() -> Result<Self> {
        // Create cache directory structure
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| anyhow!("Failed to get cache directory"))?
            .join("reel")
            .join("images");

        // Create subdirectories for different sizes
        for size_dir in &["small", "medium", "large", "original"] {
            std::fs::create_dir_all(cache_dir.join(size_dir))?;
        }

        // Build optimized HTTP client
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS))
            .connect_timeout(std::time::Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS))
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .pool_max_idle_per_host(100)
            .tcp_nodelay(true)
            .tcp_keepalive(std::time::Duration::from_secs(30))
            // Removed http2_prior_knowledge() as it's incompatible with HTTPS
            .http2_keep_alive_interval(std::time::Duration::from_secs(10))
            .http2_keep_alive_timeout(std::time::Duration::from_secs(30))
            .danger_accept_invalid_certs(true) // Plex self-signed certs
            .danger_accept_invalid_hostnames(true)
            .build()?;

        // Create LRU cache with configurable capacity
        let cache_capacity = NonZeroUsize::new(MEMORY_CACHE_SIZE).unwrap();
        let memory_cache = LruCache::new(cache_capacity);

        Ok(Self {
            client,
            cache_dir,
            memory_cache: Arc::new(RwLock::new(memory_cache)),
            cache_size: Arc::new(AtomicU64::new(0)),
            max_cache_size: MEMORY_CACHE_MAX_MB * 1024 * 1024,
            download_semaphore: Arc::new(Semaphore::new(CONCURRENT_DOWNLOADS)),
            active_downloads: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// Load an image with specified size - simplified for fast Plex thumbnails
    pub async fn load_image(&self, url: &str, _size: ImageSize) -> Result<gdk::Texture> {
        // Size is ignored since Plex handles resizing server-side
        let cache_key = url.to_string();

        // Check memory cache first - O(1) LRU lookup
        {
            let mut cache = self.memory_cache.write().await;
            if let Some(entry) = cache.get_mut(&cache_key) {
                entry.last_accessed = Instant::now();
                entry.access_count += 1;
                return Ok(entry.texture.clone());
            }
        }

        // Generate file cache key
        let file_cache_key = self.generate_cache_key(url, ImageSize::Small);
        let cache_path = self.get_cache_path(&file_cache_key, ImageSize::Small);

        // Check disk cache
        if cache_path.exists()
            && let Ok(texture) = self.load_from_file(&cache_path).await
        {
            // Add to memory cache in background
            let cache_key_clone = cache_key.clone();
            let texture_clone = texture.clone();
            let self_clone = self.clone();
            tokio::spawn(async move {
                self_clone
                    .add_to_memory_cache(cache_key_clone, texture_clone)
                    .await;
            });
            return Ok(texture);
        }

        // Download with request coalescing
        let bytes = self.download_with_coalescing(url).await?;

        // Save to disk cache in background
        let cache_path_clone = cache_path.clone();
        let bytes_clone = bytes.clone();
        tokio::spawn(async move {
            let _ = fs::write(&cache_path_clone, &bytes_clone).await;
        });

        // Create texture
        let texture = self.create_texture_from_bytes(&bytes)?;

        // Add to memory cache
        self.add_to_memory_cache(cache_key, texture.clone()).await;

        Ok(texture)
    }

    /// Download with request coalescing to prevent duplicate downloads - simplified
    async fn download_with_coalescing(&self, url: &str) -> Result<Vec<u8>> {
        // Just download directly - the semaphore will control concurrency
        self.download_image(url).await
    }

    /// Download image with improved error handling
    async fn download_image(&self, url: &str) -> Result<Vec<u8>> {
        let _permit = self.download_semaphore.acquire().await?;
        self.active_downloads.fetch_add(1, Ordering::SeqCst);

        let filename = url.split('/').next_back().unwrap_or("unknown");
        let mut retries = 2;
        let mut last_error = None;

        while retries > 0 {
            match self.download_attempt(url).await {
                Ok(bytes) => {
                    self.active_downloads.fetch_sub(1, Ordering::SeqCst);
                    let active = self.active_downloads.load(Ordering::Relaxed);
                    trace!(
                        "Downloaded {} - {}KB, {} active",
                        filename,
                        bytes.len() / 1024,
                        active
                    );
                    return Ok(bytes);
                }
                Err(e) => {
                    last_error = Some(e);
                    retries -= 1;
                    if retries > 0 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }
        }

        self.active_downloads.fetch_sub(1, Ordering::SeqCst);
        Err(last_error.unwrap_or_else(|| anyhow!("Download failed")))
    }

    /// Single download attempt
    async fn download_attempt(&self, url: &str) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(url)
            .header(
                "Accept",
                "image/webp,image/jpeg,image/png,image/*,*/*;q=0.8",
            )
            .send()
            .await
            .map_err(|e| anyhow!("Network request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!("HTTP error: {}", response.status()));
        }

        // No need to check size - Plex thumbnails are already small
        let bytes = response.bytes().await?;
        Ok(bytes.to_vec())
    }

    /// Load texture from file
    async fn load_from_file(&self, path: &Path) -> Result<gdk::Texture> {
        let bytes = fs::read(path).await?;
        self.create_texture_from_bytes(&bytes)
    }

    /// Create GDK texture from bytes
    fn create_texture_from_bytes(&self, bytes: &[u8]) -> Result<gdk::Texture> {
        let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from(bytes));
        let pixbuf = Pixbuf::from_stream(&stream, gio::Cancellable::NONE)?;
        let texture = gdk::Texture::for_pixbuf(&pixbuf);
        Ok(texture)
    }

    /// Add texture to LRU memory cache
    async fn add_to_memory_cache(&self, key: String, texture: gdk::Texture) {
        let size_bytes = texture.width() as usize * texture.height() as usize * 4;

        let mut cache = self.memory_cache.write().await;

        // LRU automatically evicts oldest entries when capacity is reached
        let old_entry = cache.put(
            key,
            CacheEntry {
                texture,
                size_bytes,
                last_accessed: Instant::now(),
                access_count: 1,
            },
        );

        // Update cache size tracking
        if let Some(old) = old_entry {
            self.cache_size
                .fetch_sub(old.size_bytes as u64, Ordering::Relaxed);
        }
        self.cache_size
            .fetch_add(size_bytes as u64, Ordering::Relaxed);
    }

    /// Generate a cache key from URL and size
    fn generate_cache_key(&self, url: &str, size: ImageSize) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        size.hash(&mut hasher);
        let hash = hasher.finish();

        // Use generic extension since we're storing original format
        format!("{:x}.img", hash)
    }

    /// Get cache path for a given key and size
    fn get_cache_path(&self, key: &str, size: ImageSize) -> PathBuf {
        let size_dir = match size {
            ImageSize::Small => "small",
            ImageSize::Medium => "medium",
            ImageSize::Large => "large",
            ImageSize::Original => "original",
        };

        self.cache_dir.join(size_dir).join(key)
    }

    /// Adaptive loading removed - Plex serves optimized sizes directly
    pub async fn load_adaptive(&self, url: &str, target_size: ImageSize) -> Result<gdk::Texture> {
        // Just load the image directly since Plex handles sizing
        self.load_image(url, target_size).await
    }

    /// Batch load multiple images efficiently - simplified version
    pub async fn batch_load(
        &self,
        requests: Vec<(String, ImageSize)>,
    ) -> Vec<Result<gdk::Texture>> {
        let start_time = Instant::now();
        let total_requests = requests.len();

        // Process each request individually but concurrently
        let mut tasks = Vec::new();

        for (url, size) in requests {
            let self_clone = self.clone();
            tasks.push(tokio::spawn(async move {
                self_clone.load_image(&url, size).await
            }));
        }

        // Wait for all tasks to complete
        let results = join_all(tasks).await;

        // Convert JoinHandle results to texture results
        let final_results: Vec<Result<gdk::Texture>> = results
            .into_iter()
            .map(|join_result| match join_result {
                Ok(texture_result) => texture_result,
                Err(e) => Err(anyhow!("Task failed: {}", e)),
            })
            .collect();

        final_results
    }

    /// Preload images based on predicted scroll
    pub async fn predictive_preload(
        &self,
        urls: Vec<(String, ImageSize)>,
        priority: PreloadPriority,
    ) {
        let delay = match priority {
            PreloadPriority::High => 0,
            PreloadPriority::Medium => 100,
            PreloadPriority::Low => 500,
        };

        if delay > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }

        // Use batch loading for better efficiency
        let _ = self.batch_load(urls).await;
    }

    /// Warm up cache with a batch of URLs
    pub async fn warm_cache(&self, urls: Vec<(String, ImageSize)>) {
        let start_time = Instant::now();
        let total = urls.len();

        // Check what's not already cached
        let mut to_load = Vec::new();
        for (url, size) in urls {
            let cache_key = format!("{}_{:?}", url, size);

            // Quick check without acquiring write lock
            let in_memory = {
                let cache = self.memory_cache.read().await;
                cache.peek(&cache_key).is_some()
            };

            if !in_memory {
                let file_cache_key = self.generate_cache_key(&url, size);
                let cache_path = self.get_cache_path(&file_cache_key, size);

                if !cache_path.exists() {
                    to_load.push((url, size));
                }
            }
        }

        if !to_load.is_empty() {
            let _ = self.batch_load(to_load).await;
        }
    }

    /// Batch check if images are cached
    pub async fn batch_check_cached(&self, urls: Vec<(String, ImageSize)>) -> Vec<bool> {
        let mut results = Vec::with_capacity(urls.len());

        for (url, size) in urls {
            let cache_key = format!("{}_{:?}", url, size);

            // Check memory cache
            let in_memory = {
                let cache = self.memory_cache.read().await;
                cache.peek(&cache_key).is_some()
            };

            if in_memory {
                results.push(true);
                continue;
            }

            // Check disk cache
            let file_cache_key = self.generate_cache_key(&url, size);
            let cache_path = self.get_cache_path(&file_cache_key, size);
            results.push(cache_path.exists());
        }

        results
    }

    /// Clear memory cache
    pub async fn clear_memory_cache(&self) {
        let mut cache = self.memory_cache.write().await;
        cache.clear();
        self.cache_size.store(0, Ordering::Relaxed);
        info!("Memory cache cleared");
    }

    /// Clear disk cache for a specific size
    pub async fn clear_disk_cache(&self, size: Option<ImageSize>) -> Result<()> {
        let dirs = if let Some(s) = size {
            vec![match s {
                ImageSize::Small => "small",
                ImageSize::Medium => "medium",
                ImageSize::Large => "large",
                ImageSize::Original => "original",
            }]
        } else {
            vec!["small", "medium", "large", "original"]
        };

        let mut total_deleted = 0;
        for dir in dirs {
            let dir_path = self.cache_dir.join(dir);
            if dir_path.exists() {
                let mut entries = tokio::fs::read_dir(&dir_path).await?;
                while let Some(entry) = entries.next_entry().await? {
                    if let Ok(metadata) = entry.metadata().await
                        && metadata.is_file()
                    {
                        tokio::fs::remove_file(entry.path()).await?;
                        total_deleted += 1;
                    }
                }
            }
        }

        info!("Deleted {} cached images", total_deleted);
        Ok(())
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> CacheStats {
        let cache = self.memory_cache.read().await;
        let memory_entries = cache.len();
        let memory_size = self.cache_size.load(Ordering::Relaxed);

        let mut disk_entries = 0;
        let mut disk_size = 0u64;

        for dir in &["small", "medium", "large", "original"] {
            let dir_path = self.cache_dir.join(dir);
            if let Ok(mut entries) = tokio::fs::read_dir(&dir_path).await {
                while let Some(entry) = entries.next_entry().await.ok().flatten() {
                    if let Ok(metadata) = entry.metadata().await
                        && metadata.is_file()
                    {
                        disk_entries += 1;
                        disk_size += metadata.len();
                    }
                }
            }
        }

        CacheStats {
            memory_entries,
            memory_size_bytes: memory_size,
            disk_entries,
            disk_size_bytes: disk_size,
            active_downloads: self.active_downloads.load(Ordering::Relaxed),
            pending_requests: 0, // No longer tracking pending requests separately
        }
    }
}

impl Clone for ImageLoader {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            cache_dir: self.cache_dir.clone(),
            memory_cache: self.memory_cache.clone(),
            cache_size: self.cache_size.clone(),
            max_cache_size: self.max_cache_size,
            download_semaphore: self.download_semaphore.clone(),
            active_downloads: self.active_downloads.clone(),
        }
    }
}

impl Default for ImageLoader {
    fn default() -> Self {
        Self::new().expect("Failed to create ImageLoader")
    }
}

/// Preload priority levels
#[derive(Debug, Clone, Copy)]
pub enum PreloadPriority {
    High,   // Load immediately
    Medium, // Load after 100ms
    Low,    // Load after 500ms
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub memory_entries: usize,
    pub memory_size_bytes: u64,
    pub disk_entries: usize,
    pub disk_size_bytes: u64,
    pub active_downloads: usize,
    pub pending_requests: usize,
}
