use gtk::prelude::*;
use relm4::prelude::*;

use super::LibraryPage;
use super::messages::LibraryPageInput;
use super::types::{
    ActiveFilter, ActiveFilterType, FilterState, FilterStatistics, SortBy, SortOrder, ViewMode,
    WatchStatus,
};

impl LibraryPage {
    /// Apply a filter state to the library page
    pub(super) fn apply_filter_state(&mut self, state: &FilterState) {
        // Restore view mode sort preferences
        self.view_mode_sort_prefs = state.view_mode_sort_prefs.clone();

        // Recently Added is an immutable filter - always use DateAdded/Descending
        if state.selected_view_mode == ViewMode::RecentlyAdded {
            self.sort_by = SortBy::DateAdded;
            self.sort_order = SortOrder::Descending;
        } else {
            // Apply sort settings for other view modes
            if let Some(prefs) = self.view_mode_sort_prefs.get(&state.selected_view_mode) {
                self.sort_by = prefs.sort_by;
                self.sort_order = prefs.sort_order;
            }
        }

        self.filter_text = state.filter_text.clone();
        self.selected_genres = state.selected_genres.clone();
        self.selected_min_year = state.selected_min_year;
        self.selected_max_year = state.selected_max_year;
        self.min_rating = state.min_rating;
        self.watch_status_filter = state.watch_status_filter;
        self.selected_media_type = state.selected_media_type.clone();
        self.selected_view_mode = state.selected_view_mode;
    }

    /// Save the current filter state to config
    pub(super) async fn save_filter_state(&self) {
        if let Some(ref library_id) = self.library_id {
            let state = FilterState::from_library_page(self);
            if let Ok(json) = serde_json::to_string(&state) {
                let library_id_clone = library_id.clone();
                relm4::spawn(async move {
                    use crate::services::config_service::config_service;
                    let _ = config_service()
                        .set_library_filter_state(library_id_clone.to_string(), json)
                        .await;
                });
            }
        }
    }

    /// Calculate statistics about filtered items
    pub(super) fn get_filter_statistics(&self) -> FilterStatistics {
        if self.total_items.is_empty() {
            return FilterStatistics::default();
        }

        // Calculate average rating
        let ratings: Vec<f32> = self
            .total_items
            .iter()
            .filter_map(|item| item.rating)
            .collect();
        let avg_rating = if !ratings.is_empty() {
            Some(ratings.iter().sum::<f32>() / ratings.len() as f32)
        } else {
            None
        };

        // Calculate year range from filtered items
        let years: Vec<i32> = self
            .total_items
            .iter()
            .filter_map(|item| item.year)
            .collect();
        let (min_year, max_year) = if !years.is_empty() {
            let min = *years.iter().min().unwrap();
            let max = *years.iter().max().unwrap();
            (Some(min), Some(max))
        } else {
            (None, None)
        };

        FilterStatistics {
            total_count: self.total_items.len(),
            avg_rating,
            min_year,
            max_year,
        }
    }

    /// Check if any filters are active
    pub(super) fn has_active_filters(&self) -> bool {
        !self.selected_genres.is_empty()
            || self.selected_min_year.is_some()
            || self.selected_max_year.is_some()
            || self.min_rating.is_some()
            || self.watch_status_filter != WatchStatus::All
            || !self.filter_text.is_empty()
            || self.selected_view_mode != ViewMode::All
    }

    /// Get list of active filters for display
    pub(super) fn get_active_filters_list(&self) -> Vec<ActiveFilter> {
        let mut filters = Vec::new();

        // Text filter
        if !self.filter_text.is_empty() {
            filters.push(ActiveFilter {
                label: format!("Search: \"{}\"", self.filter_text),
                filter_type: ActiveFilterType::Text,
            });
        }

        // Genre filters
        if !self.selected_genres.is_empty() {
            for genre in &self.selected_genres {
                filters.push(ActiveFilter {
                    label: format!("Genre: {}", genre),
                    filter_type: ActiveFilterType::Genre(genre.clone()),
                });
            }
        }

        // Year range filter
        if self.selected_min_year.is_some() || self.selected_max_year.is_some() {
            let label = match (self.selected_min_year, self.selected_max_year) {
                (Some(min), Some(max)) if min == max => format!("Year: {}", min),
                (Some(min), Some(max)) => format!("Year: {} - {}", min, max),
                (Some(min), None) => format!("Year: {} and later", min),
                (None, Some(max)) => format!("Year: {} and earlier", max),
                (None, None) => unreachable!(),
            };
            filters.push(ActiveFilter {
                label,
                filter_type: ActiveFilterType::YearRange,
            });
        }

        // Rating filter
        if let Some(rating) = self.min_rating {
            filters.push(ActiveFilter {
                label: format!("Rating: {:.1}+ ★", rating),
                filter_type: ActiveFilterType::Rating,
            });
        }

        // Watch status filter
        if self.watch_status_filter != WatchStatus::All {
            let label = match self.watch_status_filter {
                WatchStatus::Watched => "Watched".to_string(),
                WatchStatus::Unwatched => "Unwatched".to_string(),
                WatchStatus::All => unreachable!(),
            };
            filters.push(ActiveFilter {
                label,
                filter_type: ActiveFilterType::WatchStatus,
            });
        }

        filters
    }

    /// Update the active filters display with filter chips
    pub(super) fn update_active_filters_display(&self, sender: AsyncComponentSender<Self>) {
        if let Some(ref container) = self.active_filters_box {
            // Clear existing children
            while let Some(child) = container.first_child() {
                container.remove(&child);
            }

            // Add filter chips for each active filter
            let active_filters = self.get_active_filters_list();
            for filter in active_filters {
                let chip = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                chip.add_css_class("metadata-pill-modern");
                chip.add_css_class("interactive-element");
                chip.set_margin_end(4);
                chip.set_margin_bottom(2);

                let label = gtk::Label::new(Some(&filter.label));
                label.set_margin_start(6);
                label.set_margin_end(6);
                label.set_margin_top(2);
                label.set_margin_bottom(2);
                chip.append(&label);

                let close_button = gtk::Button::new();
                close_button.set_icon_name("window-close-symbolic");
                close_button.add_css_class("flat");
                close_button.add_css_class("circular");
                close_button.set_margin_end(4);
                close_button.set_margin_top(2);
                close_button.set_margin_bottom(2);

                let sender_clone = sender.clone();
                let filter_type = filter.filter_type.clone();
                close_button.connect_clicked(move |_| {
                    sender_clone.input(LibraryPageInput::RemoveFilter(filter_type.clone()));
                });

                chip.append(&close_button);
                container.append(&chip);
            }
        }
    }
}
