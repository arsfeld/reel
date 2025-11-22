---
id: task-407
title: Implement advanced filter system with popover and filter pills
status: Done
assignee:
  - '@claude'
created_date: '2025-10-06 02:54'
updated_date: '2025-10-06 03:50'
labels:
  - library
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add a filter button in the toolbar that opens a popover where users can add advanced filters (genre, year range, rating, watch status, etc.). When filters are added, they appear as removable pill chips below the toolbar. This replaces the sidebar filter panel approach.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add 'Filters' button to library page toolbar
- [x] #2 Clicking Filters button opens a popover with filter options
- [x] #3 Popover allows selecting filter type (Genre, Year Range, Rating, Watch Status)
- [x] #4 Popover allows setting filter conditions/values for selected filter type
- [x] #5 Added filters appear as pill chips below toolbar with clear labels
- [x] #6 Each filter pill has a remove button (X) to delete the filter
- [x] #7 Filter pills are interactive and show current filter values
- [x] #8 Filters persist per library across sessions
- [x] #9 All filters work additively (AND logic) with other active filters
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Examine existing filter infrastructure and understand current implementation
2. Add "Filters" button to the toolbar (line ~410-420)
3. Create a unified filter popover widget with sections for each filter type
4. Wire up popover to show/hide when Filters button is clicked
5. Implement filter configuration UI in popover (Genre, Year, Rating, Watch Status)
6. Test that filters applied from popover appear as pills below toolbar
7. Verify filters persist across sessions (already implemented in FilterState)
8. Test AND logic with multiple active filters
9. Build and verify no errors
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented unified filters popover system for library page.

## Changes Made

### UI Components
- Added "Filters" button to library page toolbar (src/ui/pages/library.rs:427-435)
- Created unified filters popover with four filter sections:
  - Genre filter: Scrollable checkbox list of available genres
  - Year Range filter: Min/max year spin buttons
  - Rating filter: Slider for minimum rating (0-10)
  - Watch Status filter: Radio buttons (All/Watched/Unwatched)

### Architecture
- Added `filters_popover` and `filters_button` fields to LibraryPage struct
- Added `ToggleFiltersPopover` input message
- Implemented `update_unified_filters_popover()` method to build popover content
- Implemented four section builder methods:
  - `build_genre_filter_section()`
  - `build_year_filter_section()`
  - `build_rating_filter_section()`
  - `build_watch_status_filter_section()`

### Integration
- Popover updates content dynamically before showing to reflect current filter state
- All filter changes trigger existing filter logic (ToggleGenreFilter, SetYearRange, etc.)
- Filter pills continue to work as before (already implemented)
- Filters persist across sessions via existing FilterState serialization
- All filters use AND logic as previously implemented

### Testing Notes
- Build succeeds with no errors
- All acceptance criteria met
- Leverages existing filter infrastructure and pill display system
<!-- SECTION:NOTES:END -->
