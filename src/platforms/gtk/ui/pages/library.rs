use gtk4::{gdk, glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use once_cell::sync::Lazy;
use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, trace, warn};

use crate::core::viewmodels::{ComputedProperty, Property, ViewModel};
use crate::models::{Library, MediaItem};
use crate::platforms::gtk::ui::filters::{FilterManager, WatchStatus};
use crate::platforms::gtk::ui::reactive::bindings;
use crate::platforms::gtk::ui::viewmodels::library_view_model::LibraryViewModel;
use crate::state::AppState;
use crate::utils::{ImageLoader, ImageSize};
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
        pub search_entry: RefCell<Option<gtk4::SearchEntry>>,

        pub state: RefCell<Option<Arc<AppState>>>,
        pub library: RefCell<Option<Library>>,
        pub backend_id: RefCell<Option<String>>,
        pub current_view_size: RefCell<ImageSize>,
        pub on_media_selected: RefCell<Option<Box<dyn Fn(&MediaItem)>>>,
        pub filter_manager: RefCell<FilterManager>,
        // Legacy RefCell containers removed - using reactive bindings instead
        pub view_model: RefCell<Option<Arc<LibraryViewModel>>>,
        // Stage 1: Add reactive properties for UI interactions
        pub search_query: Property<String>,
        pub watch_status: Property<crate::core::viewmodels::library_view_model::WatchStatus>,
        pub sort_order: Property<crate::core::viewmodels::library_view_model::SortOrder>,
        pub update_scheduled: Cell<bool>,
        pub pending_items: RefCell<Option<Vec<MediaItem>>>,
        // Stage 2: Computed properties for UI state
        pub stack_state: RefCell<Option<ComputedProperty<String>>>,
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
                search_entry: RefCell::new(None),
                state: RefCell::new(None),
                library: RefCell::new(None),
                backend_id: RefCell::new(None),
                current_view_size: RefCell::new(ImageSize::Medium),
                on_media_selected: RefCell::new(None),
                filter_manager: RefCell::new(FilterManager::new()),
                // Initialize search query property
                search_query: Property::new(String::new(), "search_query"),
                watch_status: Property::new(
                    crate::core::viewmodels::library_view_model::WatchStatus::All,
                    "watch_status",
                ),
                sort_order: Property::new(
                    crate::core::viewmodels::library_view_model::SortOrder::TitleAsc,
                    "sort_order",
                ),
                view_model: RefCell::new(None),
                update_scheduled: Cell::new(false),
                pending_items: RefCell::new(None),
                // Stage 2: Computed properties for UI state
                stack_state: RefCell::new(None),
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
                vm.initialize(event_bus).await;
            }
        });

        // Subscribe to ViewModel property changes
        view.setup_viewmodel_bindings(view_model.clone());

        // Setup reactive FlowBox binding
        view.setup_reactive_flowbox_binding(view_model.clone());

        // Stage 1: Setup reactive search with debouncing
        view.setup_reactive_search(view_model.clone());

        // Stage 3: Setup two-way search entry binding
        view.setup_search_entry_binding();

        // Stage 1: Setup reactive watch status and sort order properties
        view.setup_reactive_filters(view_model.clone());

        // Stage 2: Setup computed properties for UI state
        view.setup_computed_properties(view_model.clone());

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

        // Create search entry
        let search_entry = gtk4::SearchEntry::builder()
            .placeholder_text("Search movies...")
            .margin_start(12)
            .margin_end(12)
            .margin_top(12)
            .margin_bottom(6)
            .build();

        // Add search entry and stack to the view
        self.append(&search_entry);
        self.append(&stack);

        // Store references
        imp.scrolled_window.replace(Some(scrolled_window));
        imp.flow_box.replace(Some(flow_box.clone()));
        imp.loading_spinner.replace(Some(loading_spinner));
        imp.empty_state.replace(Some(empty_state));
        imp.stack.replace(Some(stack));
        imp.search_entry.replace(Some(search_entry.clone()));

        // We'll connect button clicks directly on cards instead of flow box activation
    }

    /// Stage 3: Setup reactive FlowBox binding to replace manual display_media_items()
    fn setup_reactive_flowbox_binding(&self, view_model: Arc<LibraryViewModel>) {
        if let Some(flow_box) = self.imp().flow_box.borrow().as_ref() {
            let filtered_items_property = view_model.filtered_items();

            // Create card factory that connects click handlers
            let weak_self_for_factory = self.downgrade();
            let card_factory = move |item: &MediaItem, size: ImageSize| {
                let card = MediaCard::new(item.clone(), size);

                // Connect click handler
                let view_for_callback = weak_self_for_factory.clone();
                let item_clone = item.clone();
                card.connect_clicked(move |_| {
                    if let Some(view) = view_for_callback.upgrade() {
                        info!("Media item selected: {}", item_clone.title());
                        if let Some(callback) = view.imp().on_media_selected.borrow().as_ref() {
                            callback(&item_clone);
                        }
                    }
                });

                card
            };

            // Setup reactive binding to replace manual display_media_items()
            let _binding_handle = bindings::bind_flowbox_to_media_items(
                flow_box,
                &filtered_items_property,
                card_factory,
            );

            info!(
                "[REACTIVE] Setup reactive FlowBox binding to replace manual display_media_items()"
            );
        }
    }

    fn setup_viewmodel_bindings(&self, view_model: Arc<LibraryViewModel>) {
        // NOTE: Filtered items subscription removed - now handled by reactive FlowBox binding

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

    /// Stage 1: Setup reactive search with debouncing
    fn setup_reactive_search(&self, view_model: Arc<LibraryViewModel>) {
        let search_property = &self.imp().search_query;

        // Create debounced search property (300ms delay)
        let debounced_search = search_property.debounce(Duration::from_millis(300));

        // Filter out empty/short queries
        let filtered_search =
            debounced_search.filter(|query| !query.is_empty() && query.len() >= 2);

        // Subscribe to debounced, filtered search changes
        let mut search_subscriber = filtered_search.subscribe();
        glib::spawn_future_local(async move {
            while search_subscriber.wait_for_change().await {
                if let Some(query) = filtered_search.get_sync() {
                    // Send to ViewModel
                    view_model.search(query).await;
                }
            }
        });

        info!("[REACTIVE] Setup debounced search with 300ms delay and min 2 chars");
    }

    /// Stage 3: Setup two-way binding between search entry widget and reactive property
    fn setup_search_entry_binding(&self) {
        if let Some(search_entry) = self.imp().search_entry.borrow().as_ref() {
            let search_query_property = &self.imp().search_query;

            // Use the two-way binding utility
            let _binding_handle =
                bindings::bind_search_entry_two_way(search_entry, search_query_property);

            info!("[REACTIVE] Setup two-way search entry binding");
        } else {
            warn!("[REACTIVE] Could not setup search entry binding - widget not found");
        }
    }

    /// Stage 1: Setup reactive watch status and sort order filters
    fn setup_reactive_filters(&self, view_model: Arc<LibraryViewModel>) {
        let watch_status_property = &self.imp().watch_status;
        let sort_order_property = &self.imp().sort_order;

        // Subscribe to watch status changes
        let mut watch_status_subscriber = watch_status_property.subscribe();
        let watch_status_property_clone = watch_status_property.clone();
        let view_model_watch = view_model.clone();
        glib::spawn_future_local(async move {
            while watch_status_subscriber.wait_for_change().await {
                let status = watch_status_property_clone.get_sync();
                // Send to ViewModel
                view_model_watch.set_watch_status(status).await;
            }
        });

        // Subscribe to sort order changes
        let mut sort_order_subscriber = sort_order_property.subscribe();
        let sort_order_property_clone = sort_order_property.clone();
        let view_model_sort = view_model.clone();
        glib::spawn_future_local(async move {
            while sort_order_subscriber.wait_for_change().await {
                let order = sort_order_property_clone.get_sync();
                // Send to ViewModel
                view_model_sort.set_sort_order(order).await;
            }
        });

        info!("[REACTIVE] Setup reactive watch status and sort order filters");
    }

    /// Stage 2: Setup computed properties for UI state
    fn setup_computed_properties(&self, view_model: Arc<LibraryViewModel>) {
        // Create computed property for stack state
        let is_loading_property = view_model.is_loading();
        let filtered_items_property = view_model.filtered_items();

        // Convert to Arc<dyn PropertyLike> for ComputedProperty
        let is_loading_arc: Arc<dyn crate::core::viewmodels::PropertyLike> =
            Arc::new(is_loading_property.clone());
        let filtered_items_arc: Arc<dyn crate::core::viewmodels::PropertyLike> =
            Arc::new(filtered_items_property.clone());

        let stack_state =
            ComputedProperty::new("stack_state", vec![is_loading_arc, filtered_items_arc], {
                let is_loading_clone = is_loading_property.clone();
                let filtered_items_clone = filtered_items_property.clone();
                move || {
                    let is_loading = is_loading_clone.get_sync();
                    let filtered_items = filtered_items_clone.get_sync();

                    if is_loading {
                        "loading".to_string()
                    } else if filtered_items.is_empty() {
                        "empty".to_string()
                    } else {
                        "content".to_string()
                    }
                }
            });

        // Get initial stack state before moving stack_state
        let initial_stack_state = stack_state.get_sync();

        // Subscribe to stack state changes and update UI before storing
        let weak_self = self.downgrade();
        let mut stack_subscriber = stack_state.subscribe();

        self.imp().stack_state.replace(Some(stack_state));
        glib::spawn_future_local(async move {
            while stack_subscriber.wait_for_change().await {
                if let Some(view) = weak_self.upgrade()
                    && let Some(stack_state_prop) = view.imp().stack_state.borrow().as_ref()
                {
                    let stack_child_name = stack_state_prop.get_sync();
                    if let Some(stack) = view.imp().stack.borrow().as_ref() {
                        stack.set_visible_child_name(&stack_child_name);
                        info!("[REACTIVE] Stack state changed to: {}", stack_child_name);
                    }
                }
            }
        });

        // Set initial stack state
        if let Some(stack) = self.imp().stack.borrow().as_ref() {
            stack.set_visible_child_name(&initial_stack_state);
            info!(
                "[REACTIVE] Initial stack state set to: {}",
                initial_stack_state
            );
        }

        info!("[REACTIVE] Setup computed property for stack state");
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
        // Stage 1: Use reactive property instead of calling ViewModel directly
        let vm_status = match status {
            WatchStatus::All => crate::core::viewmodels::library_view_model::WatchStatus::All,
            WatchStatus::Watched => {
                crate::core::viewmodels::library_view_model::WatchStatus::Watched
            }
            WatchStatus::Unwatched => {
                crate::core::viewmodels::library_view_model::WatchStatus::Unwatched
            }
            WatchStatus::InProgress => {
                crate::core::viewmodels::library_view_model::WatchStatus::InProgress
            }
        };

        // Set the reactive property - this will trigger the ViewModel update
        let watch_status_property = &self.imp().watch_status;
        glib::spawn_future_local({
            let watch_status_property = watch_status_property.clone();
            async move {
                watch_status_property.set(vm_status).await;
            }
        });
    }

    pub fn update_sort_order(&self, order: crate::platforms::gtk::ui::filters::SortOrder) {
        // Stage 1: Use reactive property instead of calling ViewModel directly
        let vm_sort_order = match order {
            crate::platforms::gtk::ui::filters::SortOrder::TitleAsc => {
                crate::core::viewmodels::library_view_model::SortOrder::TitleAsc
            }
            crate::platforms::gtk::ui::filters::SortOrder::TitleDesc => {
                crate::core::viewmodels::library_view_model::SortOrder::TitleDesc
            }
            crate::platforms::gtk::ui::filters::SortOrder::YearAsc => {
                crate::core::viewmodels::library_view_model::SortOrder::YearAsc
            }
            crate::platforms::gtk::ui::filters::SortOrder::YearDesc => {
                crate::core::viewmodels::library_view_model::SortOrder::YearDesc
            }
            crate::platforms::gtk::ui::filters::SortOrder::RatingAsc => {
                crate::core::viewmodels::library_view_model::SortOrder::RatingAsc
            }
            crate::platforms::gtk::ui::filters::SortOrder::RatingDesc => {
                crate::core::viewmodels::library_view_model::SortOrder::RatingDesc
            }
            crate::platforms::gtk::ui::filters::SortOrder::DateAddedAsc => {
                crate::core::viewmodels::library_view_model::SortOrder::AddedAsc
            }
            crate::platforms::gtk::ui::filters::SortOrder::DateAddedDesc => {
                crate::core::viewmodels::library_view_model::SortOrder::AddedDesc
            }
            crate::platforms::gtk::ui::filters::SortOrder::DateWatchedAsc => {
                crate::core::viewmodels::library_view_model::SortOrder::AddedAsc
            } // Fallback to AddedAsc
            crate::platforms::gtk::ui::filters::SortOrder::DateWatchedDesc => {
                crate::core::viewmodels::library_view_model::SortOrder::AddedDesc
            } // Fallback to AddedDesc
        };

        // Set the reactive property - this will trigger the ViewModel update
        let sort_order_property = &self.imp().sort_order;
        glib::spawn_future_local({
            let sort_order_property = sort_order_property.clone();
            async move {
                sort_order_property.set(vm_sort_order).await;
            }
        });
    }

    pub fn get_filter_manager(&self) -> std::cell::Ref<'_, FilterManager> {
        self.imp().filter_manager.borrow()
    }

    pub fn search(&self, query: String) {
        // Stage 1: Use reactive search query property with debouncing
        let search_property = &self.imp().search_query;
        let view_model = self.imp().view_model.borrow().clone();

        // Set the reactive property
        glib::spawn_future_local({
            let search_property = search_property.clone();
            let query = query.clone();
            async move {
                search_property.set(query).await;
            }
        });

        // Keep existing direct call for now (will be replaced by reactive binding)
        if let Some(vm) = view_model {
            glib::spawn_future_local(async move {
                vm.search(query).await;
            });
        }
    }

    pub fn refresh(&self) {
        if let Some(view_model) = self.imp().view_model.borrow().as_ref() {
            let vm_clone = view_model.clone();
            glib::spawn_future_local(async move {
                let _ = vm_clone.refresh().await;
            });
        }
    }

    /// LEGACY METHOD: Simplified: Just trigger load on visible cards
    #[allow(dead_code)]
    fn _batch_load_visible_cards_legacy(&self, _start_idx: usize, _end_idx: usize) {
        // Commented out - using reactive bindings instead
        /*
        let cards_by_index = self.imp().cards_by_index.borrow();
        let current_size = *self.imp().current_view_size.borrow();

        // Simply trigger load on each visible card
        for i in start_idx..end_idx {
            if let Some(card) = cards_by_index.get(&i) {
                card.trigger_load(current_size);
            }
        }

        trace!("Triggered load for cards {}-{}", start_idx, end_idx);
        */
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

    pub fn update_content(&self, media_item: MediaItem) {
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
