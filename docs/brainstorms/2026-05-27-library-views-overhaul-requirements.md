---
date: 2026-05-27
topic: library-views-overhaul
---

# Library Views Overhaul — Adwaita Filters and Sort

## Summary

Replace the in-library genre chip carousel and the in-library "Recently Added" shelf with an Adwaita-style filter system: a header **Filter** popover, an "active filters" pill row above the grid, and an expanded filter set (watch status, year range, genre, rating, runtime, content rating, resolution/HDR). Filters and sort are per-library and persisted across restarts.

---

## Problem Frame

`LibraryView` (src/components/library/mod.rs) today sits between two competing roles. Above the grid it shows a horizontal genre chip carousel, a decade dropdown, a sort dropdown, and a small filter-icon `MenuButton` — none of which scale to a library of several thousand items. Below that it carries a "Recently Added" carousel that duplicates content already reachable via `Sort: Date Added`. The result is a wide visual bar that costs vertical space without supporting real discovery: there is no way to ask "an unwatched sci-fi movie under 100 minutes I haven't seen, rated ≥7," which is the actual question users ask of a large library.

The library also doesn't read as a GNOME app. The genre chip carousel, custom shelf headers, and ad-hoc filter button arrangement don't match the Adwaita conventions used elsewhere in the desktop (`AdwOverlaySplitView`, `AdwPreferencesGroup` rows, header dropdowns). Aligning to those conventions both reduces the surface area of custom widget code and makes the app feel native to its host environment.

---

## Key Decisions

- **Filter UI shape: header popover + active-filter pill row.** Filter button in the header opens a popover containing all filter groups. Active filters render as removable pills (✘) on a sub-header row above the grid, with a "Clear all" affordance. Chosen over an `AdwOverlaySplitView` sidebar because it keeps the grid full-width on all window sizes and surfaces what's currently filtered without needing to open anything.
- **Year range replaces decade dropdown.** A range control (from/to) replaces the single-decade `DropDown` so users can ask for "1970s–1990s" or "pre-2000."
- **Watch status is a first-class filter.** Derived from existing `watch_data: HashMap<String, (f64, bool)>` on `LibraryView` — no new data plumbing, just new UI.
- **Per-library, persisted filter + sort state.** Each library's filter and sort selections survive library switches and app restart. Stored in settings keyed by library id.
- **`HomeView` is untouched.** Recently Added and Continue Watching shelves remain there as the discovery surface. The library view becomes "filtered grid + Continue Watching shelf."
- **Filter combination semantics.** AND across filter types (watch status AND year AND rating); OR within multi-select sets (genre A OR genre B; content rating PG OR PG-13).

---

## Requirements

### Removals

- R1. The genre chip carousel currently rendered as `genre_scroll` / `genre_box` is removed from the filter bar in `LibraryView`.
- R2. The "Recently Added" shelf inside `LibraryView` (`recently_added_section`, `recently_added_box`, and the `rebuild_recently_added` path) is removed.
- R3. The existing decade `DropDown` is removed and replaced by the year-range control inside the filter popover.
- R4. `HomeView` is not modified by this work — both its Continue Watching and Recently Added shelves remain in place.
- R5. The Continue Watching shelf inside `LibraryView` (`continue_watching_section`) remains in place above the grid.

### Filter set

- R6. The filter popover exposes a **Watch status** filter with three mutually exclusive options plus an "Any" default: Unwatched, In Progress, Watched. Derivation: `watched=true` → Watched; `progress_fraction > 0.0 && !watched` → In Progress; otherwise Unwatched.
- R7. The filter popover exposes a **Year range** filter with a from and a to value. Either end may be left unset, meaning "no lower / upper bound." Items with no `year` are excluded when any bound is set.
- R8. The filter popover exposes a **Genre** filter (multi-select, OR semantics) — same behaviour as today's `GenreFilter`, just relocated into the popover.
- R9. The filter popover exposes a **Minimum rating** filter as a single threshold (e.g., "≥7.0"). Items with no `rating` are excluded when a threshold is set.
- R10. The filter popover exposes a **Runtime range** filter (min and/or max minutes). Items with no `runtime_minutes` are excluded when any bound is set.
- R11. The filter popover exposes a **Content rating** filter (multi-select, OR semantics) over the distinct `content_rating` values present in the loaded items.
- R12. The filter popover exposes a **Resolution / HDR** filter (multi-select, OR semantics) over normalized buckets derived from `video_resolution`: at minimum `SD`, `720p`, `1080p`, `4K`, and `HDR` (HDR can co-select with a resolution bucket).
- R13. All active filter types combine with AND; multi-select values within a single filter type combine with OR.

### Sort

- R14. The existing sort options (`TitleAsc`, `TitleDesc`, `YearNewest`, `YearOldest`, `DateAdded`, `RatingHighest`, `RuntimeLongest`) are preserved unchanged. The sort dropdown remains in the header.

### Filter / sort UI

- R15. The header carries a **Filter** button (with a count badge when filters are active) and a **Sort** dropdown. No other filter controls live in the header.
- R16. A sub-header pill row sits between the header and the grid and is visible only when at least one filter is active. Each pill displays the filter type and value in human form (see R17) and carries an ✘ that removes that specific filter. The row also carries a "Clear all" button.
- R17. Pill label format: `Watched`, `In Progress`, `Unwatched`; `Year 1990–2009`, `Year ≥1990`, `Year ≤2009`; `Genre: Sci-Fi`; `Rating ≥7.0`; `Runtime <100 min`, `Runtime 90–120 min`; `PG-13`; `4K`, `HDR`. Multi-value selections within one filter type render as one pill per value, each individually removable. An unset range bound never produces a pill.
- R18. The filter popover groups controls as `AdwPreferencesGroup`s in this order: Watch status, Year, Genre, Rating, Runtime, Content rating, Resolution. The popover also contains a "Clear all" action.
- R19. The grid-density toggle, view-mode toggle (grid/list), and search bar all remain functional in their current positions.

### Empty and result states

- R20. When applied filters produce zero matches, the no-results page renders and includes a "Clear filters" button that resets `FilterState` (but leaves search query and sort untouched).
- R21. The library hero subtitle reflects the filtered count when filters are active: `"1,247 movies — 84 shown"`. With no filters active it shows the unfiltered total, as today.

### Persistence

- R22. Filter state (`FilterState` extended with the new fields) and `SortOrder` are persisted per library, keyed by a stable library identifier.
- R23. On library load, persisted filter and sort state for that library is restored before the grid is populated.
- R24. Switching libraries (e.g., Movies → Shows) restores that library's own persisted state — selections do not carry across libraries.
- R25. A persisted filter value that references something no longer present in the current items (e.g., a genre that no longer exists, a content rating no items carry) is silently dropped on restore rather than producing a pill that matches nothing.

---

## Acceptance Examples

- AE1. **Covers R6, R13, R16, R17.** Given a Movies library with 1,247 items, when the user opens the Filter popover, selects Watch status = Unwatched, and selects Genre = Sci-Fi, then the grid shows only unwatched Sci-Fi items, the pill row shows `Unwatched ✘` and `Genre: Sci-Fi ✘`, and the hero subtitle reads `1,247 movies — N shown`.
- AE2. **Covers R7, R17.** Given the user sets Year from 1990 with no upper bound, then one pill renders as `Year ≥1990`. If they then set the upper bound to 2009, the pill collapses to `Year 1990–2009`.
- AE3. **Covers R16.** Given two pills are active (`Unwatched`, `Sci-Fi`), when the user clicks ✘ on `Sci-Fi`, then only `Unwatched` remains active and the grid re-filters; the popover state mirrors this on next open.
- AE4. **Covers R20.** Given a filter combination that matches zero items, then the no-results page is visible and clicking "Clear filters" empties the pill row, re-renders the grid with all items, and leaves the current search query and sort unchanged.
- AE5. **Covers R22, R23, R24.** Given the user filters Movies to Unwatched and switches to Shows, then Shows shows its own persisted state (or no filters if none were saved). Switching back to Movies restores Unwatched without user action. Restarting the app preserves both.
- AE6. **Covers R25.** Given a persisted Genre filter `Sci-Fi` on Movies, when the user moves their library and `Sci-Fi` is no longer in `extract_genres(items)`, then the filter is dropped silently on load and no pill renders for it.
- AE7. **Covers R5, R6.** Given the Continue Watching shelf is showing 4 in-progress items and the user activates Watch status = Unwatched, then the grid excludes those 4 items but the Continue Watching shelf continues to display them. The shelf is independent of active filters.

---

## Scope Boundaries

### Out of scope

- Any change to `HomeView` — its Continue Watching and Recently Added shelves remain as-is.
- Collection filtering (Plex collections as a filter dimension).
- Cast / director / studio / network / language / audio-track filters.
- "Smart filters" / saved filter sets / shareable filter URLs.
- Restructuring or renaming the existing sort options.
- Changes to the search bar, grid-density toggle, view-mode (grid/list) toggle, or the alphabet jump bar.
- Performance work on filtering or grid throughput; current `apply_filters_and_sort` performance is assumed acceptable for the new filter set.

---

## Outstanding Questions

### Resolve before planning

- Stable library identifier for persistence. Today's `LibraryType` enum is coarse (e.g., movies vs shows). Persistence wants something finer — likely `(SourceType, source_id, library_section_id)` — so that a user with multiple Plex servers, or a future Jellyfin source, keeps state separate. Confirm shape before planning.
- Resolution bucket derivation. `MediaItem.video_resolution` is a free-form string today (e.g., `"1080"`, `"4k"`, `"sd"`). Confirm the canonical bucket set and the normalization function. HDR detection currently has no field — does it exist on the Plex payload and is it being persisted?

### Deferred to planning

- Exact `AdwPreferencesGroup` / row widget choices (`AdwSpinRow` vs `AdwActionRow` + spin button vs slider for ranges).
- Pill row layout under narrow window widths (single-row scroll vs wrap vs collapse-to-count). Likely scroll.
- Where the persisted state lives on disk (existing settings store vs new file) and migration of today's session-only state (no migration needed — no users yet).

---

## Dependencies / Assumptions

- `MediaItem` already carries `genres`, `year`, `rating`, `runtime_minutes`, `content_rating`, and `video_resolution`. Verified against src/services/library_filter.rs tests; these fields are populated for Plex-sourced items.
- `watch_data: HashMap<String, (f64, bool)>` is already maintained on `LibraryView` and refreshed on `SetWatchData`; the Watch status filter reads from this same source.
- The current `FilterState` (genres + decade) becomes a strict superset (genres, year_range, watch_status, rating_min, runtime_range, content_ratings, resolutions). Pure-function tests in src/services/library_filter.rs extend; the existing API stays mock-friendly.
- Settings persistence has an existing path (referenced by `settings_dialog.rs`); the new per-library filter+sort entries live in the same store rather than a separate file.
