use gtk::prelude::*;

use crate::models::media::{MediaItem, MediaType};

/// Build-generation counter. Incremented each time the home shelves are
/// cleared, so stale in-flight poster loads from a previous build (after a
/// source switch or reload) can be dropped instead of painted onto the wrong
/// card.
pub type Generation = u64;

/// Stable id for a shelf within the current build. Poster results carry the
/// id of the shelf they belong to so they survive shelves being appended in
/// arrival order.
pub type ShelfId = u64;

/// A home-screen shelf: a titled horizontal row of cards.
pub struct Shelf {
    pub id: ShelfId,
    /// Vertical container: title label + horizontal scroll of cards.
    pub section: gtk::Box,
    /// Horizontal row the cards are appended to.
    pub row: gtk::Box,
    pub cards: Vec<(HomeCard, MediaItem)>,
}

impl Shelf {
    pub fn new(id: ShelfId, title: &str) -> Self {
        let section = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .visible(false)
            .build();

        let label = gtk::Label::builder()
            .label(title)
            .halign(gtk::Align::Start)
            .css_classes(["title-2"])
            .build();

        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .max_content_height(300)
            .child(&row)
            .build();

        section.append(&label);
        section.append(&scroll);

        Self {
            id,
            section,
            row,
            cards: Vec::new(),
        }
    }
}

/// A shelf card on the home screen. Smaller than grid cards (~160×240).
pub struct HomeCard {
    pub container: gtk::Box,
    picture: gtk::Picture,
    title_label: gtk::Label,
    subtitle_label: gtk::Label,
    progress_bar: gtk::ProgressBar,
    /// Click gesture controller for this card.
    pub gesture: gtk::GestureClick,
}

impl HomeCard {
    pub fn new() -> Self {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .width_request(160)
            .css_classes(["home-card"])
            .build();

        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .width_request(160)
            .height_request(240)
            .css_classes(["home-card-poster", "loading"])
            .build();

        let progress_bar = gtk::ProgressBar::builder()
            .halign(gtk::Align::Fill)
            .valign(gtk::Align::End)
            .css_classes(["watch-progress"])
            .visible(false)
            .build();

        // Bottom gradient scrim adds depth and keeps the progress bar legible
        // over bright posters, matching the library grid cards.
        let scrim = gtk::Box::builder()
            .halign(gtk::Align::Fill)
            .valign(gtk::Align::End)
            .height_request(48)
            .css_classes(["home-card-scrim"])
            .build();

        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&picture));
        overlay.add_overlay(&scrim);
        overlay.add_overlay(&progress_bar);

        // Frame wrapper gives rounded corners, elevation shadow, and a hover
        // lift — the same treatment as the library media cards.
        let frame = gtk::Frame::builder()
            .css_classes(["home-card-frame"])
            .child(&overlay)
            .build();

        let title_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(18)
            .lines(2)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .css_classes(["home-card-title"])
            .build();

        let subtitle_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .css_classes(["home-card-subtitle", "dim-label"])
            .visible(false)
            .build();

        container.append(&frame);
        container.append(&title_label);
        container.append(&subtitle_label);

        let gesture = gtk::GestureClick::new();
        container.add_controller(gesture.clone());

        Self {
            container,
            picture,
            title_label,
            subtitle_label,
            progress_bar,
            gesture,
        }
    }

    pub fn set_media(&self, item: &MediaItem, progress: Option<f64>) {
        self.title_label.set_label(&item.title);

        match card_subtitle(item) {
            Some(sub) => {
                self.subtitle_label.set_label(&sub);
                self.subtitle_label.set_visible(true);
            }
            None => self.subtitle_label.set_visible(false),
        }

        if show_progress_bar(progress) {
            self.progress_bar.set_fraction(progress.unwrap());
            self.progress_bar.set_visible(true);
        } else {
            self.progress_bar.set_visible(false);
        }
    }

    pub fn set_poster(&self, texture: &gtk::gdk::Texture) {
        self.picture.set_paintable(Some(texture));
        self.picture.remove_css_class("loading");
        self.picture.set_opacity(1.0);
    }
}

impl Default for HomeCard {
    fn default() -> Self {
        Self::new()
    }
}

/// Subtitle for a home card: "S{n} E{m}" for episodes (falling back to the
/// parent id when season/episode numbers are missing), the release year for
/// everything else. `None` means no subtitle.
pub fn card_subtitle(item: &MediaItem) -> Option<String> {
    match item.media_type {
        MediaType::Episode => match (item.season_number, item.episode_number) {
            (Some(s), Some(e)) => Some(format!("S{s} E{e}")),
            _ => item.parent_id.clone(),
        },
        _ => item.year.map(|y| y.to_string()),
    }
}

/// Whether a progress fraction should render a visible progress bar.
/// Only partially-watched items (strictly between 0 and 1) qualify.
pub fn show_progress_bar(progress: Option<f64>) -> bool {
    matches!(progress, Some(f) if f > 0.0 && f < 1.0)
}

/// Whether a poster result tagged with `result_gen` is still current and
/// should be applied. Results from a previous build generation are stale and
/// dropped so a rebuild never paints an old texture onto a recycled card slot.
pub fn poster_result_is_current(result_gen: Generation, current_gen: Generation) -> bool {
    result_gen == current_gen
}

/// Whether a Plex hub duplicates a shelf the home already renders itself
/// (Continue Watching / On Deck / Recently Added) and should be dropped to
/// avoid showing the same content twice. Matched loosely on the hub identifier
/// because Plex's exact identifiers vary across server versions. "recommended"
/// is intentionally NOT matched by the "recent" check (different letters).
pub fn hub_duplicates_core(identifier: Option<&str>) -> bool {
    match identifier {
        Some(id) => {
            let id = id.to_lowercase();
            id.contains("ondeck")
                || id.contains("continue")
                || id.contains("recent")
                || id.contains("inprogress")
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::media::{MediaType, SourceType};

    fn item(media_type: MediaType) -> MediaItem {
        MediaItem {
            id: "plex:srv:1".into(),
            source_type: SourceType::Plex,
            source_id: "srv".into(),
            external_id: "1".into(),
            media_type,
            title: "Test".into(),
            year: None,
            overview: None,
            content_rating: None,
            rating: None,
            runtime_minutes: None,
            poster_path: None,
            series_poster_path: None,
            backdrop_path: None,
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
            watched: false,
            library_section_id: None,
        }
    }

    #[test]
    fn subtitle_movie_uses_year() {
        let mut m = item(MediaType::Movie);
        m.year = Some(2021);
        assert_eq!(card_subtitle(&m), Some("2021".to_string()));
    }

    #[test]
    fn subtitle_movie_without_year_is_none() {
        let m = item(MediaType::Movie);
        assert_eq!(card_subtitle(&m), None);
    }

    #[test]
    fn subtitle_episode_uses_season_episode() {
        let mut m = item(MediaType::Episode);
        m.season_number = Some(3);
        m.episode_number = Some(4);
        assert_eq!(card_subtitle(&m), Some("S3 E4".to_string()));
    }

    #[test]
    fn subtitle_episode_falls_back_to_parent_id() {
        let mut m = item(MediaType::Episode);
        m.parent_id = Some("plex:srv:show-7".into());
        assert_eq!(card_subtitle(&m), Some("plex:srv:show-7".to_string()));
    }

    #[test]
    fn progress_bar_visible_only_when_partial() {
        assert!(!show_progress_bar(None));
        assert!(!show_progress_bar(Some(0.0)));
        assert!(show_progress_bar(Some(0.01)));
        assert!(show_progress_bar(Some(0.5)));
        assert!(show_progress_bar(Some(0.99)));
        assert!(!show_progress_bar(Some(1.0)));
    }

    #[test]
    fn poster_result_dropped_when_generation_stale() {
        assert!(poster_result_is_current(5, 5));
        assert!(!poster_result_is_current(4, 5));
        assert!(!poster_result_is_current(6, 5));
    }

    #[test]
    fn hub_dedup_drops_core_duplicates() {
        assert!(hub_duplicates_core(Some("home.ondeck")));
        assert!(hub_duplicates_core(Some("home.continue")));
        assert!(hub_duplicates_core(Some("home.movies.recentlyAdded")));
        assert!(hub_duplicates_core(Some("tv.inprogress")));
    }

    #[test]
    fn hub_dedup_keeps_discovery_rows() {
        assert!(!hub_duplicates_core(Some("home.movies.recommended")));
        assert!(!hub_duplicates_core(Some(
            "home.television.becauseYouWatched"
        )));
        assert!(!hub_duplicates_core(Some("home.movies.genre.action")));
        assert!(!hub_duplicates_core(None));
    }
}
