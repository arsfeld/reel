---
title: "feat: Adwaita library views overhaul with filters and sort"
status: completed
date: 2026-05-27
type: feat
origin: docs/brainstorms/2026-05-27-library-views-overhaul-requirements.md
---

# feat: Adwaita library views overhaul with filters and sort

## Summary

Overhaul `LibraryView` to remove the genre chip carousel and the in-library Recently Added shelf, add an Adwaita-style filter popover plus an active-filter pill row above the grid, extend the filter set (watch status, year range, genre, rating, runtime, content rating, resolution including HDR), and persist filter + sort per library across restarts. Plex HDR data is added to the data model and extraction pipeline as part of this work.

---

## Problem Frame

Today's `LibraryView` (src/components/library/mod.rs, 1,697 lines) carries a genre chip carousel, decade dropdown, sort dropdown, and an in-library Recently Added shelf. None of these scale to a library of several thousand items, and the chrome doesn't match GNOME/Adwaita conventions. The chip row eats vertical space without supporting "an unwatched sci-fi movie under 100 minutes I haven't seen, rated ≥7" — the question users actually ask of a large library. The brainstorm (see origin: docs/brainstorms/2026-05-27-library-views-overhaul-requirements.md) settled the product shape: header Filter button → popover with seven filter groups, sub-header pill row showing what's currently filtered, per-library persisted state, no `HomeView` changes.

Two blockers the brainstorm flagged are resolved here from the codebase:
- **Library identifier:** `LibrarySection { key, title, library_type, count }` (src/models/library.rs) carries a stable section key. A composite `"{source_type}:{source_id}:{section_key}"` (e.g., `"plex:http://localhost:32400:1"`) is the persistence key.
- **HDR data:** `PlexMedia` (src/services/plex/models.rs) does **not** currently capture HDR / dynamic-range data. Per the user's direction during planning, Plex extraction is added to this plan rather than deferred.

---

## Key Technical Decisions

- **KTD1. Pure-function filter layer extends first; UI follows.** All new filter types and pill-label rendering land in src/services/library_filter.rs with full unit-test coverage before any GTK changes (see origin: docs/brainstorms/2026-05-27-library-views-overhaul-requirements.md). This matches the project's established testability rule that no business logic lives inside Relm4 `update()` methods (see CLAUDE.md "Pattern 1: Extract Pure Functions from Components").
- **KTD2. New filter UI lives in dedicated files, not in `mod.rs`.** `src/components/library/mod.rs` is already 1,697 lines (cap 2,000 per `tests/file_size_limits.rs`). The filter popover and active-pill row each get their own file: `src/components/library/filter_popover.rs` and `src/components/library/active_filters.rs`. `mod.rs` only adds wiring; the removals offset additions.
- **KTD3. Composite library identifier string.** Persistence key shape: `"{source_type}:{source_id}:{section_key}"`. Mirrors the existing `MediaItem::make_id` composite pattern. Drawback shared with `MediaItem.id`: not stable if the Plex server URL changes — acceptable because no production users exist yet.
- **KTD4. `LibrarySettings` is replaced, not extended.** Today's `LibrarySettings { default_sort, sort_ascending }` is not read anywhere in the codebase. Replace it with `LibrarySettings { per_library: HashMap<String, LibraryUiState> }` rather than carrying dead fields alongside live ones. No migration logic needed (no users).
- **KTD5. HDR data field shape: `Option<HdrFormat>` enum on `MediaItem`.** Plex exposes `videoDynamicRange` on the `Media` object (typical values: `"SDR"`, `"HDR"`, `"Dolby Vision"`). Normalize on conversion into a small enum `HdrFormat::{Hdr, DolbyVision}` (`SDR` and absent both map to `None`). Resolution and HDR are independent filter dimensions, both multi-select.
- **KTD6. Filter combination semantics carry over from origin.** AND across filter types, OR within multi-select sets (origin R13). No new semantics; new filter types follow the existing `filter_matches` pattern.
- **KTD7. Continue Watching shelf is filter-independent.** Acceptance Example AE7 in the origin pins this: the CW shelf is not affected by active filters. Implementer must not pipe `FilterState` into the CW rebuild path.

---

## Requirements Traceability

All 25 requirements from the origin (R1–R25) are covered. Mapping:

| Origin requirements | Covered by | Notes |
|---|---|---|
| R1, R2, R3, R5 (removals + CW shelf retention) | U6 | Existing CW shelf retained as-is; recently_added_section + genre carousel removed |
| R4 (HomeView untouched) | — | No src/components/home/ changes in any unit |
| R6 (watch status filter) | U2, U4 | Filter logic in U2; popover row in U4 |
| R7 (year range) | U2, U4 | Replaces decade dropdown |
| R8 (genre multi-select) | U2, U4 | Logic exists today; UI relocates to popover |
| R9 (rating min) | U2, U4 | |
| R10 (runtime range) | U2, U4 | |
| R11 (content rating multi-select) | U2, U4 | Multi-select chips inside popover |
| R12 (resolution + HDR) | U1, U2, U4 | HDR extraction in U1; filter in U2; UI in U4 |
| R13 (AND across types, OR within) | U2 | Pattern already established in `filter_matches` |
| R14 (sort options preserved) | U6 | Sort dropdown remains in header; no change to `SortOrder` |
| R15, R16, R17 (header layout + pill row + labels) | U5, U6 | Pill row component in U5; integration in U6 |
| R18 (popover groups order) | U4 | |
| R19 (search / density / view mode preserved) | U6 | No behavior changes; just survive the refactor |
| R20 (empty-state Clear filters CTA) | U7 | |
| R21 (filtered count in hero subtitle) | U7 | |
| R22, R23, R24 (per-library persisted state) | U3, U7 | Settings model in U3; load/save wiring in U7 |
| R25 (stale persisted values silently dropped) | U3, U7 | Reconciliation on load |

Acceptance Examples AE1–AE7 are covered by the test scenarios under U2, U4, U5, and U7.

---

## Implementation Units

### U1. Plex HDR data extraction

**Goal:** Capture HDR / dynamic-range information from Plex through to `MediaItem` so the resolution filter can offer an HDR option backed by real data.

**Requirements:** Supports R12; resolves the brainstorm's "verify before claiming" check on HDR data availability.

**Dependencies:** None.

**Files:**
- src/services/plex/models.rs — add `video_dynamic_range: Option<String>` to `PlexMedia`.
- src/services/plex/convert.rs — read `video_dynamic_range` from the primary media version; normalize into `HdrFormat`.
- src/models/media.rs — add `HdrFormat` enum (`Hdr`, `DolbyVision`) and `hdr: Option<HdrFormat>` field to `MediaItem`. Implement `HdrFormat::from_plex(&str) -> Option<HdrFormat>` mapping (`"HDR"`/`"HDR10"`/`"HDR10+"`/`"HLG"` → `Hdr`; `"Dolby Vision"`/`"DV"` → `DolbyVision`; everything else → `None`).
- src/db/schema.rs — add `hdr TEXT` column to both `movies` and `episodes` tables (TEXT nullable, stores `"hdr"` or `"dolby_vision"`).
- src/db/media_repo.rs — wire the new column on insert/update/select.
- src/services/plex/convert.rs (tests) — add fixtures covering each `videoDynamicRange` value Plex emits.

**Approach:**
1. Extend `PlexMedia` with `#[serde(rename = "videoDynamicRange")] pub video_dynamic_range: Option<String>`. Confirm the JSON field name against an existing Plex response or fake_server fixture during implementation (Plex docs and community-discovered JSON consistently use camelCase `videoDynamicRange`).
2. In `convert::movie_from_plex` and `convert::episode_from_plex`, pluck the field from the first `Media` entry (same pattern as `video_resolution`) and pass through `HdrFormat::from_plex`.
3. Persist as a short string (`"hdr"` / `"dolby_vision"`) — round-trip via `HdrFormat::as_str` / `HdrFormat::from_db_str`.

**Patterns to follow:**
- The existing `video_resolution` plumbing in `PlexMedia` → `convert.rs` → `MediaItem` → DB schema is the exact analog. Mirror it.

**Test scenarios:**
- `from_plex_maps_hdr_to_hdr_variant` — `"HDR"`, `"HDR10"`, `"HDR10+"`, `"HLG"` each map to `HdrFormat::Hdr`.
- `from_plex_maps_dolby_vision_variants` — `"Dolby Vision"`, `"DV"` each map to `HdrFormat::DolbyVision`.
- `from_plex_returns_none_for_sdr` — `"SDR"`, `""`, `None`, and any unknown string each map to `None`.
- `from_plex_is_case_insensitive` — `"hdr"`, `"dolby vision"` map correctly.
- `convert_movie_extracts_hdr` — feed a synthesized Plex metadata response with `videoDynamicRange: "HDR"`, assert `item.hdr == Some(HdrFormat::Hdr)`.
- `convert_movie_no_media_gives_none_hdr` — empty `Media` array → `item.hdr == None`.
- `db_roundtrip_hdr_field` — insert a movie with each `HdrFormat` variant, read it back, assert equality.
- `db_roundtrip_no_hdr` — null in DB → `item.hdr == None`.

**Verification:** `nix develop -c cargo test plex::convert` passes with new HDR tests. `nix develop -c cargo test db::media_repo` passes with HDR roundtrip. `nix develop -c cargo build` last line shows 0 errors.

---

### U2. Filter model extensions (pure logic)

**Goal:** Extend `FilterState` and `filter_matches` to cover all seven filter types, plus add a pill-label rendering helper. All pure-function work; no GTK involved.

**Requirements:** R6, R7, R9, R10, R11, R12, R13, R17. Covers AE1, AE2, AE3 at the logic level.

**Dependencies:** U1 (needs `HdrFormat` field on `MediaItem`).

**Files:**
- src/services/library_filter.rs — extend `FilterState`; add `WatchStatusFilter`, `YearRangeFilter`, `RatingFilter`, `RuntimeRangeFilter`, `ContentRatingFilter`, `ResolutionFilter`; extend `filter_matches`; add `extract_content_ratings`, `extract_resolution_buckets`, `derived_watch_status`; add `pill_labels(&FilterState) -> Vec<FilterPill>` producing `(FilterTag, String)` entries one per active value (where `FilterTag` is an enum identifying which filter the pill represents, used by the UI to wire each pill's ✘ to a specific reset action).

**Approach:**

Extend `FilterState`:

```text
FilterState {
    genres:           Option<GenreFilter>,           // existing
    year_range:       Option<YearRangeFilter>,       // replaces DecadeFilter
    watch_status:     Option<WatchStatusFilter>,
    rating_min:       Option<RatingFilter>,
    runtime_range:    Option<RuntimeRangeFilter>,
    content_ratings:  Option<ContentRatingFilter>,
    resolutions:      Option<ResolutionFilter>,
}
```

`DecadeFilter` is removed entirely — no consumers outside `LibraryView` widget code that gets rewritten in U6.

`YearRangeFilter { from: Option<i32>, to: Option<i32> }`. Items with no year are excluded when any bound is set (origin R7).

`WatchStatusFilter { status: WatchStatus }` where `WatchStatus = { Unwatched, InProgress, Watched }`. Derivation in `derived_watch_status(item_id, &watch_data) -> WatchStatus` — pure function so it's testable without GTK.

`RatingFilter { min: f64 }`. Items with no rating excluded when set.

`RuntimeRangeFilter { from: Option<i32>, to: Option<i32> }`. Items with no runtime excluded when any bound is set.

`ContentRatingFilter { selected: Vec<String> }` — OR semantics over `MediaItem.content_rating`.

`ResolutionFilter { buckets: Vec<ResolutionBucket>, hdr: Option<HdrSelection> }`. `ResolutionBucket = { SD, R720p, R1080p, R4K }` derived from `MediaItem::format_resolution`. `HdrSelection = { AnyHdr, SdrOnly }` — `AnyHdr` matches any `Some(HdrFormat::*)`; `SdrOnly` matches `None`. Bucket and HDR selections combine with AND inside the filter (e.g., "4K AND HDR" is one filter chip cluster, two pills).

`filter_matches` extends to AND-combine every set filter (origin R13). Within `ContentRatingFilter` and `ResolutionFilter`, multi-value selections OR (origin R13). `apply_filters_and_sort` signature stays the same; callers receive new behaviour for free.

`pill_labels(state) -> Vec<(FilterTag, String)>` produces one pill per active value (origin R17). Examples: `(WatchStatus, "Unwatched")`, `(YearFrom, "Year ≥1990")`, `(YearRange, "Year 1990–2009")`, `(Genre("Sci-Fi"), "Genre: Sci-Fi")`, `(Rating, "Rating ≥7.0")`, `(Runtime, "Runtime <100 min")`, `(Runtime, "Runtime 90–120 min")`, `(ContentRating("PG-13"), "PG-13")`, `(Resolution(R4K), "4K")`, `(Hdr, "HDR")`. Unset range bounds never produce a pill (R17).

**Patterns to follow:**
- Existing `filter_matches` (src/services/library_filter.rs:181) for AND-across / OR-within shape.
- Existing `extract_genres` / `extract_decades` for the "extract distinct values from items" helper pattern.

**Execution note:** Add unit tests first per the project's TDD rule for pure functions (see CLAUDE.md "Pattern 2: PlaybackTracker"). Each new filter type gets its tests landed before the implementation in the same commit.

**Test scenarios:**

Watch status (Covers R6, AE1, AE7):
- `watch_status_unwatched_matches_when_no_progress_and_not_watched`
- `watch_status_in_progress_matches_when_progress_gt_zero_and_not_watched`
- `watch_status_watched_matches_when_watched_true`
- `watch_status_in_progress_excludes_watched_items`
- `watch_status_unwatched_excludes_in_progress_items`

Year range (Covers R7, AE2):
- `year_range_both_bounds_matches_inside`
- `year_range_lower_only_matches_at_or_above`
- `year_range_upper_only_matches_at_or_below`
- `year_range_item_with_no_year_excluded_when_any_bound_set`
- `year_range_neither_bound_matches_all`
- `year_range_boundary_inclusive_both_ends`

Rating:
- `rating_min_matches_at_or_above_threshold`
- `rating_min_excludes_below_threshold`
- `rating_min_excludes_item_with_no_rating`

Runtime range:
- `runtime_range_inside_matches`
- `runtime_range_only_max_matches_at_or_below`
- `runtime_range_only_min_matches_at_or_above`
- `runtime_range_no_runtime_excluded_when_any_bound_set`

Content rating:
- `content_rating_single_selected_matches`
- `content_rating_multi_selected_or_semantics`
- `content_rating_empty_selection_matches_all`
- `content_rating_item_with_no_rating_excluded_when_set`

Resolution + HDR (Covers R12):
- `resolution_bucket_matches_within_selection`
- `resolution_4k_matches_only_4k_items`
- `resolution_multi_bucket_or_semantics`
- `resolution_buckets_derived_from_format_resolution_helper`
- `hdr_any_matches_hdr_and_dolby_vision`
- `hdr_sdr_only_matches_items_with_no_hdr_field`
- `resolution_and_hdr_combine_with_and`

Combined behaviour (Covers R13, AE1):
- `all_seven_filters_active_and_combined`
- `genre_or_within_and_watch_status_and_year_range`

Pill labels (Covers R17):
- `pill_for_unset_range_bound_not_emitted`
- `pill_for_lower_bound_only_renders_as_geq`
- `pill_for_upper_bound_only_renders_as_leq`
- `pill_for_both_bounds_renders_as_range`
- `pill_for_each_multi_value_individually`
- `pill_for_hdr_only_when_any_hdr_selected`

Stale-value reconciliation (Covers R25):
- `reconcile_filter_drops_genre_not_in_extracted_genres`
- `reconcile_filter_drops_content_rating_not_present`
- `reconcile_filter_drops_resolution_bucket_not_present`

**Verification:** `nix develop -c cargo test library_filter` passes — expect ~40 new tests on top of the ~30 today. `nix develop -c cargo clippy` clean for the file. `nix develop -c cargo build` last line shows 0 errors.

---

### U3. Library identifier and per-library settings persistence model

**Goal:** Replace `LibrarySettings` with a per-library state map; add a helper for constructing the composite library id; wire the type into `Settings` round-trip tests.

**Requirements:** R22, R23, R24. Foundation for AE5, AE6.

**Dependencies:** U2 (`LibraryUiState` serializes the extended `FilterState` and `SortOrder`).

**Files:**
- src/settings.rs — replace `LibrarySettings` body; add `LibraryUiState`; add helpers `LibrarySettings::get(library_id)` / `LibrarySettings::set(library_id, state)`.
- src/models/library.rs — add `LibrarySection::library_id(&self, source_type: SourceType, source_id: &str) -> String` returning `"{source_type}:{source_id}:{section_key}"`.
- src/services/library_filter.rs — add `Serialize` / `Deserialize` derive (via `serde` feature flag) on `FilterState`, `SortOrder`, and all new filter sub-structs. Field-level renames stay stable so future filter additions don't break old TOML.

**Approach:**

`LibrarySettings`:

```text
LibrarySettings {
    #[serde(default)]
    per_library: HashMap<String, LibraryUiState>,
}

LibraryUiState {
    #[serde(default)]
    filters: FilterState,
    #[serde(default)]
    sort: SortOrder,
}
```

`LibrarySettings::get(library_id) -> LibraryUiState` returns a clone or `Default` when absent.
`LibrarySettings::set(library_id, state)` replaces; serialization happens via the existing `Settings::save()` path called by `LibraryView`'s update loop in U7.

Old `default_sort` / `sort_ascending` fields are removed. Per KTD4, no migration is needed.

**Patterns to follow:**
- The existing `Settings` `#[serde(default)]` discipline (src/settings.rs:16) so forward-compatible TOML works.
- The existing `roundtrip_*` test layout in `src/settings.rs` tests module.

**Test scenarios:**
- `library_section_library_id_for_plex` — `LibrarySection { key: "1", .. }.library_id(Plex, "http://localhost:32400")` == `"plex:http://localhost:32400:1"`.
- `library_settings_empty_returns_default_state_for_unknown_key`.
- `library_settings_set_and_get_roundtrip_in_memory`.
- `settings_toml_roundtrip_preserves_per_library_entries` — serialize a `Settings` with two library entries (different filter states + sorts), parse back, assert structural equality.
- `settings_toml_with_partial_filter_state_fills_defaults` — TOML missing `watch_status` should deserialize with `watch_status: None`.
- `settings_toml_with_unknown_filter_field_does_not_fail` — forward-compatibility: an unknown field in the TOML is ignored.
- `library_ui_state_default_has_no_filters_and_title_sort`.

**Verification:** `nix develop -c cargo test settings` passes. `nix develop -c cargo build` last line shows 0 errors.

---

### U4. Filter popover widget module

**Goal:** Build a self-contained Adwaita filter popover containing all seven filter groups, emitting structured filter-change messages to the parent.

**Requirements:** R6, R7, R8, R9, R10, R11, R12, R18.

**Dependencies:** U2 (uses the new filter types and bucket enums), U1 (resolution/HDR rows reference the new data).

**Files:**
- src/components/library/filter_popover.rs (new) — `FilterPopover` Relm4 component.
- src/components/library/mod.rs — register the submodule (`mod filter_popover;`).

**Approach:**

`FilterPopover` is a Relm4 component that owns a `gtk::Popover` with vertical `AdwPreferencesGroup` sections in the order specified by origin R18:
1. Watch status — three radio rows in an `AdwPreferencesGroup` (Any / Unwatched / In Progress / Watched).
2. Year — `AdwPreferencesGroup` with two `AdwSpinRow`s (From / To). Each row has a "no limit" sentinel handled in the component (e.g., min value − 1 represents unset, displayed as "—").
3. Genre — `AdwPreferencesGroup` with a search-able multi-select list. Use `AdwExpanderRow` wrapping a `gtk::ListBox` of `AdwActionRow`s with check icons, populated from `extract_genres(items)`.
4. Rating — `AdwPreferencesGroup` with a single `AdwSpinRow` (minimum rating, step 0.5). Unset when at 0.0.
5. Runtime — two `AdwSpinRow`s in one group (min / max minutes).
6. Content rating — `AdwExpanderRow` with multi-select check rows, populated from `extract_content_ratings(items)`.
7. Resolution — fixed multi-select check rows for SD / 720p / 1080p / 4K, plus a separate check row "HDR".
Footer: "Clear all" button (origin R18).

Inputs from parent: `SetItems(Vec<MediaItem>)` so the popover refreshes available genres/content-ratings/buckets when the library loads or changes. `SetState(FilterState)` to restore from persisted state.

Outputs to parent: `FilterChanged(FilterState)` whenever any control changes. Parent owns the canonical `FilterState`; popover is a view onto it.

Pseudo-grammar of the message shape (directional, not implementation specification):

```text
FilterPopoverMsg
  ::= SetItems(Vec<MediaItem>)
   |  SetState(FilterState)
   |  WatchStatusSelected(Option<WatchStatus>)
   |  YearFromChanged(Option<i32>)
   |  YearToChanged(Option<i32>)
   |  GenreToggled(String, bool)
   |  RatingMinChanged(Option<f64>)
   |  RuntimeFromChanged(Option<i32>)
   |  RuntimeToChanged(Option<i32>)
   |  ContentRatingToggled(String, bool)
   |  ResolutionBucketToggled(ResolutionBucket, bool)
   |  HdrToggled(bool)
   |  ClearAll

FilterPopoverOutput
  ::= FilterChanged(FilterState)
```

The popover maintains its own working `FilterState` and emits `FilterChanged` after every input. The parent owns persistence and the pill row; the popover does not.

**Patterns to follow:**
- Existing `gtk::Popover` construction in src/components/library/mod.rs:313.
- AdwPreferencesGroup / AdwActionRow / AdwSpinRow / AdwExpanderRow usage in src/components/settings_dialog.rs.

**Execution note:** This unit has heavy widget construction. Mock-friendly logic is in U2; this unit's tests are limited to message routing.

**Test scenarios:**
- `popover_init_state_is_default_filter_state` — newly constructed popover emits no message; internal state matches `FilterState::default()`.
- `set_items_populates_genre_and_content_rating_lists` — after `SetItems(vec_with_two_genres_and_two_content_ratings)`, calling a helper to inspect available items returns those values.
- `watch_status_selected_emits_filter_changed_with_status` — pure dispatcher test: handle `WatchStatusSelected(Some(Unwatched))`, assert the next `FilterChanged` output carries `watch_status: Some(...)`.
- `clear_all_emits_default_filter_state`.
- `set_state_restores_each_filter_field` — pass in a populated `FilterState`, assert internal state matches.

`Test expectation: none --` for the actual widget layout (no visual tests; GTK layout is excluded from unit testing per CLAUDE.md "Do NOT Unit Test").

**Verification:** `nix develop -c cargo test components::library::filter_popover` passes. `nix develop -c cargo build` last line shows 0 errors. Manual smoke: open popover, toggle each filter type, confirm parent receives `FilterChanged` (verified during U6 integration).

---

### U5. Active filter pill row widget module

**Goal:** Render the active-filter pills above the grid, each with an ✘ that removes one filter, plus a "Clear all" button.

**Requirements:** R15, R16, R17. Covers AE3.

**Dependencies:** U2 (consumes `pill_labels` output).

**Files:**
- src/components/library/active_filters.rs (new) — `ActiveFiltersBar` Relm4 component or helper struct managing a `gtk::Box`.
- src/components/library/mod.rs — register submodule.

**Approach:**

A horizontal `gtk::Box` containing one Adwaita-styled pill button per pill from `pill_labels(state)`, plus a trailing "Clear all" button. Each pill renders as a `gtk::Button` with `["pill", "filter-pill"]` CSS classes and a small ✘ icon child. The row is only visible when at least one pill is present (R16); otherwise the parent hides it.

Inputs from parent: `Update(FilterState)` — rebuild pills from scratch on each state change (cheap, ≤10 pills typical).

Outputs to parent: `RemoveFilter(FilterTag)` carrying the `FilterTag` enum from `pill_labels`, and `ClearAll`.

**Patterns to follow:**
- The existing genre chip construction in src/components/library/mod.rs:1348–1374 — same widget style, different data source. Lift the pill-button helper and reuse.

**Test scenarios:**
- `pill_count_matches_pill_labels_length` — pass a `FilterState` producing 3 pills, assert 3 pill widgets exist plus "Clear all".
- `pill_row_hidden_when_state_has_no_active_filters` — bar parent sees the count and hides itself; assert count returns 0.
- `remove_filter_emits_tag_for_clicked_pill` — synthesize a click on the second pill, assert `RemoveFilter(<expected tag>)` is emitted.
- `clear_all_emits_clear_all`.
- `update_with_new_state_replaces_existing_pills` — start with 3 pills, send `Update(state_with_2_pills)`, assert exactly 2 pill widgets remain.

**Verification:** `nix develop -c cargo test components::library::active_filters` passes. `nix develop -c cargo build` last line shows 0 errors.

---

### U6. LibraryView integration: removals and wiring

**Goal:** Remove the genre chip carousel and the Recently Added shelf from `LibraryView`; wire the new filter popover and active-pill row; route their outputs into `LibraryView`'s existing `FilterState` and `SortOrder`. No persistence yet (lands in U7).

**Requirements:** R1, R2, R3, R5, R14, R15, R16, R19. Continue Watching shelf retained (R5, KTD7).

**Dependencies:** U2, U3, U4, U5. (U3 only because `LoadLibrary` will be extended to carry `LibrarySection`; persistence wiring is U7.)

**Files:**
- src/components/library/mod.rs — primary integration. Net removal of lines from genre carousel and Recently Added paths should offset additions from popover/pill wiring; aim to keep mod.rs under 2,000 (cap) and ideally trending toward 1,500.

**Approach:**

Removals:
- Drop fields: `genre_scroll`, `genre_box`, `current_genres`, `current_decades`, `decade_dropdown`, `recently_added_section`, `recently_added_box`, `filter_dot` (replaced by `filter_count_badge` only).
- Drop methods/branches: `rebuild_recently_added`, `rebuild_genre_chips` (or whatever the helper is named), decade-dropdown population, decade-dropdown signal handlers.
- Drop messages: `GenreFilterChanged(Vec<String>)`, `DecadeFilterChanged(Option<i32>)` — these are now internal to `FilterPopover`. Replace with a single `FilterStateChanged(FilterState)` from the popover output.

Additions:
- New field: `filter_popover: Controller<FilterPopover>`.
- New field: `active_filters_bar: Controller<ActiveFiltersBar>`.
- New field: `library_id: String` (composite per KTD3) — assigned when `LoadLibrary` is called. Extend `LibraryViewMsg::LoadLibrary` to carry a `LibrarySection` (and the source's `(SourceType, source_id)`), not just `LibraryType`. Callers in src/app/* are updated to pass the section.
- New widget placement in the root `gtk::Box`: filter button + sort dropdown live in the header area (existing pattern keeps); the active-filters bar sits between the existing search bar and the grid stack; Continue Watching section stays where it is above the grid (R5).
- Forward `LibraryLoaded` items into both `filter_popover.emit(SetItems(items))` and `active_filters_bar.emit(Update(state))` after any state change.

Message routing additions:
- `FilterStateChanged(FilterState)` — replace existing `FilterState` field, refresh pill row, recompute filtered indices, update filter-count badge, update hero subtitle (U7).
- `RemoveActiveFilter(FilterTag)` — apply the appropriate reset on `FilterState`, then re-route through `FilterStateChanged`.
- `ClearFilters` — already exists; expand to call `FilterState::clear()` (which now clears all seven filter types).

Sort dropdown stays where it is (in the header / popover area, but moved out of the filter popover into its own header-bar control per origin R15). The plan is for `Sort` to live in the header beside the Filter button, not inside the filter popover.

**Patterns to follow:**
- Existing Relm4 child component handling in src/components/player/ and src/components/detail/.
- Existing `Controller<T>` usage with `forward()` for child output → parent input wiring.

**Test scenarios:**
- `pure-fn covered in U2 + U4 + U5; integration smoke validated manually post-U6.`

`Test expectation: none --` for new behavioural unit tests in this unit. All behaviour was tested at the pure-function / message-routing layer in U2, U4, U5. U6 is wiring. Verification is by build + manual smoke.

**Verification:**
- `nix develop -c cargo build` last line shows 0 errors.
- `nix develop -c cargo clippy` clean.
- File-size test passes: `nix develop -c cargo test file_size_limits` — mod.rs stays under 2,000 lines (target: under 1,500 after removals).
- Manual smoke (CLAUDE.md "Manual/Visual Only" category): launch app, switch between Movies and Shows, open the filter popover, toggle each filter, watch pills appear and disappear. Verify Continue Watching shelf displays in-progress items even with Watch status = Unwatched active (KTD7, AE7).

---

### U7. Persistence wiring, filtered count, and empty-state CTA

**Goal:** Restore filter + sort state on library load; save on every change; render the filtered count in the hero subtitle; add the "Clear filters" CTA to the no-results page.

**Requirements:** R20, R21, R22, R23, R24, R25.

**Dependencies:** U3 (Settings model), U6 (LibraryView integration).

**Files:**
- src/components/library/mod.rs — wire load/save and subtitle/CTA updates.
- src/app/mod.rs — pass the `Settings` reference (or a writer handle) into `LibraryView` so it can persist. Confirm signature shape during implementation; likely add a `set_settings` input to `LibraryView` or share an `Arc<Mutex<Settings>>` (the existing pattern in src/app/handlers.rs:128 is to call `app.settings.save()` directly, so a shared mutable handle is fine).

**Approach:**

Load path (R22, R23, R25):
1. `LoadLibrary(LibrarySection, source_type, source_id)` — compute `library_id` (KTD3).
2. Look up `settings.library.get(&library_id)` → `LibraryUiState { filters, sort }`.
3. Reconcile `filters` against the loaded items: drop genres not in `extract_genres(items)`, drop content_ratings not in `extract_content_ratings(items)`, drop resolution buckets not in `extract_resolution_buckets(items)`. Reconciliation lives in src/services/library_filter.rs as `reconcile(state, items) -> FilterState` (added in U2).
4. Apply the reconciled state via `FilterStateChanged(state)` and update the sort dropdown selection.

Save path (R22, R24):
- On every `FilterStateChanged` and `SortChanged`, build `LibraryUiState { filters: self.filter_state.clone(), sort: self.sort_order }` and call `settings.library.set(&self.library_id, state); settings.save()`. Save is fire-and-forget; errors are logged but do not block UI.

Filtered count in hero subtitle (R21):
- Compute `(total, shown)` after each filter pass; format subtitle as `"{total} movies — {shown} shown"` when filters active, `"{total} movies · {subtitle}"` when no filters active.
- Extract `format_library_subtitle(total: usize, shown: usize, kind: LibraryType) -> String` as a pure function in src/components/library/mod.rs (or a helper module) so it's testable.

Empty-state CTA (R20):
- Add a `gtk::Button { label: "Clear filters" }` to the existing `no_results_page` `adw::StatusPage`. Wire to send `LibraryViewMsg::ClearFilters` (search query unchanged per R20). Show the button only when at least one filter is active; hide otherwise. (Search-no-result with no active filters keeps showing just the search-no-match status without the button.)

**Patterns to follow:**
- Existing `settings.save()` call site at src/app/handlers.rs:128 — fire-and-forget, log on failure.
- Existing `adw::StatusPage` construction at src/components/library/mod.rs (no_results_page).

**Test scenarios:**

Reconciliation (Covers R25, AE6):
- `reconcile_drops_genre_not_in_extracted_list` — already added in U2; reaffirm coverage.
- `reconcile_keeps_genre_in_extracted_list`.
- `reconcile_drops_content_rating_not_present`.
- `reconcile_drops_resolution_bucket_not_present`.

Subtitle (Covers R21):
- `format_library_subtitle_no_filter_shows_total` — `format_library_subtitle(1247, 1247, Movie)` returns the unfiltered form.
- `format_library_subtitle_with_filter_shows_shown_count` — `format_library_subtitle(1247, 84, Movie)` returns `"1,247 movies — 84 shown"` (or equivalent).
- `format_library_subtitle_singular_for_one_item`.
- `format_library_subtitle_for_show_library`.

Persistence (Covers R22, R23, R24, AE5):
- `library_id_for_two_distinct_sections_is_unique`.
- `save_then_load_restores_filter_state_for_same_library_id`.
- `save_for_one_library_does_not_pollute_another_libraries_state`.
- `app_restart_simulation_via_toml_roundtrip_preserves_per_library_state` — write `Settings` to temp TOML, parse it back, verify two libraries have separate states.

Empty-state CTA (Covers R20, AE4):
- `Test expectation: none --` widget assertion is GTK-only; verify manually by setting filters that match zero items and clicking "Clear filters". Pure logic: `should_show_clear_filters_button(state) -> bool` returns true iff `state.is_active()`. One test for that helper:
- `clear_filters_button_visible_only_when_any_filter_active`.

**Verification:**
- `nix develop -c cargo test settings` and `cargo test library_filter` pass new tests.
- `nix develop -c cargo build` last line shows 0 errors.
- Manual smoke: filter Movies to Unwatched + Sci-Fi, quit app, relaunch, verify Movies opens with those filters restored; switch to Shows, verify Shows opens with its own (possibly empty) state. Filter Movies to a combination matching zero items, verify no-results page shows "Clear filters" button, click it, verify pill row clears and grid repopulates.

---

## Scope Boundaries

### Out of scope (origin position preserved)

- `HomeView` and its Continue Watching + Recently Added shelves remain untouched (origin R4).
- Collection-based filtering.
- Cast / director / studio / network / language / audio-track filters.
- Saved filter sets, smart filters, shareable filter URLs.
- Restructuring or renaming the existing sort options (R14).
- Search bar, grid-density toggle, view-mode (grid/list) toggle, alphabet jump bar behavior (R19).
- Performance work on filter or grid throughput.

### Deferred to Follow-Up Work

- **`extract_resolution_buckets` derivation from items.** U2 enumerates the four fixed buckets, but if items in a library are all 1080p the popover still shows SD/720p/4K. A follow-up can hide buckets with zero items, mirroring the genre/content-rating extraction shape, once we want to keep the popover surface tighter.
- **Per-library default sort initialization.** Currently any unsaved library defaults to `SortOrder::TitleAsc`. A future enhancement could default Shows to `DateAdded` while Movies defaults to `TitleAsc`, based on library type. Out of scope here.
- **HDR data backfill for items synced before U1 ships.** Existing DB rows have null `hdr`; only items re-synced after U1 will have HDR populated. A one-shot backfill job is deferred — Plex re-sync covers most cases naturally.

---

## Risks and Dependencies

- **Risk: Plex `videoDynamicRange` field name discrepancy.** Plex client docs use camelCase `videoDynamicRange` and most observed JSON responses match, but older Plex Media Server versions may emit the field differently or not at all. *Mitigation:* U1 implementation pulls a real Plex response (or extends the existing fake_server fixture) to confirm the field shape before merging. If the field is absent on a tested server, fall back to scanning `Stream.colorTransfer` for `smpte2084` / `arib-std-b67` / etc. Document the chosen detection in U1's tests.
- **Risk: `mod.rs` line-cap pressure.** Despite the additive shape, U6 could push `mod.rs` near the 2,000-line cap if removals don't outweigh additions. *Mitigation:* U6 verification explicitly checks `file_size_limits.rs` test; aim for a net reduction. If pressure remains, extract `widget_builder.rs` per the convention already in src/app/widget_builder.rs.
- **Risk: Settings TOML schema drift.** Adding seven filter types to `FilterState` and serializing inside `LibrarySettings` produces a verbose per-library TOML block. *Mitigation:* `#[serde(default)]` + `Option<FilterFoo>` shape means empty filters serialize as nothing; only set filters take space. Confirm via the `settings_toml_with_partial_filter_state_fills_defaults` test in U3.
- **Risk: DB migration.** Adding `hdr` column to `movies` and `episodes`. *Mitigation:* The project uses `rusqlite` with explicit schema in src/db/schema.rs; U1 includes the column. SQLite handles `ALTER TABLE ADD COLUMN ... TEXT` cleanly. Verify by running the app against an existing DB at U1 verification time.
- **Dependency:** `libadwaita` widget availability for `AdwExpanderRow`, `AdwSpinRow`, `AdwActionRow` with check-icons (used in U4). All are present in the `libadwaita` versions already in use (see existing usage in src/components/settings_dialog.rs).

---

## System-Wide Impact

- **DB schema change.** `hdr TEXT` column added to two tables in U1. Existing databases get the column via `ALTER TABLE ADD COLUMN` on init; existing rows have null `hdr` until next sync.
- **`MediaItem` shape change.** Adds `hdr: Option<HdrFormat>`. All constructors (Plex convert, local source convert, test builders, DB readers) need to set it. Test files using `MediaItem` literally will fail to compile until updated — this is a useful forcing function for catching missed sites.
- **`LibraryViewMsg::LoadLibrary` signature change.** Was `LoadLibrary(LibraryType)`; becomes `LoadLibrary(LibrarySection, SourceType, source_id: String)` (or equivalent shape that carries the library identifier). Callers in src/app/ and src/components/sidebar.rs need to be updated. This is the largest cross-component change in the plan.
- **`Settings::library` schema change.** Old `default_sort` / `sort_ascending` fields are removed (KTD4). Any existing `settings.toml` will silently lose those fields on next save — they aren't read by any code path today, so no regression.

---

## Sources and Research

- Origin requirements doc: docs/brainstorms/2026-05-27-library-views-overhaul-requirements.md.
- CLAUDE.md guidance on TDD, file size limits, GTK CSS rules, and the "No mpv calls / no GTK in services" architecture rules.
- Existing patterns:
  - Filter logic shape: src/services/library_filter.rs (existing `FilterState`, `filter_matches`, `extract_genres`, `extract_decades`).
  - Plex extraction pipeline: src/services/plex/models.rs (`PlexMedia`) and src/services/plex/convert.rs (`movie_from_plex`, `episode_from_plex`).
  - Persistence: src/settings.rs (TOML + serde, `Settings::load`/`save`).
  - Adwaita widget composition: src/components/settings_dialog.rs.
  - Submodule split convention: src/app/widget_builder.rs and src/app/handlers.rs.
- File-size cap test: tests/file_size_limits.rs.
