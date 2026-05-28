use std::cmp::Ordering;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::models::media::MediaItem;

/// Available sort orders for the library view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
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

/// Grid view mode: poster grid or compact list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Grid,
    List,
}

impl ViewMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Grid => "grid",
            Self::List => "list",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "list" => Self::List,
            _ => Self::Grid,
        }
    }
}

/// Grid poster density: small, medium, or large.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GridDensity {
    Small,
    #[default]
    Medium,
    Large,
}

impl GridDensity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "small" => Self::Small,
            "large" => Self::Large,
            _ => Self::Medium,
        }
    }

    /// Card width in pixels.
    pub fn card_width(self) -> i32 {
        match self {
            Self::Small => 120,
            Self::Medium => 180,
            Self::Large => 240,
        }
    }

    /// Card height in pixels (1.5:1 aspect ratio).
    pub fn card_height(self) -> i32 {
        match self {
            Self::Small => 180,
            Self::Medium => 270,
            Self::Large => 360,
        }
    }

    /// Minimum grid columns at this density.
    pub fn min_columns(self) -> u32 {
        match self {
            Self::Small => 4,
            Self::Medium => 3,
            Self::Large => 2,
        }
    }

    /// All available densities.
    pub fn all() -> &'static [GridDensity] {
        &[Self::Small, Self::Medium, Self::Large]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
        }
    }
}

/// Watch status of a media item, derived from watch-progress data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchStatus {
    Unwatched,
    InProgress,
    Watched,
}

impl WatchStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unwatched => "Unwatched",
            Self::InProgress => "In Progress",
            Self::Watched => "Watched",
        }
    }
}

/// Derive a `WatchStatus` from a `(progress_fraction, watched)` pair as stored
/// in `LibraryView`'s watch_data map. `watched` wins over progress.
pub fn derived_watch_status(progress: f64, watched: bool) -> WatchStatus {
    if watched {
        WatchStatus::Watched
    } else if progress > 0.0 {
        WatchStatus::InProgress
    } else {
        WatchStatus::Unwatched
    }
}

/// Resolution bucket derived from a raw `video_resolution` string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionBucket {
    Sd,
    R720p,
    R1080p,
    R4k,
}

impl ResolutionBucket {
    pub fn label(self) -> &'static str {
        match self {
            Self::Sd => "SD",
            Self::R720p => "720p",
            Self::R1080p => "1080p",
            Self::R4k => "4K",
        }
    }

    pub fn all() -> &'static [ResolutionBucket] {
        &[Self::Sd, Self::R720p, Self::R1080p, Self::R4k]
    }

    /// Map a raw `video_resolution` string into a bucket. Returns `None` for
    /// unrecognised values (which `format_resolution` maps to generic "HD").
    pub fn from_resolution(raw: &str) -> Option<ResolutionBucket> {
        match MediaItem::format_resolution(raw) {
            "4K" => Some(Self::R4k),
            "1080p" => Some(Self::R1080p),
            "720p" => Some(Self::R720p),
            "SD" => Some(Self::Sd),
            _ => None,
        }
    }
}

/// HDR side of the resolution filter: match anything with an HDR signal, or
/// only items with no HDR signal (SDR).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HdrSelection {
    AnyHdr,
    SdrOnly,
}

/// A genre filter with OR semantics: item matches if it has ANY selected genre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenreFilter {
    pub selected_genres: Vec<String>,
}

/// Inclusive year range. Either bound may be unset, meaning "no limit".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct YearRangeFilter {
    pub from: Option<i32>,
    pub to: Option<i32>,
}

/// Watch-status filter (single selected status).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchStatusFilter {
    pub status: WatchStatus,
}

/// Minimum-rating threshold filter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RatingFilter {
    pub min: f64,
}

/// Inclusive runtime range in minutes. Either bound may be unset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RuntimeRangeFilter {
    pub from: Option<i32>,
    pub to: Option<i32>,
}

/// Content-rating filter with OR semantics over selected MPAA-style ratings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentRatingFilter {
    pub selected: Vec<String>,
}

/// Resolution + HDR filter. Buckets OR together; an HDR selection ANDs with the
/// bucket set.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ResolutionFilter {
    pub buckets: Vec<ResolutionBucket>,
    pub hdr: Option<HdrSelection>,
}

/// Combined filter state. All active filter types must pass (AND between
/// types); multi-select values within a type OR together.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FilterState {
    pub genres: Option<GenreFilter>,
    pub year_range: Option<YearRangeFilter>,
    pub watch_status: Option<WatchStatusFilter>,
    pub rating_min: Option<RatingFilter>,
    pub runtime_range: Option<RuntimeRangeFilter>,
    pub content_ratings: Option<ContentRatingFilter>,
    pub resolutions: Option<ResolutionFilter>,
}

impl FilterState {
    /// Returns true if any filter is active.
    pub fn is_active(&self) -> bool {
        self.genres.is_some()
            || self.year_range.is_some()
            || self.watch_status.is_some()
            || self.rating_min.is_some()
            || self.runtime_range.is_some()
            || self.content_ratings.is_some()
            || self.resolutions.is_some()
    }

    /// Reset all filters to inactive.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Remove a single filter value identified by a `FilterTag` (used to wire
    /// per-pill removal). Removing the last value of a multi-select type clears
    /// the whole type.
    pub fn remove(&mut self, tag: &FilterTag) {
        match tag {
            FilterTag::WatchStatus => self.watch_status = None,
            FilterTag::YearRange => self.year_range = None,
            FilterTag::Rating => self.rating_min = None,
            FilterTag::Runtime => self.runtime_range = None,
            FilterTag::Hdr => {
                if let Some(res) = self.resolutions.as_mut() {
                    res.hdr = None;
                    if res.buckets.is_empty() {
                        self.resolutions = None;
                    }
                }
            }
            FilterTag::Genre(g) => {
                if let Some(gf) = self.genres.as_mut() {
                    gf.selected_genres.retain(|x| x != g);
                    if gf.selected_genres.is_empty() {
                        self.genres = None;
                    }
                }
            }
            FilterTag::ContentRating(c) => {
                if let Some(cf) = self.content_ratings.as_mut() {
                    cf.selected.retain(|x| x != c);
                    if cf.selected.is_empty() {
                        self.content_ratings = None;
                    }
                }
            }
            FilterTag::Resolution(b) => {
                if let Some(res) = self.resolutions.as_mut() {
                    res.buckets.retain(|x| x != b);
                    if res.buckets.is_empty() && res.hdr.is_none() {
                        self.resolutions = None;
                    }
                }
            }
        }
    }
}

/// Identifies a single active filter value, so a pill's ✘ can target exactly
/// the value it represents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterTag {
    WatchStatus,
    YearRange,
    Genre(String),
    Rating,
    Runtime,
    ContentRating(String),
    Resolution(ResolutionBucket),
    Hdr,
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
/// Filters are AND-combined across types; multi-select values OR within a type.
/// `watch_status` is the item's resolved status (callers compute it from
/// watch-progress data so this stays a pure function).
pub fn filter_matches(item: &MediaItem, filter: &FilterState, watch_status: WatchStatus) -> bool {
    if let Some(ref genre_filter) = filter.genres
        && !genre_filter.selected_genres.is_empty()
        && !genre_filter
            .selected_genres
            .iter()
            .any(|g| item.genres.contains(g))
    {
        return false;
    }

    if let Some(ref yr) = filter.year_range
        && (yr.from.is_some() || yr.to.is_some())
    {
        match item.year {
            Some(year) => {
                if let Some(from) = yr.from
                    && year < from
                {
                    return false;
                }
                if let Some(to) = yr.to
                    && year > to
                {
                    return false;
                }
            }
            None => return false,
        }
    }

    if let Some(ws) = filter.watch_status
        && ws.status != watch_status
    {
        return false;
    }

    if let Some(rf) = filter.rating_min {
        match item.rating {
            Some(rating) if rating >= rf.min => {}
            _ => return false,
        }
    }

    if let Some(ref rt) = filter.runtime_range
        && (rt.from.is_some() || rt.to.is_some())
    {
        match item.runtime_minutes {
            Some(mins) => {
                if let Some(from) = rt.from
                    && mins < from
                {
                    return false;
                }
                if let Some(to) = rt.to
                    && mins > to
                {
                    return false;
                }
            }
            None => return false,
        }
    }

    if let Some(ref cr) = filter.content_ratings
        && !cr.selected.is_empty()
    {
        match item.content_rating {
            Some(ref rating) if cr.selected.contains(rating) => {}
            _ => return false,
        }
    }

    if let Some(ref res) = filter.resolutions {
        if !res.buckets.is_empty() {
            let bucket = item
                .video_resolution
                .as_deref()
                .and_then(ResolutionBucket::from_resolution);
            match bucket {
                Some(b) if res.buckets.contains(&b) => {}
                _ => return false,
            }
        }
        if let Some(hdr_sel) = res.hdr {
            let matches = match hdr_sel {
                HdrSelection::AnyHdr => item.hdr.is_some(),
                HdrSelection::SdrOnly => item.hdr.is_none(),
            };
            if !matches {
                return false;
            }
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
/// `watch_data` maps item id -> (progress_fraction, watched) and is used to
/// resolve the watch-status filter.
pub fn apply_filters_and_sort(
    items: &[MediaItem],
    query: &str,
    filter: &FilterState,
    sort: SortOrder,
    watch_data: &HashMap<String, (f64, bool)>,
) -> Vec<usize> {
    let mut indices: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            let (progress, watched) = watch_data.get(&item.id).copied().unwrap_or((0.0, false));
            let ws = derived_watch_status(progress, watched);
            search_matches(item, query) && filter_matches(item, filter, ws)
        })
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

/// Extract unique content ratings present in items, sorted alphabetically.
pub fn extract_content_ratings(items: &[MediaItem]) -> Vec<String> {
    let mut ratings: Vec<String> = items
        .iter()
        .filter_map(|item| item.content_rating.clone())
        .collect();
    ratings.sort();
    ratings.dedup();
    ratings
}

/// Extract the resolution buckets present in items, in canonical order
/// (SD → 4K).
pub fn extract_resolution_buckets(items: &[MediaItem]) -> Vec<ResolutionBucket> {
    let present: Vec<ResolutionBucket> = items
        .iter()
        .filter_map(|item| item.video_resolution.as_deref())
        .filter_map(ResolutionBucket::from_resolution)
        .collect();
    ResolutionBucket::all()
        .iter()
        .copied()
        .filter(|b| present.contains(b))
        .collect()
}

/// Build the human-readable pill list for the active filters. One entry per
/// active value; each carries the `FilterTag` so the UI can wire its ✘.
pub fn pill_labels(filter: &FilterState) -> Vec<(FilterTag, String)> {
    let mut pills = Vec::new();

    if let Some(ws) = filter.watch_status {
        pills.push((FilterTag::WatchStatus, ws.status.label().to_string()));
    }

    if let Some(yr) = filter.year_range {
        let label = match (yr.from, yr.to) {
            (Some(f), Some(t)) => Some(format!("Year {f}\u{2013}{t}")),
            (Some(f), None) => Some(format!("Year \u{2265}{f}")),
            (None, Some(t)) => Some(format!("Year \u{2264}{t}")),
            (None, None) => None,
        };
        if let Some(label) = label {
            pills.push((FilterTag::YearRange, label));
        }
    }

    if let Some(ref gf) = filter.genres {
        for g in &gf.selected_genres {
            pills.push((FilterTag::Genre(g.clone()), format!("Genre: {g}")));
        }
    }

    if let Some(rf) = filter.rating_min {
        pills.push((FilterTag::Rating, format!("Rating \u{2265}{:.1}", rf.min)));
    }

    if let Some(rt) = filter.runtime_range {
        let label = match (rt.from, rt.to) {
            (Some(f), Some(t)) => Some(format!("Runtime {f}\u{2013}{t} min")),
            (Some(f), None) => Some(format!("Runtime \u{2265}{f} min")),
            (None, Some(t)) => Some(format!("Runtime <{} min", t + 1)),
            (None, None) => None,
        };
        if let Some(label) = label {
            pills.push((FilterTag::Runtime, label));
        }
    }

    if let Some(ref cf) = filter.content_ratings {
        for c in &cf.selected {
            pills.push((FilterTag::ContentRating(c.clone()), c.clone()));
        }
    }

    if let Some(ref res) = filter.resolutions {
        for b in &res.buckets {
            pills.push((FilterTag::Resolution(*b), b.label().to_string()));
        }
        match res.hdr {
            Some(HdrSelection::AnyHdr) => pills.push((FilterTag::Hdr, "HDR".to_string())),
            Some(HdrSelection::SdrOnly) => pills.push((FilterTag::Hdr, "SDR only".to_string())),
            None => {}
        }
    }

    pills
}

/// Drop any persisted filter values that no longer correspond to something in
/// the current item set, so a restored filter never shows a pill that matches
/// nothing. Genre, content-rating, and resolution values are reconciled;
/// numeric ranges and watch status are left intact.
pub fn reconcile(filter: &FilterState, items: &[MediaItem]) -> FilterState {
    let mut out = filter.clone();

    let available_genres = extract_genres(items);
    if let Some(gf) = out.genres.as_mut() {
        gf.selected_genres.retain(|g| available_genres.contains(g));
        if gf.selected_genres.is_empty() {
            out.genres = None;
        }
    }

    let available_ratings = extract_content_ratings(items);
    if let Some(cf) = out.content_ratings.as_mut() {
        cf.selected.retain(|c| available_ratings.contains(c));
        if cf.selected.is_empty() {
            out.content_ratings = None;
        }
    }

    let available_buckets = extract_resolution_buckets(items);
    if let Some(res) = out.resolutions.as_mut() {
        res.buckets.retain(|b| available_buckets.contains(b));
        if res.buckets.is_empty() && res.hdr.is_none() {
            out.resolutions = None;
        }
    }

    out
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
    use crate::models::media::{HdrFormat, MediaType, SourceType};

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
            video_resolution: None,
            hdr: None,
            added_at: "1700000000".to_string(),
            updated_at: "1700000000".to_string(),
            playback_position_ms: None,
        }
    }

    fn movie_with(
        title: &str,
        year: Option<i32>,
        rating: Option<f64>,
        genres: Vec<&str>,
    ) -> MediaItem {
        let mut m = movie(title);
        m.year = year;
        m.rating = rating;
        m.genres = genres.into_iter().map(String::from).collect();
        m
    }

    /// Watch-status filter test uses Unwatched as the "doesn't matter" default.
    const ANY: WatchStatus = WatchStatus::Unwatched;

    fn no_watch() -> HashMap<String, (f64, bool)> {
        HashMap::new()
    }

    // === Search tests ===

    #[test]
    fn search_empty_query_matches_all() {
        let item = movie("Dune");
        assert!(search_matches(&item, ""));
        assert!(search_matches(&item, "  "));
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
    }

    #[test]
    fn search_no_match_returns_false() {
        let item = movie("Dune");
        assert!(!search_matches(&item, "Avatar"));
    }

    // === Genre filter tests ===

    fn genre_filter(genres: &[&str]) -> FilterState {
        FilterState {
            genres: Some(GenreFilter {
                selected_genres: genres.iter().map(|s| s.to_string()).collect(),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn genre_filter_single_genre_matches() {
        let item = movie_with("Dune", Some(2021), None, vec!["Action", "Sci-Fi"]);
        assert!(filter_matches(&item, &genre_filter(&["Action"]), ANY));
    }

    #[test]
    fn genre_filter_single_genre_no_match() {
        let item = movie_with("Dune", Some(2021), None, vec!["Action", "Sci-Fi"]);
        assert!(!filter_matches(&item, &genre_filter(&["Comedy"]), ANY));
    }

    #[test]
    fn genre_filter_multi_genre_or_semantics() {
        let item = movie_with("Dune", Some(2021), None, vec!["Action"]);
        assert!(filter_matches(
            &item,
            &genre_filter(&["Comedy", "Action"]),
            ANY
        ));
    }

    #[test]
    fn genre_filter_empty_selection_matches_all() {
        let item = movie_with("Dune", Some(2021), None, vec!["Action"]);
        assert!(filter_matches(&item, &genre_filter(&[]), ANY));
    }

    #[test]
    fn genre_filter_item_with_no_genres_never_matches() {
        let item = movie_with("Unknown", Some(2021), None, vec![]);
        assert!(!filter_matches(&item, &genre_filter(&["Action"]), ANY));
    }

    // === Year range tests ===

    fn year_filter(from: Option<i32>, to: Option<i32>) -> FilterState {
        FilterState {
            year_range: Some(YearRangeFilter { from, to }),
            ..Default::default()
        }
    }

    #[test]
    fn year_range_both_bounds_matches_inside() {
        let item = movie_with("M", Some(2005), None, vec![]);
        assert!(filter_matches(
            &item,
            &year_filter(Some(2000), Some(2009)),
            ANY
        ));
    }

    #[test]
    fn year_range_lower_only_matches_at_or_above() {
        let item = movie_with("M", Some(1995), None, vec![]);
        assert!(filter_matches(&item, &year_filter(Some(1990), None), ANY));
        let older = movie_with("Old", Some(1985), None, vec![]);
        assert!(!filter_matches(&older, &year_filter(Some(1990), None), ANY));
    }

    #[test]
    fn year_range_upper_only_matches_at_or_below() {
        let item = movie_with("M", Some(1999), None, vec![]);
        assert!(filter_matches(&item, &year_filter(None, Some(2000)), ANY));
        let newer = movie_with("New", Some(2010), None, vec![]);
        assert!(!filter_matches(&newer, &year_filter(None, Some(2000)), ANY));
    }

    #[test]
    fn year_range_item_with_no_year_excluded_when_any_bound_set() {
        let item = movie_with("NoYear", None, None, vec![]);
        assert!(!filter_matches(&item, &year_filter(Some(1990), None), ANY));
    }

    #[test]
    fn year_range_no_bounds_matches_all() {
        let item = movie_with("NoYear", None, None, vec![]);
        assert!(filter_matches(&item, &year_filter(None, None), ANY));
    }

    #[test]
    fn year_range_boundary_inclusive_both_ends() {
        let start = movie_with("Start", Some(2000), None, vec![]);
        let end = movie_with("End", Some(2009), None, vec![]);
        let f = year_filter(Some(2000), Some(2009));
        assert!(filter_matches(&start, &f, ANY));
        assert!(filter_matches(&end, &f, ANY));
    }

    // === Watch status tests ===

    fn watch_filter(status: WatchStatus) -> FilterState {
        FilterState {
            watch_status: Some(WatchStatusFilter { status }),
            ..Default::default()
        }
    }

    #[test]
    fn derived_watch_status_rules() {
        assert_eq!(derived_watch_status(0.0, false), WatchStatus::Unwatched);
        assert_eq!(derived_watch_status(0.5, false), WatchStatus::InProgress);
        assert_eq!(derived_watch_status(0.0, true), WatchStatus::Watched);
        // watched wins over progress
        assert_eq!(derived_watch_status(0.5, true), WatchStatus::Watched);
    }

    #[test]
    fn watch_status_unwatched_matches_unwatched_item() {
        let item = movie("Dune");
        assert!(filter_matches(
            &item,
            &watch_filter(WatchStatus::Unwatched),
            WatchStatus::Unwatched
        ));
    }

    #[test]
    fn watch_status_in_progress_excludes_watched() {
        let item = movie("Dune");
        assert!(!filter_matches(
            &item,
            &watch_filter(WatchStatus::InProgress),
            WatchStatus::Watched
        ));
    }

    #[test]
    fn watch_status_unwatched_excludes_in_progress() {
        let item = movie("Dune");
        assert!(!filter_matches(
            &item,
            &watch_filter(WatchStatus::Unwatched),
            WatchStatus::InProgress
        ));
    }

    // === Rating tests ===

    fn rating_filter(min: f64) -> FilterState {
        FilterState {
            rating_min: Some(RatingFilter { min }),
            ..Default::default()
        }
    }

    #[test]
    fn rating_min_matches_at_or_above_threshold() {
        let item = movie_with("M", Some(2020), Some(7.0), vec![]);
        assert!(filter_matches(&item, &rating_filter(7.0), ANY));
    }

    #[test]
    fn rating_min_excludes_below_threshold() {
        let item = movie_with("M", Some(2020), Some(6.9), vec![]);
        assert!(!filter_matches(&item, &rating_filter(7.0), ANY));
    }

    #[test]
    fn rating_min_excludes_item_with_no_rating() {
        let item = movie_with("M", Some(2020), None, vec![]);
        assert!(!filter_matches(&item, &rating_filter(7.0), ANY));
    }

    // === Runtime range tests ===

    fn runtime_filter(from: Option<i32>, to: Option<i32>) -> FilterState {
        FilterState {
            runtime_range: Some(RuntimeRangeFilter { from, to }),
            ..Default::default()
        }
    }

    #[test]
    fn runtime_range_inside_matches() {
        let item = movie("M"); // 120 min
        assert!(filter_matches(
            &item,
            &runtime_filter(Some(90), Some(130)),
            ANY
        ));
    }

    #[test]
    fn runtime_range_only_max_matches_at_or_below() {
        let mut item = movie("M");
        item.runtime_minutes = Some(95);
        assert!(filter_matches(&item, &runtime_filter(None, Some(100)), ANY));
        item.runtime_minutes = Some(140);
        assert!(!filter_matches(
            &item,
            &runtime_filter(None, Some(100)),
            ANY
        ));
    }

    #[test]
    fn runtime_range_only_min_matches_at_or_above() {
        let mut item = movie("M");
        item.runtime_minutes = Some(200);
        assert!(filter_matches(&item, &runtime_filter(Some(180), None), ANY));
    }

    #[test]
    fn runtime_range_no_runtime_excluded_when_any_bound_set() {
        let mut item = movie("M");
        item.runtime_minutes = None;
        assert!(!filter_matches(
            &item,
            &runtime_filter(None, Some(100)),
            ANY
        ));
    }

    // === Content rating tests ===

    fn cr_filter(ratings: &[&str]) -> FilterState {
        FilterState {
            content_ratings: Some(ContentRatingFilter {
                selected: ratings.iter().map(|s| s.to_string()).collect(),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn content_rating_single_selected_matches() {
        let mut item = movie("M");
        item.content_rating = Some("PG-13".to_string());
        assert!(filter_matches(&item, &cr_filter(&["PG-13"]), ANY));
    }

    #[test]
    fn content_rating_multi_selected_or_semantics() {
        let mut item = movie("M");
        item.content_rating = Some("R".to_string());
        assert!(filter_matches(&item, &cr_filter(&["PG-13", "R"]), ANY));
    }

    #[test]
    fn content_rating_empty_selection_matches_all() {
        let item = movie("M");
        assert!(filter_matches(&item, &cr_filter(&[]), ANY));
    }

    #[test]
    fn content_rating_item_with_no_rating_excluded_when_set() {
        let item = movie("M"); // content_rating None
        assert!(!filter_matches(&item, &cr_filter(&["PG-13"]), ANY));
    }

    // === Resolution + HDR tests ===

    fn res_filter(buckets: Vec<ResolutionBucket>, hdr: Option<HdrSelection>) -> FilterState {
        FilterState {
            resolutions: Some(ResolutionFilter { buckets, hdr }),
            ..Default::default()
        }
    }

    #[test]
    fn resolution_bucket_from_resolution_mapping() {
        assert_eq!(
            ResolutionBucket::from_resolution("4k"),
            Some(ResolutionBucket::R4k)
        );
        assert_eq!(
            ResolutionBucket::from_resolution("1080"),
            Some(ResolutionBucket::R1080p)
        );
        assert_eq!(
            ResolutionBucket::from_resolution("720"),
            Some(ResolutionBucket::R720p)
        );
        assert_eq!(
            ResolutionBucket::from_resolution("480"),
            Some(ResolutionBucket::Sd)
        );
    }

    #[test]
    fn resolution_4k_matches_only_4k_items() {
        let mut item = movie("M");
        item.video_resolution = Some("4k".to_string());
        assert!(filter_matches(
            &item,
            &res_filter(vec![ResolutionBucket::R4k], None),
            ANY
        ));
        item.video_resolution = Some("1080".to_string());
        assert!(!filter_matches(
            &item,
            &res_filter(vec![ResolutionBucket::R4k], None),
            ANY
        ));
    }

    #[test]
    fn resolution_multi_bucket_or_semantics() {
        let mut item = movie("M");
        item.video_resolution = Some("1080".to_string());
        let f = res_filter(vec![ResolutionBucket::R4k, ResolutionBucket::R1080p], None);
        assert!(filter_matches(&item, &f, ANY));
    }

    #[test]
    fn resolution_item_with_no_resolution_excluded_when_bucket_set() {
        let item = movie("M"); // video_resolution None
        assert!(!filter_matches(
            &item,
            &res_filter(vec![ResolutionBucket::R4k], None),
            ANY
        ));
    }

    #[test]
    fn hdr_any_matches_hdr_and_dolby_vision() {
        let mut item = movie("M");
        item.hdr = Some(HdrFormat::Hdr);
        assert!(filter_matches(
            &item,
            &res_filter(vec![], Some(HdrSelection::AnyHdr)),
            ANY
        ));
        item.hdr = Some(HdrFormat::DolbyVision);
        assert!(filter_matches(
            &item,
            &res_filter(vec![], Some(HdrSelection::AnyHdr)),
            ANY
        ));
        item.hdr = None;
        assert!(!filter_matches(
            &item,
            &res_filter(vec![], Some(HdrSelection::AnyHdr)),
            ANY
        ));
    }

    #[test]
    fn hdr_sdr_only_matches_items_with_no_hdr() {
        let mut item = movie("M");
        item.hdr = None;
        assert!(filter_matches(
            &item,
            &res_filter(vec![], Some(HdrSelection::SdrOnly)),
            ANY
        ));
        item.hdr = Some(HdrFormat::Hdr);
        assert!(!filter_matches(
            &item,
            &res_filter(vec![], Some(HdrSelection::SdrOnly)),
            ANY
        ));
    }

    #[test]
    fn resolution_and_hdr_combine_with_and() {
        let mut item = movie("M");
        item.video_resolution = Some("4k".to_string());
        item.hdr = Some(HdrFormat::Hdr);
        let f = res_filter(vec![ResolutionBucket::R4k], Some(HdrSelection::AnyHdr));
        assert!(filter_matches(&item, &f, ANY));
        // 4K but SDR fails the AND
        item.hdr = None;
        assert!(!filter_matches(&item, &f, ANY));
    }

    // === Combined filter tests ===

    #[test]
    fn all_filters_active_and_combined() {
        let mut item = movie_with("Dune", Some(2021), Some(8.0), vec!["Sci-Fi"]);
        item.content_rating = Some("PG-13".to_string());
        item.video_resolution = Some("4k".to_string());
        item.hdr = Some(HdrFormat::Hdr);
        item.runtime_minutes = Some(155);
        let filter = FilterState {
            genres: Some(GenreFilter {
                selected_genres: vec!["Sci-Fi".to_string()],
            }),
            year_range: Some(YearRangeFilter {
                from: Some(2020),
                to: None,
            }),
            watch_status: Some(WatchStatusFilter {
                status: WatchStatus::Unwatched,
            }),
            rating_min: Some(RatingFilter { min: 7.0 }),
            runtime_range: Some(RuntimeRangeFilter {
                from: Some(120),
                to: Some(180),
            }),
            content_ratings: Some(ContentRatingFilter {
                selected: vec!["PG-13".to_string()],
            }),
            resolutions: Some(ResolutionFilter {
                buckets: vec![ResolutionBucket::R4k],
                hdr: Some(HdrSelection::AnyHdr),
            }),
        };
        assert!(filter_matches(&item, &filter, WatchStatus::Unwatched));
        // One failing dimension (year) breaks the AND
        let mut older = item.clone();
        older.year = Some(2010);
        assert!(!filter_matches(&older, &filter, WatchStatus::Unwatched));
    }

    #[test]
    fn no_filters_active_matches_all() {
        let item = movie("Dune");
        assert!(filter_matches(&item, &FilterState::default(), ANY));
    }

    // === Sort tests ===

    #[test]
    fn sort_title_asc_alphabetical() {
        let a = movie("Arrival");
        let b = movie("Dune");
        assert_eq!(sort_compare(&a, &b, SortOrder::TitleAsc), Ordering::Less);
    }

    #[test]
    fn sort_year_newest_first() {
        let a = movie_with("Old", Some(2010), None, vec![]);
        let b = movie_with("New", Some(2024), None, vec![]);
        assert_eq!(
            sort_compare(&a, &b, SortOrder::YearNewest),
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
    fn sort_stable_tiebreak_by_title() {
        let a = movie_with("Arrival", Some(2020), Some(8.0), vec![]);
        let b = movie_with("Dune", Some(2020), Some(8.0), vec![]);
        assert_eq!(sort_compare(&a, &b, SortOrder::YearNewest), Ordering::Less);
        assert_eq!(
            sort_compare(&a, &b, SortOrder::RatingHighest),
            Ordering::Less
        );
    }

    // === apply_filters_and_sort tests ===

    fn sample_library() -> Vec<MediaItem> {
        vec![
            movie_with("Dune", Some(2021), Some(8.0), vec!["Sci-Fi", "Adventure"]),
            movie_with("Arrival", Some(2016), Some(7.9), vec!["Sci-Fi", "Drama"]),
            movie_with("The Hangover", Some(2009), Some(7.7), vec!["Comedy"]),
            movie_with("Zoolander", Some(2001), Some(6.5), vec!["Comedy"]),
            movie_with(
                "Interstellar",
                Some(2014),
                Some(8.7),
                vec!["Sci-Fi", "Drama"],
            ),
        ]
    }

    #[test]
    fn apply_search_and_genre_filter_combined() {
        let items = sample_library();
        let filter = genre_filter(&["Sci-Fi"]);
        let indices =
            apply_filters_and_sort(&items, "ar", &filter, SortOrder::TitleAsc, &no_watch());
        let titles: Vec<&str> = indices.iter().map(|&i| items[i].title.as_str()).collect();
        assert_eq!(titles, vec!["Arrival", "Interstellar"]);
    }

    #[test]
    fn apply_search_genre_year_combined() {
        let items = sample_library();
        let filter = FilterState {
            genres: Some(GenreFilter {
                selected_genres: vec!["Sci-Fi".to_string()],
            }),
            year_range: Some(YearRangeFilter {
                from: Some(2010),
                to: Some(2019),
            }),
            ..Default::default()
        };
        let indices =
            apply_filters_and_sort(&items, "i", &filter, SortOrder::TitleAsc, &no_watch());
        let titles: Vec<&str> = indices.iter().map(|&i| items[i].title.as_str()).collect();
        assert_eq!(titles, vec!["Arrival", "Interstellar"]);
    }

    #[test]
    fn apply_watch_status_uses_watch_data() {
        let items = sample_library();
        let mut watch = HashMap::new();
        // Mark "Dune" watched, "Arrival" in progress.
        watch.insert(items[0].id.clone(), (0.0, true));
        watch.insert(items[1].id.clone(), (0.4, false));
        let filter = watch_filter(WatchStatus::Unwatched);
        let indices = apply_filters_and_sort(&items, "", &filter, SortOrder::TitleAsc, &watch);
        let titles: Vec<&str> = indices.iter().map(|&i| items[i].title.as_str()).collect();
        // Dune (watched) and Arrival (in progress) excluded.
        assert_eq!(titles, vec!["Interstellar", "The Hangover", "Zoolander"]);
    }

    #[test]
    fn apply_empty_items_returns_empty() {
        let items: Vec<MediaItem> = vec![];
        let indices = apply_filters_and_sort(
            &items,
            "",
            &FilterState::default(),
            SortOrder::TitleAsc,
            &no_watch(),
        );
        assert!(indices.is_empty());
    }

    #[test]
    fn apply_sort_order_respected() {
        let items = sample_library();
        let indices = apply_filters_and_sort(
            &items,
            "",
            &FilterState::default(),
            SortOrder::YearNewest,
            &no_watch(),
        );
        let titles: Vec<&str> = indices.iter().map(|&i| items[i].title.as_str()).collect();
        assert_eq!(
            titles,
            vec![
                "Dune",
                "Arrival",
                "Interstellar",
                "The Hangover",
                "Zoolander"
            ]
        );
    }

    // === extract helpers ===

    #[test]
    fn extract_genres_unique_sorted() {
        let items = sample_library();
        assert_eq!(
            extract_genres(&items),
            vec!["Adventure", "Comedy", "Drama", "Sci-Fi"]
        );
    }

    #[test]
    fn extract_content_ratings_unique_sorted() {
        let mut items = sample_library();
        items[0].content_rating = Some("PG-13".to_string());
        items[1].content_rating = Some("R".to_string());
        items[2].content_rating = Some("PG-13".to_string());
        assert_eq!(extract_content_ratings(&items), vec!["PG-13", "R"]);
    }

    #[test]
    fn extract_content_ratings_empty_when_none_present() {
        let items = sample_library();
        assert!(extract_content_ratings(&items).is_empty());
    }

    #[test]
    fn extract_resolution_buckets_in_canonical_order() {
        let mut items = sample_library();
        items[0].video_resolution = Some("4k".to_string());
        items[1].video_resolution = Some("480".to_string());
        items[2].video_resolution = Some("1080".to_string());
        // Returned SD → 4K regardless of insertion order; 720p absent.
        assert_eq!(
            extract_resolution_buckets(&items),
            vec![
                ResolutionBucket::Sd,
                ResolutionBucket::R1080p,
                ResolutionBucket::R4k
            ]
        );
    }

    // === pill_labels tests ===

    #[test]
    fn pill_for_unset_range_bound_not_emitted() {
        let filter = FilterState::default();
        assert!(pill_labels(&filter).is_empty());
    }

    #[test]
    fn pill_for_year_lower_bound_only_renders_geq() {
        let filter = year_filter(Some(1990), None);
        let pills = pill_labels(&filter);
        assert_eq!(pills.len(), 1);
        assert_eq!(pills[0].0, FilterTag::YearRange);
        assert_eq!(pills[0].1, "Year \u{2265}1990");
    }

    #[test]
    fn pill_for_year_upper_bound_only_renders_leq() {
        let pills = pill_labels(&year_filter(None, Some(2009)));
        assert_eq!(pills[0].1, "Year \u{2264}2009");
    }

    #[test]
    fn pill_for_year_both_bounds_renders_range() {
        let pills = pill_labels(&year_filter(Some(1990), Some(2009)));
        assert_eq!(pills[0].1, "Year 1990\u{2013}2009");
    }

    #[test]
    fn pill_for_each_genre_individually() {
        let filter = genre_filter(&["Sci-Fi", "Drama"]);
        let pills = pill_labels(&filter);
        assert_eq!(pills.len(), 2);
        assert_eq!(
            pills[0],
            (
                FilterTag::Genre("Sci-Fi".to_string()),
                "Genre: Sci-Fi".to_string()
            )
        );
        assert_eq!(
            pills[1],
            (
                FilterTag::Genre("Drama".to_string()),
                "Genre: Drama".to_string()
            )
        );
    }

    #[test]
    fn pill_for_rating_and_runtime() {
        let filter = FilterState {
            rating_min: Some(RatingFilter { min: 7.0 }),
            runtime_range: Some(RuntimeRangeFilter {
                from: None,
                to: Some(99),
            }),
            ..Default::default()
        };
        let pills = pill_labels(&filter);
        assert!(pills.contains(&(FilterTag::Rating, "Rating \u{2265}7.0".to_string())));
        assert!(pills.contains(&(FilterTag::Runtime, "Runtime <100 min".to_string())));
    }

    #[test]
    fn pill_for_resolution_and_hdr() {
        let filter = res_filter(vec![ResolutionBucket::R4k], Some(HdrSelection::AnyHdr));
        let pills = pill_labels(&filter);
        assert!(pills.contains(&(
            FilterTag::Resolution(ResolutionBucket::R4k),
            "4K".to_string()
        )));
        assert!(pills.contains(&(FilterTag::Hdr, "HDR".to_string())));
    }

    #[test]
    fn pill_for_watch_status() {
        let pills = pill_labels(&watch_filter(WatchStatus::Watched));
        assert_eq!(pills[0], (FilterTag::WatchStatus, "Watched".to_string()));
    }

    // === FilterState::remove tests ===

    #[test]
    fn remove_single_genre_keeps_others() {
        let mut filter = genre_filter(&["Sci-Fi", "Drama"]);
        filter.remove(&FilterTag::Genre("Sci-Fi".to_string()));
        assert_eq!(
            filter.genres.as_ref().unwrap().selected_genres,
            vec!["Drama".to_string()]
        );
    }

    #[test]
    fn remove_last_genre_clears_filter() {
        let mut filter = genre_filter(&["Sci-Fi"]);
        filter.remove(&FilterTag::Genre("Sci-Fi".to_string()));
        assert!(filter.genres.is_none());
    }

    #[test]
    fn remove_hdr_keeps_resolution_buckets() {
        let mut filter = res_filter(vec![ResolutionBucket::R4k], Some(HdrSelection::AnyHdr));
        filter.remove(&FilterTag::Hdr);
        let res = filter.resolutions.as_ref().unwrap();
        assert_eq!(res.buckets, vec![ResolutionBucket::R4k]);
        assert!(res.hdr.is_none());
    }

    #[test]
    fn remove_last_resolution_bucket_with_no_hdr_clears_filter() {
        let mut filter = res_filter(vec![ResolutionBucket::R4k], None);
        filter.remove(&FilterTag::Resolution(ResolutionBucket::R4k));
        assert!(filter.resolutions.is_none());
    }

    // === reconcile tests ===

    #[test]
    fn reconcile_drops_genre_not_in_items() {
        let items = sample_library(); // genres: Sci-Fi, Adventure, Comedy, Drama
        let filter = genre_filter(&["Sci-Fi", "Western"]);
        let out = reconcile(&filter, &items);
        assert_eq!(
            out.genres.as_ref().unwrap().selected_genres,
            vec!["Sci-Fi".to_string()]
        );
    }

    #[test]
    fn reconcile_drops_genre_filter_entirely_when_none_present() {
        let items = sample_library();
        let filter = genre_filter(&["Western"]);
        let out = reconcile(&filter, &items);
        assert!(out.genres.is_none());
    }

    #[test]
    fn reconcile_drops_content_rating_not_present() {
        let mut items = sample_library();
        items[0].content_rating = Some("PG-13".to_string());
        let filter = cr_filter(&["PG-13", "NC-17"]);
        let out = reconcile(&filter, &items);
        assert_eq!(
            out.content_ratings.as_ref().unwrap().selected,
            vec!["PG-13".to_string()]
        );
    }

    #[test]
    fn reconcile_drops_resolution_bucket_not_present() {
        let mut items = sample_library();
        items[0].video_resolution = Some("1080".to_string());
        let filter = res_filter(vec![ResolutionBucket::R4k, ResolutionBucket::R1080p], None);
        let out = reconcile(&filter, &items);
        assert_eq!(
            out.resolutions.as_ref().unwrap().buckets,
            vec![ResolutionBucket::R1080p]
        );
    }

    #[test]
    fn reconcile_keeps_numeric_ranges_and_watch_status() {
        let items = sample_library();
        let filter = FilterState {
            year_range: Some(YearRangeFilter {
                from: Some(2000),
                to: None,
            }),
            watch_status: Some(WatchStatusFilter {
                status: WatchStatus::Unwatched,
            }),
            rating_min: Some(RatingFilter { min: 7.0 }),
            ..Default::default()
        };
        let out = reconcile(&filter, &items);
        assert!(out.year_range.is_some());
        assert!(out.watch_status.is_some());
        assert!(out.rating_min.is_some());
    }

    // === FilterState misc ===

    #[test]
    fn filter_state_default_not_active() {
        assert!(!FilterState::default().is_active());
    }

    #[test]
    fn filter_state_each_filter_marks_active() {
        assert!(genre_filter(&["A"]).is_active());
        assert!(year_filter(Some(2000), None).is_active());
        assert!(watch_filter(WatchStatus::Watched).is_active());
        assert!(rating_filter(7.0).is_active());
        assert!(runtime_filter(Some(90), None).is_active());
        assert!(cr_filter(&["R"]).is_active());
        assert!(res_filter(vec![ResolutionBucket::R4k], None).is_active());
    }

    #[test]
    fn filter_state_clear_resets() {
        let mut filter = genre_filter(&["Action"]);
        filter.year_range = Some(YearRangeFilter {
            from: Some(2000),
            to: None,
        });
        filter.clear();
        assert!(!filter.is_active());
    }

    // === SortOrder ===

    #[test]
    fn sort_order_all_returns_seven() {
        assert_eq!(SortOrder::all().len(), 7);
    }

    #[test]
    fn sort_order_default_is_title_asc() {
        assert_eq!(SortOrder::default(), SortOrder::TitleAsc);
    }
}
