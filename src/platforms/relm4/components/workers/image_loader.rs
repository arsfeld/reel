use gtk::prelude::*;
use lru::LruCache;
use relm4::prelude::*;
use relm4::{ComponentSender, Worker, WorkerHandle};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use tracing::{debug, error};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ImageSize {
    Thumbnail, // 150x225
    Card,      // 300x450
    Full,      // Original size
    Custom(u32, u32),
}

impl ImageSize {
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            ImageSize::Thumbnail => (150, 225),
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

#[derive(Debug, Clone)]
pub enum ImageLoaderInput {
    LoadImage(ImageRequest),
    CancelLoad { id: String },
    ClearCache,
    SetCacheSize(usize),
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
            memory_cache: LruCache::new(NonZeroUsize::new(100).unwrap()),
            pending_loads: HashMap::new(),
            active_loads: HashMap::new(),
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
            debug!("Loading image from cache: {}", request.id);
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

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            ImageLoaderInput::LoadImage(request) => {
                let cache_key = Self::get_cache_key(&request.url, &request.size);

                // Check memory cache first
                if let Some(texture) = self.memory_cache.get(&cache_key) {
                    sender
                        .output(ImageLoaderOutput::ImageLoaded {
                            id: request.id,
                            texture: texture.clone(),
                            size: request.size,
                        })
                        .ok();
                    return;
                }

                // Cancel any existing load for this ID
                if let Some(handle) = self.active_loads.remove(&request.id) {
                    handle.abort();
                }

                // Start async load
                let cache_path = self.get_cache_path(&request.url, &request.size);
                let req_clone = request.clone();
                let sender_clone = sender.clone();
                let cache_key_clone = cache_key.clone();
                let id = request.id.clone();

                let handle = relm4::spawn(async move {
                    match Self::load_image_async(req_clone.clone(), cache_path).await {
                        Ok(texture) => {
                            sender_clone
                                .output(ImageLoaderOutput::ImageLoaded {
                                    id: req_clone.id,
                                    texture,
                                    size: req_clone.size,
                                })
                                .ok();
                        }
                        Err(error) => {
                            sender_clone
                                .output(ImageLoaderOutput::LoadFailed {
                                    id: req_clone.id,
                                    error,
                                })
                                .ok();
                        }
                    }
                });

                self.active_loads.insert(id.clone(), handle);
                self.pending_loads.insert(id, request);
            }

            ImageLoaderInput::CancelLoad { id } => {
                if let Some(handle) = self.active_loads.remove(&id) {
                    handle.abort();
                }
                self.pending_loads.remove(&id);
            }

            ImageLoaderInput::ClearCache => {
                self.memory_cache.clear();
                self.pending_loads.clear();

                // Cancel all active loads
                for (_, handle) in self.active_loads.drain() {
                    handle.abort();
                }

                // Clear disk cache
                if let Err(e) = std::fs::remove_dir_all(&self.cache_dir) {
                    error!("Failed to clear cache directory: {}", e);
                }
                std::fs::create_dir_all(&self.cache_dir).ok();

                sender.output(ImageLoaderOutput::CacheCleared).ok();
            }

            ImageLoaderInput::SetCacheSize(size) => {
                if let Some(non_zero) = NonZeroUsize::new(size) {
                    self.memory_cache.resize(non_zero);
                }
            }
        }
    }
}

// Helper function to create an image loader instance
pub fn get_image_loader() -> WorkerHandle<ImageLoader> {
    ImageLoader::builder().detach_worker(())
}
