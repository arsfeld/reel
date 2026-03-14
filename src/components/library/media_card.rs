use gtk4::prelude::*;
use relm4::typed_view::grid::RelmGridItem;

use crate::models::media::{MediaItem, MediaType};

/// Data backing a single poster card in the library grid.
#[allow(dead_code)]
pub struct MediaCardData {
    pub media_id: String,
    pub title: String,
    pub year: Option<i32>,
    pub poster_texture: Option<gtk4::gdk::Texture>,
    pub media_type: MediaType,
    pub media_item: Option<MediaItem>,
}

impl MediaCardData {
    pub fn from_media_item(item: &MediaItem) -> Self {
        Self {
            media_id: item.id.clone(),
            title: item.title.clone(),
            year: item.year,
            poster_texture: None,
            media_type: item.media_type,
            media_item: Some(item.clone()),
        }
    }
}

pub struct MediaCardWidgets {
    picture: gtk4::Picture,
    title_label: gtk4::Label,
    year_label: gtk4::Label,
}

impl RelmGridItem for MediaCardData {
    type Root = gtk4::Box;
    type Widgets = MediaCardWidgets;

    fn setup(_item: &gtk4::ListItem) -> (Self::Root, Self::Widgets) {
        let container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(4)
            .css_classes(["media-card"])
            .width_request(180)
            .build();

        let picture = gtk4::Picture::builder()
            .content_fit(gtk4::ContentFit::Cover)
            .width_request(180)
            .height_request(270)
            .css_classes(["media-card-poster"])
            .build();

        // Wrap picture in a frame for rounded corners
        let frame = gtk4::Frame::builder()
            .css_classes(["media-card-frame"])
            .child(&picture)
            .build();

        let title_label = gtk4::Label::builder()
            .halign(gtk4::Align::Start)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .max_width_chars(20)
            .css_classes(["media-card-title"])
            .build();

        let year_label = gtk4::Label::builder()
            .halign(gtk4::Align::Start)
            .css_classes(["media-card-year", "dim-label"])
            .build();

        container.append(&frame);
        container.append(&title_label);
        container.append(&year_label);

        let widgets = MediaCardWidgets {
            picture,
            title_label,
            year_label,
        };

        (container, widgets)
    }

    fn bind(&mut self, widgets: &mut Self::Widgets, _root: &mut Self::Root) {
        widgets.title_label.set_label(&self.title);

        if let Some(year) = self.year {
            widgets.year_label.set_label(&year.to_string());
            widgets.year_label.set_visible(true);
        } else {
            widgets.year_label.set_visible(false);
        }

        if let Some(ref texture) = self.poster_texture {
            widgets.picture.set_paintable(Some(texture));
        } else {
            // Placeholder: just show empty with a background color (CSS handles it)
            widgets.picture.set_paintable(None::<&gtk4::gdk::Texture>);
        }
    }
}
