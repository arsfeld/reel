---
title: "M3: Library Search, Filter, and Sort"
type: feat
status: completed
date: 2026-03-14
---

# M3: Library Search, Filter, and Sort

## Overview

Add search, filtering, and sorting to the library view. Users can instantly search by title, filter by genre and decade, sort by multiple criteria, and combine all three. All logic lives in pure, testable functions outside of GTK/Relm4 -- the UI components are thin wrappers that delegate to the service layer.

**Scope**: Search, filter, sort only. Collections, enhanced detail pages, list view, grid density, and adaptive layout are deferred to a separate M3b plan.

## Problem Statement / Motivation

With M2 complete, users can browse their Plex library as a poster grid, but with no way to narrow down a large collection. A user with 800+ movies must visually scan the entire grid to find what they want. There is no search, no genre filtering, no year filtering, and the sort is fixed at title A-Z. This makes the library unusable at scale and falls far short of the "Infuse for Linux" goal.

## Proposed Solution

### Architecture: Client-Side Filtering

The current `LibraryView` fetches all items from Plex into memory and pushes them into `TypedGridView`. We build on this by **retaining the full `Vec<MediaItem>` in `LibraryView`** and applying search/filter/sort as pure functions on that dataset. This approach is pragmatic because:

1. The data is already loaded in memory from Plex
2. Typical libraries (100-2000 items) are trivially fast to filter client-side
3. No new database query infrastructure needed
4. Instant UI updates with no async round-trips

### Core Pattern: Pure Functions + Thin UI

Following the project's established PlaybackTracker pattern, all logic lives in a new `src/services/library_filter.rs` module:

```
User types/clicks → UI sends message → LibraryView calls pure filter function → Result fed to TypedGridView
```

The pure filter module has zero GTK dependencies and is exhaustively unit-tested.

### Search: Filtered Grid

Search operates as a real-time filter on the poster grid (not a dropdown/popover). When the user types "dune", the grid instantly shows only matching items as poster cards. This is consistent with how filters work and matches the Infuse UX pattern.

**Scope**: Title-only search for M3. Searching across cast, crew, and collections requires extending `MediaItem`, `PlexMetadata`, and the database schema -- deferred to when enhanced detail pages (cast/crew data) are added.

## Technical Approach

### Architecture

```
┌──────────────────────────────────────────────────────┐
│                    LibraryView                         │
│                                                        │
│  all_items: Vec<MediaItem>     ← Full dataset          │
│  filter_state: FilterState     ← Current filters       │
│  sort_order: SortOrder         ← Current sort          │
│  search_query: String          ← Current search text   │
│                                                        │
│  ┌──────────────────────────────────────────────────┐ │
│  │ apply_filters_and_sort()  [pure function call]    │ │
│  │   → Vec<&MediaItem>       [filtered, sorted view] │ │
│  └──────────────────────────────────────────────────┘ │
│                        │                                │
│              ┌─────────▼──────────┐                    │
│              │   TypedGridView     │                    │
│              │  (rebuilt from      │                    │
│              │   filtered result)  │                    │
│              └────────────────────┘                    │
│                                                        │
│  ┌─────────────────┐ ┌────────────┐ ┌──────────────┐ │
│  │ SearchBar (GTK)  │ │ FilterBar  │ │ SortSelector │ │
│  │ gtk4::SearchBar  │ │ Genre+Year │ │ AdwComboRow  │ │
│  └─────────────────┘ └────────────┘ └──────────────┘ │
└──────────────────────────────────────────────────────┘
```

### New Files

| File | Purpose | Tests |
|------|---------|-------|
| `src/services/library_filter.rs` | `FilterState`, `SortOrder`, `apply_filters_and_sort()`, `search_matches()`, `filter_matches()`, `sort_compare()` | 50+ unit tests |
| `src/components/library/filter_bar.rs` | `FilterBar` Relm4 `SimpleComponent` -- genre multi-select, decade buttons, clear button | Manual only (GTK) |
| `src/components/library/sort_selector.rs` | `SortSelector` -- `AdwComboRow` or similar for sort option selection | Manual only (GTK) |

### Modified Files

| File | Changes |
|------|---------|
| `src/components/library/mod.rs` | Add `all_items`, `filter_state`, `sort_order`, `search_query` fields; add `SearchBar`, `FilterBar`, `SortSelector` children; new message variants; `rebuild_grid()` method |
| `src/app.rs` | Fix `is_text_input_focused` to check `root.focus_widget()`; add Ctrl+F/`/` shortcut for search activation; stop re-fetching on navigate-back |
| `src/components/player/shortcuts.rs` | No changes needed (already supports `is_text_input_focused`) |

### Implementation Phases

#### Phase 1: Pure Filter/Sort/Search Logic (`src/services/library_filter.rs`)

**Goal**: All search, filter, and sort logic in a single, fully-tested, zero-dependency module.

**Types**:

```rust
// src/services/library_filter.rs

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

/// A single genre filter selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenreFilter {
    pub selected_genres: Vec<String>,  // OR semantics: item matches if it has ANY selected genre
}

/// A decade filter (e.g., "2020s" = 2020..2029).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecadeFilter {
    pub decade_start: i32,  // e.g., 2020
}

/// Combined filter state. All active filters must pass (AND between filter types).
#[derive(Debug, Clone, Default)]
pub struct FilterState {
    pub genres: Option<GenreFilter>,
    pub decade: Option<DecadeFilter>,
}

impl SortOrder {
    /// Human-readable label for UI display.
    pub fn label(&self) -> &'static str { ... }

    /// All available sort orders.
    pub fn all() -> &'static [SortOrder] { ... }
}

impl FilterState {
    pub fn is_active(&self) -> bool { ... }
    pub fn clear(&mut self) { ... }
}
```

**Core functions**:

```rust
/// Does a media item match the search query? Case-insensitive title substring match.
pub fn search_matches(item: &MediaItem, query: &str) -> bool { ... }

/// Does a media item pass all active filters?
pub fn filter_matches(item: &MediaItem, filter: &FilterState) -> bool { ... }

/// Compare two media items for a given sort order.
/// Items with None values sort to the end.
pub fn sort_compare(a: &MediaItem, b: &MediaItem, order: SortOrder) -> Ordering { ... }

/// Apply search, filters, and sort to a list of items. Returns indices into the original Vec.
/// This is the main entry point called by LibraryView.
pub fn apply_filters_and_sort(
    items: &[MediaItem],
    query: &str,
    filter: &FilterState,
    sort: SortOrder,
) -> Vec<usize> { ... }

/// Extract unique genres from a list of items, sorted alphabetically.
pub fn extract_genres(items: &[MediaItem]) -> Vec<String> { ... }

/// Extract available decades from items that have a year, sorted newest-first.
pub fn extract_decades(items: &[MediaItem]) -> Vec<i32> { ... }
```

**Tests** (in `#[cfg(test)] mod tests`):

Search:
- `search_empty_query_matches_all`
- `search_exact_title_matches`
- `search_partial_title_matches`
- `search_case_insensitive`
- `search_leading_trailing_whitespace_trimmed`
- `search_no_match_returns_false`
- `search_special_characters_literal_match`

Genre filter:
- `genre_filter_single_genre_matches`
- `genre_filter_single_genre_no_match`
- `genre_filter_multi_genre_or_semantics` (item has either genre)
- `genre_filter_item_with_multiple_genres`
- `genre_filter_empty_selection_matches_all`
- `genre_filter_item_with_no_genres_never_matches`

Decade filter:
- `decade_filter_item_in_decade_matches`
- `decade_filter_item_outside_decade_no_match`
- `decade_filter_boundary_start_year_matches`
- `decade_filter_boundary_end_year_matches`
- `decade_filter_item_with_no_year_no_match`

Combined filters:
- `combined_genre_and_decade_both_must_match`
- `combined_genre_and_decade_genre_fails`
- `combined_genre_and_decade_decade_fails`
- `no_filters_active_matches_all`

Sort:
- `sort_title_asc_alphabetical`
- `sort_title_desc_reverse`
- `sort_year_newest_first`
- `sort_year_oldest_first`
- `sort_year_none_sorts_last`
- `sort_date_added_newest_first`
- `sort_rating_highest_first`
- `sort_rating_none_sorts_last`
- `sort_runtime_longest_first`
- `sort_runtime_none_sorts_last`
- `sort_stable_tiebreak_by_title`

Integration (apply_filters_and_sort):
- `apply_search_and_genre_filter_combined`
- `apply_all_three_search_genre_decade`
- `apply_returns_correct_indices`
- `apply_empty_items_returns_empty`
- `apply_all_filtered_out_returns_empty`

Extract helpers:
- `extract_genres_unique_sorted`
- `extract_genres_empty_items`
- `extract_genres_skips_items_with_no_genres`
- `extract_decades_unique_sorted_newest_first`
- `extract_decades_items_with_no_year_excluded`

**Success criteria**: Module compiles with `cargo check`, all tests pass with `cargo test services::library_filter`, zero clippy warnings.

**Estimated tests**: ~40

#### Phase 2: LibraryView State Retention + Grid Rebuild

**Goal**: `LibraryView` retains the full item list and rebuilds the grid when search/filter/sort state changes.

**Changes to `LibraryView`**:

```rust
pub struct LibraryView {
    grid: TypedGridView<MediaCardData, gtk4::SingleSelection>,
    library_type: LibraryType,
    source: Option<Arc<dyn MediaSource>>,
    artwork_cache: Option<Arc<ArtworkCache>>,
    // --- Existing UI widgets ---
    stack: gtk4::Stack,
    loading_page: adw::StatusPage,
    empty_page: adw::StatusPage,
    error_page: adw::StatusPage,
    no_results_page: adw::StatusPage,  // NEW: "No results match your filters"
    grid_page: gtk4::ScrolledWindow,
    // --- NEW: state retention ---
    all_items: Vec<MediaItem>,          // Full dataset from last load
    search_query: String,               // Current search text
    filter_state: FilterState,          // Active filters
    sort_order: SortOrder,              // Current sort
}
```

**New message variants**:

```rust
pub enum LibraryViewMsg {
    // ... existing variants ...
    SearchChanged(String),
    GenreFilterChanged(Vec<String>),
    DecadeFilterChanged(Option<i32>),
    SortChanged(SortOrder),
    ClearFilters,
    FocusSearch,  // Triggered by Ctrl+F or /
}
```

**Grid rebuild logic** (`rebuild_grid` method):

1. Call `apply_filters_and_sort(&self.all_items, &self.search_query, &self.filter_state, self.sort_order)` to get filtered indices
2. Clear grid, populate with items at those indices
3. Show `no_results_page` if result is empty (with "Clear filters" button), `grid_page` otherwise
4. Re-trigger artwork loading for visible items (cached artwork will be fast)

**`LibraryLoaded` changes**: Store items in `all_items`, then call `rebuild_grid()`.

**`LoadLibrary` changes**: Only re-fetch from Plex if switching library type. If same library type (e.g., navigating back), skip fetch and just show existing data.

**Success criteria**: Grid updates instantly when filter/sort state changes. Navigating to detail and back preserves filter state. Switching Movies/TV Shows resets genre+decade filters but preserves sort order.

#### Phase 3: Search UI Component

**Goal**: `gtk4::SearchBar` with `gtk4::SearchEntry` integrated into the library toolbar.

**Widget placement**: Inside the `library_toolbar` (`adw::ToolbarView`) at the top, below the `HeaderBar`. Use `gtk4::SearchBar` which provides built-in toggle behavior and `gtk4::SearchEntry` which provides built-in debouncing via the `search-changed` signal.

**Keyboard shortcut wiring**:

In `app.rs`, when not in player view:
- `Ctrl+F` or `/` → `LibraryViewMsg::FocusSearch`
- `Escape` → close search bar (handled by `gtk4::SearchBar` natively)

**Fix `is_text_input_focused` bug** in `app.rs:197-215`:

```rust
key_controller.connect_key_pressed(move |_controller, key, _code, mods| {
    let in_player = stack_key.visible_child_name().as_deref() == Some("player");

    // Check if focus is on a text input widget
    let is_text_focused = root_ref.focus_widget()
        .map(|w| w.is::<gtk4::SearchEntry>() || w.is::<gtk4::Entry>() || w.is::<gtk4::TextView>())
        .unwrap_or(false);

    if let Some(action) = shortcuts::map_key_to_action(key, mods, is_text_focused) {
        // ... existing dispatch logic ...
    } else if !in_player && !is_text_focused {
        // Library-level shortcuts
        let ctrl = mods.contains(gdk::ModifierType::CONTROL_MASK);
        match key {
            gdk::Key::f if ctrl => {
                let _ = sender_key.send(AppMsg::FocusSearch);
                glib::Propagation::Stop
            }
            gdk::Key::slash if mods.is_empty() => {
                let _ = sender_key.send(AppMsg::FocusSearch);
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    } else {
        glib::Propagation::Proceed
    }
});
```

**Debounce**: `gtk4::SearchEntry` has a built-in 150ms delay on the `search-changed` signal. No custom debounce needed.

**Success criteria**: Ctrl+F and `/` focus the search entry. Typing filters the grid in real-time. Escape closes search. Player keyboard shortcuts (Space, M, arrows) do not fire when search entry is focused.

#### Phase 4: Filter Bar UI

**Goal**: Genre multi-select and decade selector as a horizontal bar below the search area.

**Genre filter** -- `gtk4::FlowBox` with `gtk4::ToggleButton` chips:
- Extract genres from `all_items` via `extract_genres()`
- Display as toggle buttons: `[Action] [Comedy] [Drama] [Sci-Fi] ...`
- Multiple can be selected (OR semantics)
- Toggling a genre sends `LibraryViewMsg::GenreFilterChanged(selected_genres)`
- Button state reflects current selection (toggled = active)

**Decade filter** -- `gtk4::DropDown` or `AdwComboRow`:
- Extract decades from `all_items` via `extract_decades()`
- Options: "All Years", "2020s", "2010s", "2000s", ...
- Single selection
- Changing sends `LibraryViewMsg::DecadeFilterChanged(Some(2020))` or `DecadeFilterChanged(None)` for "All"

**Clear filters button**: Visible only when `filter_state.is_active()`. Sends `LibraryViewMsg::ClearFilters`.

**Active filter count badge**: Show count on filter button/area: "(2 active)" or similar.

**Layout**:

```
┌──────────────────────────────────────────────────────┐
│ ← Library                                    ⚙ [...]│  HeaderBar
├──────────────────────────────────────────────────────┤
│ 🔍 [Search movies...                              ] │  SearchBar
├──────────────────────────────────────────────────────┤
│ [Action] [Comedy] [Drama] [Horror] [Sci-Fi] ...     │  Genre chips
│ Decade: [All Years ▾]    Sort: [Title A-Z ▾]  [✕]   │  Decade + Sort + Clear
├──────────────────────────────────────────────────────┤
│ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐           │
│ │     │ │     │ │     │ │     │ │     │           │  Poster grid
│ │ 🎬  │ │ 🎬  │ │ 🎬  │ │ 🎬  │ │ 🎬  │           │
│ │     │ │     │ │     │ │     │ │     │           │
│ └─────┘ └─────┘ └─────┘ └─────┘ └─────┘           │
```

**Success criteria**: Genre toggles filter instantly. Decade dropdown filters by decade. "Clear all" resets everything. Genre list updates when switching Movies/TV Shows (different genres available).

#### Phase 5: Sort Selector + Empty State + Polish

**Goal**: Sort dropdown, proper empty states, edge case handling.

**Sort selector** -- `gtk4::DropDown` in the filter bar row:
- Options: "Title A-Z", "Title Z-A", "Year (Newest)", "Year (Oldest)", "Date Added", "Rating", "Runtime"
- Default: "Title A-Z"
- Changing sends `LibraryViewMsg::SortChanged(SortOrder::YearNewest)`

**Empty results state** -- New `adw::StatusPage`:
- Title: "No Results"
- Description: "No items match your search and filters"
- Button: "Clear Filters" → `ClearFilters` message
- Distinct from existing "No Media" page (which means no source connected)

**Edge cases to handle**:
- Items with `rating: None` sort to the end
- Items with `year: None` don't appear in any decade filter but appear in "All Years"
- Genre chips rebuild when `all_items` changes (library reload or type switch)
- Sort order persists when switching Movies ↔ TV Shows
- Genre and decade filters reset on library type switch (genres differ)
- Search text clears on library type switch

**Success criteria**: Sort works correctly for all fields. Empty state shows appropriate message. Null values handled gracefully. State management correct across navigation.

## Alternative Approaches Considered

### 1. SQL-Based Filtering (Rejected)

**Approach**: Add `search`, `filter_by_genre`, `sort_by` methods to `MediaRepo` with parameterized SQL queries.

**Why rejected**: The current architecture fetches all items from Plex live and doesn't use `MediaRepo` for browsing. Adding SQL-based filtering would require first caching items to SQLite, then querying, creating a double data path. Client-side filtering is simpler and fast enough for typical library sizes.

**When to reconsider**: If libraries exceed ~5000 items and client-side filtering becomes slow, or when implementing SQLite FTS5 for full-text search across cast/crew/overview.

### 2. Search Dropdown/Popover (Rejected)

**Approach**: Show search results in a floating dropdown list above the grid, with type-ahead suggestions.

**Why rejected**: A dropdown is a different UX pattern (quick-jump navigation) while a filtered grid preserves the visual browsing experience. The grid approach is simpler, consistent with filter behavior, and matches Infuse/Plex web patterns. A dropdown can be added later as an enhancement.

### 3. Plex API-Side Search (Rejected)

**Approach**: Use Plex's `/library/sections/{key}/search` endpoint.

**Why rejected**: Adds network latency to every keystroke (even with debounce). Plex search behavior is opaque and not customizable. Client-side search gives us full control over scoring and can be tested without network.

## System-Wide Impact

### Interaction Graph

1. User presses Ctrl+F → `App` catches key event → sends `AppMsg::FocusSearch` → `App::update()` forwards to `LibraryViewMsg::FocusSearch` → `LibraryView` focuses the `SearchEntry`
2. User types in search → `SearchEntry::search-changed` signal fires (debounced 150ms) → `LibraryViewMsg::SearchChanged(text)` → `LibraryView` updates `search_query` → calls `rebuild_grid()` → `apply_filters_and_sort()` returns indices → grid cleared and repopulated → artwork re-loaded from cache
3. User clicks genre chip → `ToggleButton::toggled` signal → `LibraryViewMsg::GenreFilterChanged(selected)` → `LibraryView` updates `filter_state.genres` → calls `rebuild_grid()`

### Error Propagation

No new error types needed. `apply_filters_and_sort` is infallible (pure function on valid data). Artwork loading errors already handled (silent failure, no poster shown).

### State Lifecycle Risks

- **Grid rebuild on every filter change**: Clears and repopulates `TypedGridView`. This destroys scroll position. Mitigation: save scroll position before rebuild, restore after. Consider whether TypedGridView selection model can be updated without full clear.
- **Artwork re-loading**: After grid clear, artwork for all visible items is re-requested. The `ArtworkCache` has disk cache, so this is fast (texture creation from local file). But it does re-spawn async tasks. Mitigation: cache `gtk4::gdk::Texture` objects in memory (e.g., `HashMap<String, Texture>`) to avoid repeated file-to-texture conversion.
- **Race condition**: If a library load (async) completes while the user has active filters, the `LibraryLoaded` handler should apply current filters to the new data, not reset filters.

### API Surface Parity

- `LibraryViewMsg` gains 5 new variants (search, genre, decade, sort, clear, focus)
- `AppMsg` gains 1 new variant (`FocusSearch`)
- No other components affected

### Integration Test Scenarios

1. **Full flow**: Load library → search "star" → only Star Wars/Star Trek shown → clear search → all items return → toggle "Sci-Fi" genre → subset shown → change sort to "Year (Newest)" → correct order → navigate to detail → press back → filters still active
2. **Library type switch**: Apply "Action" genre filter on Movies → switch to TV Shows → genre filter should reset (TV Shows may not have "Action") → switch back to Movies → genre filter should be clear (not restored from before)
3. **Empty results**: Search for "xyznonexistent" → no results page shown with "Clear Filters" button → click clear → all items return
4. **Keyboard shortcut conflict**: Focus search entry → type "m" → should type the letter, NOT toggle mute → press Space → should type space, NOT toggle pause → press Escape → should close search bar
5. **Rapid typing**: Type "interstellar" fast (13 keystrokes) → `SearchEntry` debounces → grid should update once or twice, not 13 times

## Acceptance Criteria

### Functional Requirements

- [x] **Search**: Typing in search entry instantly filters the poster grid to items whose title contains the query (case-insensitive)
- [x] **Search shortcut**: Ctrl+F and `/` focus the search entry from anywhere in the library view
- [x] **Search dismiss**: Escape closes the search bar and clears the search, restoring full library
- [x] **Genre filter**: Clicking genre chips filters the grid to items matching any selected genre (OR)
- [x] **Genre list**: Genre chips are extracted from the current library's items (different for Movies vs TV Shows)
- [x] **Decade filter**: Selecting a decade shows only items with `year` in that range
- [x] **Decade list**: Available decades are extracted from the current library's items
- [x] **Sort**: Changing sort order re-orders the grid immediately
- [x] **Sort options**: Title A-Z, Title Z-A, Year (Newest), Year (Oldest), Date Added, Rating, Runtime
- [x] **Combined**: Search + genre + decade + sort all work together (search AND genre AND decade, then sort)
- [x] **Clear filters**: A visible "clear" button resets all filters and search
- [x] **Empty results**: When filters yield zero results, show a helpful empty state with "Clear Filters" action
- [x] **Navigate back**: Filter/sort/search state preserved when navigating to a detail page and pressing back
- [x] **Type switch**: Switching Movies ↔ TV Shows resets genre and decade filters but preserves sort order
- [x] **Keyboard safety**: Player shortcuts (Space, M, arrows, brackets) do not fire when search entry is focused

### Non-Functional Requirements

- [x] **Performance**: Filtering 2000 items completes in < 50ms (client-side Vec iteration)
- [x] **Test coverage**: `library_filter.rs` has 52 unit tests covering all search, filter, sort, and combination scenarios
- [x] **No GTK in service layer**: `library_filter.rs` has zero GTK/Relm4 dependencies
- [x] **Clippy clean**: Zero warnings with `cargo clippy`
- [x] **Existing tests pass**: All 365 tests pass (302 original + 52 new + 11 from other session)

### Quality Gates

- [x] All tests pass: `nix develop -c cargo test`
- [x] No clippy warnings: `nix develop -c cargo clippy`
- [x] Formatted: `nix develop -c cargo fmt --check`
- [x] Compiles with zero warnings

## Success Metrics

- Search finds any movie/show by partial title within 1 keystroke's debounce delay
- Filters visibly narrow the grid and can be combined
- Sort changes grid order with no visible delay
- Navigating to detail and back does not lose filter state
- No keyboard shortcut conflicts when search is focused

## Dependencies & Prerequisites

| Dependency | Status | Impact |
|-----------|--------|--------|
| M2 complete (library grid, Plex source, MediaItem model) | Done | Foundation for M3 |
| `TypedGridView` supports clear and rebuild | Verified in current code | Grid rebuild pattern |
| `gtk4::SearchBar` + `gtk4::SearchEntry` | Available in GTK4 0.10 | Built-in debounce |
| `is_text_input_focused` param in `map_key_to_action` | Exists but unused | Must wire in Phase 3 |

**No new crate dependencies required** for the core scope. `proptest` could be added for property-based search tests but is optional.

## Risk Analysis & Mitigation

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| TypedGridView clear/rebuild loses scroll position | Medium | Low | Cache scroll position via `ScrolledWindow::vadjustment`, restore after rebuild |
| Artwork re-loading on every filter change is slow | Low | Medium | Cache `gdk::Texture` objects in a `HashMap<url, Texture>` to avoid re-creation from disk |
| Genre names not normalized across Plex libraries | Medium | Low | Accept as-is from Plex; normalization is a polish concern |
| Large libraries (5000+) slow on client-side filter | Low | Medium | Profile and optimize; can move to SQL-based filtering if needed |
| GTK SearchBar key event propagation conflicts | Medium | Medium | Test thoroughly with keyboard; SearchBar handles Escape natively |

## Future Considerations

- **FTS5 search**: When cast/crew data is added (enhanced detail pages), implement SQLite FTS5 for full-text search across title, overview, cast, crew, collection names
- **Search dropdown**: Add a type-ahead dropdown that appears above the grid with top-5 results for quick navigation
- **Fuzzy matching**: Replace substring search with fuzzy scoring (Levenshtein distance, trigram similarity) for typo tolerance
- **Filter persistence**: Save last-used sort order to `window_state.toml` so it survives app restarts
- **Unwatched toggle**: Add in M4 when watch state tracking exists
- **Search across sources**: When multiple sources (Plex + Local) exist, search across all and group results by source

## Sources & References

### Internal References

- Architecture patterns: `src/player/playback_tracker.rs` (pure state machine pattern to replicate)
- Library view: `src/components/library/mod.rs` (current implementation to extend)
- Media model: `src/models/media.rs` (fields available for search/filter/sort)
- Keyboard shortcuts: `src/components/player/shortcuts.rs:24-53` (`is_text_input_focused` param)
- App keyboard wiring: `src/app.rs:194-215` (hardcoded `false` that must be fixed)

### Product Specification

- Search spec: `product.md` F2.3 (search across titles, cast, crew -- title-only for M3)
- Filter spec: `product.md` F2.2 (genre, year/decade, rating threshold, watched/unwatched, combinable)
- Sort spec: `product.md` F2.2 (Title, Year, Date Added, Rating, Runtime)
- Empty states: `product.md` F4.5 (`AdwStatusPage` for empty states)

### Roadmap

- M3 tasks: `roadmap.md:153-193`
- M3 test focus: `CLAUDE.md` milestone table (search scoring, filter logic, sort comparators)
