use gtk::prelude::*;
use relm4::typed_view::grid::RelmGridItem;

use crate::models::media::{MediaItem, MediaType};

/// Data backing a single poster card in the library grid.
#[allow(dead_code)]
pub struct MediaCardData {
    pub media_id: String,
    pub title: String,
    pub year: Option<i32>,
    pub poster_texture: Option<gtk::gdk::Texture>,
    pub poster_url: Option<String>,
    pub media_type: MediaType,
    pub media_item: Option<MediaItem>,
    /// Video resolution for badge display (e.g., "1080", "4k").
    pub video_resolution: Option<String>,
    /// Numeric rating for badge display (e.g., 8.0).
    pub rating: Option<f64>,
    /// Content rating for badge display (e.g., "PG-13", "R", "TV-MA").
    pub content_rating: Option<String>,
    /// Card dimensions (set by density; default 180x270).
    pub card_width: i32,
    pub card_height: i32,
    /// Watch progress as a fraction (0.0-1.0). None = no progress data.
    pub watch_progress: Option<f64>,
    /// Whether the item has been fully watched.
    pub watched: bool,
    /// Whether a completed offline download exists for this item (R14 badge).
    pub downloaded: bool,
    /// Direct reference to the GTK Picture widget for lazy poster updates.
    #[allow(clippy::type_complexity)]
    pub picture_widget: Option<gtk::Picture>,
    /// Direct reference to the placeholder icon widget.
    pub placeholder_widget: Option<gtk::Image>,
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
            content_rating: item.content_rating.clone(),
            card_width: 180,
            card_height: 270,
            watch_progress: None,
            watched: false,
            downloaded: false,
            picture_widget: None,
            placeholder_widget: None,
        }
    }

    fn title_with_year(&self) -> String {
        match self.year {
            Some(year) => format!("{} · {year}", self.title),
            None => self.title.clone(),
        }
    }
}

pub struct MediaCardWidgets {
    picture: gtk::Picture,
    placeholder_icon: gtk::Image,
    #[allow(dead_code)]
    frame: gtk::Frame,
    title_label: gtk::Label,
    year_label: gtk::Label,
    resolution_badge: gtk::Label,
    rating_badge: gtk::Label,
    content_rating_badge: gtk::Label,
    progress_bar: gtk::ProgressBar,
    watched_icon: gtk::Image,
    downloaded_icon: gtk::Image,
}

/// The "downloaded" overlay badge (top-left of a poster).
fn downloaded_indicator() -> gtk::Image {
    gtk::Image::builder()
        .icon_name("folder-download-symbolic")
        .pixel_size(18)
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Start)
        .margin_top(8)
        .margin_start(8)
        .css_classes(["downloaded-indicator", "media-badge"])
        .visible(false)
        .build()
}

impl RelmGridItem for MediaCardData {
    type Root = gtk::Box;
    type Widgets = MediaCardWidgets;

    fn setup(list_item: &gtk::ListItem) -> (Self::Root, Self::Widgets) {
        list_item.set_activatable(true);

        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .css_classes(["media-card"])
            .width_request(180)
            .build();

        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .width_request(180)
            .height_request(270)
            .css_classes(["media-card-poster"])
            .build();

        // Placeholder icon shown when no poster texture is loaded
        let placeholder_icon = gtk::Image::builder()
            .icon_name("video-x-generic-symbolic")
            .pixel_size(48)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .css_classes(["media-card-placeholder-icon"])
            .visible(false)
            .build();

        // Resolution badge (top-right)
        let resolution_badge = gtk::Label::builder()
            .halign(gtk::Align::End)
            .valign(gtk::Align::Start)
            .margin_top(8)
            .margin_end(8)
            .css_classes(["media-badge", "resolution-badge"])
            .visible(false)
            .build();

        // Rating badge (top-left)
        let rating_badge = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Start)
            .margin_top(8)
            .margin_start(8)
            .css_classes(["media-badge", "rating-badge"])
            .visible(false)
            .build();

        // Content rating badge (bottom-left of poster, e.g. "PG-13", "R")
        let content_rating_badge = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .valign(gtk::Align::End)
            .margin_bottom(10)
            .margin_start(8)
            .css_classes(["content-rating-badge"])
            .visible(false)
            .build();

        // Watch progress bar (bottom of poster, visible for in-progress items)
        let progress_bar = gtk::ProgressBar::builder()
            .halign(gtk::Align::Fill)
            .valign(gtk::Align::End)
            .css_classes(["watch-progress"])
            .visible(false)
            .build();

        // Watched checkmark (bottom-right of poster)
        let watched_icon = gtk::Image::builder()
            .icon_name("emblem-ok-symbolic")
            .pixel_size(20)
            .halign(gtk::Align::End)
            .valign(gtk::Align::End)
            .margin_bottom(8)
            .margin_end(8)
            .css_classes(["watched-indicator"])
            .visible(false)
            .build();

        // Downloaded indicator (top-left of poster) — distinct from the
        // bottom-right watched checkmark.
        let downloaded_icon = downloaded_indicator();

        // Bottom gradient scrim for progress/badges readability
        let scrim = gtk::Box::builder()
            .halign(gtk::Align::Fill)
            .valign(gtk::Align::End)
            .height_request(56)
            .css_classes(["media-card-scrim"])
            .build();

        // Overlay stacks badges, placeholder icon, and progress on top of the poster
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&picture));
        overlay.add_overlay(&scrim);
        overlay.add_overlay(&placeholder_icon);
        overlay.add_overlay(&resolution_badge);
        overlay.add_overlay(&rating_badge);
        overlay.add_overlay(&content_rating_badge);
        overlay.add_overlay(&progress_bar);
        overlay.add_overlay(&watched_icon);
        overlay.add_overlay(&downloaded_icon);

        // Wrap overlay in a frame for rounded corners + hover effects
        let frame = gtk::Frame::builder()
            .css_classes(["media-card-frame"])
            .child(&overlay)
            .build();

        let title_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(20)
            .lines(2)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .css_classes(["media-card-title"])
            .build();

        let year_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .css_classes(["media-card-year", "dim-label"])
            .visible(false)
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
            content_rating_badge,
            progress_bar,
            watched_icon,
            downloaded_icon,
        };

        (container, widgets)
    }

    fn bind(&mut self, widgets: &mut Self::Widgets, root: &mut Self::Root) {
        // Update card dimensions based on density
        root.set_width_request(self.card_width);
        widgets.picture.set_width_request(self.card_width);
        widgets.picture.set_height_request(self.card_height);

        root.set_widget_name(&self.media_id);
        widgets.title_label.set_label(&self.title_with_year());
        widgets.year_label.set_visible(false);

        let has_poster = self.poster_texture.is_some();

        // Save widget references so ArtworkReady can update them directly
        self.picture_widget = Some(widgets.picture.clone());
        self.placeholder_widget = Some(widgets.placeholder_icon.clone());

        if let Some(ref texture) = self.poster_texture {
            widgets.picture.set_paintable(Some(texture));
            widgets.picture.remove_css_class("loading");
            widgets.picture.set_opacity(1.0);
            widgets.placeholder_icon.set_visible(false);
        } else {
            widgets.picture.set_paintable(None::<&gtk::gdk::Texture>);
            widgets.picture.add_css_class("loading");
            widgets.picture.set_opacity(0.0);
            widgets.placeholder_icon.set_visible(true);
        }

        // Content rating badge: bottom-left, visible with poster + content rating
        if has_poster {
            if let Some(ref cr) = self.content_rating {
                widgets.content_rating_badge.set_label(cr);
                widgets.content_rating_badge.set_visible(true);
            } else {
                widgets.content_rating_badge.set_visible(false);
            }
        } else {
            widgets.content_rating_badge.set_visible(false);
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
                    widgets.rating_badge.set_label(&format!("★ {rating:.1}"));
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

        // Watch state indicators
        if self.watched {
            // Watched: show checkmark, hide progress bar
            widgets.watched_icon.set_visible(true);
            widgets.progress_bar.set_visible(false);
        } else if let Some(progress) = self.watch_progress {
            if progress > 0.0 {
                // In progress: show progress bar, hide checkmark
                widgets.progress_bar.set_fraction(progress);
                widgets.progress_bar.set_visible(true);
                widgets.watched_icon.set_visible(false);
            } else {
                widgets.progress_bar.set_visible(false);
                widgets.watched_icon.set_visible(false);
            }
        } else {
            // No watch data
            widgets.progress_bar.set_visible(false);
            widgets.watched_icon.set_visible(false);
        }

        // Downloaded indicator (independent of watch state).
        widgets.downloaded_icon.set_visible(self.downloaded);
    }
}
