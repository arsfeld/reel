use gtk4::{gio, glib, prelude::*, subclass::prelude::*};
use std::cell::RefCell;
use std::sync::Arc;
use tracing::{debug, trace};

use crate::models::MediaItem;

// Virtual list model that implements GListModel for efficient scrolling
mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct VirtualMediaListModel {
        pub items: RefCell<Vec<MediaItem>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VirtualMediaListModel {
        const NAME: &'static str = "VirtualMediaListModel";
        type Type = super::VirtualMediaListModel;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for VirtualMediaListModel {}

    impl ListModelImpl for VirtualMediaListModel {
        fn item_type(&self) -> glib::Type {
            MediaItemObject::static_type()
        }

        fn n_items(&self) -> u32 {
            self.items.borrow().len() as u32
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
            let items = self.items.borrow();
            items
                .get(position as usize)
                .map(|item| MediaItemObject::new(item.clone()).upcast())
        }
    }
}

glib::wrapper! {
    pub struct VirtualMediaListModel(ObjectSubclass<imp::VirtualMediaListModel>)
        @implements gio::ListModel;
}

impl VirtualMediaListModel {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn set_items(&self, items: Vec<MediaItem>) {
        let imp = self.imp();
        let old_len = imp.items.borrow().len();
        imp.items.replace(items.clone());
        let new_len = items.len();

        // Notify ListModel about the change
        if old_len > 0 {
            self.items_changed(0, old_len as u32, 0);
        }
        if new_len > 0 {
            self.items_changed(0, 0, new_len as u32);
        }

        debug!("VirtualMediaListModel updated with {} items", new_len);
    }

    pub fn append_items(&self, items: Vec<MediaItem>) {
        let imp = self.imp();
        let old_len = imp.items.borrow().len();
        imp.items.borrow_mut().extend(items.clone());

        // Notify about appended items
        self.items_changed(old_len as u32, 0, items.len() as u32);

        trace!("Appended {} items to VirtualMediaListModel", items.len());
    }

    pub fn clear(&self) {
        let imp = self.imp();
        let old_len = imp.items.borrow().len();
        if old_len > 0 {
            imp.items.borrow_mut().clear();
            self.items_changed(0, old_len as u32, 0);
        }
    }

    pub fn get_item(&self, position: u32) -> Option<MediaItem> {
        self.imp().items.borrow().get(position as usize).cloned()
    }

    pub fn update_item(&self, position: u32, item: MediaItem) {
        let imp = self.imp();
        let mut items = imp.items.borrow_mut();
        if let Some(existing) = items.get_mut(position as usize) {
            *existing = item;
            drop(items);
            // Notify about single item change
            self.items_changed(position, 1, 1);
        }
    }

    pub fn len(&self) -> usize {
        self.imp().items.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.imp().items.borrow().is_empty()
    }
}

// GObject wrapper for MediaItem to use in ListView
mod imp_item {
    use super::*;

    #[derive(Debug)]
    pub struct MediaItemObject {
        pub item: RefCell<MediaItem>,
    }

    impl Default for MediaItemObject {
        fn default() -> Self {
            // Create a dummy item for initialization
            Self {
                item: RefCell::new(MediaItem::Movie(crate::models::Movie {
                    id: String::new(),
                    backend_id: String::new(),
                    title: String::new(),
                    year: None,
                    duration: std::time::Duration::from_secs(0),
                    rating: None,
                    poster_url: None,
                    backdrop_url: None,
                    overview: None,
                    genres: Vec::new(),
                    cast: Vec::new(),
                    crew: Vec::new(),
                    added_at: None,
                    updated_at: None,
                    watched: false,
                    view_count: 0,
                    last_watched_at: None,
                    playback_position: None,
                    intro_marker: None,
                    credits_marker: None,
                })),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MediaItemObject {
        const NAME: &'static str = "MediaItemObject";
        type Type = super::MediaItemObject;
    }

    impl ObjectImpl for MediaItemObject {
        fn properties() -> &'static [glib::ParamSpec] {
            use once_cell::sync::Lazy;
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecString::builder("title").readwrite().build(),
                    glib::ParamSpecString::builder("id").readwrite().build(),
                    glib::ParamSpecBoolean::builder("watched")
                        .readwrite()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let item = self.item.borrow();
            match pspec.name() {
                "title" => item.title().to_value(),
                "id" => item.id().to_value(),
                "watched" => item.is_watched().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "title" | "id" | "watched" => {
                    // These are read-only in practice
                }
                _ => unimplemented!(),
            }
        }
    }
}

glib::wrapper! {
    pub struct MediaItemObject(ObjectSubclass<imp_item::MediaItemObject>);
}

impl MediaItemObject {
    pub fn new(item: MediaItem) -> Self {
        let obj: Self = glib::Object::new();
        obj.imp().item.replace(item);
        obj
    }

    pub fn item(&self) -> MediaItem {
        self.imp().item.borrow().clone()
    }

    pub fn update(&self, item: MediaItem) {
        self.imp().item.replace(item);
        self.notify("title");
        self.notify("watched");
    }
}
