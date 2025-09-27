use lru::LruCache;
use relm4::prelude::*;
use relm4::{ComponentSender, Worker, WorkerHandle};
use std::collections::{BinaryHeap, HashMap};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use tracing::{debug, error, trace};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ImageSize {
    Thumbnail, // 180x270
    Card,      // 300x450
    Full,      // Original size
    Custom(u32, u32),
}

impl ImageSize {
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            ImageSize::Thumbnail => (180, 270),
            ImageSize::Card => (300, 450),
            ImageSize::Full => (0, 0), // No resize
            ImageSize::Custom(w, h) => (*w, *h),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImageRequest {
    pub id: String,
    pub url: String,
    pub size: ImageSize,
    pub priority: u8, // 0 = highest priority
}

impl PartialEq for ImageRequest {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for ImageRequest {}

impl PartialOrd for ImageRequest {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ImageRequest {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse order because BinaryHeap is a max-heap
        // Lower priority values should be processed first
        other.priority.cmp(&self.priority)
    }
}

#[derive(Debug, Clone)]
pub enum ImageLoaderInput {
    LoadImage(ImageRequest),
    CancelLoad {
        id: String,
    },
    ClearCache,
    SetCacheSize(usize),
    LoadCompleted {
        id: String,
    }, // Internal signal that a load completed
    StoreInCache {
        key: String,
        texture: gtk::gdk::Texture,
    }, // Store loaded texture in cache
}

#[derive(Debug, Clone)]
pub enum ImageLoaderOutput {
    ImageLoaded {
        id: String,
        texture: gtk::gdk::Texture,
        size: ImageSize,
    },
    LoadFailed {
        id: String,
        error: String,
    },
    CacheCleared,
}

pub struct ImageLoader {
    cache_dir: PathBuf,
    memory_cache: LruCache<String, gtk::gdk::Texture>,
    pending_loads: HashMap<String, ImageRequest>,
    active_loads: HashMap<String, relm4::JoinHandle<()>>,
    priority_queue: BinaryHeap<ImageRequest>,
    max_concurrent_loads: usize,
}

impl ImageLoader {
    fn new() -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("reel")
            .join("images");

        std::fs::create_dir_all(&cache_dir).ok();

        Self {
            cache_dir,
            memory_cache: LruCache::new(NonZeroUsize::new(200).unwrap()), // Increased cache size
            pending_loads: HashMap::new(),
            active_loads: HashMap::new(),
            priority_queue: BinaryHeap::new(),
            max_concurrent_loads: 6, // Limit concurrent network requests
        }
    }

    fn get_cache_path(&self, url: &str, size: &ImageSize) -> PathBuf {
        let url_hash = format!("{:x}", md5::compute(url));
        let size_suffix = match size {
            ImageSize::Thumbnail => "thumb",
            ImageSize::Card => "card",
            ImageSize::Full => "full",
            ImageSize::Custom(w, h) => &format!("{}x{}", w, h),
        };

        self.cache_dir
            .join(format!("{}_{}.jpg", url_hash, size_suffix))
    }

    fn get_cache_key(url: &str, size: &ImageSize) -> String {
        format!("{}_{:?}", url, size)
    }

    async fn load_image_async(
        request: ImageRequest,
        cache_path: PathBuf,
    ) -> Result<gtk::gdk::Texture, String> {
        // Check if file exists in cache
        if cache_path.exists() {
            return load_texture_from_file(&cache_path)
                .map_err(|e| format!("Failed to load cached image: {}", e));
        }

        // Download the image
        debug!("Downloading image: {} from {}", request.id, request.url);
        let response = reqwest::get(&request.url)
            .await
            .map_err(|e| format!("Failed to download: {}", e))?;

        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read bytes: {}", e))?;

        // Process image based on size
        let processed_bytes = if request.size != ImageSize::Full {
            let (width, height) = request.size.dimensions();
            resize_image(&bytes, width, height).map_err(|e| format!("Failed to resize: {}", e))?
        } else {
            bytes.to_vec()
        };

        // Save to cache
        if let Err(e) = std::fs::write(&cache_path, &processed_bytes) {
            error!("Failed to cache image: {}", e);
        }

        // Create texture from bytes
        create_texture_from_bytes(&processed_bytes)
            .map_err(|e| format!("Failed to create texture: {}", e))
    }
}

fn load_texture_from_file(path: &PathBuf) -> Result<gtk::gdk::Texture, String> {
    gtk::gdk::Texture::from_file(&gtk::gio::File::for_path(path)).map_err(|e| e.to_string())
}

fn create_texture_from_bytes(bytes: &[u8]) -> Result<gtk::gdk::Texture, String> {
    let bytes = gtk::glib::Bytes::from(bytes);
    gtk::gdk::Texture::from_bytes(&bytes).map_err(|e| e.to_string())
}

fn resize_image(bytes: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    use image::ImageFormat;

    let img =
        image::load_from_memory(bytes).map_err(|e| format!("Failed to decode image: {}", e))?;

    let resized = if width > 0 && height > 0 {
        img.thumbnail(width, height)
    } else {
        img
    };

    let mut output = Vec::new();
    resized
        .write_to(&mut std::io::Cursor::new(&mut output), ImageFormat::Jpeg)
        .map_err(|e| format!("Failed to encode image: {}", e))?;

    Ok(output)
}

impl Worker for ImageLoader {
    type Init = ();
    type Input = ImageLoaderInput;
    type Output = ImageLoaderOutput;

    fn init(_init: Self::Init, _sender: ComponentSender<Self>) -> Self {
        Self::new()
    }

    // Note: shutdown() is no longer part of the Worker trait in the current Relm4 version
    // Resources will be cleaned up when the component is dropped

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            ImageLoaderInput::LoadImage(request) => {
                let cache_key = Self::get_cache_key(&request.url, &request.size);

                // Check memory cache first
                if let Some(texture) = self.memory_cache.get(&cache_key) {
                    trace!("Image {} found in memory cache", request.id);
                    // Ignore send errors during shutdown
                    let _ = sender.output(ImageLoaderOutput::ImageLoaded {
                        id: request.id,
                        texture: texture.clone(),
                        size: request.size,
                    });
                    return;
                }

                // Check if already pending or active
                if self.pending_loads.contains_key(&request.id)
                    || self.active_loads.contains_key(&request.id)
                {
                    trace!(
                        "Image {} already being loaded, updating priority if needed",
                        request.id
                    );
                    // Update priority if this request has higher priority
                    if let Some(existing) = self.pending_loads.get_mut(&request.id)
                        && request.priority < existing.priority
                    {
                        existing.priority = request.priority;
                        // Re-sort the queue
                        self.rebuild_priority_queue();
                    }
                    return;
                }

                // Add to priority queue
                trace!(
                    "Adding image {} to priority queue with priority {}",
                    request.id, request.priority
                );
                self.pending_loads
                    .insert(request.id.clone(), request.clone());
                self.priority_queue.push(request);

                // Process queue if we have capacity
                self.process_priority_queue(sender.clone());
            }

            ImageLoaderInput::CancelLoad { id } => {
                trace!("Cancelling load for image {}", id);

                // Cancel active load if exists
                if let Some(handle) = self.active_loads.remove(&id) {
                    handle.abort();
                    debug!("Cancelled active load for {}", id);

                    // Process next item in queue since we freed up a slot
                    self.process_priority_queue(sender.clone());
                }

                // Remove from pending
                if self.pending_loads.remove(&id).is_some() {
                    // Rebuild queue to remove the cancelled item
                    self.rebuild_priority_queue();
                }
            }

            ImageLoaderInput::ClearCache => {
                self.memory_cache.clear();
                self.pending_loads.clear();
                self.priority_queue.clear();

                // Cancel all active loads
                for (_, handle) in self.active_loads.drain() {
                    handle.abort();
                }

                // Clear disk cache
                if let Err(e) = std::fs::remove_dir_all(&self.cache_dir) {
                    error!("Failed to clear cache directory: {}", e);
                }
                std::fs::create_dir_all(&self.cache_dir).ok();

                // Only send output if channel is still open
                let _ = sender.output(ImageLoaderOutput::CacheCleared);
            }

            ImageLoaderInput::SetCacheSize(size) => {
                if let Some(non_zero) = NonZeroUsize::new(size) {
                    self.memory_cache.resize(non_zero);
                }
            }

            ImageLoaderInput::LoadCompleted { id } => {
                // Remove from active loads and pending loads
                self.active_loads.remove(&id);
                self.pending_loads.remove(&id);

                // Process next item in queue since we freed up a slot
                self.process_priority_queue(sender.clone());
            }

            ImageLoaderInput::StoreInCache { key, texture } => {
                // Store the loaded texture in memory cache
                self.memory_cache.put(key, texture);
            }
        }
    }
}

impl ImageLoader {
    fn process_priority_queue(&mut self, sender: ComponentSender<Self>) {
        // Process items from priority queue while we have capacity
        while self.active_loads.len() < self.max_concurrent_loads {
            // Get next highest priority item
            let request = loop {
                if let Some(req) = self.priority_queue.pop() {
                    // Check if this request is still pending (might have been cancelled)
                    if self.pending_loads.contains_key(&req.id) {
                        break Some(req);
                    }
                    // Skip cancelled items
                } else {
                    break None;
                }
            };

            if let Some(request) = request {
                self.start_image_load(request, sender.clone());
            } else {
                break; // No more items in queue
            }
        }
    }

    fn start_image_load(&mut self, request: ImageRequest, sender: ComponentSender<Self>) {
        let cache_key = Self::get_cache_key(&request.url, &request.size);
        let cache_path = self.get_cache_path(&request.url, &request.size);
        let req_clone = request.clone();
        let sender_clone = sender.clone();
        let cache_key_clone = cache_key.clone();
        let id = request.id.clone();

        let handle = relm4::spawn(async move {
            match Self::load_image_async(req_clone.clone(), cache_path).await {
                Ok(texture) => {
                    // Store in cache - ignore errors if channel is closed
                    let _ = sender_clone
                        .input_sender()
                        .send(ImageLoaderInput::StoreInCache {
                            key: cache_key_clone,
                            texture: texture.clone(),
                        });

                    let _ = sender_clone.output(ImageLoaderOutput::ImageLoaded {
                        id: req_clone.id.clone(),
                        texture,
                        size: req_clone.size,
                    });
                }
                Err(error) => {
                    let _ = sender_clone.output(ImageLoaderOutput::LoadFailed {
                        id: req_clone.id.clone(),
                        error,
                    });
                }
            }

            // Notify that this load is complete so we can process more - ignore if channel closed
            let _ = sender_clone
                .input_sender()
                .send(ImageLoaderInput::LoadCompleted { id: req_clone.id });
        });

        self.active_loads.insert(id, handle);
        // Note: We keep it in pending_loads until it completes or is cancelled
    }

    fn rebuild_priority_queue(&mut self) {
        // Rebuild the priority queue from pending loads
        self.priority_queue.clear();
        for request in self.pending_loads.values() {
            self.priority_queue.push(request.clone());
        }
    }
}

// Helper function to create an image loader instance
pub fn get_image_loader() -> WorkerHandle<ImageLoader> {
    ImageLoader::builder().detach_worker(())
}
