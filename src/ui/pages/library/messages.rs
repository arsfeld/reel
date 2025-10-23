use relm4::gtk::Widget;
use relm4::gtk::gdk;

use crate::db::entities::MediaItemModel;
use crate::models::{LibraryId, MediaItemId};
use crate::ui::shared::broker::BrokerMessage;

use super::types::{ActiveFilterType, FilterState, SortBy, ViewMode, WatchStatus};

#[derive(Debug)]
pub enum LibraryPageInput {
    /// Set the library to display
    SetLibrary(LibraryId),
    /// Restore filter state from saved state
    RestoreFilterState(FilterState),
    /// Load more items into view
    LoadMoreBatch,
    /// All media items loaded from database
    AllItemsLoaded {
        items: Vec<MediaItemModel>,
        library_type: Option<String>,
    },
    /// Render next batch of items
    RenderBatch,
    /// Media item selected
    MediaItemSelected(MediaItemId),
    /// Mark media item as watched
    MarkWatched(MediaItemId),
    /// Mark media item as unwatched
    MarkUnwatched(MediaItemId),
    /// Change sort order
    SetSortBy(SortBy),
    /// Toggle sort order (ascending/descending)
    ToggleSortOrder,
    /// Filter by text
    SetFilter(String),
    /// Toggle genre filter
    ToggleGenreFilter(String),
    /// Clear all genre filters
    ClearGenreFilters,
    /// Set year range filter
    SetYearRange { min: Option<i32>, max: Option<i32> },
    /// Clear year range filter
    ClearYearRange,
    /// Set rating filter (minimum rating threshold)
    SetRatingFilter(Option<f32>),
    /// Clear rating filter
    ClearRatingFilter,
    /// Set watch status filter
    SetWatchStatusFilter(WatchStatus),
    /// Clear watch status filter
    ClearWatchStatusFilter,
    /// Set media type filter (for mixed libraries)
    SetMediaTypeFilter(Option<String>),
    /// Clear all items and reload
    Refresh,
    /// Show search bar
    ShowSearch,
    /// Hide search bar
    HideSearch,
    /// Toggle filters popover
    ToggleFiltersPopover,
    /// Clear all filters
    ClearAllFilters,
    /// Remove a specific filter
    RemoveFilter(ActiveFilterType),
    /// Set view mode
    SetViewMode(ViewMode),
    /// Image loaded from worker
    ImageLoaded { id: String, texture: gdk::Texture },
    /// Image load failed
    ImageLoadFailed { id: String },
    /// Viewport scrolled, update visible range
    ViewportScrolled,
    /// Process debounced scroll event
    ProcessDebouncedScroll,
    /// Load images for visible items
    LoadVisibleImages,
    /// Message broker messages
    BrokerMsg(BrokerMessage),
}

#[derive(Debug)]
pub enum LibraryPageOutput {
    /// Navigate to media item
    NavigateToMediaItem(MediaItemId),
    /// Set header title widget (for view switcher tabs)
    SetHeaderTitleWidget(Widget),
}
