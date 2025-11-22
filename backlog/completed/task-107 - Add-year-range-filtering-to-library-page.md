---
id: task-107
title: Add year range filtering to library page
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 23:08'
updated_date: '2025-10-04 21:32'
labels: []
dependencies:
  - task-119
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Allow users to filter library content by year or year range using a dual slider or input fields. This enables finding content from specific time periods.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add year range filter UI with min/max year inputs or slider
- [x] #2 Calculate min and max years from library items
- [x] #3 Implement year range filtering logic in LibraryPage
- [x] #4 Update filter application to include year range
- [x] #5 Show active year range in filter UI
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add state fields to LibraryPage struct for year range (min/max available, selected min/max)
2. Calculate min/max years from library items in AllItemsLoaded handler
3. Add new input messages for SetYearRange and ClearYearRange
4. Add year range filter UI with popover similar to genre filter
5. Implement year range filtering logic in the filter chain
6. Update UI to show active year range filter button
7. Test the implementation with library content
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented year range filtering feature for the library page:

**Added State Fields:**
- min_year/max_year: Track available year range from library items
- selected_min_year/selected_max_year: Track user-selected year range
- year_popover/year_menu_button: UI components for year filter

**UI Components:**
- Year filter button with calendar icon in toolbar (appears when years are available)
- Popover with From/To spin buttons for selecting year range
- Clear button to reset year filter
- Dynamic label showing active year range (e.g., "2015 - 2020", "All Years")

**Filtering Logic:**
- Calculate min/max years from library items in AllItemsLoaded handler
- Apply year range filter in filter chain alongside text and genre filters
- Items without year data are excluded when year filter is active
- Filter updates trigger full library reload for consistency

**Messages:**
- SetYearRange { min, max }: Apply year range filter
- ClearYearRange: Remove year range filter

**Modified Files:**
- src/ui/pages/library.rs: Added year filtering to LibraryPage component

The implementation follows the existing genre filter pattern for consistency with the codebase.
<!-- SECTION:NOTES:END -->
