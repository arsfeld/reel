use anyhow::{Result, anyhow};
use futures::future::join_all;
use image::{ImageFormat, ImageReader};
use lru::LruCache;
use reqwest::Client;
use std::collections::VecDeque;
use std::io::Cursor;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use tokio::fs;
use tokio::sync::{Mutex, RwLock, Semaphore};
use tracing::{debug, info, trace};

#[cfg(feature = "gtk")]
use gdk4 as gdk;
#[cfg(feature = "gtk")]
use gtk4::gdk_pixbuf::Pixbuf;
#[cfg(feature = "gtk")]
use gtk4::{gio, glib, prelude::*};

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
    /// Get dimensions for poster/portrait images
    pub fn poster_dimensions(&self) -> (i32, i32) {
        match self {
            ImageSize::Small => (120, 180),
            ImageSize::Medium => (180, 270),
            ImageSize::Large => (360, 540),
            ImageSize::Original => (0, 0), // No specific size
        }
    }

    /// Get dimensions for poster/portrait images (compatibility alias)
    pub fn dimensions_for_poster(&self) -> (i32, i32) {
        self.poster_dimensions()
    }

    /// Get dimensions for landscape/backdrop images
    pub fn landscape_dimensions(&self) -> (i32, i32) {
        match self {
            ImageSize::Small => (120, 68),
            ImageSize::Medium => (320, 180),
            ImageSize::Large => (640, 360),
            ImageSize::Original => (0, 0), // No specific size
        }
    }

    /// Get dimensions for landscape/backdrop images (compatibility alias)
    pub fn dimensions_for_landscape(&self) -> (i32, i32) {
        self.landscape_dimensions()
    }

    /// Get the cache subdirectory for this size
    fn cache_subdir(&self) -> &str {
        match self {
            ImageSize::Small => "small",
            ImageSize::Medium => "medium",
            ImageSize::Large => "large",
            ImageSize::Original => "original",
        }
    }
}

/// Platform-agnostic image data
#[derive(Clone)]
pub struct ImageData {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: String, // "jpeg", "png", "webp", etc.
}

/// Manages image loading, caching, and scaling
pub struct ImageLoader {
    client: Client,
    cache_dir: PathBuf,
    memory_cache: Arc<RwLock<LruCache<(String, ImageSize), ImageData>>>,
    #[cfg(feature = "gtk")]
    texture_cache: Arc<RwLock<LruCache<(String, ImageSize), gdk::Texture>>>,
    download_semaphore: Arc<Semaphore>,
    download_queue: Arc<
        Mutex<
            VecDeque<(
                String,
                ImageSize,
                tokio::sync::oneshot::Sender<Result<ImageData>>,
            )>,
        >,
    >,
    is_scrolling: Arc<AtomicBool>,
    stats: Arc<ImageLoaderStats>,
}

impl Clone for ImageLoader {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            cache_dir: self.cache_dir.clone(),
            memory_cache: self.memory_cache.clone(),
            #[cfg(feature = "gtk")]
            texture_cache: self.texture_cache.clone(),
            download_semaphore: self.download_semaphore.clone(),
            download_queue: self.download_queue.clone(),
            is_scrolling: self.is_scrolling.clone(),
            stats: self.stats.clone(),
        }
    }
}

struct ImageLoaderStats {
    memory_hits: AtomicU64,
    disk_hits: AtomicU64,
    downloads: AtomicU64,
    memory_cache_size: AtomicUsize,
}

impl ImageLoader {
    /// Create a new ImageLoader with default settings
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| anyhow!("Could not determine cache directory"))?
            .join("reel")
            .join("images");

        // Create cache directory structure
        for subdir in &["small", "medium", "large", "original"] {
            let dir = cache_dir.join(subdir);
            std::fs::create_dir_all(&dir)?;
        }

        let loader = Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(5)) // Reduce timeout for faster failures
                .pool_max_idle_per_host(20) // Increase connection pool for Plex
                .build()?,
            cache_dir,
            memory_cache: Arc::new(RwLock::new(LruCache::new(NonZeroUsize::new(2000).unwrap()))), // Increase memory cache size
            #[cfg(feature = "gtk")]
            texture_cache: Arc::new(RwLock::new(LruCache::new(NonZeroUsize::new(1000).unwrap()))), // Increase texture cache size
            download_semaphore: Arc::new(Semaphore::new(10)), // Increase to 10 concurrent downloads for better throughput
            download_queue: Arc::new(Mutex::new(VecDeque::new())),
            is_scrolling: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(ImageLoaderStats {
                memory_hits: AtomicU64::new(0),
                disk_hits: AtomicU64::new(0),
                downloads: AtomicU64::new(0),
                memory_cache_size: AtomicUsize::new(0),
            }),
        };

        // Start background download processor
        loader.start_download_processor();

        Ok(loader)
    }

    /// Set scrolling state - when true, only cached images are returned
    pub fn set_scrolling(&self, scrolling: bool) {
        self.is_scrolling.store(scrolling, Ordering::Relaxed);
        if !scrolling {
            // Process queue when scrolling stops
            let queue = self.download_queue.clone();
            let loader = self.clone();
            tokio::spawn(async move {
                loader.process_download_queue().await;
            });
        }
    }

    /// Load an image from URL with specified size
    #[cfg(feature = "gtk")]
    pub async fn load_image(&self, url: &str, size: ImageSize) -> Result<gdk::Texture> {
        let cache_key = (url.to_string(), size);

        // Check texture cache first
        {
            let mut cache = self.texture_cache.write().await;
            if let Some(texture) = cache.get(&cache_key) {
                self.stats.memory_hits.fetch_add(1, Ordering::Relaxed);
                return Ok(texture.clone());
            }
        }

        // If scrolling, only return cached data
        if self.is_scrolling.load(Ordering::Relaxed) {
            // Try to get from memory/disk cache without downloading
            if let Ok(image_data) = self.load_cached_only(url, size).await {
                let texture = tokio::task::spawn_blocking(move || -> Result<gdk::Texture> {
                    let bytes = glib::Bytes::from(&image_data.data);
                    let stream = gio::MemoryInputStream::from_bytes(&bytes);
                    let pixbuf = Pixbuf::from_stream(&stream, gio::Cancellable::NONE)?;
                    Ok(gdk::Texture::for_pixbuf(&pixbuf))
                })
                .await??;

                // Cache the texture
                let mut cache = self.texture_cache.write().await;
                cache.put(cache_key, texture.clone());

                return Ok(texture);
            } else {
                // Return a placeholder or error when scrolling and not cached
                return Err(anyhow!("Image not cached and scrolling active"));
            }
        }

        // Normal loading path when not scrolling
        let image_data = self.load_image_data(url, size).await?;

        // Convert to Texture for GTK - do this in a blocking task to avoid UI freezes
        let texture = tokio::task::spawn_blocking(move || -> Result<gdk::Texture> {
            let bytes = glib::Bytes::from(&image_data.data);
            let stream = gio::MemoryInputStream::from_bytes(&bytes);
            let pixbuf = Pixbuf::from_stream(&stream, gio::Cancellable::NONE)?;
            let texture = gdk::Texture::for_pixbuf(&pixbuf);
            Ok(texture)
        })
        .await??;

        // Cache the texture
        let mut cache = self.texture_cache.write().await;
        cache.put(cache_key, texture.clone());

        Ok(texture)
    }

    /// Load image from cache only (no downloads)
    async fn load_cached_only(&self, url: &str, size: ImageSize) -> Result<ImageData> {
        let cache_key = (url.to_string(), size);

        // Check memory cache
        {
            let mut cache = self.memory_cache.write().await;
            if let Some(data) = cache.get(&cache_key) {
                return Ok(data.clone());
            }
        }

        // Check disk cache
        let cache_path = self.get_cache_path(url, size);
        if cache_path.exists() {
            if let Ok(data) = fs::read(&cache_path).await {
                let (width, height, format) =
                    parse_image_meta(&data).unwrap_or((0, 0, "unknown".to_string()));
                let image_data = ImageData {
                    data,
                    width,
                    height,
                    format,
                };

                // Store in memory cache
                let mut cache = self.memory_cache.write().await;
                cache.put(cache_key, image_data.clone());

                return Ok(image_data);
            }
        }

        Err(anyhow!("Image not in cache"))
    }

    /// Process download queue in background
    async fn process_download_queue(&self) {
        while let Some((url, size, sender)) = {
            let mut queue = self.download_queue.lock().await;
            queue.pop_front()
        } {
            // Skip if scrolling started again
            if self.is_scrolling.load(Ordering::Relaxed) {
                break;
            }

            let result = self.load_image_data(&url, size).await;
            let _ = sender.send(result);
        }
    }

    /// Start background download processor
    fn start_download_processor(&self) {
        let loader = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                if !loader.is_scrolling.load(Ordering::Relaxed) {
                    loader.process_download_queue().await;
                }
            }
        });
    }

    /// Load image data (platform-agnostic)
    pub async fn load_image_data(&self, url: &str, size: ImageSize) -> Result<ImageData> {
        let cache_key = (url.to_string(), size);

        // Check memory cache
        {
            let mut cache = self.memory_cache.write().await;
            if let Some(data) = cache.get(&cache_key) {
                self.stats.memory_hits.fetch_add(1, Ordering::Relaxed);
                trace!("Memory cache hit for {}", url);
                return Ok(data.clone());
            }
        }

        // Check disk cache
        let cache_path = self.get_cache_path(url, size);
        if cache_path.exists() {
            if let Ok(data) = fs::read(&cache_path).await {
                self.stats.disk_hits.fetch_add(1, Ordering::Relaxed);
                trace!("Disk cache hit for {}", url);
                // Try to parse basic metadata for width/height/format
                let (width, height, format) =
                    parse_image_meta(&data).unwrap_or((0, 0, "unknown".to_string()));
                let image_data = ImageData {
                    data,
                    width,
                    height,
                    format,
                };

                // Store in memory cache
                let mut cache = self.memory_cache.write().await;
                cache.put(cache_key, image_data.clone());

                return Ok(image_data);
            }
        }

        // Download the image
        let permit = self.download_semaphore.acquire().await?;
        let response = self.client.get(url).send().await?;
        let data = response.bytes().await?.to_vec();
        drop(permit);

        self.stats.downloads.fetch_add(1, Ordering::Relaxed);
        trace!("Downloaded image from {}", url);

        // Save to disk cache
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&cache_path, &data).await?;

        // Parse image to get dimensions and format (best-effort)
        let (width, height, format) =
            parse_image_meta(&data).unwrap_or((0, 0, "unknown".to_string()));
        let image_data = ImageData {
            data,
            width,
            height,
            format,
        };

        // Store in memory cache
        let mut cache = self.memory_cache.write().await;
        cache.put(cache_key, image_data.clone());

        Ok(image_data)
    }

    /// Check if images are cached without loading them
    pub async fn batch_check_cached(&self, urls: Vec<(String, ImageSize)>) -> Vec<bool> {
        let mut results = Vec::with_capacity(urls.len());

        for (url, size) in urls {
            let cache_path = self.get_cache_path(&url, size);
            results.push(cache_path.exists());
        }

        results
    }

    /// Batch download images
    pub async fn batch_download(&self, urls: Vec<(String, ImageSize)>) -> Vec<Result<()>> {
        let futures = urls.into_iter().map(|(url, size)| {
            let loader = self.clone();
            async move {
                loader.load_image_data(&url, size).await?;
                Ok(())
            }
        });

        join_all(futures).await
    }

    /// Warm the cache by downloading images (alias for batch_download)
    pub async fn warm_cache(&self, urls: Vec<(String, ImageSize)>) {
        let _ = self.batch_download(urls).await;
    }

    /// Clear all caches
    pub async fn clear_cache(&self) -> Result<()> {
        // Clear memory cache
        let mut cache = self.memory_cache.write().await;
        cache.clear();

        // Clear disk cache
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir).await?;
            fs::create_dir_all(&self.cache_dir).await?;

            // Recreate subdirectories
            for subdir in &["small", "medium", "large", "original"] {
                let dir = self.cache_dir.join(subdir);
                fs::create_dir_all(&dir).await?;
            }
        }

        info!("Cleared all image caches");
        Ok(())
    }

    /// Get statistics about cache usage
    pub fn get_stats(&self) -> String {
        format!(
            "ImageLoader Stats - Memory hits: {}, Disk hits: {}, Downloads: {}, Memory cache size: {}",
            self.stats.memory_hits.load(Ordering::Relaxed),
            self.stats.disk_hits.load(Ordering::Relaxed),
            self.stats.downloads.load(Ordering::Relaxed),
            self.stats.memory_cache_size.load(Ordering::Relaxed)
        )
    }

    /// Get the cache file path for a URL and size
    fn get_cache_path(&self, url: &str, size: ImageSize) -> PathBuf {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        let hash = hasher.finish();

        let filename = format!("{:x}", hash);
        self.cache_dir.join(size.cache_subdir()).join(filename)
    }
}

/// Best-effort parse to extract image width, height, and format name
fn parse_image_meta(bytes: &[u8]) -> Option<(u32, u32, String)> {
    // First try to guess format without full decode
    let guessed = image::guess_format(bytes).ok();
    let mut reader = ImageReader::new(Cursor::new(bytes));
    if let Some(fmt) = guessed {
        reader.set_format(fmt);
    }
    let reader = reader.with_guessed_format().ok()?;
    let format_str = format_to_string(reader.format()?);
    let dyn_img = reader.decode().ok()?;
    Some((dyn_img.width(), dyn_img.height(), format_str))
}

fn format_to_string(fmt: ImageFormat) -> String {
    match fmt {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpeg",
        ImageFormat::Gif => "gif",
        ImageFormat::WebP => "webp",
        ImageFormat::Pnm => "pnm",
        ImageFormat::Tiff => "tiff",
        ImageFormat::Tga => "tga",
        ImageFormat::Dds => "dds",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Ico => "ico",
        ImageFormat::Hdr => "hdr",
        ImageFormat::OpenExr => "exr",
        ImageFormat::Farbfeld => "farbfeld",
        ImageFormat::Avif => "avif",
        ImageFormat::Qoi => "qoi",
        _ => "unknown",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_size_dimensions() {
        assert_eq!(ImageSize::Small.poster_dimensions(), (120, 180));
        assert_eq!(ImageSize::Medium.poster_dimensions(), (180, 270));
        assert_eq!(ImageSize::Large.poster_dimensions(), (360, 540));

        assert_eq!(ImageSize::Small.landscape_dimensions(), (120, 68));
        assert_eq!(ImageSize::Medium.landscape_dimensions(), (320, 180));
        assert_eq!(ImageSize::Large.landscape_dimensions(), (640, 360));
    }

    #[tokio::test]
    async fn test_image_loader_creation() {
        let loader = ImageLoader::new();
        assert!(loader.is_ok());
    }
}
