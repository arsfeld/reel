use crate::models::MediaItem;

/// Generic navigation request enum for UI pages
#[derive(Debug, Clone)]
pub enum NavigationRequest {
    ShowMovieDetails(crate::models::Movie),
    ShowShowDetails(crate::models::Show),
    ShowPlayer(MediaItem),
    ShowLibrary(String, crate::models::Library), // backend_id, library
    ShowHome,
    ShowSources,
    GoBack,
}

impl NavigationRequest {
    pub fn show_movie_details(movie: crate::models::Movie) -> Self {
        Self::ShowMovieDetails(movie)
    }

    pub fn show_show_details(show: crate::models::Show) -> Self {
        Self::ShowShowDetails(show)
    }

    pub fn show_player(media_item: MediaItem) -> Self {
        Self::ShowPlayer(media_item)
    }

    pub fn show_library(backend_id: String, library: crate::models::Library) -> Self {
        Self::ShowLibrary(backend_id, library)
    }

    pub fn show_home() -> Self {
        Self::ShowHome
    }

    pub fn show_sources() -> Self {
        Self::ShowSources
    }

    pub fn go_back() -> Self {
        Self::GoBack
    }
}
