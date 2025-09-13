use anyhow::Result;
use relm4::{ComponentSender, Worker};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, error};

/// Messages that can be sent to the ImageWorker
#[derive(Debug, Clone)]
pub enum ImageWorkerInput {
    /// Load an image from URL
    LoadImage {
        id: String,
        url: String,
        cache_path: Option<PathBuf>,
    },
    /// Generate a thumbnail
    GenerateThumbnail {
        id: String,
        source_path: PathBuf,
        size: (u32, u32),
    },
    /// Clear image cache
    ClearCache,
}

/// Messages sent from the ImageWorker
#[derive(Debug, Clone)]
pub enum ImageWorkerOutput {
    /// Image loaded successfully
    ImageLoaded {
        id: String,
        data: Vec<u8>,
        path: Option<PathBuf>,
    },
    /// Thumbnail generated
    ThumbnailGenerated { id: String, path: PathBuf },
    /// Image loading failed
    LoadFailed { id: String, error: String },
    /// Cache cleared
    CacheCleared,
}

/// Worker for async image operations
pub struct ImageWorker {
    cache_dir: PathBuf,
    pending_loads: HashMap<String, String>, // id -> url
}

impl ImageWorker {
    pub fn new() -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("reel")
            .join("images");

        // Ensure cache directory exists
        let _ = std::fs::create_dir_all(&cache_dir);

        Self {
            cache_dir,
            pending_loads: HashMap::new(),
        }
    }

    async fn load_image(
        &mut self,
        id: String,
        url: String,
        cache_path: Option<PathBuf>,
        sender: ComponentSender<Self>,
    ) {
        debug!("Loading image: {} from {}", id, url);

        // Check if we have a cache path and the file exists
        let final_path = cache_path.unwrap_or_else(|| {
            let url_hash = format!("{:x}", md5::compute(&url));
            self.cache_dir.join(format!("{}.jpg", url_hash))
        });

        // Check if cached
        if final_path.exists() {
            match std::fs::read(&final_path) {
                Ok(data) => {
                    debug!("Loaded image from cache: {}", id);
                    let _ = sender.output(ImageWorkerOutput::ImageLoaded {
                        id,
                        data,
                        path: Some(final_path),
                    });
                    return;
                }
                Err(e) => {
                    debug!("Failed to read cached image: {}", e);
                }
            }
        }

        // Download the image
        match Self::download_image(&url).await {
            Ok(data) => {
                // Save to cache
                if let Err(e) = std::fs::write(&final_path, &data) {
                    error!("Failed to cache image: {}", e);
                }

                let _ = sender.output(ImageWorkerOutput::ImageLoaded {
                    id,
                    data,
                    path: Some(final_path),
                });
            }
            Err(e) => {
                error!("Failed to download image {}: {}", url, e);
                let _ = sender.output(ImageWorkerOutput::LoadFailed {
                    id,
                    error: e.to_string(),
                });
            }
        }
    }

    async fn download_image(url: &str) -> Result<Vec<u8>> {
        let response = reqwest::get(url).await?;
        let bytes = response.bytes().await?;
        Ok(bytes.to_vec())
    }

    async fn generate_thumbnail(
        &self,
        id: String,
        source_path: PathBuf,
        size: (u32, u32),
        sender: ComponentSender<Self>,
    ) {
        debug!("Generating thumbnail for: {}", id);

        let thumb_dir = self.cache_dir.join("thumbnails");
        let _ = std::fs::create_dir_all(&thumb_dir);

        let thumb_path = thumb_dir.join(format!(
            "{}_{}_x{}.jpg",
            source_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy(),
            size.0,
            size.1
        ));

        // Check if thumbnail already exists
        if thumb_path.exists() {
            let _ = sender.output(ImageWorkerOutput::ThumbnailGenerated {
                id,
                path: thumb_path,
            });
            return;
        }

        // Generate thumbnail using image crate
        match image::open(&source_path) {
            Ok(img) => {
                let thumbnail = img.thumbnail(size.0, size.1);
                if let Err(e) = thumbnail.save(&thumb_path) {
                    error!("Failed to save thumbnail: {}", e);
                    let _ = sender.output(ImageWorkerOutput::LoadFailed {
                        id,
                        error: e.to_string(),
                    });
                } else {
                    let _ = sender.output(ImageWorkerOutput::ThumbnailGenerated {
                        id,
                        path: thumb_path,
                    });
                }
            }
            Err(e) => {
                error!("Failed to open image for thumbnail: {}", e);
                let _ = sender.output(ImageWorkerOutput::LoadFailed {
                    id,
                    error: e.to_string(),
                });
            }
        }
    }

    fn clear_cache(&mut self, sender: ComponentSender<Self>) {
        debug!("Clearing image cache");

        if let Err(e) = std::fs::remove_dir_all(&self.cache_dir) {
            error!("Failed to clear cache: {}", e);
        }

        // Recreate cache directory
        let _ = std::fs::create_dir_all(&self.cache_dir);

        self.pending_loads.clear();
        let _ = sender.output(ImageWorkerOutput::CacheCleared);
    }
}

impl Worker for ImageWorker {
    type Init = ();
    type Input = ImageWorkerInput;
    type Output = ImageWorkerOutput;

    fn init(_: Self::Init, _sender: ComponentSender<Self>) -> Self {
        Self::new()
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            ImageWorkerInput::LoadImage {
                id,
                url,
                cache_path,
            } => {
                self.pending_loads.insert(id.clone(), url.clone());

                let mut worker = self.clone();
                relm4::spawn(async move {
                    worker.load_image(id, url, cache_path, sender).await;
                });
            }
            ImageWorkerInput::GenerateThumbnail {
                id,
                source_path,
                size,
            } => {
                let worker = self.clone();
                relm4::spawn(async move {
                    worker
                        .generate_thumbnail(id, source_path, size, sender)
                        .await;
                });
            }
            ImageWorkerInput::ClearCache => {
                self.clear_cache(sender);
            }
        }
    }
}

impl Clone for ImageWorker {
    fn clone(&self) -> Self {
        Self {
            cache_dir: self.cache_dir.clone(),
            pending_loads: self.pending_loads.clone(),
        }
    }
}
