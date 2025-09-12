use crate::models::MediaItem;
use serde::{Deserialize, Serialize};

/// Strongly typed library identifier to replace string parsing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryIdentifier {
    pub source_id: String,
    pub library_id: String,
}

impl LibraryIdentifier {
    pub fn new(source_id: String, library_id: String) -> Self {
        Self {
            source_id,
            library_id,
        }
    }

    /// Parse from "source_id:library_id" format
    pub fn from_string(combined: &str) -> Option<Self> {
        let parts: Vec<&str> = combined.split(':').collect();
        if parts.len() == 2 {
            Some(Self::new(parts[0].to_string(), parts[1].to_string()))
        } else {
            None
        }
    }

    /// Convert to "source_id:library_id" format
    pub fn to_string(&self) -> String {
        format!("{}:{}", self.source_id, self.library_id)
    }
}

/// Navigation context for maintaining state during navigation
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationContext {
    /// Previous page for back navigation
    pub previous_page: Option<Box<NavigationRequest>>,
    /// Window state to preserve (e.g., for player transitions)
    pub window_state: Option<WindowState>,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

/// Window state preservation
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    pub is_fullscreen: bool,
    pub scroll_position: f64,
}

/// Generic navigation request enum for UI pages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NavigationRequest {
    ShowMovieDetails(crate::models::Movie),
    ShowShowDetails(crate::models::Show),
    ShowPlayer(MediaItem),
    ShowLibrary(LibraryIdentifier, crate::models::Library),
    ShowLibraryByKey(String), // Temporary for backward compatibility
    ShowHome(Option<String>), // Optional source_id for per-source home
    ShowSources,
    GoBack,
    // New navigation targets to replace direct methods
    RefreshCurrentPage,
    ClearHistory,
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

    pub fn show_library(source_id: String, library: crate::models::Library) -> Self {
        let identifier = LibraryIdentifier::new(source_id, library.id.clone());
        Self::ShowLibrary(identifier, library)
    }

    /// Show library using the old string format (for backward compatibility)
    pub fn show_library_by_key(library_key: String) -> Self {
        Self::ShowLibraryByKey(library_key)
    }

    pub fn show_home() -> Self {
        Self::ShowHome(None) // Default: show all sources
    }

    pub fn show_home_for_source(source_id: String) -> Self {
        Self::ShowHome(Some(source_id))
    }

    pub fn show_sources() -> Self {
        Self::ShowSources
    }

    pub fn go_back() -> Self {
        Self::GoBack
    }

    pub fn refresh_current_page() -> Self {
        Self::RefreshCurrentPage
    }

    pub fn clear_history() -> Self {
        Self::ClearHistory
    }
}
