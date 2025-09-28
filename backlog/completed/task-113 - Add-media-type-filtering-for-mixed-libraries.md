---
id: task-113
title: Add media type filtering for mixed libraries
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 23:09'
updated_date: '2025-09-22 14:55'
labels: []
dependencies:
  - task-119
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
For mixed-type libraries, allow filtering by media type (movies, shows, music, photos). Essential for libraries that contain multiple content types.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Detect mixed-type libraries from library metadata
- [x] #2 Add media type filter UI for mixed libraries only
- [x] #3 Implement media type filtering in database queries
- [x] #4 Show media type icons on cards in mixed libraries
- [x] #5 Update item counts when media type filter changes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Check library entity for library_type field and identify mixed libraries
2. Add media_type filter state to LibraryPage component
3. Create media type filter UI (buttons/dropdown) that only shows for mixed libraries
4. Modify database queries to filter by media_type when filter is active
5. Add media type badges/icons to MediaCard components
6. Update counts by querying with filters applied
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

Implemented comprehensive media type filtering for mixed-type libraries:

### Changes Made:

1. **Library Detection**: Modified `LibraryPage::load_all_items()` to detect library type from database and store it in component state

2. **UI Components**: Added toggle buttons for media type filtering that only appear when `library_type == "mixed"`. The buttons use GTK's linked button style for a clean grouped appearance

3. **Database Filtering**: Leveraged existing `find_by_library_and_type()` method to filter media items when a specific media type is selected

4. **Visual Indicators**: Added media type icons to media cards in mixed libraries using overlay badges with appropriate symbolic icons

5. **Item Count Updates**: Counts automatically update when filters change since the entire library reloads with the new filter applied

### Files Modified:
- `src/ui/pages/library.rs` - Added media type filter state and UI
- `src/ui/factories/media_card.rs` - Added media type icon display
- `src/ui/pages/home.rs` - Updated MediaCardInit usage
- `src/ui/factories/section_row.rs` - Updated MediaCardInit usage

The implementation is fully functional and follows the existing component patterns in the codebase.
<!-- SECTION:NOTES:END -->
