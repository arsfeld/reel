use gtk::prelude::*;

use crate::models::media::{HdrFormat, MediaItem};

use super::HomeViewMsg;

/// Select hero candidates from a pool of items: only those that have backdrop
/// artwork are eligible (R3), capped at `max`. Input order is preserved.
pub fn hero_candidates(items: &[MediaItem], max: usize) -> Vec<MediaItem> {
    items
        .iter()
        .filter(|i| i.backdrop_path.is_some())
        .take(max)
        .cloned()
        .collect()
}

/// Next rotation index, wrapping at the end. Returns 0 for an empty set.
pub fn next_index(current: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        (current + 1) % len
    }
}

/// Previous rotation index, wrapping at the start. Returns 0 for an empty set.
pub fn prev_index(current: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        (current + len - 1) % len
    }
}

/// Metadata line for the hero, e.g. "2024 · 4K · HDR". Absent parts are
/// omitted; an item with none of the parts yields an empty string.
pub fn hero_metadata(item: &MediaItem) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(year) = item.year {
        parts.push(year.to_string());
    }
    if let Some(ref res) = item.video_resolution {
        parts.push(MediaItem::format_resolution(res).to_string());
    }
    if let Some(hdr) = item.hdr {
        parts.push(
            match hdr {
                HdrFormat::DolbyVision => "Dolby Vision",
                HdrFormat::Hdr => "HDR",
            }
            .to_string(),
        );
    }
    parts.join(" · ")
}

/// The featured hero region: a large backdrop with a gradient scrim, the item
/// title and metadata, Play / info actions, and position dots.
pub struct Hero {
    pub root: gtk::Box,
    backdrop: gtk::Picture,
    title_label: gtk::Label,
    meta_label: gtk::Label,
    dots: gtk::Box,
}

impl Hero {
    pub fn new(sender: &relm4::Sender<HomeViewMsg>) -> Self {
        let backdrop = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .height_request(360)
            .hexpand(true)
            .css_classes(["home-hero-backdrop", "loading"])
            .build();

        let scrim = gtk::Box::builder()
            .hexpand(true)
            .vexpand(true)
            .css_classes(["home-hero-scrim"])
            .build();

        let title_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .css_classes(["title-1", "home-hero-title"])
            .build();

        let meta_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .css_classes(["home-hero-meta"])
            .build();

        let play_button = gtk::Button::builder()
            .css_classes(["pill", "suggested-action"])
            .child(
                &adw::ButtonContent::builder()
                    .icon_name("media-playback-start-symbolic")
                    .label("Play")
                    .build(),
            )
            .build();

        let info_button = gtk::Button::builder()
            .icon_name("dialog-information-symbolic")
            .css_classes(["pill"])
            .build();

        let buttons = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        buttons.append(&play_button);
        buttons.append(&info_button);

        let dots = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .halign(gtk::Align::Start)
            .css_classes(["home-hero-dots"])
            .build();

        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(10)
            .valign(gtk::Align::End)
            .halign(gtk::Align::Start)
            .margin_start(32)
            .margin_end(32)
            .margin_bottom(28)
            .build();
        content.append(&title_label);
        content.append(&meta_label);
        content.append(&buttons);
        content.append(&dots);

        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&backdrop));
        overlay.add_overlay(&scrim);
        overlay.add_overlay(&content);

        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .visible(false)
            .focusable(true)
            .build();
        root.append(&overlay);

        let s = sender.clone();
        play_button.connect_clicked(move |_| {
            let _ = s.send(HomeViewMsg::HeroPlay);
        });
        let s = sender.clone();
        info_button.connect_clicked(move |_| {
            let _ = s.send(HomeViewMsg::HeroInfo);
        });

        // Left / Right arrow keys manually advance the rotation.
        let key = gtk::EventControllerKey::new();
        let s = sender.clone();
        key.connect_key_pressed(move |_, keyval, _, _| match keyval {
            gtk::gdk::Key::Left => {
                let _ = s.send(HomeViewMsg::HeroBack);
                gtk::glib::Propagation::Stop
            }
            gtk::gdk::Key::Right => {
                let _ = s.send(HomeViewMsg::HeroAdvance);
                gtk::glib::Propagation::Stop
            }
            _ => gtk::glib::Propagation::Proceed,
        });
        root.add_controller(key);

        Self {
            root,
            backdrop,
            title_label,
            meta_label,
            dots,
        }
    }

    /// Show an item: title, metadata, position dots, and reset the backdrop to
    /// its loading state until `set_backdrop` arrives.
    pub fn set_item(&self, item: &MediaItem, index: usize, total: usize) {
        self.title_label.set_label(&item.title);
        self.meta_label.set_label(&hero_metadata(item));

        self.backdrop.set_paintable(None::<&gtk::gdk::Texture>);
        self.backdrop.add_css_class("loading");

        while let Some(child) = self.dots.first_child() {
            self.dots.remove(&child);
        }
        if total > 1 {
            for i in 0..total {
                let dot = gtk::Box::builder()
                    .width_request(8)
                    .height_request(8)
                    .css_classes(if i == index {
                        ["home-hero-dot", "active"].as_slice()
                    } else {
                        ["home-hero-dot"].as_slice()
                    })
                    .build();
                self.dots.append(&dot);
            }
        }
    }

    pub fn set_backdrop(&self, texture: &gtk::gdk::Texture) {
        self.backdrop.set_paintable(Some(texture));
        self.backdrop.remove_css_class("loading");
    }

    pub fn set_revealed(&self, revealed: bool) {
        self.root.set_visible(revealed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::media::{MediaType, SourceType};

    fn item(title: &str, has_backdrop: bool) -> MediaItem {
        MediaItem {
            id: format!("plex:srv:{title}"),
            source_type: SourceType::Plex,
            source_id: "srv".into(),
            external_id: title.into(),
            media_type: MediaType::Movie,
            title: title.into(),
            year: None,
            overview: None,
            content_rating: None,
            rating: None,
            runtime_minutes: None,
            poster_path: None,
            backdrop_path: if has_backdrop {
                Some("/art".into())
            } else {
                None
            },
            genres: Vec::new(),
            parent_id: None,
            season_number: None,
            episode_number: None,
            air_date: None,
            file_path: None,
            video_resolution: None,
            hdr: None,
            added_at: String::new(),
            updated_at: String::new(),
            playback_position_ms: None,
        }
    }

    #[test]
    fn candidates_filter_out_items_without_backdrop() {
        let items = vec![item("A", true), item("B", false), item("C", true)];
        let cands = hero_candidates(&items, 10);
        assert_eq!(cands.len(), 2);
        assert_eq!(cands[0].title, "A");
        assert_eq!(cands[1].title, "C");
    }

    #[test]
    fn candidates_capped_at_max() {
        let items: Vec<MediaItem> = (0..10).map(|i| item(&format!("M{i}"), true)).collect();
        assert_eq!(hero_candidates(&items, 6).len(), 6);
    }

    #[test]
    fn next_index_wraps() {
        assert_eq!(next_index(0, 3), 1);
        assert_eq!(next_index(2, 3), 0);
        assert_eq!(next_index(0, 0), 0);
    }

    #[test]
    fn prev_index_wraps() {
        assert_eq!(prev_index(0, 3), 2);
        assert_eq!(prev_index(2, 3), 1);
        assert_eq!(prev_index(0, 0), 0);
    }

    #[test]
    fn metadata_joins_present_parts() {
        let mut m = item("X", true);
        m.year = Some(2024);
        m.video_resolution = Some("4k".into());
        m.hdr = Some(HdrFormat::Hdr);
        assert_eq!(hero_metadata(&m), "2024 · 4K · HDR");
    }

    #[test]
    fn metadata_omits_absent_parts() {
        let mut m = item("X", true);
        m.year = Some(2024);
        assert_eq!(hero_metadata(&m), "2024");
        let empty = item("Y", true);
        assert_eq!(hero_metadata(&empty), "");
    }
}
