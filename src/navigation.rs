use crate::models::library::LibraryType;

/// What the user is currently viewing.
#[derive(Debug, Clone, PartialEq)]
pub enum CurrentView {
    /// Library grid (movies or shows)
    Library(LibraryType),
    /// Movie detail page
    MovieDetail(String),
    /// Show detail page
    ShowDetail(String),
    /// Video player
    Player,
}

impl Default for CurrentView {
    fn default() -> Self {
        Self::Library(LibraryType::Movie)
    }
}
