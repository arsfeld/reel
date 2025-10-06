use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SortBy {
    Title,
    Year,
    DateAdded,
    Rating,
    LastWatched,
    Duration,
}

impl Default for SortBy {
    fn default() -> Self {
        Self::Title
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl Default for SortOrder {
    fn default() -> Self {
        Self::Ascending
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum WatchStatus {
    All,
    Watched,
    Unwatched,
}

impl Default for WatchStatus {
    fn default() -> Self {
        Self::All
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ViewMode {
    All,
    Unwatched,
    RecentlyAdded,
}

impl Default for ViewMode {
    fn default() -> Self {
        Self::All
    }
}

/// Statistics about filtered items
#[derive(Debug, Clone, Default)]
pub struct FilterStatistics {
    pub total_count: usize,
    pub avg_rating: Option<f32>,
    pub min_year: Option<i32>,
    pub max_year: Option<i32>,
}

/// Represents an active filter for display
#[derive(Debug, Clone)]
pub struct ActiveFilter {
    pub label: String,
    pub filter_type: ActiveFilterType,
}

/// Type of active filter for removal actions
#[derive(Debug, Clone)]
pub enum ActiveFilterType {
    Text,
    Genre(String),
    YearRange,
    Rating,
    WatchStatus,
}

/// Sort preferences for a specific view mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ViewModeSortPrefs {
    pub sort_by: SortBy,
    pub sort_order: SortOrder,
}

impl Default for ViewModeSortPrefs {
    fn default() -> Self {
        Self {
            sort_by: SortBy::Title,
            sort_order: SortOrder::Ascending,
        }
    }
}

/// Filter state that can be persisted and shared via URL
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FilterState {
    // Sort preferences per view mode (each view mode has independent sort settings)
    #[serde(default)]
    pub view_mode_sort_prefs: HashMap<ViewMode, ViewModeSortPrefs>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub filter_text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_genres: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_min_year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_max_year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_rating: Option<f32>,
    #[serde(default)]
    pub watch_status_filter: WatchStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_media_type: Option<String>,
    #[serde(default)]
    pub selected_view_mode: ViewMode,
}

impl Default for FilterState {
    fn default() -> Self {
        let mut view_mode_sort_prefs = HashMap::new();

        // Set default sort preferences for each view mode
        view_mode_sort_prefs.insert(
            ViewMode::All,
            ViewModeSortPrefs {
                sort_by: SortBy::Title,
                sort_order: SortOrder::Ascending,
            },
        );
        view_mode_sort_prefs.insert(
            ViewMode::Unwatched,
            ViewModeSortPrefs {
                sort_by: SortBy::Title,
                sort_order: SortOrder::Ascending,
            },
        );
        view_mode_sort_prefs.insert(
            ViewMode::RecentlyAdded,
            ViewModeSortPrefs {
                sort_by: SortBy::DateAdded,
                sort_order: SortOrder::Descending,
            },
        );

        Self {
            view_mode_sort_prefs,
            filter_text: String::new(),
            selected_genres: Vec::new(),
            selected_min_year: None,
            selected_max_year: None,
            min_rating: None,
            watch_status_filter: WatchStatus::All,
            selected_media_type: None,
            selected_view_mode: ViewMode::All,
        }
    }
}

impl FilterState {
    /// Create FilterState from LibraryPage state
    pub fn from_library_page(page: &crate::ui::pages::library::LibraryPage) -> Self {
        // Start with existing view mode sort prefs or defaults
        let mut view_mode_sort_prefs = page.view_mode_sort_prefs.clone();

        // Only save sort preferences for non-immutable view modes
        // Recently Added is immutable and always uses DateAdded/Descending
        if page.selected_view_mode != ViewMode::RecentlyAdded {
            view_mode_sort_prefs.insert(
                page.selected_view_mode,
                ViewModeSortPrefs {
                    sort_by: page.sort_by,
                    sort_order: page.sort_order,
                },
            );
        }

        Self {
            view_mode_sort_prefs,
            filter_text: page.filter_text.clone(),
            selected_genres: page.selected_genres.clone(),
            selected_min_year: page.selected_min_year,
            selected_max_year: page.selected_max_year,
            min_rating: page.min_rating,
            watch_status_filter: page.watch_status_filter,
            selected_media_type: page.selected_media_type.clone(),
            selected_view_mode: page.selected_view_mode,
        }
    }

    /// Encode filter state as URL query string
    pub fn to_url_params(&self) -> Result<String, serde_urlencoded::ser::Error> {
        serde_urlencoded::to_string(self)
    }

    /// Decode filter state from URL query string
    pub fn from_url_params(params: &str) -> Result<Self, serde_urlencoded::de::Error> {
        serde_urlencoded::from_str(params)
    }
}
