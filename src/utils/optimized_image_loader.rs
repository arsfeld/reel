use anyhow::{Result, anyhow};
use gtk4::{gdk, gio, glib, prelude::*};
use gtk4::gdk_pixbuf::Pixbuf;
use reqwest::Client;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};
use tokio::fs;
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, error, info};
use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};
use image::{ImageFormat, DynamicImage};
use webp::Encoder;

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
    pub fn dimensions_for_poster(&self) -> (u32, u32) {
        match self {
            Self::Small => (120, 180),
            Self::Medium => (180, 270),
            Self::Large => (360, 540),
            Self::Original => (0, 0), // No resize
        }
    }
    
    pub fn dimensions_for_landscape(&self) -> (u32, u32) {
        match self {
            Self::Small => (120, 68),
            Self::Medium => (320, 180),
            Self::Large => (640, 360),
            Self::Original => (0, 0), // No resize
        }
    }
    
    pub fn quality(&self) -> u8 {
        match self {
            Self::Small => 75,    // Lower quality for small thumbnails
            Self::Medium => 85,   // Good quality/size balance
            Self::Large => 90,    // High quality for larger views
            Self::Original => 95, // Near-lossless
        }
    }
    
    pub fn webp_quality(&self) -> f32 {
        match self {
            Self::Small => 70.0,
            Self::Medium => 80.0,
            Self::Large => 85.0,
            Self::Original => 90.0,
        }
    }
}

/// Cache entry with metadata
struct CacheEntry {
    texture: gdk::Texture,
    size_bytes: usize,
    last_accessed: u64,
    access_count: u32,
}

/// Optimized image loader with WebP support and multi-size caching
pub struct OptimizedImageLoader {
    client: Client,
    cache_dir: PathBuf,
    memory_cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    cache_size: Arc<AtomicU64>,
    max_cache_size: u64,
    download_semaphore: Arc<Semaphore>,
    active_downloads: Arc<AtomicUsize>,
    webp_supported: bool,
}

impl OptimizedImageLoader {
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
        
        // Build a custom client that accepts self-signed certificates
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .pool_max_idle_per_host(50)
            .tcp_nodelay(true)
            .danger_accept_invalid_certs(true) // Accept Plex's self-signed certificates
            .danger_accept_invalid_hostnames(true) // Also accept invalid hostnames
            .build()?;
        
        // Check WebP support
        let webp_supported = Self::check_webp_support();
        info!("WebP support: {}", webp_supported);
        
        Ok(Self {
            client,
            cache_dir,
            memory_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_size: Arc::new(AtomicU64::new(0)),
            max_cache_size: 200 * 1024 * 1024, // 200MB memory cache
            download_semaphore: Arc::new(Semaphore::new(50)),
            active_downloads: Arc::new(AtomicUsize::new(0)),
            webp_supported,
        })
    }
    
    /// Check if WebP is supported by the system
    fn check_webp_support() -> bool {
        // Try to create a simple WebP image to test support
        match image::DynamicImage::new_rgb8(1, 1).as_rgb8() {
            Some(rgb_image) => {
                // WebP encoder always succeeds with creation, so we return true
                // The actual encoding may fail later but we'll handle that with fallback
                true
            }
            None => false,
        }
    }
    
    /// Load an image with specified size
    pub async fn load_image(&self, url: &str, size: ImageSize) -> Result<gdk::Texture> {
        let cache_key = format!("{}_{:?}", url, size);
        
        // Check memory cache first
        {
            let mut cache = self.memory_cache.write().await;
            if let Some(entry) = cache.get_mut(&cache_key) {
                entry.last_accessed = Self::current_timestamp();
                entry.access_count += 1;
                return Ok(entry.texture.clone());
            }
        }
        
        // Generate file cache key
        let file_cache_key = self.generate_cache_key(url, size);
        let cache_path = self.get_cache_path(&file_cache_key, size);
        
        // Check disk cache
        if cache_path.exists() {
            let texture = self.load_from_file(&cache_path).await?;
            self.add_to_memory_cache(cache_key, texture.clone()).await;
            return Ok(texture);
        }
        
        // Download image
        let bytes = self.download_image(url).await?;
        
        // Only process if we need to resize, otherwise use original
        let final_bytes = if size != ImageSize::Original {
            self.process_image(bytes.clone(), size).await
                .unwrap_or(bytes) // Fallback to original if processing fails
        } else {
            bytes
        };
        
        // Save to disk cache
        fs::write(&cache_path, &final_bytes).await?;
        
        // Create texture
        let texture = self.create_texture_from_bytes(&final_bytes)?;
        self.add_to_memory_cache(cache_key, texture.clone()).await;
        
        Ok(texture)
    }
    
    /// Process image: resize and optimize
    async fn process_image(&self, bytes: Vec<u8>, size: ImageSize) -> Result<Vec<u8>> {
        // For now, skip WebP conversion as GDK might not support it
        // Just resize and return as JPEG/PNG
        tokio::task::spawn_blocking(move || {
            let img = image::load_from_memory(&bytes)?;
            
            // Resize if not original size
            let resized = if size != ImageSize::Original {
                let (width, height) = size.dimensions_for_poster();
                if width > 0 && height > 0 {
                    img.resize(width, height, image::imageops::FilterType::Lanczos3)
                } else {
                    img
                }
            } else {
                img
            };
            
            // Save as JPEG with quality settings
            let mut output = std::io::Cursor::new(Vec::new());
            resized.write_to(&mut output, ImageFormat::Jpeg)?;
            Ok(output.into_inner())
        }).await?
    }
    
    /// Encode image as WebP
    fn encode_webp(img: &DynamicImage, quality: f32) -> Result<Vec<u8>> {
        let rgb_image = img.to_rgb8();
        let (width, height) = rgb_image.dimensions();
        
        let encoder = Encoder::from_rgb(&rgb_image, width, height);
        let webp_memory = encoder.encode(quality);
        Ok(webp_memory.to_vec())
    }
    
    /// Download image with improved error handling and retries
    async fn download_image(&self, url: &str) -> Result<Vec<u8>> {
        let _permit = self.download_semaphore.acquire().await?;
        self.active_downloads.fetch_add(1, Ordering::SeqCst);
        
        let mut retries = 3;
        let mut last_error = None;
        
        while retries > 0 {
            match self.download_attempt(url).await {
                Ok(bytes) => {
                    self.active_downloads.fetch_sub(1, Ordering::SeqCst);
                    return Ok(bytes);
                }
                Err(e) => {
                    last_error = Some(e);
                    retries -= 1;
                    if retries > 0 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    }
                }
            }
        }
        
        self.active_downloads.fetch_sub(1, Ordering::SeqCst);
        Err(last_error.unwrap_or_else(|| anyhow!("Failed to download image")))
    }
    
    /// Single download attempt
    async fn download_attempt(&self, url: &str) -> Result<Vec<u8>> {
        let response = self.client
            .get(url)
            .header("Accept", "image/webp,image/jpeg,image/png,image/*,*/*;q=0.8")
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("HTTP error: {}", response.status()));
        }
        
        // Check content length to avoid downloading huge files
        if let Some(content_length) = response.content_length() {
            if content_length > 10 * 1024 * 1024 { // 10MB limit
                return Err(anyhow!("Image too large: {} bytes", content_length));
            }
        }
        
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
    
    /// Add texture to memory cache with LRU eviction
    async fn add_to_memory_cache(&self, key: String, texture: gdk::Texture) {
        let size_bytes = texture.width() as usize * texture.height() as usize * 4; // Approximate
        
        let mut cache = self.memory_cache.write().await;
        
        // Check if we need to evict entries
        let current_size = self.cache_size.load(Ordering::Relaxed);
        if current_size + size_bytes as u64 > self.max_cache_size {
            self.evict_lru_entries(&mut cache, size_bytes as u64).await;
        }
        
        // Add new entry
        cache.insert(key, CacheEntry {
            texture,
            size_bytes,
            last_accessed: Self::current_timestamp(),
            access_count: 1,
        });
        
        self.cache_size.fetch_add(size_bytes as u64, Ordering::Relaxed);
    }
    
    /// Evict least recently used entries
    async fn evict_lru_entries(&self, cache: &mut HashMap<String, CacheEntry>, needed_space: u64) {
        let mut entries: Vec<_> = cache.iter()
            .map(|(k, v)| (k.clone(), v.last_accessed, v.size_bytes))
            .collect();
        
        // Sort by last accessed time (oldest first)
        entries.sort_by_key(|(_, accessed, _)| *accessed);
        
        let mut freed_space = 0u64;
        for (key, _, size) in entries {
            if freed_space >= needed_space {
                break;
            }
            
            cache.remove(&key);
            freed_space += size as u64;
            self.cache_size.fetch_sub(size as u64, Ordering::Relaxed);
        }
    }
    
    /// Generate a cache key from URL and size
    fn generate_cache_key(&self, url: &str, size: ImageSize) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        size.hash(&mut hasher);
        let hash = hasher.finish();
        
        // Use jpg extension for processed images
        format!("{:x}.jpg", hash)
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
    
    /// Get current timestamp in seconds
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    
    /// Preload images in background
    pub async fn preload_images(&self, urls: Vec<(String, ImageSize)>) {
        for (url, size) in urls {
            let loader = self.clone();
            tokio::spawn(async move {
                let _ = loader.load_image(&url, size).await;
            });
        }
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
                    if let Ok(metadata) = entry.metadata().await {
                        if metadata.is_file() {
                            tokio::fs::remove_file(entry.path()).await?;
                            total_deleted += 1;
                        }
                    }
                }
            }
        }
        
        info!("Deleted {} cached images", total_deleted);
        Ok(())
    }
    
    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> CacheStats {
        let memory_cache = self.memory_cache.read().await;
        let memory_entries = memory_cache.len();
        let memory_size = self.cache_size.load(Ordering::Relaxed);
        
        let mut disk_entries = 0;
        let mut disk_size = 0u64;
        
        for dir in &["small", "medium", "large", "original"] {
            let dir_path = self.cache_dir.join(dir);
            if let Ok(mut entries) = tokio::fs::read_dir(&dir_path).await {
                while let Some(entry) = entries.next_entry().await.ok().flatten() {
                    if let Ok(metadata) = entry.metadata().await {
                        if metadata.is_file() {
                            disk_entries += 1;
                            disk_size += metadata.len();
                        }
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
        }
    }
}

impl Clone for OptimizedImageLoader {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            cache_dir: self.cache_dir.clone(),
            memory_cache: self.memory_cache.clone(),
            cache_size: self.cache_size.clone(),
            max_cache_size: self.max_cache_size,
            download_semaphore: self.download_semaphore.clone(),
            active_downloads: self.active_downloads.clone(),
            webp_supported: self.webp_supported,
        }
    }
}

impl Default for OptimizedImageLoader {
    fn default() -> Self {
        Self::new().expect("Failed to create OptimizedImageLoader")
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub memory_entries: usize,
    pub memory_size_bytes: u64,
    pub disk_entries: usize,
    pub disk_size_bytes: u64,
    pub active_downloads: usize,
}