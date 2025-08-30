use dispatch::Queue;
use lru::LruCache;
use objc2::{AnyThread, rc::Retained};
use objc2_app_kit::NSImage;
use objc2_foundation::{NSData, NSString};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error};

const MEMORY_CACHE_SIZE: usize = 500;
const DISK_CACHE_DIR: &str = "image_cache";

pub struct ImageCache {
    memory_cache: Arc<RwLock<LruCache<String, Retained<NSImage>>>>,
    pending_requests: Arc<RwLock<HashMap<String, Vec<ImageLoadCallback>>>>,
    disk_cache_path: std::path::PathBuf,
}

type ImageLoadCallback = Arc<dyn Fn(Option<Retained<NSImage>>) + Send + Sync + 'static>;

impl ImageCache {
    pub fn new() -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            .join("reel")
            .join(DISK_CACHE_DIR);

        // Create cache directory if it doesn't exist
        std::fs::create_dir_all(&cache_dir).ok();

        Self {
            memory_cache: Arc::new(RwLock::new(LruCache::new(
                std::num::NonZeroUsize::new(MEMORY_CACHE_SIZE).unwrap(),
            ))),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            disk_cache_path: cache_dir,
        }
    }

    pub async fn get_image(&self, url: &str, callback: ImageLoadCallback) {
        // Check memory cache first
        {
            let mut cache = self.memory_cache.write().await;
            if let Some(image) = cache.get(url) {
                debug!("Image cache hit (memory): {}", url);
                callback(Some(image.clone()));
                return;
            }
        }

        // Check if request is already pending
        {
            let mut pending = self.pending_requests.write().await;
            if let Some(callbacks) = pending.get_mut(url) {
                debug!("Image request already pending: {}", url);
                callbacks.push(callback);
                return;
            }
            pending.insert(url.to_string(), vec![callback]);
        }

        // Check disk cache
        let cache_path = self.get_cache_path(url);
        if cache_path.exists() {
            if let Ok(data) = std::fs::read(&cache_path) {
                if let Some(image) = Self::create_image_from_data(data) {
                    debug!("Image cache hit (disk): {}", url);
                    self.deliver_image(url, Some(image)).await;
                    return;
                }
            }
        }

        // TODO: Simplified network loading - removed complex threading due to NSImage thread safety
        // For now, just deliver None to avoid Send issues with NSImage
        self.store_and_deliver(url, None).await;
    }

    async fn load_image_data_from_network(&self, url: &str) -> anyhow::Result<Vec<u8>> {
        debug!("Loading image data from network: {}", url);

        let response = reqwest::get(url).await?;
        let bytes = response.bytes().await?;
        let data = bytes.to_vec();

        // Save to disk cache
        let cache_path = self.get_cache_path(url);
        tokio::fs::write(&cache_path, &data).await.ok();

        Ok(data)
    }

    async fn store_and_deliver(&self, url: &str, image: Option<Retained<NSImage>>) {
        // Store in memory cache if we have an image
        if let Some(ref img) = image {
            let mut cache = self.memory_cache.write().await;
            cache.put(url.to_string(), img.clone());
        }

        // Deliver to waiting callbacks
        self.deliver_image(url, image).await;
    }

    async fn deliver_image(&self, url: &str, image: Option<Retained<NSImage>>) {
        let callbacks = {
            let mut pending = self.pending_requests.write().await;
            pending.remove(url)
        };

        if let Some(callbacks) = callbacks {
            for callback in callbacks {
                if let Some(ref img) = image {
                    callback(Some(img.clone()));
                } else {
                    callback(None);
                }
            }
        }
    }

    fn create_image_from_data(data: Vec<u8>) -> Option<Retained<NSImage>> {
        unsafe {
            let ns_data = NSData::from_vec(data);
            NSImage::initWithData(NSImage::alloc(), &ns_data)
        }
    }

    fn get_cache_path(&self, url: &str) -> std::path::PathBuf {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        let hash = hasher.finish();

        self.disk_cache_path.join(format!("{:x}.jpg", hash))
    }

    pub async fn clear_memory_cache(&self) {
        let mut cache = self.memory_cache.write().await;
        cache.clear();
        debug!("Cleared memory image cache");
    }

    pub async fn clear_disk_cache(&self) -> anyhow::Result<()> {
        tokio::fs::remove_dir_all(&self.disk_cache_path).await?;
        tokio::fs::create_dir_all(&self.disk_cache_path).await?;
        debug!("Cleared disk image cache");
        Ok(())
    }
}

impl Clone for ImageCache {
    fn clone(&self) -> Self {
        Self {
            memory_cache: self.memory_cache.clone(),
            pending_requests: self.pending_requests.clone(),
            disk_cache_path: self.disk_cache_path.clone(),
        }
    }
}

// TODO: Simplified image cache - complex threading removed due to NSImage thread safety issues
pub fn get_image_cache() -> ImageCache {
    ImageCache::new()
}
