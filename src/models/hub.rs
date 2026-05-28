use super::media::MediaItem;

/// A curated home-screen row sourced from a media server's hub API
/// (e.g. Plex `/hubs`): Recommended, "Because you watched", genre rows.
///
/// `identifier` is the source's stable hub id (e.g. `home.movies.recommended`)
/// and is what the home view uses to drop hubs it already renders as core
/// shelves (Continue Watching, Recently Added).
#[derive(Debug, Clone)]
#[allow(dead_code)] // fields consumed by the home hub-shelf rendering (U7)
pub struct MediaHub {
    pub title: String,
    pub identifier: Option<String>,
    pub items: Vec<MediaItem>,
}
