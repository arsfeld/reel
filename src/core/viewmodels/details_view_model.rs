use super::{ComputedProperty, Property, PropertySubscriber, ViewModel};
use crate::events::{DatabaseEvent, EventBus, EventFilter, EventPayload, EventType};
use crate::models::{MediaItem, StreamInfo};
use crate::services::DataService;
use crate::state::AppState;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaMetadata {
    pub cast: Vec<Person>,
    pub crew: Vec<Person>,
    pub genres: Vec<String>,
    pub studios: Vec<String>,
    pub tags: Vec<String>,
    pub content_rating: Option<String>,
    pub original_title: Option<String>,
    pub tagline: Option<String>,
    pub runtime_minutes: Option<i32>,
    pub release_date: Option<chrono::NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub name: String,
    pub role: Option<String>,
    pub photo_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RelatedMedia {
    pub similar: Vec<MediaItem>,
    pub recommended: Vec<MediaItem>,
    pub from_same_series: Vec<MediaItem>,
}

#[derive(Debug, Clone)]
pub struct DetailedMediaInfo {
    pub media: MediaItem,
    pub metadata: MediaMetadata,
    pub playback_progress: Option<(u64, u64)>, // (position_ms, duration_ms)
    pub related: RelatedMedia,
}

pub struct DetailsViewModel {
    data_service: Arc<DataService>,
    app_state: Option<std::sync::Weak<AppState>>,
    current_item: Property<Option<DetailedMediaInfo>>,
    media_id: Property<Option<String>>,
    is_loading: Property<bool>,
    error: Property<Option<String>>,
    is_favorite: Property<bool>,
    is_watched: Property<bool>,
    user_rating: Property<Option<f32>>,
    related_items: Property<RelatedMedia>,
    show_more_info: Property<bool>,
    selected_tab: Property<DetailTab>,
    // Episode-specific properties for shows
    current_season: Property<Option<i32>>,
    episodes: Property<Vec<MediaItem>>,
    seasons: Property<Vec<i32>>,
    is_loading_episodes: Property<bool>,
    // Stream info properties for reactive loading
    stream_info: Property<Option<StreamInfo>>,
    stream_info_loading: Property<bool>,
    stream_info_error: Property<Option<String>>,
    event_bus: Option<Arc<EventBus>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DetailTab {
    Overview,
    Cast,
    Crew,
    Related,
    Reviews,
    Technical,
}

impl DetailsViewModel {
    pub fn new(data_service: Arc<DataService>) -> Self {
        Self {
            data_service,
            app_state: None,
            current_item: Property::new(None, "current_item"),
            media_id: Property::new(None, "media_id"),
            is_loading: Property::new(false, "is_loading"),
            error: Property::new(None, "error"),
            is_favorite: Property::new(false, "is_favorite"),
            is_watched: Property::new(false, "is_watched"),
            user_rating: Property::new(None, "user_rating"),
            related_items: Property::new(
                RelatedMedia {
                    similar: Vec::new(),
                    recommended: Vec::new(),
                    from_same_series: Vec::new(),
                },
                "related_items",
            ),
            show_more_info: Property::new(false, "show_more_info"),
            selected_tab: Property::new(DetailTab::Overview, "selected_tab"),
            current_season: Property::new(None, "current_season"),
            episodes: Property::new(Vec::new(), "episodes"),
            seasons: Property::new(Vec::new(), "seasons"),
            is_loading_episodes: Property::new(false, "is_loading_episodes"),
            // Stream info properties
            stream_info: Property::new(None, "stream_info"),
            stream_info_loading: Property::new(false, "stream_info_loading"),
            stream_info_error: Property::new(None, "stream_info_error"),
            event_bus: None,
        }
    }

    pub async fn load_media_item(&self, media: MediaItem) -> Result<()> {
        self.is_loading.set(true).await;
        self.error.set(None).await;
        self.media_id.set(Some(media.id().to_string())).await;

        let metadata = self.extract_metadata(&media).await;

        let playback_progress = self
            .data_service
            .get_playback_progress(&media.id())
            .await
            .ok()
            .flatten();

        if let Some((position_ms, duration_ms)) = playback_progress {
            // Consider watched if >90% complete
            let watched = position_ms as f64 / duration_ms as f64 > 0.9;
            self.is_watched.set(watched).await;
        }

        let related = self.load_related_media(&media).await;
        self.related_items.set(related.clone()).await;

        let detailed_info = DetailedMediaInfo {
            media: media.clone(),
            metadata,
            playback_progress,
            related,
        };

        self.current_item.set(Some(detailed_info)).await;

        // If this is a show, load episodes for the first season
        if let MediaItem::Show(show) = &media {
            info!(
                "DetailsViewModel::load_media_item: show loaded id={} title={}",
                show.id, show.title
            );
            self.load_seasons_for_show(&media).await;

            // Load episodes for the first season
            if let Some(first_season) = self.seasons.get().await.first() {
                info!(
                    "DetailsViewModel::load_media_item: first season determined: {}",
                    first_season
                );
                let _ = self
                    .load_episodes_for_season(&media.id(), *first_season)
                    .await;
            }
        }

        // Load stream info for movies (shows don't have direct stream info)
        if let MediaItem::Movie(_) = &media {
            self.load_stream_info_async(&media).await;
        }

        self.is_loading.set(false).await;

        Ok(())
    }

    pub async fn load_media(&self, media_id: String) -> Result<()> {
        self.is_loading.set(true).await;
        self.error.set(None).await;
        self.media_id.set(Some(media_id.clone())).await;

        match self.data_service.get_media_item(&media_id).await {
            Ok(Some(media)) => {
                let metadata = self.extract_metadata(&media).await;

                let playback_progress = self
                    .data_service
                    .get_playback_progress(&media_id)
                    .await
                    .ok()
                    .flatten();

                if let Some((position_ms, duration_ms)) = playback_progress {
                    // Consider watched if >90% complete
                    let watched = position_ms as f64 / duration_ms as f64 > 0.9;
                    self.is_watched.set(watched).await;
                }

                let related = self.load_related_media(&media).await;
                self.related_items.set(related.clone()).await;

                let detailed_info = DetailedMediaInfo {
                    media: media.clone(),
                    metadata,
                    playback_progress,
                    related,
                };

                self.current_item.set(Some(detailed_info)).await;

                // If this is a show, load episodes for the first season
                if let MediaItem::Show(show) = &media {
                    info!(
                        "DetailsViewModel::load_media: show loaded id={} title={}",
                        show.id, show.title
                    );
                    self.load_seasons_for_show(&media).await;

                    // Load episodes for the first season
                    if let Some(first_season) = self.seasons.get().await.first() {
                        info!(
                            "DetailsViewModel::load_media: first season determined: {}",
                            first_season
                        );
                        let _ = self
                            .load_episodes_for_season(&media_id, *first_season)
                            .await;
                    }
                }

                // Load stream info for movies (shows don't have direct stream info)
                if let MediaItem::Movie(_) = &media {
                    self.load_stream_info_async(&media).await;
                }

                self.is_loading.set(false).await;

                Ok(())
            }
            Ok(None) => {
                let msg = format!("Media item {} not found", media_id);
                self.error.set(Some(msg.clone())).await;
                self.is_loading.set(false).await;
                Err(anyhow::anyhow!(msg))
            }
            Err(e) => {
                error!("Failed to load media details: {}", e);
                self.error.set(Some(e.to_string())).await;
                self.is_loading.set(false).await;
                Err(e)
            }
        }
    }

    // Removed - no longer needed, using proper From/TryFrom traits

    async fn extract_metadata(&self, media: &MediaItem) -> MediaMetadata {
        // Extract genres and runtime based on media type
        let genres = match media {
            MediaItem::Movie(m) => m.genres.clone(),
            MediaItem::Show(s) => s.genres.clone(),
            MediaItem::MusicAlbum(a) => a.genres.clone(),
            _ => Vec::new(),
        };

        let runtime_minutes = match media {
            MediaItem::Movie(m) => Some((m.duration.as_secs() / 60) as i32),
            MediaItem::Episode(e) => Some((e.duration.as_secs() / 60) as i32),
            _ => None,
        };

        MediaMetadata {
            cast: Vec::new(),
            crew: Vec::new(),
            genres,
            studios: Vec::new(),
            tags: Vec::new(),
            content_rating: None,
            original_title: None,
            tagline: None,
            runtime_minutes,
            release_date: None,
        }
    }

    async fn load_related_media(&self, media: &MediaItem) -> RelatedMedia {
        let mut similar = Vec::new();
        let mut recommended = Vec::new();
        let mut from_same_series = Vec::new();

        // Extract library_id from the media item's ID (format: "backend_id:library_id:type:item_id")
        let library_id = media.id().split(':').nth(1).unwrap_or_default().to_string();

        if let Ok(library_items) = self.data_service.get_media_items(&library_id).await {
            let media_genres = match media {
                MediaItem::Movie(m) => &m.genres,
                MediaItem::Show(s) => &s.genres,
                MediaItem::MusicAlbum(a) => &a.genres,
                _ => {
                    return RelatedMedia {
                        similar,
                        recommended,
                        from_same_series,
                    };
                }
            };

            let media_rating = match media {
                MediaItem::Movie(m) => m.rating,
                MediaItem::Show(s) => s.rating,
                _ => None,
            };

            for item in library_items.iter().take(100) {
                if item.id() == media.id() {
                    continue;
                }

                // Get genres for the current item
                let item_genres = match item {
                    MediaItem::Movie(m) => &m.genres,
                    MediaItem::Show(s) => &s.genres,
                    MediaItem::MusicAlbum(a) => &a.genres,
                    _ => continue,
                };

                // Check genre overlap
                let genre_overlap = media_genres
                    .iter()
                    .filter(|g| item_genres.contains(g))
                    .count();

                if genre_overlap > 0 && similar.len() < 10 {
                    similar.push(item.clone());
                }

                // Check rating similarity
                let item_rating = match item {
                    MediaItem::Movie(m) => m.rating,
                    MediaItem::Show(s) => s.rating,
                    _ => None,
                };

                if let (Some(rating1), Some(rating2)) = (media_rating, item_rating)
                    && (rating1 - rating2).abs() < 0.5
                    && recommended.len() < 10
                {
                    recommended.push(item.clone());
                }

                // Check if both are episodes from the same show
                if let (MediaItem::Episode(ep1), MediaItem::Episode(ep2)) = (media, item)
                    && ep1.show_id.as_ref() == ep2.show_id.as_ref()
                {
                    from_same_series.push(item.clone());
                }
            }
        }

        RelatedMedia {
            similar,
            recommended,
            from_same_series,
        }
    }

    pub async fn toggle_favorite(&self) {
        let is_fav = !self.is_favorite.get().await;
        self.is_favorite.set(is_fav).await;
    }

    pub async fn mark_as_watched(&self) {
        self.is_watched.set(true).await;

        if let Some(media_id) = self.media_id.get().await
            && let Some(info) = self.current_item.get().await
        {
            let duration_ms = match &info.media {
                MediaItem::Movie(movie) => movie.duration.as_millis() as i64,
                MediaItem::Show(_) => 0, // Shows don't have duration
                MediaItem::Episode(episode) => episode.duration.as_millis() as i64,
                _ => 0,
            };

            let _ = self
                .data_service
                .update_playback_progress(
                    &media_id,
                    duration_ms, // Mark as watched by setting position = duration
                    duration_ms,
                    true, // watched
                )
                .await;
        }
    }

    pub async fn mark_as_unwatched(&self) {
        self.is_watched.set(false).await;

        if let Some(media_id) = self.media_id.get().await
            && let Ok(Some((_, duration_ms))) =
                self.data_service.get_playback_progress(&media_id).await
        {
            // Reset playback progress to beginning
            let _ = self
                .data_service
                .update_playback_progress(
                    &media_id,
                    0, // position_ms
                    duration_ms as i64,
                    false, // watched
                )
                .await;
        }
    }

    pub async fn set_user_rating(&self, rating: f32) {
        self.user_rating.set(Some(rating)).await;
    }

    pub async fn select_tab(&self, tab: DetailTab) {
        self.selected_tab.set(tab).await;
    }

    pub async fn toggle_more_info(&self) {
        let show = !self.show_more_info.get().await;
        self.show_more_info.set(show).await;
    }

    /// Load episodes for a specific season of a show
    pub async fn load_episodes_for_season(&self, show_id: &str, season_number: i32) -> Result<()> {
        info!(
            "DetailsViewModel::load_episodes_for_season: show_id={} season={} (begin)",
            show_id, season_number
        );
        self.is_loading_episodes.set(true).await;
        self.current_season.set(Some(season_number)).await;

        match self
            .data_service
            .get_episodes_by_season(show_id, season_number)
            .await
        {
            Ok(episodes) => {
                info!(
                    "DetailsViewModel::load_episodes_for_season: loaded {} episodes",
                    episodes.len()
                );
                self.episodes.set(episodes.clone()).await;
                self.is_loading_episodes.set(false).await;

                // Check if the entire season is watched
                let all_watched = if !episodes.is_empty() {
                    let mut watched_count = 0;
                    for episode in &episodes {
                        if let Ok(Some((position_ms, duration_ms))) =
                            self.data_service.get_playback_progress(episode.id()).await
                            && position_ms as f64 / duration_ms as f64 > 0.9
                        {
                            watched_count += 1;
                        }
                    }
                    watched_count == episodes.len()
                } else {
                    false
                };

                self.is_watched.set(all_watched).await;
                Ok(())
            }
            Err(e) => {
                error!(
                    "Failed to load episodes for season {}: {}",
                    season_number, e
                );
                self.is_loading_episodes.set(false).await;
                Err(e)
            }
        }
    }

    /// Load all episodes for a show
    pub async fn load_all_episodes_for_show(&self, show_id: &str) -> Result<Vec<MediaItem>> {
        match self.data_service.get_episodes_by_show(show_id).await {
            Ok(episodes) => Ok(episodes),
            Err(e) => {
                error!("Failed to load episodes for show {}: {}", show_id, e);
                Err(e)
            }
        }
    }

    /// Determine available seasons from episodes in the database
    async fn load_seasons_for_show(&self, show: &MediaItem) {
        let show_id = match show {
            MediaItem::Show(s) => &s.id,
            _ => return, // Not a show
        };

        if let Ok(all_episodes) = self.data_service.get_episodes_by_show(show_id).await {
            info!(
                "DetailsViewModel::load_seasons_for_show: show_id={} episodes_in_db={}",
                show_id,
                all_episodes.len()
            );
            let mut seasons: Vec<i32> = all_episodes
                .iter()
                .filter_map(|item| {
                    if let MediaItem::Episode(ep) = item {
                        Some(ep.season_number as i32)
                    } else {
                        None
                    }
                })
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            seasons.sort();
            info!("Computed seasons list: {:?}", seasons);

            if seasons.is_empty() {
                // Fallback: show at least one season so UI can operate
                info!("Seasons list empty, falling back to [1]");
                self.seasons.set(vec![1]).await;
            } else {
                self.seasons.set(seasons).await;
            }
        } else {
            // Default to a reasonable number of seasons if we can't load episodes
            info!(
                "get_episodes_by_show failed for {}, defaulting seasons to [1]",
                show_id
            );
            self.seasons.set(vec![1]).await;
        }
    }

    /// Mark all episodes in the current season as watched
    pub async fn mark_season_as_watched(&self) {
        if let Some(_season) = self.current_season.get().await {
            let episodes = self.episodes.get().await;
            for episode_item in episodes {
                if let MediaItem::Episode(episode) = episode_item {
                    let duration_ms = episode.duration.as_millis() as i64;
                    let _ = self
                        .data_service
                        .update_playback_progress(&episode.id, duration_ms, duration_ms, true)
                        .await;
                }
            }
            self.is_watched.set(true).await;
        }
    }

    /// Mark all episodes in the current season as unwatched
    pub async fn mark_season_as_unwatched(&self) {
        if let Some(_season) = self.current_season.get().await {
            let episodes = self.episodes.get().await;
            for episode_item in episodes {
                if let MediaItem::Episode(episode) = episode_item {
                    let duration_ms = episode.duration.as_millis() as i64;
                    let _ = self
                        .data_service
                        .update_playback_progress(&episode.id, 0, duration_ms, false)
                        .await;
                }
            }
            self.is_watched.set(false).await;
        }
    }

    /// Change to a different season
    pub async fn select_season(&self, season_number: i32) -> Result<()> {
        if let Some(media_id) = self.media_id.get().await {
            info!(
                "DetailsViewModel::select_season: media_id={} season={}",
                media_id, season_number
            );
            self.load_episodes_for_season(&media_id, season_number)
                .await
        } else {
            Err(anyhow::anyhow!("No media loaded"))
        }
    }

    async fn handle_event(&self, event: DatabaseEvent) {
        if let Some(media_id) = self.media_id.get().await {
            match event.event_type {
                // Perform a targeted, silent merge to avoid toggling is_loading
                EventType::MediaUpdated => {
                    if let EventPayload::Media { id, .. } = event.payload
                        && id == media_id
                        && let Ok(Some(updated_item)) =
                            self.data_service.get_media_item(&media_id).await
                    {
                        // Update current detailed info in place without resetting the UI
                        if let Some(mut info) = self.current_item.get().await {
                            // Refresh lightweight metadata from the updated item
                            let new_metadata = self.extract_metadata(&updated_item).await;
                            info.media = updated_item;
                            info.metadata = new_metadata;
                            // Preserve playback_progress and related items
                            self.current_item.set(Some(info)).await;
                        } else {
                            // If nothing loaded yet, avoid loader: set minimal info
                            let metadata = self.extract_metadata(&updated_item).await;
                            let minimal = DetailedMediaInfo {
                                media: updated_item,
                                metadata,
                                playback_progress: None,
                                related: RelatedMedia {
                                    similar: vec![],
                                    recommended: vec![],
                                    from_same_series: vec![],
                                },
                            };
                            self.current_item.set(Some(minimal)).await;
                        }
                    }
                }
                EventType::MediaDeleted => {
                    if let EventPayload::Media { id, .. } = event.payload
                        && id == media_id
                    {
                        self.current_item.set(None).await;
                        self.error
                            .set(Some("Media item has been deleted".to_string()))
                            .await;
                    }
                }
                EventType::PlaybackPositionUpdated | EventType::PlaybackCompleted => {
                    if let EventPayload::Playback {
                        media_id: event_media_id,
                        ..
                    } = event.payload
                        && event_media_id == media_id
                        && let Ok(Some((position_ms, duration_ms))) =
                            self.data_service.get_playback_progress(&media_id).await
                    {
                        let watched = position_ms as f64 / duration_ms as f64 > 0.9;
                        self.is_watched.set(watched).await;

                        if let Some(mut info) = self.current_item.get().await {
                            info.playback_progress = Some((position_ms, duration_ms));
                            self.current_item.set(Some(info)).await;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub fn current_item(&self) -> &Property<Option<DetailedMediaInfo>> {
        &self.current_item
    }

    pub fn is_loading(&self) -> &Property<bool> {
        &self.is_loading
    }

    pub fn is_favorite(&self) -> &Property<bool> {
        &self.is_favorite
    }

    pub fn is_watched(&self) -> &Property<bool> {
        &self.is_watched
    }

    pub fn related_items(&self) -> &Property<RelatedMedia> {
        &self.related_items
    }

    pub fn selected_tab(&self) -> &Property<DetailTab> {
        &self.selected_tab
    }

    pub fn current_season(&self) -> &Property<Option<i32>> {
        &self.current_season
    }

    pub fn episodes(&self) -> &Property<Vec<MediaItem>> {
        &self.episodes
    }

    pub fn seasons(&self) -> &Property<Vec<i32>> {
        &self.seasons
    }

    pub fn is_loading_episodes(&self) -> &Property<bool> {
        &self.is_loading_episodes
    }

    pub fn stream_info(&self) -> &Property<Option<StreamInfo>> {
        &self.stream_info
    }

    pub fn stream_info_loading(&self) -> &Property<bool> {
        &self.stream_info_loading
    }

    pub fn stream_info_error(&self) -> &Property<Option<String>> {
        &self.stream_info_error
    }

    // Show info computed properties
    pub fn show_network(&self) -> ComputedProperty<Option<String>> {
        ComputedProperty::new("show_network", vec![Arc::new(self.current_item.clone())], {
            let current_item = self.current_item.clone();
            move || {
                if let Some(detailed_info) = current_item.get_sync() {
                    if let MediaItem::Show(_show) = &detailed_info.media {
                        // TODO: Add network field to Show struct or extract from metadata
                        // For now, return None since Show struct doesn't have network field
                        return None;
                    }
                }
                None
            }
        })
    }

    pub fn show_status(&self) -> ComputedProperty<Option<String>> {
        ComputedProperty::new("show_status", vec![Arc::new(self.current_item.clone())], {
            let current_item = self.current_item.clone();
            move || {
                if let Some(detailed_info) = current_item.get_sync() {
                    if let MediaItem::Show(_show) = &detailed_info.media {
                        // TODO: Add status field to Show struct or extract from metadata
                        // For now, return None since Show struct doesn't have status field
                        return None;
                    }
                }
                None
            }
        })
    }

    pub fn show_content_rating(&self) -> ComputedProperty<Option<String>> {
        ComputedProperty::new(
            "show_content_rating",
            vec![Arc::new(self.current_item.clone())],
            {
                let current_item = self.current_item.clone();
                move || {
                    if let Some(detailed_info) = current_item.get_sync() {
                        if let MediaItem::Show(_show) = &detailed_info.media {
                            // Extract content rating from metadata
                            return detailed_info.metadata.content_rating.clone();
                        }
                    }
                    None
                }
            },
        )
    }

    pub fn set_app_state(&mut self, app_state: std::sync::Weak<AppState>) {
        self.app_state = Some(app_state);
    }

    async fn load_stream_info_async(&self, media: &MediaItem) {
        if let MediaItem::Movie(movie) = media {
            self.stream_info_loading.set(true).await;
            self.stream_info_error.set(None).await;

            if let Some(app_state_weak) = &self.app_state {
                if let Some(app_state) = app_state_weak.upgrade() {
                    let backend_id = &movie.backend_id;
                    match app_state.source_coordinator.get_backend(backend_id).await {
                        Some(backend) => match backend.get_stream_url(&movie.id).await {
                            Ok(stream_info) => {
                                self.stream_info.set(Some(stream_info)).await;
                            }
                            Err(e) => {
                                error!("Failed to load stream info: {}", e);
                                self.stream_info_error.set(Some(e.to_string())).await;
                            }
                        },
                        None => {
                            let msg = "Backend not available".to_string();
                            self.stream_info_error.set(Some(msg)).await;
                        }
                    }
                } else {
                    let msg = "App state not available".to_string();
                    self.stream_info_error.set(Some(msg)).await;
                }
            } else {
                let msg = "App state not initialized".to_string();
                self.stream_info_error.set(Some(msg)).await;
            }

            self.stream_info_loading.set(false).await;
        }
    }
}

#[async_trait::async_trait]
impl ViewModel for DetailsViewModel {
    async fn initialize(&self, event_bus: Arc<EventBus>) {
        let filter = EventFilter::new().with_types(vec![
            EventType::MediaUpdated,
            EventType::MediaDeleted,
            EventType::PlaybackPositionUpdated,
            EventType::PlaybackCompleted,
        ]);

        let mut subscriber = event_bus.subscribe_filtered(filter);
        let self_clone = self.clone();

        tokio::spawn(async move {
            while let Ok(event) = subscriber.recv().await {
                self_clone.handle_event(event).await;
            }
        });
    }

    fn subscribe_to_property(&self, property_name: &str) -> Option<PropertySubscriber> {
        match property_name {
            "current_item" => Some(self.current_item.subscribe()),
            "is_loading" => Some(self.is_loading.subscribe()),
            "is_favorite" => Some(self.is_favorite.subscribe()),
            "is_watched" => Some(self.is_watched.subscribe()),
            "related_items" => Some(self.related_items.subscribe()),
            "selected_tab" => Some(self.selected_tab.subscribe()),
            // Added: expose episode/season updates to the UI
            "episodes" => Some(self.episodes.subscribe()),
            "seasons" => Some(self.seasons.subscribe()),
            "is_loading_episodes" => Some(self.is_loading_episodes.subscribe()),
            // Added: expose stream info updates to the UI
            "stream_info" => Some(self.stream_info.subscribe()),
            "stream_info_loading" => Some(self.stream_info_loading.subscribe()),
            "stream_info_error" => Some(self.stream_info_error.subscribe()),
            _ => None,
        }
    }

    async fn refresh(&self) {
        if let Some(media_id) = self.media_id.get().await {
            let _ = self.load_media(media_id).await;
        }
    }
}

impl Clone for DetailsViewModel {
    fn clone(&self) -> Self {
        Self {
            data_service: self.data_service.clone(),
            app_state: self.app_state.clone(),
            current_item: self.current_item.clone(),
            media_id: self.media_id.clone(),
            is_loading: self.is_loading.clone(),
            error: self.error.clone(),
            is_favorite: self.is_favorite.clone(),
            is_watched: self.is_watched.clone(),
            user_rating: self.user_rating.clone(),
            related_items: self.related_items.clone(),
            show_more_info: self.show_more_info.clone(),
            selected_tab: self.selected_tab.clone(),
            current_season: self.current_season.clone(),
            episodes: self.episodes.clone(),
            seasons: self.seasons.clone(),
            is_loading_episodes: self.is_loading_episodes.clone(),
            stream_info: self.stream_info.clone(),
            stream_info_loading: self.stream_info_loading.clone(),
            stream_info_error: self.stream_info_error.clone(),
            event_bus: self.event_bus.clone(),
        }
    }
}
