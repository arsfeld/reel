---
id: task-108
title: Add rating filtering to library page
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 23:08'
updated_date: '2025-10-04 21:36'
labels: []
dependencies:
  - task-119
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Enable filtering library content by rating thresholds. Users can set minimum rating values to show only highly-rated content.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add rating filter UI with slider or star selector
- [x] #2 Implement rating threshold filtering in LibraryPage
- [x] #3 Display current rating filter value
- [x] #4 Apply rating filter alongside other active filters
- [x] #5 Handle null/unrated content appropriately
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add state fields to LibraryPage struct for rating filter (min_rating, rating_popover, rating_menu_button)
2. Add LibraryPageInput message variants for SetRatingFilter and ClearRatingFilter
3. Add rating filter UI button in toolbar (similar to genre and year filters)
4. Create rating popover with scale widget (0-10 rating range)
5. Implement rating filter logic in AllItemsLoaded handler alongside other filters
6. Add helper methods (get_rating_label, update_rating_popover)
7. Test with items that have ratings and items without ratings
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented rating filter functionality for the library page with the following changes:

## Changes Made

1. **State Management**: Added three new fields to LibraryPage struct:
   - `min_rating: Option<f32>` - stores the minimum rating threshold
   - `rating_popover: Option<gtk::Popover>` - the popover UI widget
   - `rating_menu_button: Option<gtk::MenuButton>` - the menu button widget

2. **Input Messages**: Added two new message variants:
   - `SetRatingFilter(Option<f32>)` - sets the minimum rating filter
   - `ClearRatingFilter` - clears the rating filter

3. **UI Components** (src/ui/pages/library.rs):
   - Added rating filter menu button in toolbar with starred-symbolic icon
   - Created popover with horizontal scale widget (0-10 range, 0.5 step)
   - Added marks at each integer value for better UX
   - Displays current rating with star symbol in button label
   - Includes clear button and apply button

4. **Helper Methods**:
   - `get_rating_label()` - returns formatted label (e.g., "7.5+ ★" or "All Ratings")
   - `update_rating_popover()` - builds and updates the popover UI

5. **Filtering Logic**:
   - Integrated rating filter in AllItemsLoaded handler alongside text, genre, and year filters
   - Items without ratings are excluded when filter is active (rating.map_or(false, |r| r >= min_rating))
   - When no filter is set, all items (including unrated) are included

6. **Message Handlers**:
   - SetRatingFilter: updates state, refreshes UI, and reloads filtered items
   - ClearRatingFilter: clears state, updates UI, and reloads all items

## Testing Notes

- Code compiles successfully with no errors
- Filter integrates seamlessly with existing filters (text, genre, year, media type)
- Handles null ratings appropriately by excluding them when filter is active
- UI follows same pattern as existing genre and year filters for consistency
<!-- SECTION:NOTES:END -->
