use crate::core::viewmodels::{LibraryViewModel, ViewModel};
use crate::models::MediaItem;
use anyhow::Result;
use chrono::Datelike;
use slint::{SharedString, VecModel};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, info};

// Include the compiled Slint UI to get access to generated types
slint::include_modules!();

#[derive(Clone)]
pub struct MediaItemData {
    pub id: SharedString,
    pub title: SharedString,
    pub poster_url: SharedString,
    pub year: i32,
    pub rating: f32,
    pub duration: i32,
}

impl From<MediaItem> for MediaItemData {
    fn from(item: MediaItem) -> Self {
        match item {
            MediaItem::Movie(movie) => Self {
                id: movie.id.into(),
                title: movie.title.into(),
                poster_url: movie.poster_url.unwrap_or_default().into(),
                year: movie.year.unwrap_or(0) as i32,
                rating: movie.rating.unwrap_or(0.0),
                duration: movie.duration.as_secs() as i32,
            },
            MediaItem::Show(show) => Self {
                id: show.id.into(),
                title: show.title.into(),
                poster_url: show.poster_url.unwrap_or_default().into(),
                year: show.year.unwrap_or(0) as i32,
                rating: show.rating.unwrap_or(0.0),
                duration: 0, // Shows don't have a single duration
            },
            MediaItem::Episode(episode) => Self {
                id: episode.id.into(),
                title: episode.title.into(),
                poster_url: episode.thumbnail_url.unwrap_or_default().into(),
                year: episode.air_date.map(|date| date.year()).unwrap_or(0),
                rating: 0.0, // Episodes don't have ratings in this model
                duration: episode.duration.as_secs() as i32,
            },
            MediaItem::MusicAlbum(album) => Self {
                id: album.id.into(),
                title: album.title.into(),
                poster_url: album.cover_url.unwrap_or_default().into(),
                year: album.year.unwrap_or(0) as i32,
                rating: 0.0, // Music albums don't have ratings in this simple model
                duration: album.duration.as_secs() as i32,
            },
            MediaItem::MusicTrack(track) => Self {
                id: track.id.into(),
                title: track.title.into(),
                poster_url: track.cover_url.unwrap_or_default().into(),
                year: 0,     // Tracks don't have years directly
                rating: 0.0, // Music tracks don't have ratings in this simple model
                duration: track.duration.as_secs() as i32,
            },
            MediaItem::Photo(photo) => Self {
                id: photo.id.into(),
                title: photo.title.into(),
                poster_url: photo.thumbnail_url.unwrap_or_default().into(),
                year: photo.date_taken.map(|date| date.year()).unwrap_or(0),
                rating: 0.0, // Photos don't have ratings
                duration: 0, // Photos don't have duration
            },
        }
    }
}

pub struct LibraryAdapter {
    view_model: Arc<LibraryViewModel>,
    is_loading: Arc<AtomicBool>,
    error_message: Arc<tokio::sync::RwLock<String>>,
    current_items: Arc<tokio::sync::RwLock<Vec<MediaItemData>>>,
}

impl LibraryAdapter {
    pub fn new(view_model: Arc<LibraryViewModel>) -> Result<Self> {
        let is_loading = Arc::new(AtomicBool::new(false));
        let error_message = Arc::new(tokio::sync::RwLock::new(String::new()));
        let current_items = Arc::new(tokio::sync::RwLock::new(Vec::new()));

        let mut adapter = Self {
            view_model,
            is_loading,
            error_message,
            current_items,
        };

        // Set up property subscriptions
        adapter.setup_subscriptions()?;

        Ok(adapter)
    }

    pub fn create_items_model(&self) -> Arc<VecModel<MediaItemData>> {
        Arc::new(VecModel::default())
    }

    pub async fn sync_to_vecmodel(&self, vec_model: &VecModel<MediaItemData>) {
        let items = self.current_items.read().await;
        vec_model.set_vec(items.clone());
    }

    pub fn is_loading(&self) -> bool {
        self.is_loading.load(Ordering::Relaxed)
    }

    pub async fn get_error_message(&self) -> String {
        self.error_message.read().await.clone()
    }

    fn setup_subscriptions(&mut self) -> Result<()> {
        info!("Setting up LibraryAdapter subscriptions");

        // Subscribe to filtered_items changes
        let mut items_subscriber = self
            .view_model
            .subscribe_to_property("filtered_items")
            .ok_or_else(|| anyhow::anyhow!("Failed to subscribe to filtered_items property"))?;

        let view_model = self.view_model.clone();

        {
            let current_items = self.current_items.clone();
            tokio::spawn(async move {
                debug!("Starting filtered_items subscription loop");
                while items_subscriber.wait_for_change().await {
                    debug!("filtered_items changed, updating internal model");
                    let items = view_model.filtered_items().get().await;
                    let items_len = items.len();
                    let slint_items: Vec<MediaItemData> =
                        items.into_iter().map(MediaItemData::from).collect();

                    {
                        let mut current_items_guard = current_items.write().await;
                        *current_items_guard = slint_items;
                    }

                    debug!("Internal items model updated with {} items", items_len);
                }
                debug!("filtered_items subscription loop ended");
            });
        }

        // Subscribe to loading state changes
        let mut loading_subscriber = self
            .view_model
            .subscribe_to_property("is_loading")
            .ok_or_else(|| anyhow::anyhow!("Failed to subscribe to is_loading property"))?;

        let is_loading = self.is_loading.clone();
        let view_model = self.view_model.clone();

        tokio::spawn(async move {
            debug!("Starting is_loading subscription loop");
            while loading_subscriber.wait_for_change().await {
                let loading = view_model.is_loading().get().await;
                is_loading.store(loading, Ordering::Relaxed);
                debug!("Loading state updated to: {}", loading);
            }
            debug!("is_loading subscription loop ended");
        });

        // Subscribe to error changes
        let mut error_subscriber = self
            .view_model
            .subscribe_to_property("error")
            .ok_or_else(|| anyhow::anyhow!("Failed to subscribe to error property"))?;

        let error_message = self.error_message.clone();
        let view_model = self.view_model.clone();

        tokio::spawn(async move {
            debug!("Starting error subscription loop");
            while error_subscriber.wait_for_change().await {
                let error = view_model.error().get().await;
                let message = error.unwrap_or_default();
                {
                    let mut error_guard = error_message.write().await;
                    *error_guard = message.clone();
                }
                debug!("Error message updated to: {}", message);
            }
            debug!("error subscription loop ended");
        });

        info!("LibraryAdapter subscriptions set up successfully");
        Ok(())
    }
}
