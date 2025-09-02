use gtk4::{gdk, glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use once_cell::sync::Lazy;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;
use tracing::{error, info, trace, warn};

use crate::constants::*;
use crate::models::{Episode, Library, MediaItem, Movie, Show};
use crate::platforms::gtk::ui::filters::{FilterManager, WatchStatus};
use crate::platforms::gtk::ui::viewmodels::library_view_model::LibraryViewModel;
use crate::state::AppState;
use crate::utils::{ImageLoader, ImageSize};
use sea_orm::prelude::Json;
use serde_json::Value;
use std::time::Duration;

// Global image loader instance
static IMAGE_LOADER: Lazy<ImageLoader> =
    Lazy::new(|| ImageLoader::new().expect("Failed to create ImageLoader"));

mod imp {
    use super::*;

    pub struct LibraryView {
        pub scrolled_window: RefCell<Option<gtk4::ScrolledWindow>>,
        pub flow_box: RefCell<Option<gtk4::FlowBox>>,
        pub loading_spinner: RefCell<Option<gtk4::Spinner>>,
        pub empty_state: RefCell<Option<adw::StatusPage>>,
        pub stack: RefCell<Option<gtk4::Stack>>,

        pub state: RefCell<Option<Arc<AppState>>>,
        pub library: RefCell<Option<Library>>,
        pub backend_id: RefCell<Option<String>>,
        pub current_view_size: RefCell<ImageSize>,
        pub on_media_selected: RefCell<Option<Box<dyn Fn(&MediaItem)>>>,
        pub filter_manager: RefCell<FilterManager>,
        pub all_media_items: RefCell<Vec<MediaItem>>,
        pub filtered_items: RefCell<Vec<MediaItem>>,
        pub cards_by_index: RefCell<HashMap<usize, MediaCard>>,
        pub cards_by_id: RefCell<HashMap<String, MediaCard>>,
        pub view_model: RefCell<Option<Arc<LibraryViewModel>>>,
        pub update_scheduled: Cell<bool>,
        pub pending_items: RefCell<Option<Vec<MediaItem>>>,
    }

    impl std::fmt::Debug for LibraryView {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("LibraryView")
                .field("scrolled_window", &self.scrolled_window)
                .field("flow_box", &self.flow_box)
                .field("loading_spinner", &self.loading_spinner)
                .field("empty_state", &self.empty_state)
                .field("stack", &self.stack)
                .field("state", &"Arc<AppState>")
                .field("library", &self.library)
                .field("backend_id", &self.backend_id)
                .field("current_view_size", &self.current_view_size)
                .field("on_media_selected", &"Option<Callback>")
                .finish()
        }
    }

    impl Default for LibraryView {
        fn default() -> Self {
            Self {
                scrolled_window: RefCell::new(None),
                flow_box: RefCell::new(None),
                loading_spinner: RefCell::new(None),
                empty_state: RefCell::new(None),
                stack: RefCell::new(None),
                state: RefCell::new(None),
                library: RefCell::new(None),
                backend_id: RefCell::new(None),
                current_view_size: RefCell::new(ImageSize::Medium),
                on_media_selected: RefCell::new(None),
                filter_manager: RefCell::new(FilterManager::new()),
                all_media_items: RefCell::new(Vec::new()),
                filtered_items: RefCell::new(Vec::new()),
                cards_by_index: RefCell::new(HashMap::new()),
                cards_by_id: RefCell::new(HashMap::new()),
                view_model: RefCell::new(None),
                update_scheduled: Cell::new(false),
                pending_items: RefCell::new(None),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LibraryView {
        const NAME: &'static str = "LibraryView";
        type Type = super::LibraryView;
        type ParentType = gtk4::Box;
    }

    impl ObjectImpl for LibraryView {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_ui();
        }
    }

    impl WidgetImpl for LibraryView {}
    impl BoxImpl for LibraryView {}
}

glib::wrapper! {
    pub struct LibraryView(ObjectSubclass<imp::LibraryView>)
        @extends gtk4::Widget, gtk4::Box,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Orientable;
}

impl LibraryView {
    pub fn new(state: Arc<AppState>) -> Self {
        let view: Self = glib::Object::builder()
            .property("orientation", gtk4::Orientation::Vertical)
            .property("spacing", 0)
            .build();

        // Initialize ViewModel with data service from state
        let data_service = state.data_service.clone();
        let view_model = Arc::new(LibraryViewModel::new(data_service));
        view.imp().view_model.replace(Some(view_model.clone()));

        // Initialize ViewModel with EventBus
        glib::spawn_future_local({
            let vm = view_model.clone();
            let event_bus = state.event_bus.clone();
            async move {
                use crate::platforms::gtk::ui::viewmodels::ViewModel;
                vm.initialize(event_bus).await;
            }
        });

        // Subscribe to ViewModel property changes
        view.setup_viewmodel_bindings(view_model);

        view.imp().state.replace(Some(state));
        view.imp().current_view_size.replace(ImageSize::Medium);
        view
    }

    pub fn set_on_media_selected<F>(&self, callback: F)
    where
        F: Fn(&MediaItem) + 'static,
    {
        self.imp()
            .on_media_selected
            .replace(Some(Box::new(callback)));
    }

    fn setup_ui(&self) {
        let imp = self.imp();

        // Create main stack for loading/content/empty states
        let stack = gtk4::Stack::builder()
            .transition_type(gtk4::StackTransitionType::Crossfade)
            .transition_duration(200)
            .build();

        // Loading state
        let loading_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(12)
            .valign(gtk4::Align::Center)
            .halign(gtk4::Align::Center)
            .build();

        let loading_spinner = gtk4::Spinner::builder()
            .spinning(true)
            .width_request(48)
            .height_request(48)
            .build();

        loading_box.append(&loading_spinner);
        loading_box.append(&gtk4::Label::new(Some("Loading library...")));

        stack.add_named(&loading_box, Some("loading"));

        // Content state - scrolled window with flow box for media grid
        let scrolled_window = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .build();

        let flow_box = gtk4::FlowBox::builder()
            .column_spacing(16) // Tighter spacing for better density
            .row_spacing(20) // Good vertical spacing
            .homogeneous(true)
            .min_children_per_line(4) // More items per row with smaller sizes
            .max_children_per_line(12) // Allow more on wide screens
            .selection_mode(gtk4::SelectionMode::None)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .valign(gtk4::Align::Start)
            .build();

        // Adapt columns based on window width
        let weak_self = self.downgrade();
        flow_box.connect_map(move |fb| {
            if let Some(view) = weak_self.upgrade()
                && let Some(window) = view.root().and_then(|r| r.downcast::<gtk4::Window>().ok())
            {
                let flow_box_weak = fb.downgrade();
                window.connect_default_width_notify(move |window| {
                    if let Some(flow_box) = flow_box_weak.upgrade() {
                        let width = window.default_width();

                        // Adjust image size and columns based on window width
                        let (_, min_cols, max_cols) = if width < 800 {
                            (ImageSize::Small, 3, 6)
                        } else if width < 1200 {
                            (ImageSize::Medium, 4, 8)
                        } else {
                            (ImageSize::Medium, 5, 12)
                        };

                        flow_box.set_min_children_per_line(min_cols);
                        flow_box.set_max_children_per_line(max_cols);
                    }
                });
            }
        });

        scrolled_window.set_child(Some(&flow_box));
        stack.add_named(&scrolled_window, Some("content"));

        // Empty state
        let empty_state = adw::StatusPage::builder()
            .title("No Content")
            .description("This library doesn't have any items yet")
            .icon_name("folder-symbolic")
            .build();

        stack.add_named(&empty_state, Some("empty"));

        // Add stack directly to the view
        self.append(&stack);

        // Store references
        imp.scrolled_window.replace(Some(scrolled_window));
        imp.flow_box.replace(Some(flow_box.clone()));
        imp.loading_spinner.replace(Some(loading_spinner));
        imp.empty_state.replace(Some(empty_state));
        imp.stack.replace(Some(stack));

        // We'll connect button clicks directly on cards instead of flow box activation
    }

    fn setup_viewmodel_bindings(&self, view_model: Arc<LibraryViewModel>) {
        let weak_self = self.downgrade();

        // Subscribe to filtered items changes (these are what should be displayed)
        let mut items_subscriber = view_model.filtered_items().subscribe();
        glib::spawn_future_local(async move {
            while items_subscriber.wait_for_change().await {
                if let Some(view) = weak_self.upgrade()
                    && let Some(vm) = &*view.imp().view_model.borrow()
                {
                    let items = vm.filtered_items().get_sync();
                    view.update_items_from_viewmodel(items);
                }
            }
        });

        // Subscribe to loading state
        let weak_self_loading = self.downgrade();
        let mut loading_subscriber = view_model.is_loading().subscribe();
        glib::spawn_future_local(async move {
            while loading_subscriber.wait_for_change().await {
                if let Some(view) = weak_self_loading.upgrade()
                    && let Some(vm) = &*view.imp().view_model.borrow()
                {
                    let is_loading = vm.is_loading().get_sync();
                    if let Some(stack) = view.imp().stack.borrow().as_ref() {
                        if is_loading {
                            stack.set_visible_child_name("loading");
                        } else {
                            // Check ViewModel's filtered_items using sync access
                            let vm_items = vm.filtered_items().get_sync();
                            if vm_items.is_empty() {
                                stack.set_visible_child_name("empty");
                            } else {
                                stack.set_visible_child_name("content");
                            }
                        }
                    }
                }
            }
        });

        // Subscribe to error state
        let weak_self_error = self.downgrade();
        let mut error_subscriber = view_model.error().subscribe();
        glib::spawn_future_local(async move {
            while error_subscriber.wait_for_change().await {
                if let Some(view) = weak_self_error.upgrade()
                    && let Some(vm) = &*view.imp().view_model.borrow()
                    && let Some(err_msg) = vm.error().get_sync()
                {
                    tracing::error!("Library error: {}", err_msg);
                    // Could show error in UI here
                }
            }
        });
    }

    fn update_items_from_viewmodel(&self, items: Vec<crate::models::MediaItem>) {
        let start = Instant::now();
        info!(
            "[PERF] update_items_from_viewmodel: Received {} items from ViewModel",
            items.len()
        );

        // Store pending items and schedule batched update
        self.imp().pending_items.replace(Some(items));

        // Schedule UI update if not already scheduled
        if !self.imp().update_scheduled.get() {
            self.imp().update_scheduled.set(true);
            info!("[PERF] Scheduling UI update via idle_add_local_once");

            let weak_self = self.downgrade();
            glib::idle_add_local_once(move || {
                let idle_start = Instant::now();
                if let Some(view) = weak_self.upgrade() {
                    // Process pending items
                    if let Some(items) = view.imp().pending_items.take() {
                        info!(
                            "[PERF] Processing {} pending items in idle callback",
                            items.len()
                        );
                        view.display_media_items(items);
                    }
                    view.imp().update_scheduled.set(false);
                }
                let idle_elapsed = idle_start.elapsed();
                if idle_elapsed.as_millis() > 16 {
                    warn!("[PERF] Slow idle callback execution: {:?}", idle_elapsed);
                }
            });
        } else {
            info!("[PERF] UI update already scheduled, skipping");
        }

        let elapsed = start.elapsed();
        if elapsed.as_millis() > 2 {
            warn!("[PERF] Slow update scheduling: {:?}", elapsed);
        }
    }

    fn convert_db_item_to_ui_model(
        &self,
        db_item: crate::db::entities::media_items::Model,
    ) -> Option<MediaItem> {
        match db_item.media_type.as_str() {
            "movie" => {
                let movie = Movie {
                    id: db_item.id,
                    backend_id: db_item.source_id,
                    title: db_item.title,
                    year: db_item.year.map(|y| y as u32),
                    duration: Duration::from_millis(db_item.duration_ms.unwrap_or(0) as u64),
                    rating: db_item.rating,
                    poster_url: db_item.poster_url,
                    backdrop_url: db_item.backdrop_url,
                    overview: db_item.overview,
                    genres: self.extract_genres_from_json(&db_item.genres),
                    cast: self.extract_cast_from_metadata(&db_item.metadata),
                    crew: self.extract_crew_from_metadata(&db_item.metadata),
                    added_at: db_item.added_at.map(|dt| dt.and_utc()),
                    updated_at: Some(db_item.updated_at.and_utc()),
                    watched: self.extract_watched_status(&db_item.metadata),
                    view_count: self
                        .extract_number_from_metadata(&db_item.metadata, "view_count")
                        .unwrap_or(0),
                    last_watched_at: self
                        .extract_datetime_from_metadata(&db_item.metadata, "last_watched_at"),
                    playback_position: self.extract_playback_position(&db_item.metadata),
                    intro_marker: None,
                    credits_marker: None,
                };
                Some(MediaItem::Movie(movie))
            }
            "show" => {
                let show = Show {
                    id: db_item.id,
                    backend_id: db_item.source_id,
                    title: db_item.title,
                    year: db_item.year.map(|y| y as u32),
                    seasons: Vec::new(), // TODO: Load seasons separately
                    rating: db_item.rating,
                    poster_url: db_item.poster_url,
                    backdrop_url: db_item.backdrop_url,
                    overview: db_item.overview,
                    genres: self.extract_genres_from_json(&db_item.genres),
                    cast: self.extract_cast_from_metadata(&db_item.metadata),
                    added_at: db_item.added_at.map(|dt| dt.and_utc()),
                    updated_at: Some(db_item.updated_at.and_utc()),
                    watched_episode_count: 0, // TODO: Get from progress tracking
                    total_episode_count: 0,   // TODO: Calculate from episodes
                    last_watched_at: None,    // TODO: Get from playback progress
                };
                Some(MediaItem::Show(show))
            }
            "episode" => {
                let episode = Episode {
                    id: db_item.id,
                    backend_id: db_item.source_id,
                    show_id: db_item.parent_id,
                    title: db_item.title,
                    season_number: self
                        .extract_number_from_metadata(&db_item.metadata, "season_number")
                        .unwrap_or(1),
                    episode_number: self
                        .extract_number_from_metadata(&db_item.metadata, "episode_number")
                        .unwrap_or(1),
                    duration: Duration::from_millis(db_item.duration_ms.unwrap_or(0) as u64),
                    thumbnail_url: db_item.poster_url, // Episodes use poster_url for thumbnail
                    overview: db_item.overview,
                    air_date: db_item.added_at.map(|dt| dt.and_utc()),
                    watched: self.extract_watched_status(&db_item.metadata),
                    view_count: self
                        .extract_number_from_metadata(&db_item.metadata, "view_count")
                        .unwrap_or(0),
                    last_watched_at: self
                        .extract_datetime_from_metadata(&db_item.metadata, "last_watched_at"),
                    playback_position: self.extract_playback_position(&db_item.metadata),
                    show_title: self.extract_from_metadata(&db_item.metadata, "show_title"),
                    show_poster_url: db_item.backdrop_url, // Use backdrop for show poster
                    intro_marker: None,
                    credits_marker: None,
                };
                Some(MediaItem::Episode(episode))
            }
            _ => {
                trace!("Unsupported media type: {}", db_item.media_type);
                None
            }
        }
    }

    fn extract_genres_from_json(&self, genres_json: &Option<Json>) -> Vec<String> {
        genres_json
            .as_ref()
            .and_then(|json| serde_json::from_value(json.clone()).ok())
            .unwrap_or_default()
    }

    fn extract_from_metadata(&self, metadata: &Option<Json>, key: &str) -> Option<String> {
        metadata
            .as_ref()
            .and_then(|json| {
                serde_json::from_value::<std::collections::HashMap<String, Value>>(json.clone())
                    .ok()
            })
            .and_then(|obj| obj.get(key).and_then(|v| v.as_str().map(String::from)))
    }

    fn extract_number_from_metadata(&self, metadata: &Option<Json>, key: &str) -> Option<u32> {
        metadata
            .as_ref()
            .and_then(|json| {
                serde_json::from_value::<std::collections::HashMap<String, Value>>(json.clone())
                    .ok()
            })
            .and_then(|obj| obj.get(key).cloned())
            .and_then(|v| {
                // Try as number first, then as string that can be parsed
                v.as_u64()
                    .map(|n| n as u32)
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })
    }

    fn extract_cast_from_metadata(&self, metadata: &Option<Json>) -> Vec<crate::models::Person> {
        metadata
            .as_ref()
            .and_then(|json| {
                serde_json::from_value::<std::collections::HashMap<String, Value>>(json.clone())
                    .ok()
            })
            .and_then(|obj| obj.get("cast").cloned())
            .and_then(|cast_val| {
                serde_json::from_value::<Vec<serde_json::Map<String, Value>>>(cast_val).ok()
            })
            .map(|cast_array| {
                cast_array
                    .into_iter()
                    .filter_map(|member| {
                        let name = member.get("name")?.as_str()?.to_string();
                        let character = member
                            .get("character")
                            .and_then(|c| c.as_str())
                            .map(String::from);
                        let profile_url = member
                            .get("profile_path")
                            .and_then(|p| p.as_str())
                            .map(String::from);

                        Some(crate::models::Person {
                            id: member
                                .get("id")
                                .and_then(|id| id.as_str())
                                .unwrap_or("")
                                .to_string(),
                            name,
                            role: character,
                            image_url: profile_url,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn extract_crew_from_metadata(&self, metadata: &Option<Json>) -> Vec<crate::models::Person> {
        metadata
            .as_ref()
            .and_then(|json| {
                serde_json::from_value::<std::collections::HashMap<String, Value>>(json.clone())
                    .ok()
            })
            .and_then(|obj| obj.get("crew").cloned())
            .and_then(|crew_val| {
                serde_json::from_value::<Vec<serde_json::Map<String, Value>>>(crew_val).ok()
            })
            .map(|crew_array| {
                crew_array
                    .into_iter()
                    .filter_map(|member| {
                        let name = member.get("name")?.as_str()?.to_string();
                        let job = member.get("job")?.as_str()?.to_string();
                        let department = member
                            .get("department")
                            .and_then(|d| d.as_str())
                            .map(String::from);
                        let profile_url = member
                            .get("profile_path")
                            .and_then(|p| p.as_str())
                            .map(String::from);

                        Some(crate::models::Person {
                            id: member
                                .get("id")
                                .and_then(|id| id.as_str())
                                .unwrap_or("")
                                .to_string(),
                            name,
                            role: Some(job),
                            image_url: profile_url,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn extract_watched_status(&self, metadata: &Option<Json>) -> bool {
        metadata
            .as_ref()
            .and_then(|json| {
                serde_json::from_value::<std::collections::HashMap<String, Value>>(json.clone())
                    .ok()
            })
            .and_then(|obj| obj.get("watched").cloned())
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    fn extract_datetime_from_metadata(
        &self,
        metadata: &Option<Json>,
        key: &str,
    ) -> Option<chrono::DateTime<chrono::Utc>> {
        metadata
            .as_ref()
            .and_then(|json| {
                serde_json::from_value::<std::collections::HashMap<String, Value>>(json.clone())
                    .ok()
            })
            .and_then(|obj| obj.get(key).cloned())
            .and_then(|v| v.as_str().map(String::from))
            .and_then(|date_str| chrono::DateTime::parse_from_rfc3339(&date_str).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc))
    }

    fn extract_playback_position(&self, metadata: &Option<Json>) -> Option<std::time::Duration> {
        metadata
            .as_ref()
            .and_then(|json| {
                serde_json::from_value::<std::collections::HashMap<String, Value>>(json.clone())
                    .ok()
            })
            .and_then(|obj| obj.get("playback_position_ms").cloned())
            .and_then(|v| v.as_u64())
            .map(std::time::Duration::from_millis)
    }

    pub async fn load_library(&self, backend_id: String, library: Library) {
        let start = Instant::now();
        info!("Loading library: {} ({})", library.title, library.id);

        let imp = self.imp();

        // Show loading state immediately when switching libraries
        if let Some(stack) = imp.stack.borrow().as_ref() {
            stack.set_visible_child_name("loading");
        }

        // Store backend ID and library
        imp.backend_id.replace(Some(backend_id.clone()));
        imp.library.replace(Some(library.clone()));

        // Use ViewModel for loading - no fallback
        if let Some(view_model) = imp.view_model.borrow().as_ref() {
            match view_model.set_library(library.id.clone()).await {
                Ok(_) => {
                    trace!("Library loaded via ViewModel");
                    // ViewModel handles everything through property bindings
                }
                Err(e) => {
                    error!("Failed to load library via ViewModel: {}", e);
                    // Show error state
                    if let Some(stack) = imp.stack.borrow().as_ref() {
                        stack.set_visible_child_name("empty");
                    }
                }
            }
        } else {
            error!("No ViewModel available for library loading");
            if let Some(stack) = imp.stack.borrow().as_ref() {
                stack.set_visible_child_name("empty");
            }
        }

        let elapsed = start.elapsed();
        if elapsed.as_millis() > 100 {
            warn!(
                "Slow library load: {:?} for library {}",
                elapsed, library.title
            );
        }
    }

    async fn preload_initial_images(&self, items: &[MediaItem]) {
        // Simplified: removed predictive preloading as it may cause issues
        // Images will load on-demand through trigger_load() instead
        trace!("Skipping predictive preload for {} items", items.len());
    }

    fn display_media_items(&self, items: Vec<MediaItem>) {
        let start = Instant::now();
        info!(
            "[PERF] display_media_items: Starting with {} items",
            items.len()
        );
        let imp = self.imp();

        // Store new filtered items
        let old_items = imp.filtered_items.borrow().clone();
        info!(
            "[PERF] Old items count: {}, New items count: {}",
            old_items.len(),
            items.len()
        );
        imp.filtered_items.replace(items.clone());

        let flow_box = imp.flow_box.borrow().clone();
        let scrolled_window = imp.scrolled_window.borrow().clone();
        let stack = imp.stack.borrow().clone();
        let current_size = *imp.current_view_size.borrow();

        if let Some(flow_box) = flow_box {
            if items.is_empty() {
                info!("[PERF] Items empty, clearing flow_box and showing empty state");
                // Clear and show empty state
                while let Some(child) = flow_box.first_child() {
                    flow_box.remove(&child);
                }
                imp.cards_by_index.borrow_mut().clear();
                imp.cards_by_id.borrow_mut().clear();

                // Show empty state
                if let Some(stack) = stack {
                    stack.set_visible_child_name("empty");
                }
            } else {
                // Perform differential update only if we have existing items
                if !old_items.is_empty() && self.should_use_differential_update(&old_items, &items)
                {
                    info!(
                        "[PERF] Using differential update for {} -> {} items",
                        old_items.len(),
                        items.len()
                    );
                    self.differential_update_items(&flow_box, &old_items, &items, current_size);
                } else {
                    // Full refresh for initial load or major changes
                    info!(
                        "[PERF] Using full refresh for {} items (old: {})",
                        items.len(),
                        old_items.len()
                    );
                    self.full_refresh_items(
                        &flow_box,
                        items.clone(),
                        current_size,
                        scrolled_window,
                        stack,
                    );
                }
            }
        } else {
            warn!("[PERF] No flow_box available!");
        }

        let elapsed = start.elapsed();
        warn!(
            "[PERF] display_media_items completed in {:?} for {} items (>16ms: {})",
            elapsed,
            items.len(),
            elapsed.as_millis() > 16
        );
    }

    /// Check if we should use differential update (for minor changes) or full refresh
    fn should_use_differential_update(
        &self,
        old_items: &[MediaItem],
        new_items: &[MediaItem],
    ) -> bool {
        // Use differential update for small changes to avoid flicker
        // Full refresh for major changes or different ordering

        // If more than 50% of items changed, do full refresh
        let changed_threshold = old_items.len() / 2;
        let mut changes = 0;

        for old_item in old_items {
            if !new_items.iter().any(|new| new.id() == old_item.id()) {
                changes += 1;
                if changes > changed_threshold {
                    return false;
                }
            }
        }

        true
    }

    /// Perform differential update - only add/remove changed items
    fn differential_update_items(
        &self,
        flow_box: &gtk4::FlowBox,
        old_items: &[MediaItem],
        new_items: &[MediaItem],
        current_size: ImageSize,
    ) {
        let start = Instant::now();
        info!(
            "[PERF] differential_update_items: old={}, new={}",
            old_items.len(),
            new_items.len()
        );
        let imp = self.imp();
        let mut cards_by_index = imp.cards_by_index.borrow_mut();
        let mut cards_by_id = imp.cards_by_id.borrow_mut();

        // Build lookup maps
        let old_ids: std::collections::HashSet<String> =
            old_items.iter().map(|item| item.id().to_string()).collect();
        let new_ids: std::collections::HashSet<String> =
            new_items.iter().map(|item| item.id().to_string()).collect();

        // Find items to remove
        let to_remove: Vec<String> = old_ids.difference(&new_ids).cloned().collect();
        info!("[PERF] Items to remove: {}", to_remove.len());

        // Find items to add
        let to_add: Vec<String> = new_ids.difference(&old_ids).cloned().collect();
        info!("[PERF] Items to add: {}", to_add.len());

        // Update existing cards' content for items present in both sets (e.g., progress changes)
        let common_ids: Vec<String> = old_ids.intersection(&new_ids).cloned().collect();
        info!("[PERF] Common items to update: {}", common_ids.len());
        let new_by_id: HashMap<String, &MediaItem> = new_items
            .iter()
            .map(|it| (it.id().to_string(), it))
            .collect();
        for id in &common_ids {
            if let Some(card) = cards_by_id.get(id)
                && let Some(new_item) = new_by_id.get(id)
            {
                card.update_content((*new_item).clone());
            }
        }

        // Remove cards for items that are no longer in the list
        if !to_remove.is_empty() {
            let remove_start = Instant::now();
            let mut removed_count = 0;
            let mut child = flow_box.first_child();
            while let Some(flow_child) = child {
                let next = flow_child.next_sibling();

                if let Some(fc) = flow_child.downcast_ref::<gtk4::FlowBoxChild>()
                    && let Some(card) = fc.child().and_then(|w| w.downcast::<MediaCard>().ok())
                {
                    let card_id = card.media_item().id().to_string();
                    if to_remove.contains(&card_id) {
                        flow_box.remove(&flow_child);
                        removed_count += 1;
                        // Remove from maps
                        cards_by_index.retain(|_, c| c.media_item().id() != card_id);
                        cards_by_id.remove(&card_id);
                    }
                }

                child = next;
            }
            let remove_elapsed = remove_start.elapsed();
            info!(
                "[PERF] Removed {} cards in {:?}",
                removed_count, remove_elapsed
            );
        }

        // Add new cards for items that weren't in the old list
        let add_start = Instant::now();
        let mut added_count = 0;
        for (idx, item) in new_items.iter().enumerate() {
            let item_id = item.id().to_string();
            if to_add.contains(&item_id) {
                added_count += 1;
                let card = MediaCard::new(item.clone(), current_size);

                // Connect click handler
                let view_weak = self.downgrade();
                let item_clone = item.clone();
                card.connect_clicked(move |_| {
                    if let Some(view) = view_weak.upgrade() {
                        info!("Media item selected: {}", item_clone.title());
                        if let Some(callback) = view.imp().on_media_selected.borrow().as_ref() {
                            callback(&item_clone);
                        }
                    }
                });

                let child = gtk4::FlowBoxChild::new();
                child.set_child(Some(&card));

                // Insert at correct position
                if idx < flow_box.observe_children().n_items() as usize {
                    flow_box.insert(&child, idx as i32);
                } else {
                    flow_box.append(&child);
                }

                cards_by_index.insert(idx, card.clone());
                cards_by_id.insert(item_id, card.clone());

                // Trigger load for new card
                card.trigger_load(current_size);
            }
        }
        if added_count > 0 {
            let add_elapsed = add_start.elapsed();
            info!("[PERF] Added {} cards in {:?}", added_count, add_elapsed);
        }

        // Reorder if needed (only if items are the same but order changed)
        if to_add.is_empty() && to_remove.is_empty() && old_items.len() == new_items.len() {
            // Check if order changed
            let order_changed = old_items
                .iter()
                .zip(new_items.iter())
                .any(|(old, new)| old.id() != new.id());

            if order_changed {
                // For reordering, it's simpler to rebuild
                self.full_refresh_items(flow_box, new_items.to_vec(), current_size, None, None);
            } else {
                // Ensure indices map matches current children
                drop(cards_by_index);
                self.rebuild_cards_index_map(flow_box);
            }
        } else {
            // After structural changes, rebuild index map
            drop(cards_by_index);
            self.rebuild_cards_index_map(flow_box);
        }

        // Show content
        if let Some(stack) = imp.stack.borrow().as_ref() {
            stack.set_visible_child_name("content");
        }

        let elapsed = start.elapsed();
        warn!(
            "[PERF] differential_update_items completed in {:?} (add: {}, remove: {}, update: {}) >16ms: {}",
            elapsed,
            to_add.len(),
            to_remove.len(),
            common_ids.len(),
            elapsed.as_millis() > 16
        );
    }

    /// Perform full refresh - clear and recreate all items
    fn full_refresh_items(
        &self,
        flow_box: &gtk4::FlowBox,
        items: Vec<MediaItem>,
        current_size: ImageSize,
        scrolled_window: Option<gtk4::ScrolledWindow>,
        stack: Option<gtk4::Stack>,
    ) {
        let start = Instant::now();
        info!(
            "[PERF] full_refresh_items: Starting with {} items",
            items.len()
        );
        let imp = self.imp();
        imp.cards_by_index.borrow_mut().clear();
        imp.cards_by_id.borrow_mut().clear();

        // Clear existing items
        let mut removed_count = 0;
        while let Some(child) = flow_box.first_child() {
            flow_box.remove(&child);
            removed_count += 1;
        }
        info!(
            "[PERF] Removed {} existing children from flow_box",
            removed_count
        );

        if !items.is_empty() {
            // Store items for lazy creation
            let items_rc = Rc::new(items);
            let cards_rc = Rc::new(RefCell::new(Vec::new()));

            // Function to create cards in batches
            let weak_self = self.downgrade();
            let flow_box_weak = flow_box.downgrade();
            let cards_for_create = cards_rc.clone();
            let items_for_create = items_rc.clone();

            let create_cards_batch = Rc::new(move |start: usize, end: usize| {
                if let Some(view) = weak_self.upgrade()
                    && let Some(flow_box) = flow_box_weak.upgrade()
                {
                    let mut cards = cards_for_create.borrow_mut();
                    let mut cards_by_index = view.imp().cards_by_index.borrow_mut();
                    let mut cards_by_id = view.imp().cards_by_id.borrow_mut();

                    for i in start..end.min(items_for_create.len()) {
                        if i >= cards.len() {
                            // Create card only if not already created
                            if let Some(item) = items_for_create.get(i) {
                                let card = MediaCard::new(item.clone(), current_size);

                                // Connect click handler to each card
                                let view_weak = view.downgrade();
                                let item_clone = card.media_item();
                                card.connect_clicked(move |_| {
                                    if let Some(view) = view_weak.upgrade() {
                                        info!("Media item selected: {}", item_clone.title());
                                        if let Some(callback) =
                                            view.imp().on_media_selected.borrow().as_ref()
                                        {
                                            callback(&item_clone);
                                        }
                                    }
                                });

                                let child = gtk4::FlowBoxChild::new();
                                child.set_child(Some(&card));
                                flow_box.append(&child);
                                let id_key = item.id().to_string();
                                cards.push(card.clone());
                                cards_by_index.insert(i, card.clone());
                                cards_by_id.insert(id_key, card);
                            }
                        }
                    }
                }
            });

            // Defer initial card creation to avoid blocking
            let create_initial = create_cards_batch.clone();
            let weak_self = self.downgrade();
            glib::idle_add_local_once(move || {
                info!("[PERF] Creating initial {} cards", INITIAL_CARDS_TO_CREATE);
                let create_start = Instant::now();
                create_initial(0, INITIAL_CARDS_TO_CREATE);
                let create_elapsed = create_start.elapsed();
                if create_elapsed.as_millis() > 16 {
                    warn!("[PERF] Initial card creation took {:?}", create_elapsed);
                }

                // Simplified: trigger load directly on created cards
                if let Some(view) = weak_self.upgrade() {
                    glib::timeout_add_local_once(
                        std::time::Duration::from_millis(100),
                        move || {
                            // Trigger load on initial visible cards
                            let load_start = Instant::now();
                            let cards_idx = view.imp().cards_by_index.borrow();
                            let load_count = INITIAL_IMAGES_TO_LOAD.min(cards_idx.len());
                            info!("[PERF] Triggering initial load for {} cards", load_count);
                            for i in 0..load_count {
                                if let Some(card) = cards_idx.get(&i) {
                                    card.trigger_load(ImageSize::Medium);
                                }
                            }
                            let load_elapsed = load_start.elapsed();
                            if load_elapsed.as_millis() > 16 {
                                warn!("[PERF] Initial image triggers took {:?}", load_elapsed);
                            }
                        },
                    );
                }
            });

            // Show content
            if let Some(ref stack) = stack {
                stack.set_visible_child_name("content");
            }

            // Set up progressive loading with card creation and batch loading
            if let Some(ref scrolled_window) = scrolled_window {
                self.setup_progressive_loading_with_batch(
                    scrolled_window.clone(),
                    flow_box.clone(),
                    create_cards_batch.clone(),
                    cards_rc.clone(),
                    items_rc.len(),
                );
            }
            // Ensure index map consistency after creation
            self.rebuild_cards_index_map(flow_box);
        }

        let elapsed = start.elapsed();
        warn!(
            "[PERF] full_refresh_items completed in {:?} (>32ms: {})",
            elapsed,
            elapsed.as_millis() > 32
        );
    }

    fn rebuild_cards_index_map(&self, flow_box: &gtk4::FlowBox) {
        let mut index_map: HashMap<usize, MediaCard> = HashMap::new();
        let mut idx = 0usize;
        let mut child = flow_box.first_child();
        while let Some(flow_child) = child {
            if let Some(fc) = flow_child.downcast_ref::<gtk4::FlowBoxChild>() {
                if let Some(card) = fc.child().and_then(|w| w.downcast::<MediaCard>().ok()) {
                    index_map.insert(idx, card);
                }
                idx += 1;
            }
            child = flow_child.next_sibling();
        }
        self.imp().cards_by_index.replace(index_map);
    }

    fn setup_progressive_loading(
        &self,
        scrolled_window: gtk4::ScrolledWindow,
        flow_box: gtk4::FlowBox,
    ) {
        let adjustment = scrolled_window.vadjustment();
        let update_counter: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let scroll_end_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

        adjustment.connect_value_changed(move |adj| {
            // Set scrolling state immediately
            IMAGE_LOADER.set_scrolling(true);
            trace!(
                "[PERF] Scroll event: scrolling set to true, viewport: {:.0}-{:.0}",
                adj.value(),
                adj.value() + adj.page_size()
            );

            let flow_box = flow_box.clone();
            let counter = update_counter.clone();
            let timer_ref = scroll_end_timer.clone();

            let viewport_top = adj.value();
            let viewport_height = adj.page_size();

            let current_count = {
                let mut c = counter.borrow_mut();
                *c += 1;
                *c
            };

            // Cancel previous timer
            if let Some(timer_id) = timer_ref.borrow_mut().take() {
                timer_id.remove();
            }

            // Set new timer to detect when scrolling stops
            let timer_id =
                glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
                    // Scrolling has stopped for 150ms
                    info!("[PERF] Scroll stopped after 150ms, loading images in viewport");
                    IMAGE_LOADER.set_scrolling(false);

                    let viewport_bottom = viewport_top + viewport_height;

                    // Load everything in a large buffer since images are small
                    let load_margin = viewport_height * 3.0; // Load 3 screens in each direction

                    let load_top = (viewport_top - load_margin).max(0.0);
                    let load_bottom = viewport_bottom + load_margin;

                    // Single pass loading - all at same quality
                    info!(
                        "[PERF] Loading cards in range: {:.0}-{:.0} (viewport: {:.0}-{:.0})",
                        load_top, load_bottom, viewport_top, viewport_bottom
                    );
                    Self::load_cards_in_range(&flow_box, load_top, load_bottom, ImageSize::Medium);

                    glib::ControlFlow::Break
                });

            *timer_ref.borrow_mut() = Some(timer_id);
        });
    }

    fn load_visible_cards(flow_box: &gtk4::FlowBox, max_items: usize) {
        let mut loaded = 0;
        let mut child = flow_box.first_child();

        while let Some(flow_child) = child {
            if loaded >= max_items {
                break;
            }

            if let Some(fc) = flow_child.downcast_ref::<gtk4::FlowBoxChild>()
                && let Some(card) = fc.child().and_then(|w| w.downcast::<MediaCard>().ok())
            {
                card.trigger_load(ImageSize::Medium);
                loaded += 1;
            }

            child = flow_child.next_sibling();
        }
    }

    fn load_cards_in_range(
        flow_box: &gtk4::FlowBox,
        visible_top: f64,
        visible_bottom: f64,
        size: ImageSize,
    ) {
        let start = Instant::now();
        let mut child = flow_box.first_child();
        let mut cards_to_load = Vec::new();
        let mut total_cards = 0;

        while let Some(flow_child) = child {
            if let Some(fc) = flow_child.downcast_ref::<gtk4::FlowBoxChild>() {
                total_cards += 1;
                // Use natural size for visibility calculations
                let (_, natural) = fc.preferred_size();
                let child_height = natural.height() as f64;

                // Approximate position based on index
                let index = fc.index() as f64;
                let approx_row = (index / 6.0).floor(); // Estimate based on typical columns
                let child_top = approx_row * (child_height + 20.0); // Include row spacing
                let child_bottom = child_top + child_height;

                if child_bottom >= visible_top
                    && child_top <= visible_bottom
                    && let Some(card) = fc.child().and_then(|w| w.downcast::<MediaCard>().ok())
                {
                    cards_to_load.push((card, size));
                }
            }

            child = flow_child.next_sibling();
        }

        info!(
            "[PERF] load_cards_in_range: Loading {} of {} cards in range {:.0}-{:.0}",
            cards_to_load.len(),
            total_cards,
            visible_top,
            visible_bottom
        );

        // Batch load for efficiency
        for (card, size) in &cards_to_load {
            card.trigger_load(*size);
        }

        let elapsed = start.elapsed();
        if elapsed.as_millis() > 16 {
            warn!(
                "[PERF] load_cards_in_range took {:?} to trigger {} loads",
                elapsed,
                cards_to_load.len()
            );
        }
    }

    pub fn navigate_back(&self) {
        info!("Navigating back to libraries");
        let mut widget: Option<gtk4::Widget> = self.parent();
        while let Some(w) = widget {
            if w.type_() == crate::platforms::gtk::ui::main_window::ReelMainWindow::static_type() {
                if let Some(window) =
                    w.downcast_ref::<crate::platforms::gtk::ui::main_window::ReelMainWindow>()
                {
                    window.show_libraries_view();
                }
                break;
            }
            widget = w.parent();
        }
    }

    pub fn apply_filters(&self) {
        // Filters are now handled by the ViewModel
        // This method is kept for backward compatibility but does nothing
        info!("apply_filters called - now handled by ViewModel");
    }

    pub fn update_watch_status_filter(&self, status: WatchStatus) {
        if let Some(view_model) = self.imp().view_model.borrow().as_ref() {
            let vm_status = match status {
                WatchStatus::All => crate::platforms::gtk::ui::viewmodels::library_view_model::WatchStatus::All,
                WatchStatus::Watched => {
                    crate::platforms::gtk::ui::viewmodels::library_view_model::WatchStatus::Watched
                }
                WatchStatus::Unwatched => {
                    crate::platforms::gtk::ui::viewmodels::library_view_model::WatchStatus::Unwatched
                }
                WatchStatus::InProgress => {
                    crate::platforms::gtk::ui::viewmodels::library_view_model::WatchStatus::InProgress
                }
            };
            let vm_clone = view_model.clone();
            glib::spawn_future_local(async move {
                vm_clone.set_watch_status(vm_status).await;
            });
        }
    }

    pub fn update_sort_order(&self, order: crate::platforms::gtk::ui::filters::SortOrder) {
        if let Some(view_model) = self.imp().view_model.borrow().as_ref() {
            let vm_sort_order = match order {
                crate::platforms::gtk::ui::filters::SortOrder::TitleAsc => {
                    crate::platforms::gtk::ui::viewmodels::library_view_model::SortOrder::TitleAsc
                }
                crate::platforms::gtk::ui::filters::SortOrder::TitleDesc => {
                    crate::platforms::gtk::ui::viewmodels::library_view_model::SortOrder::TitleDesc
                }
                crate::platforms::gtk::ui::filters::SortOrder::YearAsc => {
                    crate::platforms::gtk::ui::viewmodels::library_view_model::SortOrder::YearAsc
                }
                crate::platforms::gtk::ui::filters::SortOrder::YearDesc => {
                    crate::platforms::gtk::ui::viewmodels::library_view_model::SortOrder::YearDesc
                }
                crate::platforms::gtk::ui::filters::SortOrder::RatingAsc => {
                    crate::platforms::gtk::ui::viewmodels::library_view_model::SortOrder::RatingAsc
                }
                crate::platforms::gtk::ui::filters::SortOrder::RatingDesc => {
                    crate::platforms::gtk::ui::viewmodels::library_view_model::SortOrder::RatingDesc
                }
                crate::platforms::gtk::ui::filters::SortOrder::DateAddedAsc => {
                    crate::platforms::gtk::ui::viewmodels::library_view_model::SortOrder::AddedAsc
                }
                crate::platforms::gtk::ui::filters::SortOrder::DateAddedDesc => {
                    crate::platforms::gtk::ui::viewmodels::library_view_model::SortOrder::AddedDesc
                }
                crate::platforms::gtk::ui::filters::SortOrder::DateWatchedAsc => {
                    crate::platforms::gtk::ui::viewmodels::library_view_model::SortOrder::AddedAsc
                } // Fallback to AddedAsc
                crate::platforms::gtk::ui::filters::SortOrder::DateWatchedDesc => {
                    crate::platforms::gtk::ui::viewmodels::library_view_model::SortOrder::AddedDesc
                } // Fallback to AddedDesc
            };
            let vm_clone = view_model.clone();
            glib::spawn_future_local(async move {
                vm_clone.set_sort_order(vm_sort_order).await;
            });
        }
    }

    pub fn get_filter_manager(&self) -> std::cell::Ref<FilterManager> {
        self.imp().filter_manager.borrow()
    }

    pub fn search(&self, query: String) {
        if let Some(view_model) = self.imp().view_model.borrow().as_ref() {
            let vm_clone = view_model.clone();
            glib::spawn_future_local(async move {
                vm_clone.search(query).await;
            });
        }
    }

    pub fn refresh(&self) {
        if let Some(view_model) = self.imp().view_model.borrow().as_ref() {
            let vm_clone = view_model.clone();
            glib::spawn_future_local(async move {
                use crate::platforms::gtk::ui::viewmodels::ViewModel;
                let _ = vm_clone.refresh().await;
            });
        }
    }

    /// Simplified: Just trigger load on visible cards
    fn batch_load_visible_cards(&self, start_idx: usize, end_idx: usize) {
        let cards_by_index = self.imp().cards_by_index.borrow();
        let current_size = *self.imp().current_view_size.borrow();

        // Simply trigger load on each visible card
        for i in start_idx..end_idx {
            if let Some(card) = cards_by_index.get(&i) {
                card.trigger_load(current_size);
            }
        }

        trace!("Triggered load for cards {}-{}", start_idx, end_idx);
    }

    fn setup_progressive_loading_with_batch(
        &self,
        scrolled_window: gtk4::ScrolledWindow,
        flow_box: gtk4::FlowBox,
        create_cards: Rc<dyn Fn(usize, usize)>,
        cards_rc: Rc<RefCell<Vec<MediaCard>>>,
        total_items: usize,
    ) {
        let adjustment = scrolled_window.vadjustment();
        let update_counter: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let weak_self = self.downgrade();

        adjustment.connect_value_changed(move |adj| {
            let flow_box = flow_box.clone();
            let counter = update_counter.clone();
            let cards_rc = cards_rc.clone();
            let create_cards = create_cards.clone();
            let weak_self = weak_self.clone();

            let viewport_top = adj.value();
            let viewport_height = adj.page_size();

            let current_count = {
                let mut c = counter.borrow_mut();
                *c += 1;
                *c
            };

            // Moderate delay to balance responsiveness and performance
            let counter_inner = counter.clone();
            glib::timeout_add_local(
                std::time::Duration::from_millis(SCROLL_DEBOUNCE_MS),
                move || {
                    if *counter_inner.borrow() != current_count {
                        return glib::ControlFlow::Break;
                    }

                    let viewport_bottom = viewport_top + viewport_height;

                    // Calculate which cards should be visible
                    let card_height = 270.0; // Approximate height for medium cards
                    let cards_per_row = 6.0; // Approximate
                    let row_height = card_height + 20.0; // Include spacing

                    let start_row = ((viewport_top - IMAGE_VIEWPORT_BUFFER) / row_height)
                        .floor()
                        .max(0.0) as usize;
                    let end_row =
                        ((viewport_bottom + IMAGE_VIEWPORT_BUFFER) / row_height).ceil() as usize;

                    let start_idx = start_row * cards_per_row as usize;
                    let end_idx = ((end_row + 1) * cards_per_row as usize).min(total_items);

                    // Create cards if needed - reasonable batch size
                    let current_card_count = cards_rc.borrow().len();
                    if end_idx > current_card_count {
                        let batch_size = (end_idx - current_card_count).min(CARD_BATCH_SIZE);
                        create_cards(current_card_count, current_card_count + batch_size);
                    }

                    // Batch load images for visible cards
                    if let Some(view) = weak_self.upgrade() {
                        view.batch_load_visible_cards(start_idx, end_idx);
                    }

                    glib::ControlFlow::Break
                },
            );
        });
    }
}

// Optimized Media Card Widget
mod imp_card {
    use super::*;
    use glib::source::SourceId;

    #[derive(Debug)]
    pub struct MediaCard {
        pub overlay: RefCell<Option<gtk4::Overlay>>,
        pub image: RefCell<Option<gtk4::Picture>>,
        pub info_box: RefCell<Option<gtk4::Box>>,
        pub title_label: RefCell<Option<gtk4::Label>>,
        pub subtitle_label: RefCell<Option<gtk4::Label>>,
        pub media_item: RefCell<Option<MediaItem>>,
        pub loading_spinner: RefCell<Option<gtk4::Spinner>>,
        pub watched_indicator: RefCell<Option<gtk4::Box>>,
        pub progress_bar: RefCell<Option<gtk4::ProgressBar>>,
        pub image_loaded: RefCell<bool>,
        pub image_loading: RefCell<bool>,
        pub load_handle: RefCell<Option<SourceId>>,
        pub current_size: RefCell<ImageSize>,
        pub default_size: RefCell<ImageSize>,
    }

    impl Default for MediaCard {
        fn default() -> Self {
            Self {
                overlay: RefCell::new(None),
                image: RefCell::new(None),
                info_box: RefCell::new(None),
                title_label: RefCell::new(None),
                subtitle_label: RefCell::new(None),
                media_item: RefCell::new(None),
                loading_spinner: RefCell::new(None),
                watched_indicator: RefCell::new(None),
                progress_bar: RefCell::new(None),
                image_loaded: RefCell::new(false),
                image_loading: RefCell::new(false),
                load_handle: RefCell::new(None),
                current_size: RefCell::new(ImageSize::Medium),
                default_size: RefCell::new(ImageSize::Medium),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MediaCard {
        const NAME: &'static str = "MediaCard";
        type Type = super::MediaCard;
        type ParentType = gtk4::Button;
    }

    impl ObjectImpl for MediaCard {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_card_ui();
        }
    }

    impl WidgetImpl for MediaCard {}
    impl ButtonImpl for MediaCard {}
}

glib::wrapper! {
    pub struct MediaCard(ObjectSubclass<imp_card::MediaCard>)
        @extends gtk4::Widget, gtk4::Button,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Actionable;
}

impl MediaCard {
    pub fn new(media_item: MediaItem, default_size: ImageSize) -> Self {
        let card: Self = glib::Object::builder().build();

        card.imp().media_item.replace(Some(media_item.clone()));
        card.imp().default_size.replace(default_size);
        card.imp().current_size.replace(default_size);
        card.update_content(media_item);
        card
    }

    fn setup_card_ui(&self) {
        let imp = self.imp();

        self.add_css_class("flat");
        self.add_css_class("media-card");
        self.add_css_class("poster-card");

        let overlay = gtk4::Overlay::new();
        overlay.add_css_class("poster-overlay");

        // Dynamic sizing based on current size setting
        let size = *imp.default_size.borrow();
        let (width, height) = size.dimensions_for_poster();

        let image = gtk4::Picture::builder()
            .width_request(width as i32)
            .height_request(height as i32)
            .content_fit(gtk4::ContentFit::Cover)
            .build();

        image.add_css_class("rounded-poster");

        self.set_placeholder_image(&image);

        overlay.set_child(Some(&image));

        // Loading spinner overlay
        let spinner_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .build();

        let loading_spinner = gtk4::Spinner::builder()
            .spinning(false)
            .width_request(24)
            .height_request(24)
            .build();

        spinner_box.append(&loading_spinner);
        overlay.add_overlay(&spinner_box);

        // Info box with gradient background
        let info_wrapper = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .valign(gtk4::Align::End)
            .build();

        info_wrapper.add_css_class("poster-info-gradient");

        // Inner box with padding for text
        let info_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(2)
            .margin_bottom(8)
            .margin_start(8)
            .margin_end(8)
            .margin_top(8)
            .build();

        info_box.add_css_class("media-card-info");

        let title_classes = if size == ImageSize::Small {
            vec!["caption", "bold"]
        } else {
            vec!["title-4"]
        };

        let title_label = gtk4::Label::builder()
            .xalign(0.0)
            .single_line_mode(true)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .css_classes(title_classes)
            .build();

        let subtitle_label = gtk4::Label::builder()
            .xalign(0.0)
            .single_line_mode(true)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .css_classes(vec!["subtitle"])
            .visible(size != ImageSize::Small) // Hide subtitle on small cards
            .build();

        info_box.append(&title_label);
        info_box.append(&subtitle_label);

        info_wrapper.append(&info_box);
        overlay.add_overlay(&info_wrapper);

        // Add unwatched indicator overlay (top-right corner glowing dot)
        let unwatched_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .halign(gtk4::Align::End)
            .valign(gtk4::Align::Start)
            .margin_top(8)
            .margin_end(8)
            .visible(false)
            .build();

        // Create a glowing dot indicator for unwatched content
        let unwatched_dot = gtk4::Box::builder()
            .width_request(14)
            .height_request(14)
            .build();

        unwatched_dot.add_css_class("unwatched-glow-dot");
        unwatched_box.append(&unwatched_dot);
        unwatched_box.add_css_class("unwatched-indicator");

        overlay.add_overlay(&unwatched_box);

        // Add progress bar overlay (bottom)
        let progress_bar = gtk4::ProgressBar::builder()
            .valign(gtk4::Align::End)
            .visible(false)
            .build();

        progress_bar.add_css_class("media-progress");
        overlay.add_overlay(&progress_bar);

        self.set_child(Some(&overlay));

        // Store references
        imp.overlay.replace(Some(overlay));
        imp.image.replace(Some(image));
        imp.info_box.replace(Some(info_box));
        imp.title_label.replace(Some(title_label));
        imp.subtitle_label.replace(Some(subtitle_label));
        imp.loading_spinner.replace(Some(loading_spinner));
        imp.watched_indicator.replace(Some(unwatched_box));
        imp.progress_bar.replace(Some(progress_bar));
    }

    fn set_placeholder_image(&self, picture: &gtk4::Picture) {
        // Don't set any icon - just leave it empty for a cleaner look
        // The rounded poster frame with gradient overlay will be enough
        picture.set_paintable(None::<&gdk::Paintable>);
    }

    fn update_content(&self, media_item: MediaItem) {
        let imp = self.imp();

        // For episodes, show the show name as title and episode info as subtitle
        if let MediaItem::Episode(ref episode) = media_item {
            if let Some(title_label) = imp.title_label.borrow().as_ref() {
                // Use show title if available, otherwise fall back to episode title
                if let Some(ref show_title) = episode.show_title {
                    title_label.set_text(show_title);
                } else {
                    title_label.set_text(&episode.title);
                }
            }

            if let Some(subtitle_label) = imp.subtitle_label.borrow().as_ref() {
                // Format: "S1E5 • Episode Title"
                let subtitle = format!(
                    "S{}E{} • {}",
                    episode.season_number, episode.episode_number, episode.title
                );
                subtitle_label.set_text(&subtitle);
            }
        } else {
            // For non-episodes, use the regular title
            if let Some(title_label) = imp.title_label.borrow().as_ref() {
                title_label.set_text(media_item.title());
            }

            if let Some(subtitle_label) = imp.subtitle_label.borrow().as_ref() {
                let subtitle = match &media_item {
                    MediaItem::Movie(movie) => {
                        if let Some(year) = movie.year {
                            format!("{}", year)
                        } else {
                            String::new()
                        }
                    }
                    MediaItem::Show(show) => {
                        // If seasons aren't loaded yet (homepage), use episode count if available
                        if show.seasons.is_empty() {
                            if show.total_episode_count > 0 {
                                let episodes = show.total_episode_count;
                                if episodes == 1 {
                                    "1 episode".to_string()
                                } else {
                                    format!("{} episodes", episodes)
                                }
                            } else {
                                "TV Series".to_string() // Fallback when no info available
                            }
                        } else {
                            let season_count = show.seasons.len();
                            if season_count == 1 {
                                "1 season".to_string()
                            } else {
                                format!("{} seasons", season_count)
                            }
                        }
                    }
                    _ => String::new(),
                };
                subtitle_label.set_text(&subtitle);
            }
        }

        // Update unwatched indicator - show for unwatched items
        if let Some(unwatched_indicator) = imp.watched_indicator.borrow().as_ref() {
            unwatched_indicator.set_visible(!media_item.is_watched());
        }

        // Update progress bar
        if let Some(progress_bar) = imp.progress_bar.borrow().as_ref() {
            if let Some(progress) = media_item.watch_progress() {
                // Only show progress bar if partially watched
                if media_item.is_partially_watched() {
                    progress_bar.set_fraction(progress as f64);
                    progress_bar.set_visible(true);
                } else {
                    progress_bar.set_visible(false);
                }
            } else {
                progress_bar.set_visible(false);
            }
        }
    }

    pub fn set_loaded_texture(&self, texture: gdk::Texture) {
        let imp = self.imp();

        // Set the texture directly
        if let Some(ref image) = *imp.image.borrow() {
            image.set_paintable(Some(&texture));
            imp.image_loaded.replace(true);

            // Hide loading spinner
            if let Some(ref spinner) = *imp.loading_spinner.borrow() {
                spinner.set_spinning(false);
                spinner.set_visible(false);
            }
        }
    }

    pub fn trigger_load(&self, size: ImageSize) {
        let imp = self.imp();

        // Update requested size if different
        let current_size = *imp.current_size.borrow();
        if current_size != size {
            imp.current_size.replace(size);
        }

        // Check if already loaded at this size or loading
        if *imp.image_loaded.borrow() && current_size == size {
            trace!("[PERF] Card already loaded, skipping");
            return;
        }

        if *imp.image_loading.borrow() {
            trace!("[PERF] Card already loading, skipping");
            return;
        }

        *imp.image_loading.borrow_mut() = true;

        if let Some(media_item) = imp.media_item.borrow().as_ref() {
            trace!("[PERF] Triggering image load for: {}", media_item.title());
            self.load_poster_image(media_item, size);
        }
    }

    fn load_poster_image(&self, media_item: &MediaItem, size: ImageSize) {
        let poster_url = match media_item {
            MediaItem::Movie(movie) => movie.poster_url.clone(),
            MediaItem::Show(show) => show.poster_url.clone(),
            MediaItem::Episode(episode) => {
                // Use show poster if available, otherwise fall back to episode thumbnail
                episode
                    .show_poster_url
                    .clone()
                    .or(episode.thumbnail_url.clone())
            }
            _ => None,
        };

        if let Some(url) = poster_url {
            let imp = self.imp();

            // Don't show spinner immediately to avoid flicker for cached images
            let image_ref = imp.image.borrow().as_ref().unwrap().clone();
            let spinner_ref = imp.loading_spinner.borrow().as_ref().unwrap().clone();
            let weak_self = self.downgrade();

            // Load image in background thread pool to avoid blocking main thread
            let url_clone = url.clone();
            let (tx, rx) = tokio::sync::oneshot::channel();

            // Spawn the download task on tokio runtime
            tokio::spawn(async move {
                let result = IMAGE_LOADER.load_image(&url_clone, size).await;
                let _ = tx.send(result);
            });

            // Handle the result on the main thread
            glib::spawn_future_local(async move {
                let load_start = Instant::now();
                if let Ok(result) = rx.await {
                    match result {
                        Ok(texture) => {
                            let load_elapsed = load_start.elapsed();
                            if load_elapsed.as_millis() > 100 {
                                trace!("[PERF] Slow image load: {:?} for {}", load_elapsed, url);
                            }

                            // Update UI with low priority to not block scrolling
                            let image_ref = image_ref.clone();
                            let spinner_ref = spinner_ref.clone();
                            let weak_self = weak_self.clone();

                            glib::idle_add_local_full(glib::Priority::LOW, move || {
                                if let Some(card) = weak_self.upgrade() {
                                    image_ref.set_paintable(Some(&texture));
                                    spinner_ref.set_spinning(false);
                                    spinner_ref.set_visible(false);

                                    let imp = card.imp();
                                    *imp.image_loaded.borrow_mut() = true;
                                    *imp.image_loading.borrow_mut() = false;
                                }
                                glib::ControlFlow::Break
                            });
                        }
                        Err(e) => {
                            trace!("[PERF] Image load failed: {}", e);
                            let weak_self = weak_self.clone();
                            glib::idle_add_local_once(move || {
                                if let Some(card) = weak_self.upgrade() {
                                    let imp = card.imp();
                                    *imp.image_loading.borrow_mut() = false;
                                }
                            });
                        }
                    }
                }
            });
        }
    }

    pub fn media_item(&self) -> MediaItem {
        self.imp().media_item.borrow().as_ref().unwrap().clone()
    }
}
