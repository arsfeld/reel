use gtk4::{glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use tracing::{error, info};

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
        pub section_widgets: RefCell<HashMap<String, SectionWidgets>>,
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
                                page.sync_sections(sections);
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

    fn sync_sections(&self, new_sections: Vec<HomeSection>) {
        let imp = self.imp();
        let main_box = &imp.main_box;
        let mut section_widgets = imp.section_widgets.borrow_mut();
        let old_sections = imp.sections.borrow();

        // Build a set of new section IDs for quick lookup
        let new_section_ids: Vec<String> = new_sections.iter().map(|s| s.id.clone()).collect();

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
        for (index, section) in new_sections.iter().enumerate() {
            if section.items.is_empty() {
                continue;
            }

            if let Some(widgets) = section_widgets.get(&section.id) {
                // Section exists - update its items if needed
                let old_section = old_sections.iter().find(|s| s.id == section.id);
                if let Some(old) = old_section {
                    if !Self::items_equal(&old.items, &section.items) {
                        self.update_section_items(widgets, section);
                    }
                }

                // Ensure it's at the right position by moving to end
                // (GTK doesn't have reorder_child_after in GTK4)
                main_box.remove(&widgets.container);
                main_box.append(&widgets.container);
            } else {
                // New section - create it
                let widgets = self.create_section_widget(section);
                main_box.append(&widgets.container);
                section_widgets.insert(section.id.clone(), widgets);
            }
        }

        // Update stored sections
        drop(old_sections);
        imp.sections.replace(new_sections);

        // Show empty state if no sections
        if section_widgets.is_empty() {
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

        // Add new items
        for item in &section.items[..section.items.len().min(20)] {
            let card = self.create_media_card(item);
            items_box.append(&card);
        }

        // Trigger load on visible items
        glib::timeout_add_local_once(std::time::Duration::from_millis(100), {
            let items_box = items_box.clone();
            move || {
                let mut child = items_box.first_child();
                let mut count = 0;
                while let Some(widget) = child {
                    if count >= HOME_INITIAL_IMAGES_PER_SECTION {
                        break;
                    }
                    if let Some(media_card) = widget.downcast_ref::<super::library::MediaCard>() {
                        media_card.trigger_load(ImageSize::Small);
                    }
                    child = widget.next_sibling();
                    count += 1;
                }
            }
        });
    }

    fn create_section_widget(&self, section: &HomeSection) -> imp::SectionWidgets {
        // This is essentially the old display_sections code for a single section
        // but returning the widgets for tracking

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

        // Setup scroll handler for lazy loading
        let section_items = Rc::new(section.items.clone());
        let cards_rc = Rc::new(RefCell::new(cards.clone()));
        let self_weak = self.downgrade();
        let items_box_weak = items_box.downgrade();
        let scroll_counter: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));

        scrolled.hadjustment().connect_value_changed(move |h_adj| {
            let value = h_adj.value();
            let page_size = h_adj.page_size();
            let counter = scroll_counter.clone();
            let current_count = {
                let mut c = counter.borrow_mut();
                *c += 1;
                *c
            };

            let cards_for_load = cards_rc.clone();
            let section_items_for_create = section_items.clone();
            let self_weak_for_create = self_weak.clone();
            let items_box_weak_for_create = items_box_weak.clone();
            let counter_inner = counter.clone();

            glib::timeout_add_local(
                std::time::Duration::from_millis(SCROLL_DEBOUNCE_MS),
                move || {
                    if *counter_inner.borrow() != current_count {
                        return glib::ControlFlow::Break;
                    }

                    // Calculate which cards are visible
                    let card_width = 144.0;
                    let start_idx = (value / card_width).floor() as usize;
                    let end_idx = ((value + page_size) / card_width).ceil() as usize + 3;

                    // Create cards if needed
                    if let Some(page) = self_weak_for_create.upgrade()
                        && let Some(items_box) = items_box_weak_for_create.upgrade()
                    {
                        let mut cards = cards_for_load.borrow_mut();
                        for i in cards.len()..end_idx.min(section_items_for_create.len()).min(20) {
                            if let Some(item) = section_items_for_create.get(i) {
                                let card = page.create_media_card(item);
                                items_box.append(&card);
                                cards.push(card.clone());
                            }
                        }

                        // Trigger load on visible cards
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

        // Trigger initial loads
        glib::timeout_add_local_once(std::time::Duration::from_millis(100), {
            let cards = cards.clone();
            move || {
                for (i, card) in cards.iter().enumerate() {
                    if i >= HOME_INITIAL_IMAGES_PER_SECTION {
                        break;
                    }
                    if let Some(media_card) = card.downcast_ref::<super::library::MediaCard>() {
                        media_card.trigger_load(ImageSize::Small);
                    }
                }
            }
        });

        scrolled.set_child(Some(&items_box));
        section_box.append(&scrolled);

        imp::SectionWidgets {
            container: section_box,
            items_box,
            scrolled,
            cards,
        }
    }

    // Old display_sections becomes unused
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
            let cards_for_initial = cards_rc.clone();
            glib::idle_add_local_once(move || {
                create_initial(0, HOME_INITIAL_CARDS_PER_SECTION);

                // Immediately trigger load on initial cards
                glib::timeout_add_local_once(std::time::Duration::from_millis(100), move || {
                    let cards = cards_for_initial.borrow();
                    for (i, card) in cards.iter().enumerate() {
                        if i >= HOME_INITIAL_IMAGES_PER_SECTION {
                            break;
                        }
                        if let Some(media_card) = card.downcast_ref::<super::library::MediaCard>() {
                            media_card.trigger_load(ImageSize::Small);
                        }
                    }
                });
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

                        // Simplified: Just trigger load on visible cards directly
                        let cards = cards_for_load.borrow();
                        for i in start_idx..end_idx.min(cards.len()) {
                            if let Some(card) = cards.get(i)
                                && let Some(media_card) =
                                    card.downcast_ref::<super::library::MediaCard>()
                            {
                                media_card.trigger_load(ImageSize::Small);
                            }
                        }

                        glib::ControlFlow::Break
                    },
                );
            });

            // Removed redundant trigger_initial_loads - cards already load in create_initial above

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

    // Removed batch_load_visible_cards - no longer needed with simplified approach
}
