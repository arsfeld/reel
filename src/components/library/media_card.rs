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
    pub poster_url: Option<String>,
    pub media_type: MediaType,
    pub media_item: Option<MediaItem>,
    /// Video resolution for badge display (e.g., "1080", "4k").
    pub video_resolution: Option<String>,
    /// Numeric rating for badge display (e.g., 8.0).
    pub rating: Option<f64>,
    /// Card dimensions (set by density; default 180x270).
    pub card_width: i32,
    pub card_height: i32,
}

impl MediaCardData {
    pub fn from_media_item(item: &MediaItem) -> Self {
        Self {
            media_id: item.id.clone(),
            title: item.title.clone(),
            year: item.year,
            poster_texture: None,
            poster_url: None,
            media_type: item.media_type,
            media_item: Some(item.clone()),
            video_resolution: item.video_resolution.clone(),
            rating: item.rating,
            card_width: 180,
            card_height: 270,
        }
    }
}

pub struct MediaCardWidgets {
    picture: gtk4::Picture,
    placeholder_icon: gtk4::Image,
    #[allow(dead_code)]
    frame: gtk4::Frame,
    title_label: gtk4::Label,
    year_label: gtk4::Label,
    resolution_badge: gtk4::Label,
    rating_badge: gtk4::Label,
    #[allow(dead_code)]
    progress_bar: gtk4::ProgressBar,
}

impl RelmGridItem for MediaCardData {
    type Root = gtk4::Box;
    type Widgets = MediaCardWidgets;

    fn setup(_item: &gtk4::ListItem) -> (Self::Root, Self::Widgets) {
        let container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(0)
            .css_classes(["media-card"])
            .width_request(180)
            .build();

        let picture = gtk4::Picture::builder()
            .content_fit(gtk4::ContentFit::Cover)
            .width_request(180)
            .height_request(270)
            .css_classes(["media-card-poster"])
            .build();

        // Placeholder icon shown when no poster texture is loaded
        let placeholder_icon = gtk4::Image::builder()
            .icon_name("video-x-generic-symbolic")
            .pixel_size(48)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .css_classes(["media-card-placeholder-icon"])
            .visible(false)
            .build();

        // Resolution badge (top-right)
        let resolution_badge = gtk4::Label::builder()
            .halign(gtk4::Align::End)
            .valign(gtk4::Align::Start)
            .margin_top(8)
            .margin_end(8)
            .css_classes(["media-badge", "resolution-badge"])
            .visible(false)
            .build();

        // Rating badge (top-left)
        let rating_badge = gtk4::Label::builder()
            .halign(gtk4::Align::Start)
            .valign(gtk4::Align::Start)
            .margin_top(8)
            .margin_start(8)
            .css_classes(["media-badge", "rating-badge"])
            .visible(false)
            .build();

        // Watch progress bar (bottom, hidden until M4)
        let progress_bar = gtk4::ProgressBar::builder()
            .halign(gtk4::Align::Fill)
            .valign(gtk4::Align::End)
            .css_classes(["watch-progress"])
            .visible(false)
            .build();

        // Overlay stacks badges, placeholder icon, and progress on top of the poster
        let overlay = gtk4::Overlay::new();
        overlay.set_child(Some(&picture));
        overlay.add_overlay(&placeholder_icon);
        overlay.add_overlay(&resolution_badge);
        overlay.add_overlay(&rating_badge);
        overlay.add_overlay(&progress_bar);

        // Wrap overlay in a frame for rounded corners + hover effects
        let frame = gtk4::Frame::builder()
            .css_classes(["media-card-frame"])
            .child(&overlay)
            .build();

        let title_label = gtk4::Label::builder()
            .halign(gtk4::Align::Start)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .max_width_chars(20)
            .lines(2)
            .wrap(true)
            .wrap_mode(gtk4::pango::WrapMode::WordChar)
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
            placeholder_icon,
            frame,
            title_label,
            year_label,
            resolution_badge,
            rating_badge,
            progress_bar,
        };

        (container, widgets)
    }

    fn bind(&mut self, widgets: &mut Self::Widgets, root: &mut Self::Root) {
        // Update card dimensions based on density
        root.set_width_request(self.card_width);
        widgets.picture.set_width_request(self.card_width);
        widgets.picture.set_height_request(self.card_height);

        widgets.title_label.set_label(&self.title);

        if let Some(year) = self.year {
            widgets.year_label.set_label(&year.to_string());
            widgets.year_label.set_visible(true);
        } else {
            widgets.year_label.set_visible(false);
        }

        let has_poster = self.poster_texture.is_some();

        if let Some(ref texture) = self.poster_texture {
            widgets.picture.set_paintable(Some(texture));
            widgets.picture.remove_css_class("loading");
            widgets.picture.set_opacity(1.0);
            widgets.placeholder_icon.set_visible(false);
        } else {
            widgets.picture.set_paintable(None::<&gtk4::gdk::Texture>);
            widgets.picture.add_css_class("loading");
            widgets.picture.set_opacity(0.0);
            widgets.placeholder_icon.set_visible(true);
        }

        // Resolution badge: visible only with poster + resolution data
        if has_poster {
            if let Some(ref res) = self.video_resolution {
                let label = MediaItem::format_resolution(res);
                widgets.resolution_badge.set_label(label);
                widgets.resolution_badge.set_visible(true);
            } else {
                widgets.resolution_badge.set_visible(false);
            }
        } else {
            widgets.resolution_badge.set_visible(false);
        }

        // Rating badge: visible only with poster + valid rating
        if has_poster {
            if let Some(rating) = self.rating {
                if rating > 0.0 {
                    widgets.rating_badge.set_label(&format!("{rating:.1}"));
                    widgets.rating_badge.set_visible(true);
                } else {
                    widgets.rating_badge.set_visible(false);
                }
            } else {
                widgets.rating_badge.set_visible(false);
            }
        } else {
            widgets.rating_badge.set_visible(false);
        }
    }
}
