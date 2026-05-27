use crate::models::library::LibraryType;

/// What the user is currently viewing.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum CurrentView {
    /// Home page (Continue Watching + Recently Added)
    Home,
    /// Library grid (movies or shows)
    Library(LibraryType),
    /// Movie detail page
    MovieDetail(String),
    /// Show detail page
    ShowDetail(String),
    /// Collections list
    Collections,
    /// Collection detail page (items in a collection)
    CollectionDetail(String),
    /// Video player
    Player,
}

impl Default for CurrentView {
    fn default() -> Self {
        Self::Home
    }
}
