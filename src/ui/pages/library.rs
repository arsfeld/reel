use gtk4::{gio, glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tracing::{info, error, debug};
use once_cell::sync::Lazy;
use tokio;
use anyhow;

use crate::models::{Library, LibraryType, MediaItem};
use crate::state::AppState;
use crate::utils::{OptimizedImageLoader, ImageSize};

// Global optimized image loader instance
static IMAGE_LOADER: Lazy<OptimizedImageLoader> = Lazy::new(|| {
    OptimizedImageLoader::new().expect("Failed to create OptimizedImageLoader")
});

mod imp {
    use super::*;
    
    #[derive(Debug, Default)]
    pub struct LibraryView {
        pub scrolled_window: RefCell<Option<gtk4::ScrolledWindow>>,
        pub flow_box: RefCell<Option<gtk4::FlowBox>>,
        pub loading_spinner: RefCell<Option<gtk4::Spinner>>,
        pub empty_state: RefCell<Option<adw::StatusPage>>,
        pub stack: RefCell<Option<gtk4::Stack>>,
        
        pub state: RefCell<Option<Arc<AppState>>>,
        pub library: RefCell<Option<Library>>,
        pub backend_id: RefCell<Option<String>>,
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
        view
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
            .column_spacing(12)  // Gap between posters
            .row_spacing(16)     // Space between rows
            .homogeneous(false)  // Allow dynamic sizing
            .min_children_per_line(2)  // Minimum 2 posters per row
            .max_children_per_line(30) // Allow many columns for ultra-wide screens
            .selection_mode(gtk4::SelectionMode::None)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .valign(gtk4::Align::Start)
            .build();
        
        scrolled_window.set_child(Some(&flow_box));
        
        // Remove the responsive code that was setting min/max to same value
        // The FlowBox will handle responsiveness automatically with its min/max settings
        
        stack.add_named(&scrolled_window, Some("content"));
        
        // Empty state
        let empty_state = adw::StatusPage::builder()
            .title("No Content")
            .description("This library doesn't have any items yet")
            .icon_name("folder-symbolic")
            .build();
        
        stack.add_named(&empty_state, Some("empty"));
        
        // Add stack directly to the view (no header bar)
        self.append(&stack);
        
        // Store references
        imp.scrolled_window.replace(Some(scrolled_window));
        imp.flow_box.replace(Some(flow_box.clone()));
        imp.loading_spinner.replace(Some(loading_spinner));
        imp.empty_state.replace(Some(empty_state));
        imp.stack.replace(Some(stack));
        
        // Store the flow box reference before connecting signals
        let _flow_box_clone = flow_box.clone();
        
        // Connect flow box child activation
        flow_box.connect_child_activated(glib::clone!(@weak self as view => move |_, child| {
            if let Some(flow_child) = child.downcast_ref::<gtk4::FlowBoxChild>() {
                if let Some(card) = flow_child.child().and_then(|w| w.downcast::<MediaCard>().ok()) {
                    let media_item = card.media_item();
                    info!("Media item selected: {}", media_item.title());
                    // TODO: Navigate to media detail view
                }
            }
        }));
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
                match sync_manager.get_cached_movies(&backend_id, &library.id).await {
                    Ok(movies) => movies.into_iter()
                        .map(|m| MediaItem::Movie(m))
                        .collect::<Vec<_>>(),
                    Err(e) => {
                        error!("Failed to load movies: {}", e);
                        Vec::new()
                    }
                }
            }
            LibraryType::Shows => {
                match sync_manager.get_cached_shows(&backend_id, &library.id).await {
                    Ok(shows) => shows.into_iter()
                        .map(|s| MediaItem::Show(s))
                        .collect::<Vec<_>>(),
                    Err(e) => {
                        error!("Failed to load shows: {}", e);
                        Vec::new()
                    }
                }
            }
            _ => {
                info!("Library type {:?} not yet implemented", library.library_type);
                Vec::new()
            }
        };
        
        // Update UI with media items
        self.display_media_items(media_items);
    }
    
    fn display_media_items(&self, items: Vec<MediaItem>) {
        let imp = self.imp();
        
        let flow_box = imp.flow_box.borrow().clone();
        let scrolled_window = imp.scrolled_window.borrow().clone();
        let stack = imp.stack.borrow().clone();
        
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
                // Add media cards
                let mut cards = Vec::new();
                for item in items {
                    let card = MediaCard::new(item);
                    let child = gtk4::FlowBoxChild::new();
                    child.set_child(Some(&card));
                    flow_box.append(&child);
                    cards.push(card);
                }
                
                // Show content
                if let Some(stack) = stack {
                    stack.set_visible_child_name("content");
                }
                
                // Set up lazy loading for the scrolled window
                if let Some(scrolled_window) = scrolled_window {
                    self.setup_lazy_loading(scrolled_window, flow_box.clone());
                }
                
                // Initially load first visible items
                let flow_box_clone = flow_box.clone();
                glib::idle_add_local_once(move || {
                    Self::load_visible_cards(&flow_box_clone, 30); // Load first 30 items for smoother initial experience
                });
            }
        }
    }
    
    fn setup_lazy_loading(&self, scrolled_window: gtk4::ScrolledWindow, flow_box: gtk4::FlowBox) {
        let adjustment = scrolled_window.vadjustment();
        
        // Use a counter instead of trying to remove sources
        let update_counter: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        
        adjustment.connect_value_changed(move |adj| {
            let flow_box = flow_box.clone();
            let counter = update_counter.clone();
            
            // Get values we need from adjustment
            let viewport_top = adj.value();
            let viewport_height = adj.page_size();
            
            // Increment counter to invalidate any pending updates
            let current_count = {
                let mut c = counter.borrow_mut();
                *c += 1;
                *c
            };
            
            // Set new timer with shorter delay for more responsive loading
            let counter_inner = counter.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                // Check if this timer is still valid
                if *counter_inner.borrow() != current_count {
                    // A newer timer was scheduled, skip this one
                    return glib::ControlFlow::Break;
                }
                
                let viewport_bottom = viewport_top + viewport_height;
                
                // Calculate visible range with larger prefetch margin for smoother scrolling
                let prefetch_margin = viewport_height * 2.0; // Prefetch 2 screens ahead and behind
                let visible_top = (viewport_top - prefetch_margin).max(0.0);
                let visible_bottom = viewport_bottom + prefetch_margin;
                
                Self::load_cards_in_range(&flow_box, visible_top, visible_bottom);
                
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
            
            if let Some(fc) = flow_child.downcast_ref::<gtk4::FlowBoxChild>() {
                if let Some(card) = fc.child().and_then(|w| w.downcast::<MediaCard>().ok()) {
                    card.trigger_load();
                    loaded += 1;
                }
            }
            
            child = flow_child.next_sibling();
        }
    }
    
    fn load_cards_in_range(flow_box: &gtk4::FlowBox, visible_top: f64, visible_bottom: f64) {
        let mut child = flow_box.first_child();
        let mut cards_to_load = Vec::new();
        
        while let Some(flow_child) = child {
            if let Some(fc) = flow_child.downcast_ref::<gtk4::FlowBoxChild>() {
                // Get the child's position
                let allocation = fc.allocation();
                let child_top = allocation.y() as f64;
                let child_bottom = child_top + allocation.height() as f64;
                
                // Check if child is in or near visible range (start loading before fully visible)
                // Add extra margin so items start loading before they come into view
                let load_margin = 100.0; // Start loading 100 pixels before visible
                if child_bottom >= (visible_top - load_margin) && child_top <= (visible_bottom + load_margin) {
                    if let Some(card) = fc.child().and_then(|w| w.downcast::<MediaCard>().ok()) {
                        cards_to_load.push(card);
                    }
                }
            }
            
            child = flow_child.next_sibling();
        }
        
        // Trigger all loads at once - the semaphore will handle concurrency limiting
        // This ensures maximum parallelism for faster loading
        for card in cards_to_load {
            card.trigger_load();
        }
    }
    
    pub fn navigate_back(&self) {
        info!("Navigating back to libraries");
        // Find the parent window and switch back to libraries view
        let mut widget: Option<gtk4::Widget> = self.parent();
        while let Some(w) = widget {
            if w.type_() == crate::ui::window_blueprint::ReelMainWindow::static_type() {
                // We found the main window
                if let Some(window) = w.downcast_ref::<crate::ui::window_blueprint::ReelMainWindow>() {
                    window.show_libraries_view();
                }
                break;
            }
            widget = w.parent();
        }
    }
}

// Generic Media Card Widget
mod imp_card {
    use super::*;
    use glib::source::SourceId;
    
    #[derive(Debug, Default)]
    pub struct MediaCard {
        pub overlay: RefCell<Option<gtk4::Overlay>>,
        pub image: RefCell<Option<gtk4::Picture>>,
        pub info_box: RefCell<Option<gtk4::Box>>,
        pub title_label: RefCell<Option<gtk4::Label>>,
        pub subtitle_label: RefCell<Option<gtk4::Label>>,
        pub media_item: RefCell<Option<MediaItem>>,
        pub loading_spinner: RefCell<Option<gtk4::Spinner>>,
        pub image_loaded: RefCell<bool>,
        pub image_loading: RefCell<bool>,
        pub load_handle: RefCell<Option<SourceId>>,
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
    pub fn new(media_item: MediaItem) -> Self {
        let card: Self = glib::Object::builder()
            .build();
        
        card.imp().media_item.replace(Some(media_item.clone()));
        card.update_content(media_item);
        card
    }
    
    fn setup_card_ui(&self) {
        let imp = self.imp();
        
        // Remove button styling and add card styling
        self.add_css_class("flat");
        self.add_css_class("media-card");
        self.add_css_class("poster-card");
        
        // Create overlay for poster and info
        let overlay = gtk4::Overlay::new();
        overlay.add_css_class("poster-overlay");
        
        // Create picture for poster/thumbnail with proper movie poster aspect ratio (2:3)
        let image = gtk4::Picture::builder()
            .width_request(120)   // Smaller width for poster
            .height_request(180)  // Height maintains 2:3 aspect ratio
            .content_fit(gtk4::ContentFit::Cover)
            .build();
        
        // Add rounded corners to the image
        image.add_css_class("rounded-poster");
        
        // Set placeholder image
        self.set_placeholder_image(&image);
        
        overlay.set_child(Some(&image));
        
        // Add loading spinner overlay
        let spinner_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .build();
        
        let loading_spinner = gtk4::Spinner::builder()
            .spinning(false)
            .width_request(32)
            .height_request(32)
            .build();
        
        spinner_box.append(&loading_spinner);
        overlay.add_overlay(&spinner_box);
        
        // Create info box at the bottom with gradient background
        let info_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(4)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .valign(gtk4::Align::End)
            .build();
        
        info_box.add_css_class("media-card-info");
        info_box.add_css_class("poster-info-gradient");
        
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
        
        overlay.add_overlay(&info_box);
        
        self.set_child(Some(&overlay));
        
        // Store references
        imp.overlay.replace(Some(overlay));
        imp.image.replace(Some(image));
        imp.info_box.replace(Some(info_box));
        imp.title_label.replace(Some(title_label));
        imp.subtitle_label.replace(Some(subtitle_label));
        imp.loading_spinner.replace(Some(loading_spinner));
    }
    
    fn set_placeholder_image(&self, picture: &gtk4::Picture) {
        // Use theme icon as placeholder
        let icon_theme = gtk4::IconTheme::for_display(&self.display());
        let icon = icon_theme.lookup_icon(
            "video-x-generic-symbolic",
            &[],
            128,
            1,
            gtk4::TextDirection::Ltr,
            gtk4::IconLookupFlags::empty()
        );
        picture.set_paintable(Some(&icon));
    }
    
    fn update_content(&self, media_item: MediaItem) {
        let imp = self.imp();
        
        // Update labels based on media type
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
                    let season_count = show.seasons.len();
                    if season_count == 1 {
                        "1 season".to_string()
                    } else {
                        format!("{} seasons", season_count)
                    }
                }
                _ => String::new(),
            };
            subtitle_label.set_text(&subtitle);
        }
        
        // Don't load image immediately - wait for visibility check
        // The image will be loaded when the card becomes visible
    }
    
    pub fn trigger_load(&self) {
        let imp = self.imp();
        
        // Check if already loaded or loading
        if *imp.image_loaded.borrow() || *imp.image_loading.borrow() {
            return;
        }
        
        // Mark as loading
        *imp.image_loading.borrow_mut() = true;
        
        // Get the media item
        if let Some(media_item) = imp.media_item.borrow().as_ref() {
            self.load_poster_image(media_item);
        }
    }
    
    fn load_poster_image(&self, media_item: &MediaItem) {
        let poster_url = match media_item {
            MediaItem::Movie(movie) => movie.poster_url.clone(),
            MediaItem::Show(show) => show.poster_url.clone(),
            _ => None,
        };
        
        if let Some(url) = poster_url {
            let imp = self.imp();
            
            // Show loading spinner
            if let Some(spinner) = imp.loading_spinner.borrow().as_ref() {
                spinner.set_spinning(true);
                spinner.set_visible(true);
            }
            
            // Clone references for async closure
            let image_ref = imp.image.borrow().as_ref().unwrap().clone();
            let spinner_ref = imp.loading_spinner.borrow().as_ref().unwrap().clone();
            let weak_self = self.downgrade();
            
            // Load image asynchronously using glib spawn for GTK compatibility
            // Use Medium size for poster thumbnails in the grid view
            glib::spawn_future_local(async move {
                match IMAGE_LOADER.load_image(&url, ImageSize::Medium).await {
                    Ok(texture) => {
                        let image_ref = image_ref.clone();
                        let spinner_ref = spinner_ref.clone();
                        let weak_self = weak_self.clone();
                        glib::idle_add_local_once(move || {
                            if let Some(card) = weak_self.upgrade() {
                                image_ref.set_paintable(Some(&texture));
                                spinner_ref.set_spinning(false);
                                spinner_ref.set_visible(false);
                                
                                // Mark as loaded
                                let imp = card.imp();
                                *imp.image_loaded.borrow_mut() = true;
                                *imp.image_loading.borrow_mut() = false;
                                
                                debug!("Successfully loaded poster image");
                            }
                        });
                    }
                    Err(e) => {
                        let image_ref = image_ref.clone();
                        let spinner_ref = spinner_ref.clone();
                        let weak_self = weak_self.clone();
                        glib::idle_add_local_once(move || {
                            error!("Failed to load poster image: {}", e);
                            spinner_ref.set_spinning(false);
                            spinner_ref.set_visible(false);
                            
                            // Keep placeholder image on error  
                            if let Some(card) = weak_self.upgrade() {
                                card.set_placeholder_image(&image_ref);
                                
                                // Mark as not loading anymore
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