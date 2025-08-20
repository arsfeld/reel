use gtk4::{gio, glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::models::{HomeSection, HomeSectionType, MediaItem};
use crate::state::AppState;
use crate::utils::ImageSize;
use super::library::MediaCard;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct HomePage {
        pub scrolled_window: gtk4::ScrolledWindow,
        pub main_box: gtk4::Box,
        pub sections: RefCell<Vec<HomeSection>>,
        pub state: RefCell<Option<Arc<AppState>>>,
        pub on_media_selected: RefCell<Option<Box<dyn Fn(&MediaItem)>>>,
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
            
            self.scrolled_window.set_hscrollbar_policy(gtk4::PolicyType::Never);
            self.scrolled_window.set_vscrollbar_policy(gtk4::PolicyType::Automatic);
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
        
        // Load homepage data
        page.load_homepage();
        
        page
    }
    
    pub fn set_on_media_selected<F>(&self, callback: F)
    where
        F: Fn(&MediaItem) + 'static,
    {
        self.imp().on_media_selected.replace(Some(Box::new(callback)));
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
            
            // Add media cards for each item (show max 20 items per section)
            for item in section.items.iter().take(20) {
                let card = self.create_media_card(item);
                items_box.append(&card);
            }
            
            scrolled.set_child(Some(&items_box));
            section_box.append(&scrolled);
            
            main_box.append(&section_box);
        }
    }
    
    fn create_media_card(&self, item: &MediaItem) -> gtk4::Widget {
        // Use medium size for homepage cards
        let card = MediaCard::new(item.clone(), ImageSize::Medium);
        // Trigger image loading immediately for homepage cards
        card.trigger_load(ImageSize::Medium);
        
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
}