/// What the user is currently viewing.
#[derive(Debug, Clone, PartialEq, Default)]
#[allow(dead_code)]
pub enum CurrentView {
    /// Home page (Continue Watching + Recently Added)
    #[default]
    Home,
    /// Library grid, identified by its Plex section key.
    Library(String),
    /// Movie detail page
    MovieDetail(String),
    /// Show detail page
    ShowDetail(String),
    /// Collections list
    Collections,
    /// Offline downloads list (top-level, source-independent).
    Downloads,
    /// Collection detail page (items in a collection)
    CollectionDetail(String),
    /// Video player
    Player,
}
