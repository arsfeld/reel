use gtk4::{gdk, glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use once_cell::sync::Lazy;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use tracing::{error, info, trace};

use crate::constants::*;
use crate::models::{Library, LibraryType, MediaItem};
use crate::state::AppState;
use crate::ui::filters::{FilterManager, SortOrder, WatchStatus};
use crate::utils::{ImageLoader, ImageSize};

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

    pub async fn load_library(&self, backend_id: String, library: Library) {
        info!("Loading library: {} ({})", library.title, library.id);

        let imp = self.imp();

        // Store backend ID and library
        imp.backend_id.replace(Some(backend_id.clone()));
        imp.library.replace(Some(library.clone()));

        // Show loading state
        if let Some(stack) = imp.stack.borrow().as_ref() {
            stack.set_visible_child_name("loading");
        }

        // Load media items based on library type
        let state = imp.state.borrow().as_ref().unwrap().clone();
        let sync_manager = state.sync_manager.clone();

        let media_items = match library.library_type {
            LibraryType::Movies => {
                match sync_manager
                    .get_cached_movies(&backend_id, &library.id)
                    .await
                {
                    Ok(movies) => movies.into_iter().map(MediaItem::Movie).collect::<Vec<_>>(),
                    Err(e) => {
                        error!("Failed to load movies: {}", e);
                        Vec::new()
                    }
                }
            }
            LibraryType::Shows => {
                match sync_manager
                    .get_cached_shows(&backend_id, &library.id)
                    .await
                {
                    Ok(shows) => shows.into_iter().map(MediaItem::Show).collect::<Vec<_>>(),
                    Err(e) => {
                        error!("Failed to load shows: {}", e);
                        Vec::new()
                    }
                }
            }
            _ => {
                info!(
                    "Library type {:?} not yet implemented",
                    library.library_type
                );
                Vec::new()
            }
        };

        // Store all media items
        self.imp().all_media_items.replace(media_items.clone());

        // Apply filters and display items
        // Removed predictive preloading - cards will load images on-demand
        self.apply_filters();
    }

    async fn preload_initial_images(&self, items: &[MediaItem]) {
        // Simplified: removed predictive preloading as it may cause issues
        // Images will load on-demand through trigger_load() instead
        trace!("Skipping predictive preload for {} items", items.len());
    }

    fn display_media_items(&self, items: Vec<MediaItem>) {
        let imp = self.imp();

        // Store filtered items for batch loading
        imp.filtered_items.replace(items.clone());
        imp.cards_by_index.borrow_mut().clear();

        let flow_box = imp.flow_box.borrow().clone();
        let scrolled_window = imp.scrolled_window.borrow().clone();
        let stack = imp.stack.borrow().clone();
        let current_size = *imp.current_view_size.borrow();

        if let Some(flow_box) = flow_box {
            // Clear existing items
            while let Some(child) = flow_box.first_child() {
                flow_box.remove(&child);
            }

            if items.is_empty() {
                // Show empty state
                if let Some(stack) = stack {
                    stack.set_visible_child_name("empty");
                }
            } else {
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
                                    cards.push(card.clone());
                                    cards_by_index.insert(i, card);
                                }
                            }
                        }
                    }
                });

                // Defer initial card creation to avoid blocking
                let create_initial = create_cards_batch.clone();
                let weak_self = self.downgrade();
                glib::idle_add_local_once(move || {
                    create_initial(0, INITIAL_CARDS_TO_CREATE);

                    // Simplified: trigger load directly on created cards
                    if let Some(view) = weak_self.upgrade() {
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(100),
                            move || {
                                // Trigger load on initial visible cards
                                let cards = view.imp().cards_by_index.borrow();
                                for i in 0..INITIAL_IMAGES_TO_LOAD.min(cards.len()) {
                                    if let Some(card) = cards.get(&i) {
                                        card.trigger_load(ImageSize::Medium);
                                    }
                                }
                            },
                        );
                    }
                });

                // Show content
                if let Some(stack) = stack {
                    stack.set_visible_child_name("content");
                }

                // Set up progressive loading with card creation and batch loading
                if let Some(scrolled_window) = scrolled_window {
                    self.setup_progressive_loading_with_batch(
                        scrolled_window,
                        flow_box.clone(),
                        create_cards_batch.clone(),
                        cards_rc.clone(),
                        items_rc.len(),
                    );
                }
            }
        }
    }

    fn setup_progressive_loading(
        &self,
        scrolled_window: gtk4::ScrolledWindow,
        flow_box: gtk4::FlowBox,
    ) {
        let adjustment = scrolled_window.vadjustment();
        let update_counter: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));

        adjustment.connect_value_changed(move |adj| {
            let flow_box = flow_box.clone();
            let counter = update_counter.clone();

            let viewport_top = adj.value();
            let viewport_height = adj.page_size();

            let current_count = {
                let mut c = counter.borrow_mut();
                *c += 1;
                *c
            };

            // Very short delay since images load fast
            let counter_inner = counter.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(15), move || {
                if *counter_inner.borrow() != current_count {
                    return glib::ControlFlow::Break;
                }

                let viewport_bottom = viewport_top + viewport_height;

                // Load everything in a large buffer since images are small
                let load_margin = viewport_height * 3.0; // Load 3 screens in each direction

                let load_top = (viewport_top - load_margin).max(0.0);
                let load_bottom = viewport_bottom + load_margin;

                // Single pass loading - all at same quality
                Self::load_cards_in_range(&flow_box, load_top, load_bottom, ImageSize::Medium);

                glib::ControlFlow::Break
            });
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
        let mut child = flow_box.first_child();
        let mut cards_to_load = Vec::new();

        while let Some(flow_child) = child {
            if let Some(fc) = flow_child.downcast_ref::<gtk4::FlowBoxChild>() {
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

        // Batch load for efficiency
        for (card, size) in cards_to_load {
            card.trigger_load(size);
        }
    }

    pub fn navigate_back(&self) {
        info!("Navigating back to libraries");
        let mut widget: Option<gtk4::Widget> = self.parent();
        while let Some(w) = widget {
            if w.type_() == crate::ui::main_window::ReelMainWindow::static_type() {
                if let Some(window) = w.downcast_ref::<crate::ui::main_window::ReelMainWindow>() {
                    window.show_libraries_view();
                }
                break;
            }
            widget = w.parent();
        }
    }

    pub fn apply_filters(&self) {
        let imp = self.imp();
        let all_items = imp.all_media_items.borrow().clone();
        let filter_manager = imp.filter_manager.borrow();

        // Apply all filters using the FilterManager
        let filtered_items = filter_manager.apply_filters(all_items);

        info!(
            "Applied filters: {} items shown of {} total",
            filtered_items.len(),
            imp.all_media_items.borrow().len()
        );

        // Display filtered items
        self.display_media_items(filtered_items);
    }

    pub fn update_watch_status_filter(&self, status: WatchStatus) {
        let imp = self.imp();
        imp.filter_manager.borrow_mut().set_watch_status(status);
        self.apply_filters();
    }

    pub fn update_sort_order(&self, order: SortOrder) {
        let imp = self.imp();
        imp.filter_manager.borrow_mut().set_sort_order(order);
        self.apply_filters();
    }

    pub fn get_filter_manager(&self) -> std::cell::Ref<FilterManager> {
        self.imp().filter_manager.borrow()
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
            return;
        }

        if *imp.image_loading.borrow() {
            return;
        }

        *imp.image_loading.borrow_mut() = true;

        if let Some(media_item) = imp.media_item.borrow().as_ref() {
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

            if let Some(spinner) = imp.loading_spinner.borrow().as_ref() {
                spinner.set_spinning(true);
                spinner.set_visible(true);
            }

            let image_ref = imp.image.borrow().as_ref().unwrap().clone();
            let spinner_ref = imp.loading_spinner.borrow().as_ref().unwrap().clone();
            let weak_self = self.downgrade();

            // Load with specified size for optimization
            glib::spawn_future_local(async move {
                match IMAGE_LOADER.load_image(&url, size).await {
                    Ok(texture) => {
                        let image_ref = image_ref.clone();
                        let spinner_ref = spinner_ref.clone();
                        let weak_self = weak_self.clone();

                        glib::idle_add_local_once(move || {
                            if let Some(card) = weak_self.upgrade() {
                                image_ref.set_paintable(Some(&texture));
                                spinner_ref.set_spinning(false);
                                spinner_ref.set_visible(false);

                                let imp = card.imp();
                                *imp.image_loaded.borrow_mut() = true;
                                *imp.image_loading.borrow_mut() = false;
                            }
                        });
                    }
                    Err(e) => {
                        let image_ref = image_ref.clone();
                        let spinner_ref = spinner_ref.clone();
                        let weak_self = weak_self.clone();

                        glib::idle_add_local_once(move || {
                            error!("Failed to load poster: {}", e);
                            spinner_ref.set_spinning(false);
                            spinner_ref.set_visible(false);

                            if let Some(card) = weak_self.upgrade() {
                                card.set_placeholder_image(&image_ref);
                                let imp = card.imp();
                                *imp.image_loading.borrow_mut() = false;
                            }
                        });
                    }
                }
            });
        }
    }

    pub fn media_item(&self) -> MediaItem {
        self.imp().media_item.borrow().as_ref().unwrap().clone()
    }
}
