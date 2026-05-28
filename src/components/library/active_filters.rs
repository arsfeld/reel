//! Active-filter pill row shown above the grid.
//!
//! Renders one removable pill per active filter value (from
//! `library_filter::pill_labels`) plus a "Clear all" button. The whole row is
//! hidden when no filters are active. Clicking a pill emits
//! `LibraryViewMsg::RemoveActiveFilter`; "Clear all" emits `ClearFilters`.

use gtk::prelude::*;
use relm4::Sender;

use crate::services::library_filter::{FilterState, pill_labels};

use super::LibraryViewMsg;

pub struct ActiveFiltersBar {
    /// Outer container to embed in the library view; hidden when no pills.
    pub container: gtk::Box,
    /// Inner horizontal box holding the pill buttons (rebuilt on update).
    pills_box: gtk::Box,
    sender: Sender<LibraryViewMsg>,
}

impl ActiveFiltersBar {
    pub fn new(sender: Sender<LibraryViewMsg>) -> Self {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .margin_start(16)
            .margin_end(16)
            .margin_top(4)
            .margin_bottom(4)
            .css_classes(["active-filters-bar"])
            .visible(false)
            .build();

        let pills_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .hexpand(true)
            .build();

        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .hexpand(true)
            .child(&pills_box)
            .build();

        let clear_btn = gtk::Button::builder()
            .label("Clear all")
            .css_classes(["flat"])
            .valign(gtk::Align::Center)
            .build();
        let sender_clear = sender.clone();
        clear_btn.connect_clicked(move |_| {
            let _ = sender_clear.send(LibraryViewMsg::ClearFilters);
        });

        container.append(&scroller);
        container.append(&clear_btn);

        Self {
            container,
            pills_box,
            sender,
        }
    }

    /// Rebuild the pill row from the active filter state. Hides the whole row
    /// when there are no active filters.
    pub fn update(&self, state: &FilterState) {
        while let Some(child) = self.pills_box.first_child() {
            self.pills_box.remove(&child);
        }

        let pills = pill_labels(state);
        self.container.set_visible(!pills.is_empty());

        for (tag, label) in pills {
            let pill = gtk::Button::builder()
                .css_classes(["pill", "filter-pill"])
                .build();

            let inner = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(4)
                .build();
            inner.append(&gtk::Label::new(Some(&label)));
            inner.append(&gtk::Image::from_icon_name("window-close-symbolic"));
            pill.set_child(Some(&inner));
            pill.set_tooltip_text(Some(&format!("Remove {label}")));

            let sender = self.sender.clone();
            pill.connect_clicked(move |_| {
                let _ = sender.send(LibraryViewMsg::RemoveActiveFilter(tag.clone()));
            });
            self.pills_box.append(&pill);
        }
    }
}
