// DEPRECATED: This module is kept for reference only.
// Use OptimizedImageLoader instead for better performance with:
// - WebP support
// - Multiple size variants
// - LRU cache eviction
// - Better error handling

use anyhow::{Result, anyhow};
use gtk4::{gdk, gio, glib};
use gtk4::gdk_pixbuf::Pixbuf;
use reqwest::Client;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::fs;
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, error, info};
use std::collections::HashMap;

/// Image loader with caching support and download throttling
/// DEPRECATED: Use OptimizedImageLoader instead
pub struct ImageLoader {
    client: Client,
    cache_dir: PathBuf,
    memory_cache: Arc<RwLock<HashMap<String, gdk::Texture>>>,
    download_semaphore: Arc<Semaphore>,
    active_downloads: Arc<AtomicUsize>,
}

impl ImageLoader {
    pub fn new() -> Result<Self> {
        // Create cache directory
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| anyhow!("Failed to get cache directory"))?
            .join("reel")
            .join("images");
        
        // Ensure cache directory exists
        std::fs::create_dir_all(&cache_dir)?;
        
        // Build a custom client that accepts self-signed certificates
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .pool_max_idle_per_host(50)
            .tcp_nodelay(true)
            .danger_accept_invalid_certs(true) // Accept Plex's self-signed certificates
            .danger_accept_invalid_hostnames(true) // Also accept invalid hostnames
            .build()?;
        
        Ok(Self {
            client,
            cache_dir,
            memory_cache: Arc::new(RwLock::new(HashMap::new())),
            download_semaphore: Arc::new(Semaphore::new(50)), // Allow many concurrent downloads
            active_downloads: Arc::new(AtomicUsize::new(0)),
        })
    }
    
    /// Load an image from URL or cache
    pub async fn load_image(&self, url: &str) -> Result<gdk::Texture> {
        // Check memory cache first
        {
            let cache = self.memory_cache.read().await;
            if let Some(texture) = cache.get(url) {
                return Ok(texture.clone());
            }
        }
        
        // Generate cache key from URL
        let cache_key = self.generate_cache_key(url);
        let cache_path = self.cache_dir.join(&cache_key);
        
        // Check disk cache
        if cache_path.exists() {
            let texture = self.load_from_file(&cache_path).await?;
            
            // Store in memory cache
            {
                let mut cache = self.memory_cache.write().await;
                cache.insert(url.to_string(), texture.clone());
            }
            
            return Ok(texture);
        }
        
        // Download from URL
        let bytes = self.download_image(url).await?;
        
        // Save to disk cache
        fs::write(&cache_path, &bytes).await?;
        
        // Create texture from bytes
        let texture = self.create_texture_from_bytes(&bytes)?;
        
        // Store in memory cache
        {
            let mut cache = self.memory_cache.write().await;
            cache.insert(url.to_string(), texture.clone());
        }
        
        Ok(texture)
    }
    
    /// Download image from URL with throttling
    async fn download_image(&self, url: &str) -> Result<Vec<u8>> {
        // Acquire semaphore permit to limit concurrent downloads
        let _permit = self.download_semaphore.acquire().await?;
        
        // Track active downloads (remove debug logging for performance)
        self.active_downloads.fetch_add(1, Ordering::SeqCst);
        
        let result = async {
            let response = self.client
                .get(url)
                .send()
                .await
                .map_err(|e| {
                    debug!("Failed to fetch {}: {}", url, e);
                    anyhow!("Network error: {}", e)
                })?;
            
            if !response.status().is_success() {
                return Err(anyhow!("HTTP {} for {}", response.status(), url));
            }
            
            let bytes = response.bytes().await?;
            Ok(bytes.to_vec())
        }.await;
        
        // Decrement active downloads counter
        self.active_downloads.fetch_sub(1, Ordering::SeqCst);
        
        result
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
    
    /// Generate a cache key from URL
    fn generate_cache_key(&self, url: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        let hash = hasher.finish();
        
        // Extract file extension if present
        let extension = url.rsplit('.')
            .next()
            .and_then(|ext| {
                let ext = ext.split('?').next()?; // Remove query params
                if ext.len() < 10 && ext.chars().all(|c| c.is_alphanumeric()) {
                    Some(ext)
                } else {
                    None
                }
            })
            .unwrap_or("jpg");
        
        format!("{:x}.{}", hash, extension)
    }
    
    /// Clear memory cache
    pub async fn clear_memory_cache(&self) {
        let mut cache = self.memory_cache.write().await;
        cache.clear();
        info!("Memory cache cleared");
    }
    
    /// Clear disk cache
    pub async fn clear_disk_cache(&self) -> Result<()> {
        let mut entries = tokio::fs::read_dir(&self.cache_dir).await?;
        let mut count = 0;
        
        while let Some(entry) = entries.next_entry().await? {
            if let Ok(metadata) = entry.metadata().await {
                if metadata.is_file() {
                    tokio::fs::remove_file(entry.path()).await?;
                    count += 1;
                }
            }
        }
        
        info!("Deleted {} cached images", count);
        Ok(())
    }
    
    /// Get cache size in bytes
    pub async fn get_cache_size(&self) -> Result<u64> {
        let mut total_size = 0u64;
        let mut entries = tokio::fs::read_dir(&self.cache_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            if let Ok(metadata) = entry.metadata().await {
                if metadata.is_file() {
                    total_size += metadata.len();
                }
            }
        }
        
        Ok(total_size)
    }
}

impl Default for ImageLoader {
    fn default() -> Self {
        Self::new().expect("Failed to create ImageLoader")
    }
}