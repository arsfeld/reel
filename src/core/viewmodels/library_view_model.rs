use super::{Property, PropertySubscriber, ViewModel};
use crate::db::entities::libraries::Model as Library;
use crate::db::repository::MediaRepository;
use crate::events::{DatabaseEvent, EventBus, EventFilter, EventType};
use crate::models::MediaItem;
use crate::services::DataService;
use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone)]
pub enum SortOrder {
    TitleAsc,
    TitleDesc,
    YearAsc,
    YearDesc,
    RatingAsc,
    RatingDesc,
    AddedAsc,
    AddedDesc,
}

#[derive(Debug, Clone)]
pub struct FilterOptions {
    pub search: String,
    pub genres: Vec<String>,
    pub years: Option<(i32, i32)>,
    pub min_rating: Option<f32>,
    pub watch_status: WatchStatus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WatchStatus {
    All,
    Watched,
    Unwatched,
    InProgress,
}

impl Default for FilterOptions {
    fn default() -> Self {
        Self {
            search: String::new(),
            genres: Vec::new(),
            years: None,
            min_rating: None,
            watch_status: WatchStatus::All,
        }
    }
}

#[derive(Debug)]
pub struct LibraryViewModel {
    data_service: Arc<DataService>,
    current_library: Property<Option<Library>>,
    items: Property<Vec<MediaItem>>,
    filtered_items: Property<Vec<MediaItem>>,
    filter_options: Property<FilterOptions>,
    sort_order: Property<SortOrder>,
    is_loading: Property<bool>,
    is_syncing: Property<bool>,
    error: Property<Option<String>>,
    selected_items: Property<Vec<String>>,
    event_bus: Option<Arc<EventBus>>,
    update_batch: Arc<Mutex<UpdateBatch>>,
    last_sync_time: Arc<Mutex<Option<chrono::NaiveDateTime>>>,
}

#[derive(Debug)]
struct UpdateBatch {
    last_update: Option<Instant>,
    pending_refresh: bool,
}

impl LibraryViewModel {
    pub fn new(data_service: Arc<DataService>) -> Self {
        Self {
            data_service,
            current_library: Property::new(None, "current_library"),
            items: Property::new(Vec::new(), "items"),
            filtered_items: Property::new(Vec::new(), "filtered_items"),
            filter_options: Property::new(FilterOptions::default(), "filter_options"),
            sort_order: Property::new(SortOrder::TitleAsc, "sort_order"),
            is_loading: Property::new(false, "is_loading"),
            is_syncing: Property::new(false, "is_syncing"),
            error: Property::new(None, "error"),
            selected_items: Property::new(Vec::new(), "selected_items"),
            event_bus: None,
            update_batch: Arc::new(Mutex::new(UpdateBatch {
                last_update: None,
                pending_refresh: false,
            })),
            last_sync_time: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn set_library(&self, library_id: String) -> Result<()> {
        let start = Instant::now();
        info!("[PERF] set_library: Starting for library {}", library_id);
        self.is_loading.set(true).await;
        self.error.set(None).await;
        // When switching libraries, reset incremental sync state
        {
            let mut last = self.last_sync_time.lock().await;
            *last = None;
        }
        // Clear any stale items from the previous library to avoid flicker/confusion
        self.items.set(Vec::new()).await;
        self.filtered_items.set(Vec::new()).await;

        match self.data_service.get_library(&library_id).await {
            Ok(Some(library)) => {
                self.current_library.set(Some(library.clone())).await;

                let items_start = Instant::now();
                match self.data_service.get_media_items(&library_id).await {
                    Ok(items) => {
                        let items_elapsed = items_start.elapsed();
                        info!(
                            "[PERF] get_media_items completed in {:?} ({} items)",
                            items_elapsed,
                            items.len()
                        );

                        let set_start = Instant::now();
                        self.items.set(items.clone()).await;
                        let set_elapsed = set_start.elapsed();
                        info!("[PERF] Setting items completed in {:?}", set_elapsed);

                        self.apply_filters_and_sort().await;
                        self.is_loading.set(false).await;

                        let total_elapsed = start.elapsed();
                        warn!("[PERF] set_library total: {:?}", total_elapsed);
                        Ok(())
                    }
                    Err(e) => {
                        error!("Failed to load media items: {}", e);
                        self.error.set(Some(e.to_string())).await;
                        self.is_loading.set(false).await;
                        Err(e)
                    }
                }
            }
            Ok(None) => {
                let msg = format!("Library {} not found", library_id);
                self.error.set(Some(msg.clone())).await;
                self.is_loading.set(false).await;
                Err(anyhow::anyhow!(msg))
            }
            Err(e) => {
                error!("Failed to load library: {}", e);
                self.error.set(Some(e.to_string())).await;
                self.is_loading.set(false).await;
                Err(e)
            }
        }
    }

    async fn refresh_items_silently(&self) -> Result<()> {
        // Only do incremental updates if we have a last sync time
        if let Some(library) = self.current_library.get().await {
            let last_sync = *self.last_sync_time.lock().await;

            let items = if let Some(since) = last_sync {
                // Get only items modified since last sync
                match self
                    .data_service
                    .get_media_items_since(&library.id, since)
                    .await
                {
                    Ok(new_items) if !new_items.is_empty() => {
                        debug!("Found {} updated items since last sync", new_items.len());

                        // Merge with existing items
                        let mut current = self.items.get().await;
                        let new_ids: std::collections::HashSet<String> =
                            new_items.iter().map(|item| item.id().to_string()).collect();

                        // Remove old versions of updated items
                        current.retain(|item| !new_ids.contains(&item.id().to_string()));

                        // Add new/updated items
                        current.extend(new_items);
                        current
                    }
                    Ok(_) => {
                        // No updates, keep existing items
                        return Ok(());
                    }
                    Err(e) => {
                        error!("Failed to get incremental updates: {}", e);
                        // Fall back to full refresh
                        self.data_service.get_media_items(&library.id).await?
                    }
                }
            } else {
                // First load or fallback - get all items
                self.data_service.get_media_items(&library.id).await?
            };

            // Update items and sync time
            self.items.set(items).await;
            *self.last_sync_time.lock().await = Some(chrono::Utc::now().naive_utc());
            self.apply_filters_and_sort().await;
            Ok(())
        } else {
            Ok(())
        }
    }

    pub async fn set_filter(&self, filter: FilterOptions) {
        self.filter_options.set(filter).await;
        self.apply_filters_and_sort().await;
    }

    pub async fn set_sort_order(&self, order: SortOrder) {
        self.sort_order.set(order).await;
        self.apply_filters_and_sort().await;
    }

    pub async fn search(&self, query: String) {
        self.filter_options.update(|f| f.search = query).await;
        self.apply_filters_and_sort().await;
    }

    pub async fn set_watch_status(&self, status: WatchStatus) {
        self.filter_options
            .update(|f| f.watch_status = status)
            .await;
        self.apply_filters_and_sort().await;
    }

    pub async fn toggle_selection(&self, item_id: String) {
        self.selected_items
            .update(|items| {
                if let Some(pos) = items.iter().position(|id| id == &item_id) {
                    items.remove(pos);
                } else {
                    items.push(item_id);
                }
            })
            .await;
    }

    pub async fn clear_selection(&self) {
        self.selected_items.set(Vec::new()).await;
    }

    async fn apply_filters_and_sort(&self) {
        let start = Instant::now();
        let items = self.items.get().await;
        let filter = self.filter_options.get().await;
        let sort_order = self.sort_order.get().await;

        info!(
            "[PERF] apply_filters_and_sort: Starting with {} items",
            items.len()
        );

        let filter_start = Instant::now();
        let mut filtered: Vec<MediaItem> = items
            .into_iter()
            .filter(|item| {
                // Filter out episodes - library view should only show shows and movies
                if matches!(item, MediaItem::Episode(_)) {
                    return false;
                }

                if !filter.search.is_empty() {
                    let search_lower = filter.search.to_lowercase();
                    let title_match = item.title().to_lowercase().contains(&search_lower);
                    let overview_match = match item {
                        MediaItem::Movie(m) => m
                            .overview
                            .as_ref()
                            .is_some_and(|o| o.to_lowercase().contains(&search_lower)),
                        MediaItem::Show(s) => s
                            .overview
                            .as_ref()
                            .is_some_and(|o| o.to_lowercase().contains(&search_lower)),
                        MediaItem::Episode(e) => e
                            .overview
                            .as_ref()
                            .is_some_and(|o| o.to_lowercase().contains(&search_lower)),
                        _ => false,
                    };

                    if !title_match && !overview_match {
                        return false;
                    }
                }

                if let Some((min_year, max_year)) = filter.years {
                    if let Some(year) = Self::extract_year(item) {
                        let year_i32 = year as i32;
                        if year_i32 < min_year || year_i32 > max_year {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                if let Some(min_rating) = filter.min_rating {
                    if let Some(rating) = Self::extract_rating(item) {
                        if rating < min_rating {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                // Apply watch status filter
                if filter.watch_status != WatchStatus::All {
                    let is_watched = Self::extract_watched_status(item);
                    let playback_position = Self::extract_playback_position(item);
                    let has_progress =
                        playback_position.is_some() && playback_position.unwrap().as_millis() > 0;
                    let is_partially_watched = has_progress && !is_watched;

                    match filter.watch_status {
                        WatchStatus::Watched => {
                            if !is_watched {
                                return false;
                            }
                        }
                        WatchStatus::Unwatched => {
                            if is_watched || is_partially_watched {
                                return false;
                            }
                        }
                        WatchStatus::InProgress => {
                            if !is_partially_watched {
                                return false;
                            }
                        }
                        WatchStatus::All => {} // No filtering
                    }
                }

                true
            })
            .collect();

        let filter_elapsed = filter_start.elapsed();
        info!(
            "[PERF] Filtering completed in {:?} ({} items passed filter)",
            filter_elapsed,
            filtered.len()
        );

        let sort_start = Instant::now();
        match sort_order {
            SortOrder::TitleAsc => filtered.sort_by(|a, b| a.title().cmp(b.title())),
            SortOrder::TitleDesc => filtered.sort_by(|a, b| b.title().cmp(a.title())),
            SortOrder::YearAsc => filtered.sort_by(|a, b| {
                let a_year = Self::extract_year(a);
                let b_year = Self::extract_year(b);
                a_year.cmp(&b_year)
            }),
            SortOrder::YearDesc => filtered.sort_by(|a, b| {
                let a_year = Self::extract_year(a);
                let b_year = Self::extract_year(b);
                b_year.cmp(&a_year)
            }),
            SortOrder::RatingAsc => filtered.sort_by(|a, b| {
                let a_rating = Self::extract_rating(a);
                let b_rating = Self::extract_rating(b);
                a_rating
                    .partial_cmp(&b_rating)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            SortOrder::RatingDesc => filtered.sort_by(|a, b| {
                let a_rating = Self::extract_rating(a);
                let b_rating = Self::extract_rating(b);
                b_rating
                    .partial_cmp(&a_rating)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            SortOrder::AddedAsc => {
                // Sort by added_at timestamp when available in database
                // For now, maintain original order
                // filtered.sort_by(|a, b| a.added_at.cmp(&b.added_at))
            }
            SortOrder::AddedDesc => {
                // Sort by added_at timestamp when available in database
                // For now, maintain original order
                // filtered.sort_by(|a, b| b.added_at.cmp(&a.added_at))
            }
        }

        let sort_elapsed = sort_start.elapsed();
        info!("[PERF] Sorting completed in {:?}", sort_elapsed);

        let set_start = Instant::now();
        self.filtered_items.set(filtered).await;
        let set_elapsed = set_start.elapsed();
        info!(
            "[PERF] Setting filtered_items completed in {:?}",
            set_elapsed
        );

        let total_elapsed = start.elapsed();
        warn!(
            "[PERF] apply_filters_and_sort total: {:?} (filter: {:?}, sort: {:?}, set: {:?})",
            total_elapsed, filter_elapsed, sort_elapsed, set_elapsed
        );
    }

    fn extract_year(item: &MediaItem) -> Option<u32> {
        match item {
            MediaItem::Movie(m) => m.year,
            MediaItem::Show(s) => s.year,
            MediaItem::MusicAlbum(a) => a.year,
            _ => None,
        }
    }

    fn extract_rating(item: &MediaItem) -> Option<f32> {
        match item {
            MediaItem::Movie(m) => m.rating,
            MediaItem::Show(s) => s.rating,
            _ => None,
        }
    }

    fn extract_watched_status(item: &MediaItem) -> bool {
        // For now, check if item has been watched based on type
        // In the future, this should come from playback_progress table
        match item {
            MediaItem::Movie(m) => {
                // Check if movie has been watched (would need playback progress)
                false
            }
            MediaItem::Episode(e) => {
                // Check if episode has been watched
                false
            }
            _ => false,
        }
    }

    fn extract_playback_position(item: &MediaItem) -> Option<std::time::Duration> {
        // For now, return None
        // In the future, this should query the playback_progress table
        None
    }

    async fn handle_event(&self, event: DatabaseEvent) {
        match event.event_type {
            EventType::SyncStarted => {
                // Mark that sync is in progress
                self.is_syncing.set(true).await;
            }
            EventType::SyncCompleted | EventType::SyncFailed => {
                // Sync finished, do final refresh
                self.is_syncing.set(false).await;
                let _ = self.refresh_items_silently().await;
            }
            EventType::MediaBatchCreated => {
                // Prefer targeted updates using payload IDs when possible
                if let crate::events::EventPayload::MediaBatch {
                    ids, library_id, ..
                } = &event.payload
                {
                    // Only process if this is the current library
                    if let Some(current) = self.current_library.get().await
                        && current.id == *library_id
                    {
                        match self.data_service.get_media_items_by_ids(ids).await {
                            Ok(updated_items) if !updated_items.is_empty() => {
                                // Merge into current items
                                self.merge_updated_items(updated_items).await;
                            }
                            Ok(_) => {}
                            Err(e) => {
                                error!("Failed to fetch batch items: {}", e);
                                // Fall back to silent refresh
                                let _ = self.refresh_items_silently().await;
                            }
                        }
                        return;
                    }
                }
                // Fallback: Previous batching behavior
                if self.is_syncing.get().await {
                    let mut batch = self.update_batch.lock().await;
                    batch.pending_refresh = true;
                    if batch.last_update.is_none()
                        || batch.last_update.unwrap().elapsed() > Duration::from_secs(5)
                    {
                        batch.last_update = Some(Instant::now());
                        batch.pending_refresh = false;
                        drop(batch);
                        let _ = self.refresh_items_silently().await;
                        let update_batch = self.update_batch.clone();
                        let self_clone = self.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            let mut batch = update_batch.lock().await;
                            if batch.pending_refresh {
                                batch.pending_refresh = false;
                                drop(batch);
                                let _ = self_clone.refresh_items_silently().await;
                            }
                        });
                    }
                } else {
                    let _ = self.refresh_items_silently().await;
                }
            }
            EventType::MediaCreated | EventType::MediaUpdated | EventType::MediaDeleted => {
                // Individual item changes - apply targeted updates when possible
                if let crate::events::EventPayload::Media { id, library_id, .. } = &event.payload
                    && let Some(current) = self.current_library.get().await
                    && current.id == *library_id
                {
                    match event.event_type {
                        EventType::MediaDeleted => {
                            self.items
                                .update(|items| items.retain(|it| it.id() != id))
                                .await;
                            self.apply_filters_and_sort().await;
                        }
                        _ => match self.data_service.get_media_item(id).await {
                            Ok(Some(item)) => {
                                self.merge_updated_items(vec![item]).await;
                            }
                            Ok(None) => {}
                            Err(e) => {
                                error!("Failed to fetch media {}: {}", id, e);
                            }
                        },
                    }
                    return;
                }
                // If not current library or no payload, debounce a small silent refresh when idle
                if !self.is_syncing.get().await {
                    let mut batch = self.update_batch.lock().await;
                    let now = Instant::now();
                    if let Some(last) = batch.last_update
                        && now.duration_since(last) < Duration::from_millis(800)
                    {
                        batch.pending_refresh = true;
                        return;
                    }
                    batch.last_update = Some(now);
                    batch.pending_refresh = false;
                    drop(batch);
                    let _ = self.refresh_items_silently().await;
                }
            }
            EventType::LibraryUpdated => {
                if let Some(lib) = self.current_library.get().await
                    && let Ok(Some(updated)) = self.data_service.get_library(&lib.id).await
                {
                    self.current_library.set(Some(updated)).await;
                }
            }
            EventType::PlaybackPositionUpdated => {
                if let crate::events::EventPayload::Playback {
                    media_id, position, ..
                } = &event.payload
                {
                    let id = media_id.clone();
                    let pos = *position;
                    // Update in-memory item to reflect progress without DB roundtrip
                    self.items
                        .update(|items| {
                            for it in items.iter_mut() {
                                if it.id() == id {
                                    match it {
                                        MediaItem::Movie(m) => m.playback_position = pos,
                                        MediaItem::Episode(e) => e.playback_position = pos,
                                        _ => {}
                                    }
                                    break;
                                }
                            }
                        })
                        .await;
                    self.apply_filters_and_sort().await;
                }
            }
            _ => {}
        }
    }

    async fn merge_updated_items(&self, updated_items: Vec<MediaItem>) {
        // Replace any existing entries with same IDs, then append new ones
        let updated_ids: std::collections::HashSet<String> =
            updated_items.iter().map(|it| it.id().to_string()).collect();
        self.items
            .update(|items| {
                items.retain(|it| !updated_ids.contains(it.id()));
                items.extend(updated_items.clone());
            })
            .await;
        self.apply_filters_and_sort().await;
    }

    pub fn items(&self) -> &Property<Vec<MediaItem>> {
        &self.items
    }

    pub fn filtered_items(&self) -> &Property<Vec<MediaItem>> {
        &self.filtered_items
    }

    pub fn current_library(&self) -> &Property<Option<Library>> {
        &self.current_library
    }

    pub fn is_loading(&self) -> &Property<bool> {
        &self.is_loading
    }

    pub fn is_syncing(&self) -> &Property<bool> {
        &self.is_syncing
    }

    pub fn error(&self) -> &Property<Option<String>> {
        &self.error
    }

    pub fn selected_items(&self) -> &Property<Vec<String>> {
        &self.selected_items
    }

    pub async fn refresh(&self) -> Result<()> {
        // Reload items for the current library
        if let Some(library) = self.current_library.get().await {
            self.set_library(library.id.clone()).await
        } else {
            Ok(())
        }
    }

    pub async fn select_item(&self, item_id: String) {
        let mut selected = self.selected_items.get().await;
        if !selected.contains(&item_id) {
            selected.push(item_id);
            self.selected_items.set(selected).await;
        }
    }
}

#[async_trait::async_trait]
impl ViewModel for LibraryViewModel {
    async fn initialize(&self, event_bus: Arc<EventBus>) {
        let filter = EventFilter::new().with_types(vec![
            EventType::MediaCreated,
            EventType::MediaUpdated,
            EventType::MediaDeleted,
            EventType::MediaBatchCreated,
            EventType::LibraryUpdated,
            EventType::LibraryItemCountChanged,
            EventType::SyncStarted,
            EventType::SyncCompleted,
            EventType::SyncFailed,
            EventType::PlaybackPositionUpdated,
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
            "items" => Some(self.items.subscribe()),
            "filtered_items" => Some(self.filtered_items.subscribe()),
            "current_library" => Some(self.current_library.subscribe()),
            "is_loading" => Some(self.is_loading.subscribe()),
            "is_syncing" => Some(self.is_syncing.subscribe()),
            "error" => Some(self.error.subscribe()),
            "selected_items" => Some(self.selected_items.subscribe()),
            _ => None,
        }
    }

    async fn refresh(&self) {
        if let Some(library) = self.current_library.get().await {
            let _ = self.set_library(library.id).await;
        }
    }

    fn dispose(&self) {}
}

impl Clone for LibraryViewModel {
    fn clone(&self) -> Self {
        Self {
            data_service: self.data_service.clone(),
            current_library: self.current_library.clone(),
            items: self.items.clone(),
            filtered_items: self.filtered_items.clone(),
            filter_options: self.filter_options.clone(),
            sort_order: self.sort_order.clone(),
            is_loading: self.is_loading.clone(),
            is_syncing: self.is_syncing.clone(),
            error: self.error.clone(),
            selected_items: self.selected_items.clone(),
            event_bus: self.event_bus.clone(),
            update_batch: self.update_batch.clone(),
            last_sync_time: self.last_sync_time.clone(),
        }
    }
}
