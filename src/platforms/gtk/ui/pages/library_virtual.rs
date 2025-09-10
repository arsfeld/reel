use gtk4::{gdk, glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use once_cell::sync::Lazy;
use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, trace, warn};

use crate::core::viewmodels::ViewModel;
use crate::models::{Library, MediaItem};
use crate::platforms::gtk::ui::filters::{FilterManager, WatchStatus};
use crate::platforms::gtk::ui::viewmodels::library_view_model::LibraryViewModel;
use crate::platforms::gtk::ui::widgets::virtual_media_list::{
    MediaItemObject, VirtualMediaListModel,
};
use crate::state::AppState;
use crate::utils::{ImageLoader, ImageSize};

// Global image loader instance
static IMAGE_LOADER: Lazy<ImageLoader> =
    Lazy::new(|| ImageLoader::new().expect("Failed to create ImageLoader"));

mod imp {
    use super::*;

    pub struct LibraryVirtualView {
        pub scrolled_window: RefCell<Option<gtk4::ScrolledWindow>>,
        pub grid_view: RefCell<Option<gtk4::GridView>>,
        pub list_model: RefCell<Option<VirtualMediaListModel>>,
        pub selection_model: RefCell<Option<gtk4::NoSelection>>,
        pub loading_spinner: RefCell<Option<gtk4::Spinner>>,
        pub empty_state: RefCell<Option<adw::StatusPage>>,
        pub stack: RefCell<Option<gtk4::Stack>>,

        pub state: RefCell<Option<Arc<AppState>>>,
        pub library: RefCell<Option<Library>>,
        pub backend_id: RefCell<Option<String>>,
        pub current_view_size: RefCell<ImageSize>,
        pub on_media_selected: RefCell<Option<Box<dyn Fn(&MediaItem)>>>,
        pub filter_manager: RefCell<FilterManager>,
        pub view_model: RefCell<Option<Arc<LibraryViewModel>>>,
        pub update_scheduled: Cell<bool>,
        pub pending_items: RefCell<Option<Vec<MediaItem>>>,
    }

    impl Default for LibraryVirtualView {
        fn default() -> Self {
            Self {
                scrolled_window: RefCell::new(None),
                grid_view: RefCell::new(None),
                list_model: RefCell::new(None),
                selection_model: RefCell::new(None),
                loading_spinner: RefCell::new(None),
                empty_state: RefCell::new(None),
                stack: RefCell::new(None),
                state: RefCell::new(None),
                library: RefCell::new(None),
                backend_id: RefCell::new(None),
                current_view_size: RefCell::new(ImageSize::Medium),
                on_media_selected: RefCell::new(None),
                filter_manager: RefCell::new(FilterManager::new()),
                view_model: RefCell::new(None),
                update_scheduled: Cell::new(false),
                pending_items: RefCell::new(None),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LibraryVirtualView {
        const NAME: &'static str = "LibraryVirtualView";
        type Type = super::LibraryVirtualView;
        type ParentType = gtk4::Box;
    }

    impl ObjectImpl for LibraryVirtualView {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_ui();
        }
    }

    impl WidgetImpl for LibraryVirtualView {}
    impl BoxImpl for LibraryVirtualView {}
}

glib::wrapper! {
    pub struct LibraryVirtualView(ObjectSubclass<imp::LibraryVirtualView>)
        @extends gtk4::Widget, gtk4::Box,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Orientable;
}

impl LibraryVirtualView {
    pub fn new(state: Arc<AppState>) -> Self {
        let view: Self = glib::Object::builder()
            .property("orientation", gtk4::Orientation::Vertical)
            .property("spacing", 0)
            .build();

        // Initialize ViewModel
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

        // Content state - GridView with virtual scrolling
        let scrolled_window = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .build();

        // Create virtual list model
        let list_model = VirtualMediaListModel::new();

        // Wrap in NoSelection model (we handle selection ourselves)
        let selection_model = gtk4::NoSelection::new(Some(list_model.clone()));

        // Create item factory for efficient view recycling
        let factory = gtk4::SignalListItemFactory::new();

        // Setup - create the widgets
        let weak_self = self.downgrade();
        factory.connect_setup(move |_, list_item| {
            let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
            let card = VirtualMediaCard::new();

            // Connect click handler
            let weak_self = weak_self.clone();
            card.connect_clicked(move |card| {
                if let Some(view) = weak_self.upgrade() {
                    if let Some(item) = card.media_item() {
                        info!("Media item selected: {}", item.title());
                        if let Some(callback) = view.imp().on_media_selected.borrow().as_ref() {
                            callback(&item);
                        }
                    }
                }
            });

            list_item.set_child(Some(&card));
        });

        // Bind - update the widgets with data
        factory.connect_bind(move |_, list_item| {
            let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
            let card = list_item
                .child()
                .and_downcast::<VirtualMediaCard>()
                .expect("The child should be a VirtualMediaCard");

            let item_obj = list_item
                .item()
                .and_downcast::<MediaItemObject>()
                .expect("The item should be a MediaItemObject");

            card.bind(&item_obj);
        });

        // Unbind - clean up when recycling
        factory.connect_unbind(move |_, list_item| {
            let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
            let card = list_item
                .child()
                .and_downcast::<VirtualMediaCard>()
                .expect("The child should be a VirtualMediaCard");

            card.unbind();
        });

        // Create GridView with virtual scrolling
        let grid_view = gtk4::GridView::builder()
            .model(&selection_model)
            .factory(&factory)
            .min_columns(3)
            .max_columns(8)
            .single_click_activate(false)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();

        // Adjust columns based on window width
        let weak_self = self.downgrade();
        grid_view.connect_map(move |gv| {
            if let Some(view) = weak_self.upgrade()
                && let Some(window) = view.root().and_then(|r| r.downcast::<gtk4::Window>().ok())
            {
                let grid_view_weak = gv.downgrade();
                window.connect_default_width_notify(move |window| {
                    if let Some(grid_view) = grid_view_weak.upgrade() {
                        let width = window.default_width();

                        // Adjust columns based on window width
                        let (min_cols, max_cols) = if width < 800 {
                            (3, 5)
                        } else if width < 1200 {
                            (4, 6)
                        } else if width < 1600 {
                            (5, 7)
                        } else {
                            (6, 8)
                        };

                        grid_view.set_min_columns(min_cols);
                        grid_view.set_max_columns(max_cols);
                    }
                });
            }
        });

        scrolled_window.set_child(Some(&grid_view));
        stack.add_named(&scrolled_window, Some("content"));

        // Empty state
        let empty_state = adw::StatusPage::builder()
            .title("No Content")
            .description("This library doesn't have any items yet")
            .icon_name("folder-symbolic")
            .build();

        stack.add_named(&empty_state, Some("empty"));

        // Add stack to the view
        self.append(&stack);

        // Store references
        imp.scrolled_window.replace(Some(scrolled_window));
        imp.grid_view.replace(Some(grid_view));
        imp.list_model.replace(Some(list_model));
        imp.selection_model.replace(Some(selection_model));
        imp.loading_spinner.replace(Some(loading_spinner));
        imp.empty_state.replace(Some(empty_state));
        imp.stack.replace(Some(stack));
    }

    fn setup_viewmodel_bindings(&self, view_model: Arc<LibraryViewModel>) {
        let weak_self = self.downgrade();

        // Subscribe to filtered items changes
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
                    error!("Library error: {}", err_msg);
                }
            }
        });
    }

    fn update_items_from_viewmodel(&self, items: Vec<MediaItem>) {
        let start = Instant::now();
        trace!("Received {} items from ViewModel", items.len());

        // Store pending items and schedule batched update
        self.imp().pending_items.replace(Some(items));

        // Schedule UI update if not already scheduled
        if !self.imp().update_scheduled.get() {
            self.imp().update_scheduled.set(true);

            let weak_self = self.downgrade();
            glib::idle_add_local_once(move || {
                if let Some(view) = weak_self.upgrade() {
                    // Process pending items
                    if let Some(items) = view.imp().pending_items.take() {
                        view.display_media_items(items);
                    }
                    view.imp().update_scheduled.set(false);
                }
            });
        }

        let elapsed = start.elapsed();
        if elapsed.as_millis() > 2 {
            warn!("Slow update scheduling: {:?}", elapsed);
        }
    }

    fn display_media_items(&self, items: Vec<MediaItem>) {
        let start = Instant::now();
        let imp = self.imp();

        if let Some(list_model) = imp.list_model.borrow().as_ref() {
            if items.is_empty() {
                list_model.clear();
                // Show empty state
                if let Some(stack) = imp.stack.borrow().as_ref() {
                    stack.set_visible_child_name("empty");
                }
            } else {
                // Update the virtual list model with new items (clone to avoid move)
                list_model.set_items(items.clone());

                // Show content
                if let Some(stack) = imp.stack.borrow().as_ref() {
                    stack.set_visible_child_name("content");
                }
            }
        }

        let elapsed = start.elapsed();
        if elapsed.as_millis() > 16 {
            warn!(
                "Slow UI update in display_media_items: {:?} for {} items",
                elapsed,
                items.len()
            );
        }
    }

    pub async fn load_library(&self, backend_id: String, library: Library) {
        let start = Instant::now();
        info!("Loading library: {} ({})", library.title, library.id);

        let imp = self.imp();

        // Show loading state
        if let Some(stack) = imp.stack.borrow().as_ref() {
            stack.set_visible_child_name("loading");
        }

        // Store backend ID and library
        imp.backend_id.replace(Some(backend_id.clone()));
        imp.library.replace(Some(library.clone()));

        // Use ViewModel for loading
        if let Some(view_model) = imp.view_model.borrow().as_ref() {
            match view_model.set_library(library.id.clone()).await {
                Ok(_) => {
                    trace!("Library loaded via ViewModel");
                }
                Err(e) => {
                    error!("Failed to load library via ViewModel: {}", e);
                    if let Some(stack) = imp.stack.borrow().as_ref() {
                        stack.set_visible_child_name("empty");
                    }
                }
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

    // Filter and sort methods - delegate to ViewModel
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
                }
                crate::platforms::gtk::ui::filters::SortOrder::DateWatchedDesc => {
                    crate::platforms::gtk::ui::viewmodels::library_view_model::SortOrder::AddedDesc
                }
            };
            let vm_clone = view_model.clone();
            glib::spawn_future_local(async move {
                vm_clone.set_sort_order(vm_sort_order).await;
            });
        }
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
                let _ = vm_clone.refresh().await;
            });
        }
    }
}

// Virtual Media Card - Efficient recycled card widget
mod imp_card {
    use super::*;

    #[derive(Debug)]
    pub struct VirtualMediaCard {
        pub overlay: RefCell<Option<gtk4::Overlay>>,
        pub image: RefCell<Option<gtk4::Picture>>,
        pub info_box: RefCell<Option<gtk4::Box>>,
        pub title_label: RefCell<Option<gtk4::Label>>,
        pub subtitle_label: RefCell<Option<gtk4::Label>>,
        pub loading_spinner: RefCell<Option<gtk4::Spinner>>,
        pub watched_indicator: RefCell<Option<gtk4::Box>>,
        pub progress_bar: RefCell<Option<gtk4::ProgressBar>>,
        pub media_item: RefCell<Option<MediaItem>>,
        pub image_loading_handle: RefCell<Option<glib::JoinHandle<()>>>,
    }

    impl Default for VirtualMediaCard {
        fn default() -> Self {
            Self {
                overlay: RefCell::new(None),
                image: RefCell::new(None),
                info_box: RefCell::new(None),
                title_label: RefCell::new(None),
                subtitle_label: RefCell::new(None),
                loading_spinner: RefCell::new(None),
                watched_indicator: RefCell::new(None),
                progress_bar: RefCell::new(None),
                media_item: RefCell::new(None),
                image_loading_handle: RefCell::new(None),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VirtualMediaCard {
        const NAME: &'static str = "VirtualMediaCard";
        type Type = super::VirtualMediaCard;
        type ParentType = gtk4::Button;
    }

    impl ObjectImpl for VirtualMediaCard {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_card_ui();
        }

        fn dispose(&self) {
            // Cancel any pending image loads
            if let Some(handle) = self.image_loading_handle.take() {
                handle.abort();
            }
        }
    }

    impl WidgetImpl for VirtualMediaCard {}
    impl ButtonImpl for VirtualMediaCard {}
}

glib::wrapper! {
    pub struct VirtualMediaCard(ObjectSubclass<imp_card::VirtualMediaCard>)
        @extends gtk4::Widget, gtk4::Button,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Actionable;
}

impl VirtualMediaCard {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    fn setup_card_ui(&self) {
        let imp = self.imp();

        self.add_css_class("flat");
        self.add_css_class("media-card");
        self.add_css_class("poster-card");

        let overlay = gtk4::Overlay::new();
        overlay.add_css_class("poster-overlay");

        // Standard poster dimensions
        let (width, height) = ImageSize::Medium.dimensions_for_poster();

        let image = gtk4::Picture::builder()
            .width_request(width as i32)
            .height_request(height as i32)
            .content_fit(gtk4::ContentFit::Cover)
            .build();

        image.add_css_class("rounded-poster");
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

        let info_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(2)
            .margin_bottom(8)
            .margin_start(8)
            .margin_end(8)
            .margin_top(8)
            .build();

        info_box.add_css_class("media-card-info");

        let title_label = gtk4::Label::builder()
            .xalign(0.0)
            .single_line_mode(true)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .css_classes(vec!["title-4"])
            .build();

        let subtitle_label = gtk4::Label::builder()
            .xalign(0.0)
            .single_line_mode(true)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .css_classes(vec!["subtitle"])
            .build();

        info_box.append(&title_label);
        info_box.append(&subtitle_label);

        info_wrapper.append(&info_box);
        overlay.add_overlay(&info_wrapper);

        // Unwatched indicator
        let unwatched_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .halign(gtk4::Align::End)
            .valign(gtk4::Align::Start)
            .margin_top(8)
            .margin_end(8)
            .visible(false)
            .build();

        let unwatched_dot = gtk4::Box::builder()
            .width_request(14)
            .height_request(14)
            .build();

        unwatched_dot.add_css_class("unwatched-glow-dot");
        unwatched_box.append(&unwatched_dot);
        unwatched_box.add_css_class("unwatched-indicator");

        overlay.add_overlay(&unwatched_box);

        // Progress bar
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

    pub fn bind(&self, item_obj: &MediaItemObject) {
        let item = item_obj.item();
        self.imp().media_item.replace(Some(item.clone()));

        // Update UI elements
        self.update_content(&item);

        // Load image asynchronously
        self.load_poster_image(&item);
    }

    pub fn unbind(&self) {
        // Cancel any pending image loads
        if let Some(handle) = self.imp().image_loading_handle.take() {
            handle.abort();
        }

        // Clear the image
        if let Some(image) = self.imp().image.borrow().as_ref() {
            image.set_paintable(None::<&gdk::Paintable>);
        }

        // Hide spinner
        if let Some(spinner) = self.imp().loading_spinner.borrow().as_ref() {
            spinner.set_spinning(false);
            spinner.set_visible(false);
        }

        self.imp().media_item.replace(None);
    }

    fn update_content(&self, media_item: &MediaItem) {
        let imp = self.imp();

        // Update title and subtitle
        if let MediaItem::Episode(episode) = media_item {
            if let Some(title_label) = imp.title_label.borrow().as_ref() {
                if let Some(ref show_title) = episode.show_title {
                    title_label.set_text(show_title);
                } else {
                    title_label.set_text(&episode.title);
                }
            }

            if let Some(subtitle_label) = imp.subtitle_label.borrow().as_ref() {
                let subtitle = format!(
                    "S{}E{} • {}",
                    episode.season_number, episode.episode_number, episode.title
                );
                subtitle_label.set_text(&subtitle);
            }
        } else {
            if let Some(title_label) = imp.title_label.borrow().as_ref() {
                title_label.set_text(media_item.title());
            }

            if let Some(subtitle_label) = imp.subtitle_label.borrow().as_ref() {
                let subtitle = match media_item {
                    MediaItem::Movie(movie) => {
                        movie.year.map(|y| y.to_string()).unwrap_or_default()
                    }
                    MediaItem::Show(show) => {
                        if show.total_episode_count > 0 {
                            format!("{} episodes", show.total_episode_count)
                        } else {
                            "TV Series".to_string()
                        }
                    }
                    _ => String::new(),
                };
                subtitle_label.set_text(&subtitle);
            }
        }

        // Update watched indicator
        if let Some(unwatched_indicator) = imp.watched_indicator.borrow().as_ref() {
            unwatched_indicator.set_visible(!media_item.is_watched());
        }

        // Update progress bar
        if let Some(progress_bar) = imp.progress_bar.borrow().as_ref() {
            if let Some(progress) = media_item.watch_progress() {
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

    fn load_poster_image(&self, media_item: &MediaItem) {
        let poster_url = match media_item {
            MediaItem::Movie(movie) => movie.poster_url.clone(),
            MediaItem::Show(show) => show.poster_url.clone(),
            MediaItem::Episode(episode) => episode
                .show_poster_url
                .clone()
                .or(episode.thumbnail_url.clone()),
            _ => None,
        };

        if let Some(url) = poster_url {
            let imp = self.imp();

            // Show spinner
            if let Some(spinner) = imp.loading_spinner.borrow().as_ref() {
                spinner.set_spinning(true);
                spinner.set_visible(true);
            }

            let image_ref = imp.image.borrow().as_ref().unwrap().clone();
            let spinner_ref = imp.loading_spinner.borrow().as_ref().unwrap().clone();

            // Cancel previous load if any
            if let Some(handle) = imp.image_loading_handle.take() {
                handle.abort();
            }

            // Start new load
            let handle = glib::spawn_future_local(async move {
                match IMAGE_LOADER.load_image(&url, ImageSize::Medium).await {
                    Ok(texture) => {
                        glib::idle_add_local_once(move || {
                            image_ref.set_paintable(Some(&texture));
                            spinner_ref.set_spinning(false);
                            spinner_ref.set_visible(false);
                        });
                    }
                    Err(e) => {
                        trace!("Failed to load poster: {}", e);
                        glib::idle_add_local_once(move || {
                            spinner_ref.set_spinning(false);
                            spinner_ref.set_visible(false);
                        });
                    }
                }
            });

            imp.image_loading_handle.replace(Some(handle));
        }
    }

    pub fn media_item(&self) -> Option<MediaItem> {
        self.imp().media_item.borrow().clone()
    }
}
