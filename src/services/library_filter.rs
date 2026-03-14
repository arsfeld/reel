use std::cmp::Ordering;

use crate::models::media::MediaItem;

/// Available sort orders for the library view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    #[default]
    TitleAsc,
    TitleDesc,
    YearNewest,
    YearOldest,
    DateAdded,
    RatingHighest,
    RuntimeLongest,
}

impl SortOrder {
    /// Human-readable label for UI display.
    pub fn label(self) -> &'static str {
        match self {
            Self::TitleAsc => "Title A-Z",
            Self::TitleDesc => "Title Z-A",
            Self::YearNewest => "Year (Newest)",
            Self::YearOldest => "Year (Oldest)",
            Self::DateAdded => "Date Added",
            Self::RatingHighest => "Rating",
            Self::RuntimeLongest => "Runtime",
        }
    }

    /// All available sort orders.
    pub fn all() -> &'static [SortOrder] {
        &[
            Self::TitleAsc,
            Self::TitleDesc,
            Self::YearNewest,
            Self::YearOldest,
            Self::DateAdded,
            Self::RatingHighest,
            Self::RuntimeLongest,
        ]
    }
}

/// A genre filter with OR semantics: item matches if it has ANY selected genre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenreFilter {
    pub selected_genres: Vec<String>,
}

/// A decade filter (e.g., decade_start=2020 matches years 2020..=2029).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecadeFilter {
    pub decade_start: i32,
}

/// Combined filter state. All active filters must pass (AND between filter types).
#[derive(Debug, Clone, Default)]
pub struct FilterState {
    pub genres: Option<GenreFilter>,
    pub decade: Option<DecadeFilter>,
}

impl FilterState {
    /// Returns true if any filter is active.
    pub fn is_active(&self) -> bool {
        self.genres.is_some() || self.decade.is_some()
    }

    /// Reset all filters to inactive.
    pub fn clear(&mut self) {
        self.genres = None;
        self.decade = None;
    }
}

/// Does a media item match the search query?
/// Case-insensitive title substring match. Empty query matches everything.
pub fn search_matches(item: &MediaItem, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }
    item.title.to_lowercase().contains(&query.to_lowercase())
}

/// Does a media item pass all active filters?
/// Filters are AND-combined: item must pass every active filter.
pub fn filter_matches(item: &MediaItem, filter: &FilterState) -> bool {
    if let Some(ref genre_filter) = filter.genres
        && !genre_filter.selected_genres.is_empty()
        && !genre_filter
            .selected_genres
            .iter()
            .any(|g| item.genres.contains(g))
    {
        return false;
    }

    if let Some(ref decade_filter) = filter.decade {
        match item.year {
            Some(year) => {
                if year < decade_filter.decade_start
                    || year > decade_filter.decade_start + 9
                {
                    return false;
                }
            }
            None => return false,
        }
    }

    true
}

/// Compare two media items for a given sort order.
/// Items with None values sort to the end (after items with values).
/// Ties are broken by title ascending for stable sort.
pub fn sort_compare(a: &MediaItem, b: &MediaItem, order: SortOrder) -> Ordering {
    let primary = match order {
        SortOrder::TitleAsc => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
        SortOrder::TitleDesc => b.title.to_lowercase().cmp(&a.title.to_lowercase()),
        SortOrder::YearNewest => cmp_option_desc(&a.year, &b.year),
        SortOrder::YearOldest => cmp_option_asc(&a.year, &b.year),
        SortOrder::DateAdded => b.added_at.cmp(&a.added_at),
        SortOrder::RatingHighest => cmp_option_f64_desc(&a.rating, &b.rating),
        SortOrder::RuntimeLongest => cmp_option_desc(&a.runtime_minutes, &b.runtime_minutes),
    };

    if primary == Ordering::Equal && !matches!(order, SortOrder::TitleAsc | SortOrder::TitleDesc) {
        a.title.to_lowercase().cmp(&b.title.to_lowercase())
    } else {
        primary
    }
}

/// Apply search, filters, and sort to a list of items.
/// Returns indices into the original Vec, in sorted order.
pub fn apply_filters_and_sort(
    items: &[MediaItem],
    query: &str,
    filter: &FilterState,
    sort: SortOrder,
) -> Vec<usize> {
    let mut indices: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| search_matches(item, query) && filter_matches(item, filter))
        .map(|(i, _)| i)
        .collect();

    indices.sort_by(|&a, &b| sort_compare(&items[a], &items[b], sort));
    indices
}

/// Extract unique genres from a list of items, sorted alphabetically.
pub fn extract_genres(items: &[MediaItem]) -> Vec<String> {
    let mut genres: Vec<String> = items
        .iter()
        .flat_map(|item| item.genres.iter().cloned())
        .collect();
    genres.sort();
    genres.dedup();
    genres
}

/// Extract available decades from items that have a year, sorted newest-first.
/// Returns the start year of each decade (e.g., 2020 for "2020s").
pub fn extract_decades(items: &[MediaItem]) -> Vec<i32> {
    let mut decades: Vec<i32> = items
        .iter()
        .filter_map(|item| item.year)
        .map(|y| (y / 10) * 10)
        .collect();
    decades.sort_unstable();
    decades.dedup();
    decades.reverse();
    decades
}

/// Compare two Options in ascending order, None sorts last.
fn cmp_option_asc<T: Ord>(a: &Option<T>, b: &Option<T>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => a.cmp(b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Compare two Options in descending order, None sorts last.
fn cmp_option_desc<T: Ord>(a: &Option<T>, b: &Option<T>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => b.cmp(a),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Compare two Option<f64> in descending order, None sorts last.
fn cmp_option_f64_desc(a: &Option<f64>, b: &Option<f64>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => b.partial_cmp(a).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::media::{MediaType, SourceType};

    fn movie(title: &str) -> MediaItem {
        MediaItem {
            id: format!("plex:test:{title}"),
            source_type: SourceType::Plex,
            source_id: "http://localhost:32400".to_string(),
            external_id: title.to_string(),
            media_type: MediaType::Movie,
            title: title.to_string(),
            year: Some(2021),
            overview: None,
            content_rating: None,
            rating: Some(7.5),
            runtime_minutes: Some(120),
            poster_path: None,
            backdrop_path: None,
            genres: vec!["Action".to_string(), "Sci-Fi".to_string()],
            parent_id: None,
            season_number: None,
            episode_number: None,
            air_date: None,
            file_path: None,
            added_at: "1700000000".to_string(),
            updated_at: "1700000000".to_string(),
        }
    }

    fn movie_with(title: &str, year: Option<i32>, rating: Option<f64>, genres: Vec<&str>) -> MediaItem {
        let mut m = movie(title);
        m.year = year;
        m.rating = rating;
        m.genres = genres.into_iter().map(String::from).collect();
        m
    }

    // === Search tests ===

    #[test]
    fn search_empty_query_matches_all() {
        let item = movie("Dune");
        assert!(search_matches(&item, ""));
        assert!(search_matches(&item, "  "));
    }

    #[test]
    fn search_exact_title_matches() {
        let item = movie("Dune");
        assert!(search_matches(&item, "Dune"));
    }

    #[test]
    fn search_partial_title_matches() {
        let item = movie("Interstellar");
        assert!(search_matches(&item, "stell"));
    }

    #[test]
    fn search_case_insensitive() {
        let item = movie("Dune");
        assert!(search_matches(&item, "dune"));
        assert!(search_matches(&item, "DUNE"));
        assert!(search_matches(&item, "dUnE"));
    }

    #[test]
    fn search_leading_trailing_whitespace_trimmed() {
        let item = movie("Dune");
        assert!(search_matches(&item, "  Dune  "));
    }

    #[test]
    fn search_no_match_returns_false() {
        let item = movie("Dune");
        assert!(!search_matches(&item, "Avatar"));
    }

    #[test]
    fn search_special_characters_literal_match() {
        let item = movie("Spider-Man: No Way Home");
        assert!(search_matches(&item, "spider-man"));
        assert!(search_matches(&item, ": no"));
    }

    // === Genre filter tests ===

    #[test]
    fn genre_filter_single_genre_matches() {
        let item = movie_with("Dune", Some(2021), None, vec!["Action", "Sci-Fi"]);
        let filter = FilterState {
            genres: Some(GenreFilter {
                selected_genres: vec!["Action".to_string()],
            }),
            decade: None,
        };
        assert!(filter_matches(&item, &filter));
    }

    #[test]
    fn genre_filter_single_genre_no_match() {
        let item = movie_with("Dune", Some(2021), None, vec!["Action", "Sci-Fi"]);
        let filter = FilterState {
            genres: Some(GenreFilter {
                selected_genres: vec!["Comedy".to_string()],
            }),
            decade: None,
        };
        assert!(!filter_matches(&item, &filter));
    }

    #[test]
    fn genre_filter_multi_genre_or_semantics() {
        let item = movie_with("Dune", Some(2021), None, vec!["Action"]);
        let filter = FilterState {
            genres: Some(GenreFilter {
                selected_genres: vec!["Comedy".to_string(), "Action".to_string()],
            }),
            decade: None,
        };
        // Item has "Action" which is in the selection → matches (OR)
        assert!(filter_matches(&item, &filter));
    }

    #[test]
    fn genre_filter_item_with_multiple_genres() {
        let item = movie_with("Dune", Some(2021), None, vec!["Sci-Fi", "Adventure", "Drama"]);
        let filter = FilterState {
            genres: Some(GenreFilter {
                selected_genres: vec!["Drama".to_string()],
            }),
            decade: None,
        };
        assert!(filter_matches(&item, &filter));
    }

    #[test]
    fn genre_filter_empty_selection_matches_all() {
        let item = movie_with("Dune", Some(2021), None, vec!["Action"]);
        let filter = FilterState {
            genres: Some(GenreFilter {
                selected_genres: vec![],
            }),
            decade: None,
        };
        assert!(filter_matches(&item, &filter));
    }

    #[test]
    fn genre_filter_item_with_no_genres_never_matches() {
        let item = movie_with("Unknown", Some(2021), None, vec![]);
        let filter = FilterState {
            genres: Some(GenreFilter {
                selected_genres: vec!["Action".to_string()],
            }),
            decade: None,
        };
        assert!(!filter_matches(&item, &filter));
    }

    // === Decade filter tests ===

    #[test]
    fn decade_filter_item_in_decade_matches() {
        let item = movie_with("Dune", Some(2021), None, vec![]);
        let filter = FilterState {
            genres: None,
            decade: Some(DecadeFilter { decade_start: 2020 }),
        };
        assert!(filter_matches(&item, &filter));
    }

    #[test]
    fn decade_filter_item_outside_decade_no_match() {
        let item = movie_with("Dune", Some(2021), None, vec![]);
        let filter = FilterState {
            genres: None,
            decade: Some(DecadeFilter { decade_start: 2010 }),
        };
        assert!(!filter_matches(&item, &filter));
    }

    #[test]
    fn decade_filter_boundary_start_year_matches() {
        let item = movie_with("Movie2020", Some(2020), None, vec![]);
        let filter = FilterState {
            genres: None,
            decade: Some(DecadeFilter { decade_start: 2020 }),
        };
        assert!(filter_matches(&item, &filter));
    }

    #[test]
    fn decade_filter_boundary_end_year_matches() {
        let item = movie_with("Movie2029", Some(2029), None, vec![]);
        let filter = FilterState {
            genres: None,
            decade: Some(DecadeFilter { decade_start: 2020 }),
        };
        assert!(filter_matches(&item, &filter));
    }

    #[test]
    fn decade_filter_item_with_no_year_no_match() {
        let item = movie_with("NoYear", None, None, vec![]);
        let filter = FilterState {
            genres: None,
            decade: Some(DecadeFilter { decade_start: 2020 }),
        };
        assert!(!filter_matches(&item, &filter));
    }

    // === Combined filter tests ===

    #[test]
    fn combined_genre_and_decade_both_must_match() {
        let item = movie_with("Dune", Some(2021), None, vec!["Sci-Fi"]);
        let filter = FilterState {
            genres: Some(GenreFilter {
                selected_genres: vec!["Sci-Fi".to_string()],
            }),
            decade: Some(DecadeFilter { decade_start: 2020 }),
        };
        assert!(filter_matches(&item, &filter));
    }

    #[test]
    fn combined_genre_and_decade_genre_fails() {
        let item = movie_with("Dune", Some(2021), None, vec!["Sci-Fi"]);
        let filter = FilterState {
            genres: Some(GenreFilter {
                selected_genres: vec!["Comedy".to_string()],
            }),
            decade: Some(DecadeFilter { decade_start: 2020 }),
        };
        assert!(!filter_matches(&item, &filter));
    }

    #[test]
    fn combined_genre_and_decade_decade_fails() {
        let item = movie_with("Dune", Some(2021), None, vec!["Sci-Fi"]);
        let filter = FilterState {
            genres: Some(GenreFilter {
                selected_genres: vec!["Sci-Fi".to_string()],
            }),
            decade: Some(DecadeFilter { decade_start: 2010 }),
        };
        assert!(!filter_matches(&item, &filter));
    }

    #[test]
    fn no_filters_active_matches_all() {
        let item = movie("Dune");
        let filter = FilterState::default();
        assert!(filter_matches(&item, &filter));
    }

    // === Sort tests ===

    #[test]
    fn sort_title_asc_alphabetical() {
        let a = movie("Arrival");
        let b = movie("Dune");
        assert_eq!(sort_compare(&a, &b, SortOrder::TitleAsc), Ordering::Less);
    }

    #[test]
    fn sort_title_desc_reverse() {
        let a = movie("Arrival");
        let b = movie("Dune");
        assert_eq!(sort_compare(&a, &b, SortOrder::TitleDesc), Ordering::Greater);
    }

    #[test]
    fn sort_title_case_insensitive() {
        let a = movie("arrival");
        let b = movie("Dune");
        assert_eq!(sort_compare(&a, &b, SortOrder::TitleAsc), Ordering::Less);
    }

    #[test]
    fn sort_year_newest_first() {
        let a = movie_with("Old", Some(2010), None, vec![]);
        let b = movie_with("New", Some(2024), None, vec![]);
        assert_eq!(sort_compare(&a, &b, SortOrder::YearNewest), Ordering::Greater);
    }

    #[test]
    fn sort_year_oldest_first() {
        let a = movie_with("Old", Some(2010), None, vec![]);
        let b = movie_with("New", Some(2024), None, vec![]);
        assert_eq!(sort_compare(&a, &b, SortOrder::YearOldest), Ordering::Less);
    }

    #[test]
    fn sort_year_none_sorts_last() {
        let with_year = movie_with("HasYear", Some(2020), None, vec![]);
        let no_year = movie_with("NoYear", None, None, vec![]);
        // None should sort after Some in both directions
        assert_eq!(
            sort_compare(&with_year, &no_year, SortOrder::YearNewest),
            Ordering::Less
        );
        assert_eq!(
            sort_compare(&with_year, &no_year, SortOrder::YearOldest),
            Ordering::Less
        );
    }

    #[test]
    fn sort_date_added_newest_first() {
        let mut a = movie("Old");
        a.added_at = "1600000000".to_string();
        let mut b = movie("New");
        b.added_at = "1700000000".to_string();
        assert_eq!(sort_compare(&a, &b, SortOrder::DateAdded), Ordering::Greater);
    }

    #[test]
    fn sort_rating_highest_first() {
        let a = movie_with("Low", Some(2020), Some(5.0), vec![]);
        let b = movie_with("High", Some(2020), Some(9.0), vec![]);
        assert_eq!(
            sort_compare(&a, &b, SortOrder::RatingHighest),
            Ordering::Greater
        );
    }

    #[test]
    fn sort_rating_none_sorts_last() {
        let with_rating = movie_with("Rated", Some(2020), Some(7.0), vec![]);
        let no_rating = movie_with("Unrated", Some(2020), None, vec![]);
        assert_eq!(
            sort_compare(&with_rating, &no_rating, SortOrder::RatingHighest),
            Ordering::Less
        );
    }

    #[test]
    fn sort_runtime_longest_first() {
        let mut short = movie("Short");
        short.runtime_minutes = Some(90);
        let mut long = movie("Long");
        long.runtime_minutes = Some(180);
        assert_eq!(
            sort_compare(&short, &long, SortOrder::RuntimeLongest),
            Ordering::Greater
        );
    }

    #[test]
    fn sort_runtime_none_sorts_last() {
        let mut with_runtime = movie("Has");
        with_runtime.runtime_minutes = Some(120);
        let mut no_runtime = movie("None");
        no_runtime.runtime_minutes = None;
        assert_eq!(
            sort_compare(&with_runtime, &no_runtime, SortOrder::RuntimeLongest),
            Ordering::Less
        );
    }

    #[test]
    fn sort_stable_tiebreak_by_title() {
        let a = movie_with("Arrival", Some(2020), Some(8.0), vec![]);
        let b = movie_with("Dune", Some(2020), Some(8.0), vec![]);
        // Same year, should tiebreak by title
        assert_eq!(sort_compare(&a, &b, SortOrder::YearNewest), Ordering::Less);
        assert_eq!(sort_compare(&a, &b, SortOrder::RatingHighest), Ordering::Less);
    }

    // === apply_filters_and_sort tests ===

    fn sample_library() -> Vec<MediaItem> {
        vec![
            movie_with("Dune", Some(2021), Some(8.0), vec!["Sci-Fi", "Adventure"]),
            movie_with("Arrival", Some(2016), Some(7.9), vec!["Sci-Fi", "Drama"]),
            movie_with("The Hangover", Some(2009), Some(7.7), vec!["Comedy"]),
            movie_with("Zoolander", Some(2001), Some(6.5), vec!["Comedy"]),
            movie_with("Interstellar", Some(2014), Some(8.7), vec!["Sci-Fi", "Drama"]),
        ]
    }

    #[test]
    fn apply_search_and_genre_filter_combined() {
        let items = sample_library();
        let filter = FilterState {
            genres: Some(GenreFilter {
                selected_genres: vec!["Sci-Fi".to_string()],
            }),
            decade: None,
        };
        let indices = apply_filters_and_sort(&items, "ar", &filter, SortOrder::TitleAsc);
        // "Arrival" and "Interstellar" match "ar" + Sci-Fi
        let titles: Vec<&str> = indices.iter().map(|&i| items[i].title.as_str()).collect();
        assert_eq!(titles, vec!["Arrival", "Interstellar"]);
    }

    #[test]
    fn apply_all_three_search_genre_decade() {
        let items = sample_library();
        let filter = FilterState {
            genres: Some(GenreFilter {
                selected_genres: vec!["Sci-Fi".to_string()],
            }),
            decade: Some(DecadeFilter { decade_start: 2010 }),
        };
        // Search "i", Sci-Fi, 2010s → "Arrival" (2016) and "Interstellar" (2014)
        let indices = apply_filters_and_sort(&items, "i", &filter, SortOrder::TitleAsc);
        let titles: Vec<&str> = indices.iter().map(|&i| items[i].title.as_str()).collect();
        assert_eq!(titles, vec!["Arrival", "Interstellar"]);
    }

    #[test]
    fn apply_returns_correct_indices() {
        let items = sample_library();
        let filter = FilterState::default();
        let indices = apply_filters_and_sort(&items, "Dune", &filter, SortOrder::TitleAsc);
        assert_eq!(indices, vec![0]); // "Dune" is at index 0
    }

    #[test]
    fn apply_empty_items_returns_empty() {
        let items: Vec<MediaItem> = vec![];
        let filter = FilterState::default();
        let indices = apply_filters_and_sort(&items, "", &filter, SortOrder::TitleAsc);
        assert!(indices.is_empty());
    }

    #[test]
    fn apply_all_filtered_out_returns_empty() {
        let items = sample_library();
        let filter = FilterState::default();
        let indices = apply_filters_and_sort(&items, "xyznonexistent", &filter, SortOrder::TitleAsc);
        assert!(indices.is_empty());
    }

    #[test]
    fn apply_sort_order_respected() {
        let items = sample_library();
        let filter = FilterState::default();
        let indices = apply_filters_and_sort(&items, "", &filter, SortOrder::YearNewest);
        let titles: Vec<&str> = indices.iter().map(|&i| items[i].title.as_str()).collect();
        assert_eq!(
            titles,
            vec!["Dune", "Arrival", "Interstellar", "The Hangover", "Zoolander"]
        );
    }

    // === extract_genres tests ===

    #[test]
    fn extract_genres_unique_sorted() {
        let items = sample_library();
        let genres = extract_genres(&items);
        assert_eq!(genres, vec!["Adventure", "Comedy", "Drama", "Sci-Fi"]);
    }

    #[test]
    fn extract_genres_empty_items() {
        let items: Vec<MediaItem> = vec![];
        let genres = extract_genres(&items);
        assert!(genres.is_empty());
    }

    #[test]
    fn extract_genres_skips_items_with_no_genres() {
        let items = vec![
            movie_with("A", Some(2020), None, vec!["Action"]),
            movie_with("B", Some(2020), None, vec![]),
        ];
        let genres = extract_genres(&items);
        assert_eq!(genres, vec!["Action"]);
    }

    // === extract_decades tests ===

    #[test]
    fn extract_decades_unique_sorted_newest_first() {
        let items = sample_library(); // 2021, 2016, 2009, 2001, 2014
        let decades = extract_decades(&items);
        assert_eq!(decades, vec![2020, 2010, 2000]);
    }

    #[test]
    fn extract_decades_items_with_no_year_excluded() {
        let items = vec![
            movie_with("A", Some(2020), None, vec![]),
            movie_with("B", None, None, vec![]),
        ];
        let decades = extract_decades(&items);
        assert_eq!(decades, vec![2020]);
    }

    #[test]
    fn extract_decades_empty_items() {
        let items: Vec<MediaItem> = vec![];
        let decades = extract_decades(&items);
        assert!(decades.is_empty());
    }

    // === FilterState tests ===

    #[test]
    fn filter_state_default_not_active() {
        let filter = FilterState::default();
        assert!(!filter.is_active());
    }

    #[test]
    fn filter_state_with_genre_is_active() {
        let filter = FilterState {
            genres: Some(GenreFilter {
                selected_genres: vec!["Action".to_string()],
            }),
            decade: None,
        };
        assert!(filter.is_active());
    }

    #[test]
    fn filter_state_clear_resets() {
        let mut filter = FilterState {
            genres: Some(GenreFilter {
                selected_genres: vec!["Action".to_string()],
            }),
            decade: Some(DecadeFilter { decade_start: 2020 }),
        };
        filter.clear();
        assert!(!filter.is_active());
        assert!(filter.genres.is_none());
        assert!(filter.decade.is_none());
    }

    // === SortOrder tests ===

    #[test]
    fn sort_order_all_returns_seven() {
        assert_eq!(SortOrder::all().len(), 7);
    }

    #[test]
    fn sort_order_labels_not_empty() {
        for order in SortOrder::all() {
            assert!(!order.label().is_empty());
        }
    }

    #[test]
    fn sort_order_default_is_title_asc() {
        assert_eq!(SortOrder::default(), SortOrder::TitleAsc);
    }
}
