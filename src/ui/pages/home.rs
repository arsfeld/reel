use gtk4::{glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use tracing::{error, info, trace};

use super::library::MediaCard;
use crate::constants::*;
use crate::models::{HomeSection, MediaItem};
use crate::state::AppState;
use crate::utils::{ImageLoader, ImageSize};

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct HomePage {
        pub scrolled_window: gtk4::ScrolledWindow,
        pub main_box: gtk4::Box,
        pub sections: RefCell<Vec<HomeSection>>,
        pub state: RefCell<Option<Arc<AppState>>>,
        pub on_media_selected: RefCell<Option<Box<dyn Fn(&MediaItem)>>>,
        pub image_loader: RefCell<Option<Arc<ImageLoader>>>,
        pub section_cards: RefCell<HashMap<String, Vec<gtk4::Widget>>>,
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
    pub fn new(state: Arc<AppState>) -> Self {
        let page: Self = glib::Object::builder().build();
        page.imp().state.replace(Some(state.clone()));

        // Initialize image loader
        if let Ok(loader) = ImageLoader::new() {
            page.imp().image_loader.replace(Some(Arc::new(loader)));
        }

        // Load homepage data
        page.load_homepage();

        page
    }

    pub fn set_on_media_selected<F>(&self, callback: F)
    where
        F: Fn(&MediaItem) + 'static,
    {
        self.imp()
            .on_media_selected
            .replace(Some(Box::new(callback)));
    }

    fn load_homepage(&self) {
        let imp = self.imp();
        let state = imp.state.borrow().clone().unwrap();
        let page_weak = self.downgrade();

        glib::spawn_future_local(async move {
            let backend_manager = state.backend_manager.read().await;
            if let Some(backend) = backend_manager.get_active() {
                match backend.get_home_sections().await {
                    Ok(sections) => {
                        info!("Loaded {} homepage sections", sections.len());
                        glib::idle_add_local_once(move || {
                            if let Some(page) = page_weak.upgrade() {
                                // Update UI with sections
                                page.imp().sections.replace(sections.clone());
                                page.display_sections(sections);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to load homepage sections: {}", e);
                    }
                }
            }
        });
    }

    fn display_sections(&self, sections: Vec<HomeSection>) {
        let imp = self.imp();
        let main_box = &imp.main_box;

        // Clear existing content
        while let Some(child) = main_box.first_child() {
            main_box.remove(&child);
        }

        // Check if we have sections first
        if sections.is_empty() {
            let empty_state = adw::StatusPage::builder()
                .icon_name("folder-symbolic")
                .title("No Content Available")
                .description("Connect to a media server to see your content here")
                .build();

            main_box.append(&empty_state);
            return;
        }

        // Add each section
        for section in sections {
            if section.items.is_empty() {
                continue;
            }

            // Create section container
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

            // Store section items for lazy loading
            let section_items = Rc::new(section.items.clone());
            let cards_rc = Rc::new(RefCell::new(Vec::new()));

            // Create a deferred loading function
            let self_weak = self.downgrade();
            let items_box_weak = items_box.downgrade();
            let cards_for_create = cards_rc.clone();
            let section_items_for_create = section_items.clone();

            // Function to create cards in batches
            let create_cards_batch = Rc::new(move |start: usize, end: usize| {
                if let Some(page) = self_weak.upgrade()
                    && let Some(items_box) = items_box_weak.upgrade()
                {
                    let mut cards = cards_for_create.borrow_mut();

                    for i in start..end.min(section_items_for_create.len()).min(20) {
                        if i >= cards.len() {
                            // Create card only if not already created
                            if let Some(item) = section_items_for_create.get(i) {
                                let card = page.create_media_card(item);
                                cards.push(card.clone());
                                items_box.append(&card);
                            }
                        }
                    }
                }
            });

            // Defer initial card creation
            let create_initial = create_cards_batch.clone();
            glib::idle_add_local_once(move || {
                create_initial(0, HOME_INITIAL_CARDS_PER_SECTION);
            });

            // Setup scroll handler to create and load more as needed with debouncing
            let cards_for_scroll = cards_rc.clone();
            let create_batch_for_scroll = create_cards_batch.clone();
            let section_id_for_scroll = section.id.clone();
            let self_weak_for_scroll = self.downgrade();
            let scroll_counter: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));

            scrolled.hadjustment().connect_value_changed(move |h_adj| {
                let value = h_adj.value();
                let page_size = h_adj.page_size();
                let counter = scroll_counter.clone();

                // Increment counter for this scroll event
                let current_count = {
                    let mut c = counter.borrow_mut();
                    *c += 1;
                    *c
                };

                // Debounce the actual loading
                let cards_for_load = cards_for_scroll.clone();
                let create_batch_for_load = create_batch_for_scroll.clone();
                let section_id_for_load = section_id_for_scroll.clone();
                let self_weak_for_load = self_weak_for_scroll.clone();
                let counter_inner = counter.clone();

                glib::timeout_add_local(
                    std::time::Duration::from_millis(SCROLL_DEBOUNCE_MS),
                    move || {
                        // Check if this is still the latest scroll event
                        if *counter_inner.borrow() != current_count {
                            return glib::ControlFlow::Break;
                        }

                        // Calculate which cards are visible
                        let card_width = 144.0; // Small card width + spacing (132 + 12)
                        let start_idx = (value / card_width).floor() as usize;
                        let end_idx = ((value + page_size) / card_width).ceil() as usize + 3; // +3 for buffer

                        // Create cards if needed
                        create_batch_for_load(start_idx, end_idx);

                        // Batch load visible cards instead of loading one by one
                        if let Some(page) = self_weak_for_load.upgrade() {
                            page.batch_load_visible_cards(&section_id_for_load, start_idx, end_idx);
                        } else {
                            // Fallback to individual loading if batch loading unavailable
                            let cards = cards_for_load.borrow();
                            for i in start_idx..end_idx.min(cards.len()) {
                                if let Some(card) = cards.get(i)
                                    && let Some(media_card) =
                                        card.downcast_ref::<super::library::MediaCard>()
                                {
                                    media_card.trigger_load(ImageSize::Small);
                                }
                            }
                        }

                        glib::ControlFlow::Break
                    },
                );
            });

            // Trigger initial load when scrolled window is mapped with delay
            let cards_for_map = cards_rc.clone();
            scrolled.connect_map(move |_| {
                // Small delay to let UI settle
                let cards_clone = cards_for_map.clone();
                glib::timeout_add_local_once(
                    std::time::Duration::from_millis(INITIAL_LOAD_DELAY_MS),
                    move || {
                        let cards = cards_clone.borrow();
                        // Load visible cards
                        for (i, card) in cards.iter().enumerate() {
                            if i < HOME_INITIAL_IMAGES_PER_SECTION {
                                if let Some(media_card) =
                                    card.downcast_ref::<super::library::MediaCard>()
                                {
                                    media_card.trigger_load(ImageSize::Small);
                                }
                            } else {
                                break;
                            }
                        }
                    },
                );
            });

            scrolled.set_child(Some(&items_box));
            section_box.append(&scrolled);

            main_box.append(&section_box);
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

    pub fn refresh(&self) {
        self.load_homepage();
    }

    /// Batch load images for visible cards
    fn batch_load_visible_cards(&self, section_id: &str, start_idx: usize, end_idx: usize) {
        let image_loader = match self.imp().image_loader.borrow().as_ref() {
            Some(loader) => loader.clone(),
            None => return,
        };

        let sections = self.imp().sections.borrow();
        let section = match sections.iter().find(|s| s.id == section_id) {
            Some(s) => s,
            None => return,
        };

        // Only process if we have items in range
        if start_idx >= section.items.len() {
            return;
        }

        // Collect URLs for batch loading - only items that might need loading
        let mut batch_requests = Vec::new();
        let items_to_load =
            &section.items[start_idx.min(section.items.len())..end_idx.min(section.items.len())];

        for item in items_to_load {
            if let Some(url) = match item {
                MediaItem::Movie(m) => m.poster_url.as_ref(),
                MediaItem::Show(s) => s.poster_url.as_ref(),
                MediaItem::Episode(e) => e.thumbnail_url.as_ref(),
                _ => None,
            } {
                batch_requests.push((url.clone(), ImageSize::Small));
            }
        }

        if !batch_requests.is_empty() {
            // Only log at trace level to reduce noise
            trace!(
                "Batch loading {} images for section {} (indices {}-{})",
                batch_requests.len(),
                section_id,
                start_idx,
                end_idx
            );

            // Load images in background - the image loader will handle deduplication
            glib::spawn_future_local(async move {
                let _ = image_loader.batch_load(batch_requests).await;
            });
        }
    }
}
