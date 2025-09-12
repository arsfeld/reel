use gtk4::{glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, trace};

use super::library::MediaCard;
use crate::constants::*;
use crate::core::viewmodels::property::{ComputedProperty, Property};
use crate::core::viewmodels::sidebar_view_model::SidebarViewModel;
use crate::models::{HomeSection, HomeSectionType, MediaItem};
use crate::platforms::gtk::ui::navigation_request::NavigationRequest;
use crate::platforms::gtk::ui::viewmodels::ViewModel;
use crate::platforms::gtk::ui::viewmodels::home_view_model::{HomeViewModel, SectionType};
use crate::state::AppState;
use crate::utils::{ImageLoader, ImageSize};

mod imp {
    use super::*;

    pub struct HomePage {
        pub container_box: gtk4::Box,   // Main container
        pub source_selector: gtk4::Box, // Source tabs/buttons
        pub scrolled_window: gtk4::ScrolledWindow,
        pub main_box: gtk4::Box,
        pub sections: Property<Vec<HomeSection>>,
        pub filtered_sections: RefCell<Option<ComputedProperty<Vec<HomeSection>>>>, // Computed: sections filtered by content and source
        pub has_content: RefCell<Option<ComputedProperty<bool>>>, // Computed: whether filtered_sections is not empty
        pub state: RefCell<Option<Arc<AppState>>>,
        pub on_media_selected: RefCell<Option<Box<dyn Fn(&MediaItem)>>>,
        pub image_loader: RefCell<Option<Arc<ImageLoader>>>,
        pub section_cards: RefCell<HashMap<String, Vec<gtk4::Widget>>>,
        pub section_widgets: RefCell<HashMap<String, SectionWidgets>>,
        pub view_model: RefCell<Option<Arc<HomeViewModel>>>,
        pub sidebar_view_model: RefCell<Option<Arc<SidebarViewModel>>>,
        pub current_source_id: Property<Option<String>>,
        pub debounced_source_id: RefCell<Option<ComputedProperty<Option<String>>>>, // Debounced version for API calls
    }

    impl Default for HomePage {
        fn default() -> Self {
            Self {
                container_box: gtk4::Box::default(),
                source_selector: gtk4::Box::default(),
                scrolled_window: gtk4::ScrolledWindow::default(),
                main_box: gtk4::Box::default(),
                sections: Property::new(Vec::new(), "sections"),
                filtered_sections: RefCell::new(None),
                has_content: RefCell::new(None),
                state: RefCell::default(),
                on_media_selected: RefCell::default(),
                image_loader: RefCell::default(),
                section_cards: RefCell::default(),
                section_widgets: RefCell::default(),
                view_model: RefCell::default(),
                sidebar_view_model: RefCell::default(),
                current_source_id: Property::new(None, "current_source_id"),
                debounced_source_id: RefCell::new(None),
            }
        }
    }

    pub struct SectionWidgets {
        pub container: gtk4::Box,
        pub items_box: gtk4::Box,
        pub scrolled: gtk4::ScrolledWindow,
        pub cards: Vec<gtk4::Widget>,
    }

    impl std::fmt::Debug for HomePage {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("HomePage")
                .field("scrolled_window", &self.scrolled_window)
                .field("main_box", &self.main_box)
                .field("sections", &self.sections)
                .field("state", &"Arc<AppState>")
                .field("on_media_selected", &"Option<Callback>")
                .finish()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for HomePage {
        const NAME: &'static str = "HomePage";
        type Type = super::HomePage;
        type ParentType = gtk4::Box;
    }

    impl ObjectImpl for HomePage {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            obj.set_orientation(gtk4::Orientation::Vertical);
            obj.set_vexpand(true);
            obj.set_hexpand(true);

            // Setup container
            self.container_box
                .set_orientation(gtk4::Orientation::Vertical);
            self.container_box.set_vexpand(true);
            self.container_box.set_hexpand(true);

            // Setup scrolled window
            self.scrolled_window
                .set_hscrollbar_policy(gtk4::PolicyType::Never);
            self.scrolled_window
                .set_vscrollbar_policy(gtk4::PolicyType::Automatic);
            self.scrolled_window.set_vexpand(true);
            self.scrolled_window.set_hexpand(true);

            self.main_box.set_orientation(gtk4::Orientation::Vertical);
            self.main_box.set_spacing(24);
            self.main_box.set_margin_top(12);
            self.main_box.set_margin_bottom(12);
            self.main_box.set_margin_start(12);
            self.main_box.set_margin_end(12);

            self.scrolled_window.set_child(Some(&self.main_box));

            // Add scrolled window directly to page (source selector now in header)
            obj.append(&self.scrolled_window);
        }
    }

    impl WidgetImpl for HomePage {}
    impl BoxImpl for HomePage {}
}

glib::wrapper! {
    pub struct HomePage(ObjectSubclass<imp::HomePage>)
        @extends gtk4::Widget, gtk4::Box,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Orientable;
}

impl HomePage {
    pub fn new<F>(
        state: Arc<AppState>,
        source_id: Option<String>, // Filter by source
        setup_header: F,
        navigation_handler: impl Fn(NavigationRequest) + 'static,
    ) -> Self
    where
        F: Fn(&gtk4::Widget) + 'static,
    {
        let page: Self = glib::Object::builder().build();

        // Initialize HomeViewModel with source filter
        let data_service = state.data_service.clone();
        let view_model = Arc::new(HomeViewModel::new(data_service.clone()));
        page.imp().view_model.replace(Some(view_model.clone()));

        // Initialize SidebarViewModel to get source info
        let sidebar_vm = Arc::new(SidebarViewModel::new(data_service));
        page.imp()
            .sidebar_view_model
            .replace(Some(sidebar_vm.clone()));

        // Store current source_id
        if let Some(source_id) = &source_id {
            let current_source_property = page.imp().current_source_id.clone();
            let source_id = source_id.clone();
            glib::spawn_future_local(async move {
                current_source_property.set(Some(source_id)).await;
            });
        }

        // Set the source filter if provided
        if let Some(ref source_id) = source_id {
            let vm = view_model.clone();
            let source_id_clone = source_id.clone();
            glib::spawn_future_local(async move {
                let _ = vm.set_source_filter(Some(source_id_clone)).await;
            });
        }

        // Initialize ViewModel with EventBus and bind to initialization state
        glib::spawn_future_local({
            let vm = view_model.clone();
            let event_bus = state.event_bus.clone();
            let source_coordinator = state.source_coordinator.clone();
            async move {
                use crate::platforms::gtk::ui::viewmodels::ViewModel;
                vm.initialize(event_bus).await;

                // Bind to initialization state for progressive enhancement
                if let Some(init_state) = source_coordinator.get_initialization_state() {
                    vm.bind_to_initialization_state(&init_state).await;

                    // Handle partial initialization gracefully
                    let _ = vm.handle_partial_initialization().await;
                } else {
                    // No initialization state yet, just load from cache
                    let _ = vm.load_home_content_from_cache().await;
                }
            }
        });

        // Setup ViewModel bindings
        page.setup_viewmodel_bindings(view_model);

        page.imp().state.replace(Some(state.clone()));

        // Setup source selector after state is initialized
        page.setup_source_selector(sidebar_vm.clone());

        // Initialize image loader
        if let Ok(loader) = ImageLoader::new() {
            page.imp().image_loader.replace(Some(Arc::new(loader)));
        }

        // Setup header with title and source selector
        page.setup_header_with_selector(setup_header);

        // Setup computed property updates
        page.setup_computed_properties();

        // Set up internal media selection handler
        let _state_clone = state.clone();
        page.set_on_media_selected(move |media_item| {
            info!("HomePage - Media selected: {}", media_item.title());

            use crate::models::MediaItem;
            match media_item {
                MediaItem::Movie(movie) => {
                    info!("HomePage - Navigating to movie details");
                    navigation_handler(NavigationRequest::ShowMovieDetails(movie.clone()));
                }
                MediaItem::Episode(_) => {
                    info!("HomePage - Navigating to episode player");
                    navigation_handler(NavigationRequest::ShowPlayer(media_item.clone()));
                }
                MediaItem::Show(show) => {
                    info!("HomePage - Navigating to show details");
                    navigation_handler(NavigationRequest::ShowShowDetails(show.clone()));
                }
                _ => {
                    info!("HomePage - Unsupported media type");
                }
            }
        });

        page
    }

    pub fn setup_header_with_selector<F>(&self, setup_header: F)
    where
        F: Fn(&gtk4::Widget) + 'static,
    {
        // Create a header box that contains the source selector
        let header_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);

        // Create source selector but don't populate it yet (will be done by setup_source_selector)
        let source_selector = &self.imp().source_selector;
        source_selector.set_orientation(gtk4::Orientation::Horizontal);
        source_selector.set_halign(gtk4::Align::Center);
        source_selector.add_css_class("linked");
        source_selector.add_css_class("pill");

        header_box.append(source_selector);

        // Call the setup_header callback with the complete header box
        setup_header(header_box.upcast_ref());
    }

    fn setup_source_selector(&self, sidebar_vm: Arc<SidebarViewModel>) {
        let imp = self.imp();
        let weak_self = self.downgrade();

        // Subscribe to sources changes
        let mut sources_subscriber = sidebar_vm.sources().subscribe();
        glib::spawn_future_local(async move {
            while sources_subscriber.wait_for_change().await {
                if let Some(page) = weak_self.upgrade() {
                    page.refresh_source_selector().await;
                }
            }
        });

        // Subscribe to current_source_id changes for reactive button state updates
        let weak_self_reactive = self.downgrade();
        let mut current_source_subscriber = imp.current_source_id.subscribe();
        glib::spawn_future_local(async move {
            while current_source_subscriber.wait_for_change().await {
                if let Some(page) = weak_self_reactive.upgrade() {
                    let current_source_id = page.imp().current_source_id.get().await;
                    page.update_source_selector_buttons(current_source_id.as_deref());
                }
            }
        });

        // Initialize sidebar VM with EventBus
        let sidebar_vm_clone = sidebar_vm.clone();
        let state = imp.state.borrow().clone().unwrap();
        glib::spawn_future_local(async move {
            sidebar_vm_clone.initialize(state.event_bus.clone()).await;
        });
    }

    async fn refresh_source_selector(&self) {
        let imp = self.imp();

        // Clear existing buttons
        while let Some(child) = imp.source_selector.first_child() {
            imp.source_selector.remove(&child);
        }

        if let Some(sidebar_vm) = &*imp.sidebar_view_model.borrow() {
            let sources = sidebar_vm.sources().get().await;
            let current_source_id = imp.current_source_id.get().await;

            // Add "All Sources" button
            let all_button = gtk4::ToggleButton::with_label("All");
            all_button.set_active(current_source_id.is_none());

            let weak_self = self.downgrade();
            all_button.connect_clicked(move |_| {
                if let Some(page) = weak_self.upgrade() {
                    page.switch_source(None);
                }
            });
            imp.source_selector.append(&all_button);

            // Add button for each source
            for source in &sources {
                let button = gtk4::ToggleButton::with_label(&source.name);
                let is_current = current_source_id.as_ref() == Some(&source.id);
                button.set_active(is_current);

                // Store the source ID as a data attribute for later reference
                unsafe {
                    button.set_data("source_id", source.id.clone());
                }

                let source_id = source.id.clone();
                let weak_self = self.downgrade();
                button.connect_clicked(move |_| {
                    if let Some(page) = weak_self.upgrade() {
                        page.switch_source(Some(source_id.clone()));
                    }
                });

                imp.source_selector.append(&button);
            }

            // Show selector only if there are multiple sources
            if sources.len() > 1 || !sources.is_empty() {
                imp.source_selector.set_visible(true);
            } else {
                imp.source_selector.set_visible(false);
            }
        }
    }

    fn switch_source(&self, source_id: Option<String>) {
        let imp = self.imp();

        // Update current_source_id Property immediately for UI responsiveness
        // The debounced version will handle the API calls after the delay
        let current_source_property = imp.current_source_id.clone();
        glib::spawn_future_local(async move {
            current_source_property.set(source_id).await;
        });
    }

    async fn handle_debounced_source_change(&self) {
        let imp = self.imp();

        // Get the debounced source ID
        let source_id = if let Some(debounced_prop) = &*imp.debounced_source_id.borrow() {
            debounced_prop.get().await
        } else {
            return; // Debounced property not initialized yet
        };

        // Update the ViewModel filter with debounced API calls
        if let Some(view_model) = &*imp.view_model.borrow() {
            let vm = view_model.clone();

            // Show loading state during the switch
            let _ = vm.is_loading().set(true).await;

            // Update the filter (this will reload content from cache)
            // This is now debounced, so rapid source switching won't spam API calls
            let _ = vm.set_source_filter(source_id).await;

            // Loading state will be cleared by the ViewModel automatically
        }
    }

    fn update_source_selector_buttons(&self, current_source_id: Option<&str>) {
        let imp = self.imp();
        let source_selector = &imp.source_selector;

        // Update button states
        let mut child = source_selector.first_child();
        let mut button_index = 0;

        while let Some(widget) = child {
            if let Some(button) = widget.downcast_ref::<gtk4::ToggleButton>() {
                if button_index == 0 {
                    // "All" button (first button)
                    button.set_active(current_source_id.is_none());
                } else {
                    // Source-specific buttons - check against stored source ID
                    unsafe {
                        if let Some(button_source_id_ptr) = button.data::<String>("source_id") {
                            let button_source_id = button_source_id_ptr.as_ref();
                            let should_be_active = current_source_id == Some(button_source_id);
                            button.set_active(should_be_active);
                        } else {
                            button.set_active(false);
                        }
                    }
                }
            }
            child = widget.next_sibling();
            button_index += 1;
        }
    }

    fn setup_viewmodel_bindings(&self, view_model: Arc<HomeViewModel>) {
        let weak_self = self.downgrade();

        // Subscribe to recent items changes
        let mut recent_subscriber = view_model.recently_added().subscribe();
        glib::spawn_future_local(async move {
            while recent_subscriber.wait_for_change().await {
                if let Some(page) = weak_self.upgrade() {
                    // Refresh home sections when recent items update
                    page.refresh_sections();
                }
            }
        });

        // Subscribe to continue watching changes
        let weak_self_continue = self.downgrade();
        let mut continue_subscriber = view_model.continue_watching().subscribe();
        glib::spawn_future_local(async move {
            while continue_subscriber.wait_for_change().await {
                if let Some(page) = weak_self_continue.upgrade() {
                    page.refresh_sections();
                }
            }
        });

        // Subscribe to sections changes
        let weak_self_sections = self.downgrade();
        let mut sections_subscriber = view_model.sections().subscribe();
        glib::spawn_future_local(async move {
            while sections_subscriber.wait_for_change().await {
                if let Some(page) = weak_self_sections.upgrade() {
                    page.refresh_sections();
                }
            }
        });

        // Subscribe to loading state
        let weak_self_loading = self.downgrade();
        let mut loading_subscriber = view_model.is_loading().subscribe();
        glib::spawn_future_local(async move {
            while loading_subscriber.wait_for_change().await {
                if let Some(page) = weak_self_loading.upgrade()
                    && let Some(vm) = &*page.imp().view_model.borrow()
                {
                    let is_loading = vm.is_loading().get().await;
                    info!("Home loading state: {}", is_loading);
                }
            }
        });
    }

    pub fn set_on_media_selected<F>(&self, callback: F)
    where
        F: Fn(&MediaItem) + 'static,
    {
        self.imp()
            .on_media_selected
            .replace(Some(Box::new(callback)));
    }

    async fn render_sections_declaratively(&self) {
        let imp = self.imp();
        let main_box = &imp.main_box;
        let mut section_widgets = imp.section_widgets.borrow_mut();

        let filtered_sections = if let Some(filtered_prop) = &*imp.filtered_sections.borrow() {
            filtered_prop.get().await
        } else {
            return; // Computed properties not initialized yet
        };

        let old_sections = imp.sections.get().await;

        // Build a set of new section IDs for quick lookup
        let new_section_ids: Vec<String> = filtered_sections.iter().map(|s| s.id.clone()).collect();

        // Remove sections that no longer exist
        let mut to_remove = Vec::new();
        for old_id in section_widgets.keys() {
            if !new_section_ids.contains(old_id) {
                to_remove.push(old_id.clone());
            }
        }
        for id in to_remove {
            if let Some(widgets) = section_widgets.remove(&id) {
                main_box.remove(&widgets.container);
            }
        }

        // Update or create sections
        for (_index, section) in filtered_sections.iter().enumerate() {
            if let Some(widgets) = section_widgets.get(&section.id) {
                // Section exists - update its items if needed
                let old_section = old_sections.iter().find(|s| s.id == section.id);
                if let Some(old) = old_section
                    && !Self::items_equal(&old.items, &section.items)
                {
                    self.update_section_items(widgets, section);
                }

                // Ensure it's at the right position by moving to end
                main_box.remove(&widgets.container);
                main_box.append(&widgets.container);
            } else {
                // New section - create it
                let widgets = self.create_section_widget(section);
                main_box.append(&widgets.container);
                section_widgets.insert(section.id.clone(), widgets);
            }
        }
    }

    async fn update_empty_state(&self) {
        let imp = self.imp();
        let main_box = &imp.main_box;

        let has_content = if let Some(has_content_prop) = &*imp.has_content.borrow() {
            has_content_prop.get().await
        } else {
            return; // Computed properties not initialized yet
        };

        if has_content {
            // Remove any empty state widgets (StatusPage) that might exist
            let mut children_to_remove = Vec::new();
            let mut child = main_box.first_child();
            while let Some(widget) = child {
                if widget.type_() == adw::StatusPage::static_type() {
                    children_to_remove.push(widget.clone());
                }
                child = widget.next_sibling();
            }
            for widget in children_to_remove {
                main_box.remove(&widget);
            }
        } else {
            // Clear everything first
            while let Some(child) = main_box.first_child() {
                main_box.remove(&child);
            }

            let empty_state = adw::StatusPage::builder()
                .icon_name("folder-symbolic")
                .title("No Content Available")
                .description("Connect to a media server to see your content here")
                .build();

            main_box.append(&empty_state);
        }
    }

    fn refresh_sections(&self) {
        // Use ViewModel data instead of calling backend APIs directly
        if let Some(view_model) = &*self.imp().view_model.borrow() {
            let vm = view_model.clone();
            let weak_self = self.downgrade();

            glib::spawn_future_local(async move {
                if let Some(page) = weak_self.upgrade() {
                    // Get sections from ViewModel (database-backed)
                    let sections = vm.sections().get().await;

                    // Convert MediaSections to HomeSections
                    let home_sections: Vec<HomeSection> = sections
                        .into_iter()
                        .map(|section| HomeSection {
                            id: match &section.section_type {
                                SectionType::ContinueWatching => "continue_watching".to_string(),
                                SectionType::RecentlyAdded => "recently_added".to_string(),
                                SectionType::Library(id) => format!("library_{}", id),
                                _ => "other".to_string(),
                            },
                            title: section.title,
                            section_type: match &section.section_type {
                                SectionType::ContinueWatching => HomeSectionType::ContinueWatching,
                                SectionType::RecentlyAdded => HomeSectionType::RecentlyAdded,
                                SectionType::Library(_) => {
                                    HomeSectionType::Custom("Library".to_string())
                                }
                                _ => HomeSectionType::Custom("Other".to_string()),
                            },
                            items: section.items,
                        })
                        .collect();

                    // Update sections Property - computed properties and reactive bindings handle the rest
                    let sections_property = page.imp().sections.clone();
                    glib::spawn_future_local(async move {
                        sections_property.set(home_sections).await;
                    });
                }
            });
        }
    }

    fn items_equal(items1: &[MediaItem], items2: &[MediaItem]) -> bool {
        if items1.len() != items2.len() {
            return false;
        }
        items1.iter().zip(items2.iter()).all(|(a, b)| {
            // Compare by ID to check if same item
            match (a, b) {
                (MediaItem::Movie(m1), MediaItem::Movie(m2)) => m1.id == m2.id,
                (MediaItem::Show(s1), MediaItem::Show(s2)) => s1.id == s2.id,
                (MediaItem::Episode(e1), MediaItem::Episode(e2)) => e1.id == e2.id,
                _ => false,
            }
        })
    }

    fn update_section_items(&self, widgets: &imp::SectionWidgets, section: &HomeSection) {
        // For now, just recreate the items - could be optimized further
        // to reuse existing cards where possible
        let items_box = &widgets.items_box;

        // Clear existing items
        while let Some(child) = items_box.first_child() {
            items_box.remove(&child);
        }

        // Add new items with immediate visibility check
        let mut new_cards = Vec::new();
        for item in &section.items[..section.items.len().min(20)] {
            let card = self.create_media_card(item);
            items_box.append(&card);
            new_cards.push(card);
        }

        // Immediately load initial visible items (no delay)
        for (i, card) in new_cards.iter().enumerate() {
            if i >= HOME_INITIAL_IMAGES_PER_SECTION {
                break;
            }
            if let Some(media_card) = card.downcast_ref::<super::library::MediaCard>() {
                media_card.trigger_load(ImageSize::Small);
            }
        }
    }

    fn create_section_widget(&self, section: &HomeSection) -> imp::SectionWidgets {
        let section_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(12)
            .build();

        // Create section header
        let header_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .build();

        let title_label = gtk4::Label::builder()
            .label(&section.title)
            .halign(gtk4::Align::Start)
            .css_classes(["title-2"])
            .build();

        header_box.append(&title_label);

        // Add "View All" button if there are many items
        if section.items.len() > 10 {
            let view_all_button = gtk4::Button::builder()
                .label("View All")
                .halign(gtk4::Align::End)
                .hexpand(true)
                .css_classes(["flat"])
                .build();

            header_box.append(&view_all_button);
        }

        section_box.append(&header_box);

        // Create horizontal scrollable list for items
        let scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Never)
            .height_request(280) // Fixed height for media cards
            .build();

        let items_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .build();

        // Create initial cards
        let mut cards = Vec::new();
        for item in &section.items[..section.items.len().min(HOME_INITIAL_CARDS_PER_SECTION)] {
            let card = self.create_media_card(item);
            items_box.append(&card);
            cards.push(card);
        }

        // Setup scroll handler for lazy loading with improved viewport detection
        let section_items = Rc::new(section.items.clone());
        let cards_rc = Rc::new(RefCell::new(cards.clone()));
        let self_weak = self.downgrade();
        let items_box_weak = items_box.downgrade();
        let last_loaded_range: Rc<RefCell<(usize, usize)>> = Rc::new(RefCell::new((0, 0)));

        // Improved scroll handler without debouncing for immediate response
        scrolled.hadjustment().connect_value_changed(move |h_adj| {
            let value = h_adj.value();
            let page_size = h_adj.page_size();
            let upper = h_adj.upper();

            // Calculate visible range with pre-fetching
            let card_width = 144.0; // 132px card + 12px spacing
            let visible_start = (value / card_width).floor() as usize;
            let visible_end = ((value + page_size) / card_width).ceil() as usize;

            // Pre-fetch strategy: load 3 cards before and 5 cards after visible range
            let prefetch_before = 3;
            let prefetch_after = 5;
            let load_start = visible_start.saturating_sub(prefetch_before);
            let load_end = (visible_end + prefetch_after).min(20); // Cap at 20 items per section

            if let Some(page) = self_weak.upgrade()
                && let Some(items_box) = items_box_weak.upgrade()
            {
                let mut cards = cards_rc.borrow_mut();
                let section_items = section_items.clone();

                // Check if we need to update (avoid redundant operations)
                let (last_start, last_end) = *last_loaded_range.borrow();
                let needs_update = load_start < last_start || load_end > last_end;

                if needs_update {
                    // Create cards up to load_end if needed
                    for i in cards.len()..load_end.min(section_items.len()).min(20) {
                        if let Some(item) = section_items.get(i) {
                            let card = page.create_media_card(item);
                            items_box.append(&card);
                            cards.push(card.clone());
                        }
                    }

                    // Load images for all cards in the load range
                    for i in load_start..load_end.min(cards.len()) {
                        if let Some(card) = cards.get(i)
                            && let Some(media_card) =
                                card.downcast_ref::<super::library::MediaCard>()
                        {
                            media_card.trigger_load(ImageSize::Small);
                        }
                    }

                    // Update last loaded range
                    *last_loaded_range.borrow_mut() = (load_start, load_end);
                }

                // Log scroll position for debugging
                let scroll_percentage = if upper > 0.0 {
                    (value / upper * 100.0) as i32
                } else {
                    0
                };
                trace!(
                    "Horizontal scroll at {}%, visible cards: {}-{}, loaded: {}-{}",
                    scroll_percentage, visible_start, visible_end, load_start, load_end
                );
            }
        });

        // Immediately load initial visible cards
        for (i, card) in cards.iter().enumerate() {
            if i >= HOME_INITIAL_IMAGES_PER_SECTION {
                break;
            }
            if let Some(media_card) = card.downcast_ref::<super::library::MediaCard>() {
                media_card.trigger_load(ImageSize::Small);
            }
        }

        scrolled.set_child(Some(&items_box));
        section_box.append(&scrolled);

        imp::SectionWidgets {
            container: section_box,
            items_box,
            scrolled,
            cards,
        }
    }

    fn create_media_card(&self, item: &MediaItem) -> gtk4::Widget {
        // Use small size for homepage cards for faster loading
        let card = MediaCard::new(item.clone(), ImageSize::Small);
        // Don't trigger load immediately - let viewport detection handle it
        // card.trigger_load(ImageSize::Small); // Removed for lazy loading

        // Connect click handler
        let item_clone = item.clone();
        let self_weak = self.downgrade();
        card.connect_clicked(move |_| {
            if let Some(page) = self_weak.upgrade() {
                info!("Homepage - Media item selected: {}", item_clone.title());
                if let Some(callback) = page.imp().on_media_selected.borrow().as_ref() {
                    callback(&item_clone);
                }
            }
        });

        card.upcast()
    }

    fn setup_computed_properties(&self) {
        let imp = self.imp();

        // Create computed property for filtered_sections using the new operators
        let sections_prop = imp.sections.clone();
        let current_source_prop = imp.current_source_id.clone();
        let current_source_prop_for_debounce = current_source_prop.clone(); // Clone for debouncing

        // Use ComputedProperty with dependencies for filtered_sections
        let filtered_sections = ComputedProperty::new(
            "filtered_sections",
            vec![
                Arc::new(sections_prop.clone()),
                Arc::new(current_source_prop.clone()),
            ],
            move || {
                let sections = sections_prop.get_sync();
                let _current_source_id = current_source_prop.get_sync();

                // Filter sections that have content
                // TODO: Add source filtering logic when needed
                sections
                    .into_iter()
                    .filter(|section| !section.items.is_empty()) // Must have content
                    .collect()
            },
        );

        // Create computed property for has_content using the map operator on filtered_sections
        let has_content = filtered_sections.map(|sections: Vec<HomeSection>| !sections.is_empty());

        // Create debounced source property - 300ms delay for API calls
        let debounced_source_id =
            current_source_prop_for_debounce.debounce(Duration::from_millis(300));

        // Store the computed properties
        imp.filtered_sections.replace(Some(filtered_sections));
        imp.has_content.replace(Some(has_content));
        imp.debounced_source_id.replace(Some(debounced_source_id));

        // Set up subscriptions to the computed properties for UI updates
        self.setup_computed_property_subscriptions();
    }

    fn setup_computed_property_subscriptions(&self) {
        let imp = self.imp();

        // Subscribe to filtered_sections changes for declarative UI binding
        if let Some(filtered_sections) = &*imp.filtered_sections.borrow() {
            let weak_self = self.downgrade();
            let mut filtered_subscriber = filtered_sections.subscribe();
            glib::spawn_future_local(async move {
                while filtered_subscriber.wait_for_change().await {
                    if let Some(page) = weak_self.upgrade() {
                        page.render_sections_declaratively().await;
                    }
                }
            });
        }

        // Subscribe to has_content changes for empty state management
        if let Some(has_content) = &*imp.has_content.borrow() {
            let weak_self = self.downgrade();
            let mut content_subscriber = has_content.subscribe();
            glib::spawn_future_local(async move {
                while content_subscriber.wait_for_change().await {
                    if let Some(page) = weak_self.upgrade() {
                        page.update_empty_state().await;
                    }
                }
            });
        }

        // Subscribe to debounced_source_id changes for API calls
        if let Some(debounced_source_id) = &*imp.debounced_source_id.borrow() {
            let weak_self = self.downgrade();
            let mut debounced_subscriber = debounced_source_id.subscribe();
            glib::spawn_future_local(async move {
                while debounced_subscriber.wait_for_change().await {
                    if let Some(page) = weak_self.upgrade() {
                        page.handle_debounced_source_change().await;
                    }
                }
            });
        }
    }

    pub fn refresh(&self) {
        self.refresh_sections();
    }

    // Removed batch_load_visible_cards - no longer needed with simplified approach
}
