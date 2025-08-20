use std::collections::HashMap;
use std::fmt;
use serde::{Deserialize, Serialize};
use crate::models::MediaItem;

/// Represents different types of filters that can be applied to a library
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FilterType {
    WatchStatus,
    SortOrder,
    Genre,
    Year,
    Rating,
    Resolution,
    ContentRating,
}

/// Watch status filter options
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum WatchStatus {
    All,
    Watched,
    Unwatched,
    InProgress,
}

impl fmt::Display for WatchStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WatchStatus::All => write!(f, "All"),
            WatchStatus::Watched => write!(f, "Watched"),
            WatchStatus::Unwatched => write!(f, "Unwatched"),
            WatchStatus::InProgress => write!(f, "In Progress"),
        }
    }
}

/// Sort order options
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SortOrder {
    TitleAsc,
    TitleDesc,
    YearAsc,
    YearDesc,
    RatingAsc,
    RatingDesc,
    DateAddedAsc,
    DateAddedDesc,
    DateWatchedAsc,
    DateWatchedDesc,
}

impl fmt::Display for SortOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SortOrder::TitleAsc => write!(f, "Title (A-Z)"),
            SortOrder::TitleDesc => write!(f, "Title (Z-A)"),
            SortOrder::YearAsc => write!(f, "Year (Oldest)"),
            SortOrder::YearDesc => write!(f, "Year (Newest)"),
            SortOrder::RatingAsc => write!(f, "Rating (Low-High)"),
            SortOrder::RatingDesc => write!(f, "Rating (High-Low)"),
            SortOrder::DateAddedAsc => write!(f, "Date Added (Oldest)"),
            SortOrder::DateAddedDesc => write!(f, "Date Added (Newest)"),
            SortOrder::DateWatchedAsc => write!(f, "Date Watched (Oldest)"),
            SortOrder::DateWatchedDesc => write!(f, "Date Watched (Newest)"),
        }
    }
}

/// A single filter criterion
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FilterCriterion {
    WatchStatus(WatchStatus),
    SortOrder(SortOrder),
    Genre(String),
    YearRange(Option<u32>, Option<u32>), // (min_year, max_year)
    MinRating(f32),
    Resolution(String), // "4K", "1080p", "720p", etc.
    ContentRating(String), // "PG", "PG-13", "R", etc.
}

/// Container for all active filters
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FilterSet {
    filters: HashMap<FilterType, FilterCriterion>,
}

impl FilterSet {
    pub fn new() -> Self {
        Self {
            filters: HashMap::new(),
        }
    }
    
    /// Add or update a filter
    pub fn set_filter(&mut self, filter_type: FilterType, criterion: FilterCriterion) {
        self.filters.insert(filter_type, criterion);
    }
    
    /// Remove a filter
    pub fn remove_filter(&mut self, filter_type: &FilterType) {
        self.filters.remove(filter_type);
    }
    
    /// Clear all filters
    pub fn clear(&mut self) {
        self.filters.clear();
    }
    
    /// Get a specific filter
    pub fn get_filter(&self, filter_type: &FilterType) -> Option<&FilterCriterion> {
        self.filters.get(filter_type)
    }
    
    /// Check if any filters are active
    pub fn has_filters(&self) -> bool {
        !self.filters.is_empty()
    }
    
    /// Get the watch status filter
    pub fn watch_status(&self) -> WatchStatus {
        match self.get_filter(&FilterType::WatchStatus) {
            Some(FilterCriterion::WatchStatus(status)) => *status,
            _ => WatchStatus::All,
        }
    }
    
    /// Get the sort order
    pub fn sort_order(&self) -> SortOrder {
        match self.get_filter(&FilterType::SortOrder) {
            Some(FilterCriterion::SortOrder(order)) => *order,
            _ => SortOrder::TitleAsc,
        }
    }
}

/// Manager for handling library filters
pub struct FilterManager {
    filter_set: FilterSet,
}

impl FilterManager {
    pub fn new() -> Self {
        Self {
            filter_set: FilterSet::new(),
        }
    }
    
    /// Apply all filters to a list of media items
    pub fn apply_filters(&self, items: Vec<MediaItem>) -> Vec<MediaItem> {
        let mut filtered = items;
        
        // Apply watch status filter
        filtered = self.filter_by_watch_status(filtered);
        
        // Apply other filters (to be implemented as needed)
        filtered = self.filter_by_genre(filtered);
        filtered = self.filter_by_year(filtered);
        filtered = self.filter_by_rating(filtered);
        
        // Apply sorting
        filtered = self.sort_items(filtered);
        
        filtered
    }
    
    fn filter_by_watch_status(&self, items: Vec<MediaItem>) -> Vec<MediaItem> {
        let watch_status = self.filter_set.watch_status();
        
        match watch_status {
            WatchStatus::All => items,
            WatchStatus::Watched => {
                items.into_iter()
                    .filter(|item| item.is_watched())
                    .collect()
            }
            WatchStatus::Unwatched => {
                items.into_iter()
                    .filter(|item| !item.is_watched())
                    .collect()
            }
            WatchStatus::InProgress => {
                items.into_iter()
                    .filter(|item| item.is_partially_watched())
                    .collect()
            }
        }
    }
    
    fn filter_by_genre(&self, items: Vec<MediaItem>) -> Vec<MediaItem> {
        if let Some(FilterCriterion::Genre(genre)) = self.filter_set.get_filter(&FilterType::Genre) {
            items.into_iter()
                .filter(|item| {
                    match item {
                        MediaItem::Movie(movie) => {
                            movie.genres.iter().any(|g| g.eq_ignore_ascii_case(genre))
                        }
                        MediaItem::Show(show) => {
                            show.genres.iter().any(|g| g.eq_ignore_ascii_case(genre))
                        }
                        _ => false,
                    }
                })
                .collect()
        } else {
            items
        }
    }
    
    fn filter_by_year(&self, items: Vec<MediaItem>) -> Vec<MediaItem> {
        if let Some(FilterCriterion::YearRange(min_year, max_year)) = 
            self.filter_set.get_filter(&FilterType::Year) {
            items.into_iter()
                .filter(|item| {
                    let year = match item {
                        MediaItem::Movie(movie) => movie.year,
                        MediaItem::Show(show) => show.year,
                        _ => None,
                    };
                    
                    if let Some(y) = year {
                        let above_min = min_year.map_or(true, |min| y >= min);
                        let below_max = max_year.map_or(true, |max| y <= max);
                        above_min && below_max
                    } else {
                        false
                    }
                })
                .collect()
        } else {
            items
        }
    }
    
    fn filter_by_rating(&self, items: Vec<MediaItem>) -> Vec<MediaItem> {
        if let Some(FilterCriterion::MinRating(min_rating)) = 
            self.filter_set.get_filter(&FilterType::Rating) {
            items.into_iter()
                .filter(|item| {
                    let rating = match item {
                        MediaItem::Movie(movie) => movie.rating,
                        MediaItem::Show(show) => show.rating,
                        _ => None,
                    };
                    
                    rating.map_or(false, |r| r >= *min_rating)
                })
                .collect()
        } else {
            items
        }
    }
    
    fn sort_items(&self, mut items: Vec<MediaItem>) -> Vec<MediaItem> {
        let sort_order = self.filter_set.sort_order();
        
        items.sort_by(|a, b| {
            match sort_order {
                SortOrder::TitleAsc => a.title().cmp(b.title()),
                SortOrder::TitleDesc => b.title().cmp(a.title()),
                SortOrder::YearAsc => {
                    let year_a = self.get_year(a);
                    let year_b = self.get_year(b);
                    year_a.cmp(&year_b)
                }
                SortOrder::YearDesc => {
                    let year_a = self.get_year(a);
                    let year_b = self.get_year(b);
                    year_b.cmp(&year_a)
                }
                SortOrder::RatingAsc => {
                    let rating_a = self.get_rating(a);
                    let rating_b = self.get_rating(b);
                    rating_a.partial_cmp(&rating_b).unwrap_or(std::cmp::Ordering::Equal)
                }
                SortOrder::RatingDesc => {
                    let rating_a = self.get_rating(a);
                    let rating_b = self.get_rating(b);
                    rating_b.partial_cmp(&rating_a).unwrap_or(std::cmp::Ordering::Equal)
                }
                SortOrder::DateAddedAsc => {
                    // TODO: Implement when we have date_added field
                    a.title().cmp(b.title())
                }
                SortOrder::DateAddedDesc => {
                    // TODO: Implement when we have date_added field
                    b.title().cmp(a.title())
                }
                SortOrder::DateWatchedAsc => {
                    // TODO: Implement when we have last_watched field
                    a.title().cmp(b.title())
                }
                SortOrder::DateWatchedDesc => {
                    // TODO: Implement when we have last_watched field
                    b.title().cmp(a.title())
                }
            }
        });
        
        items
    }
    
    fn get_year(&self, item: &MediaItem) -> Option<u32> {
        match item {
            MediaItem::Movie(movie) => movie.year,
            MediaItem::Show(show) => show.year,
            _ => None,
        }
    }
    
    fn get_rating(&self, item: &MediaItem) -> Option<f32> {
        match item {
            MediaItem::Movie(movie) => movie.rating,
            MediaItem::Show(show) => show.rating,
            _ => None,
        }
    }
    
    /// Get the current filter set
    pub fn filter_set(&self) -> &FilterSet {
        &self.filter_set
    }
    
    /// Get a mutable reference to the filter set
    pub fn filter_set_mut(&mut self) -> &mut FilterSet {
        &mut self.filter_set
    }
    
    /// Update watch status filter
    pub fn set_watch_status(&mut self, status: WatchStatus) {
        if status == WatchStatus::All {
            self.filter_set.remove_filter(&FilterType::WatchStatus);
        } else {
            self.filter_set.set_filter(
                FilterType::WatchStatus,
                FilterCriterion::WatchStatus(status)
            );
        }
    }
    
    /// Update sort order
    pub fn set_sort_order(&mut self, order: SortOrder) {
        self.filter_set.set_filter(
            FilterType::SortOrder,
            FilterCriterion::SortOrder(order)
        );
    }
}

impl Default for FilterManager {
    fn default() -> Self {
        Self::new()
    }
}