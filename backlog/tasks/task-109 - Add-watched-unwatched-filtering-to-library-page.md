---
id: task-109
title: Add watched/unwatched filtering to library page
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 23:08'
updated_date: '2025-10-04 21:43'
labels: []
dependencies:
  - task-119
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Allow users to filter library content based on watch status - show all, only watched, or only unwatched items. Essential for managing viewing progress.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add watched status filter UI with toggle or dropdown
- [x] #2 Query playback progress from database for watch status
- [x] #3 Implement watch status filtering logic
- [x] #4 Show watch status indicator on media cards
- [x] #5 Update filter to work with other active filters
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add watch status filter state to LibraryPage struct
2. Add watch status filter UI (menu button and popover) in view
3. Add SetWatchStatusFilter and ClearWatchStatusFilter input messages
4. Add watch status filtering logic in AllItemsLoaded handler
5. Implement update_watch_status_popover helper method
6. Test filtering with mixed watched/unwatched content
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented watch status filtering for the library page with the following changes:

## Changes Made

1. **Added WatchStatus enum** (src/ui/pages/library.rs:94-99)
   - Three states: All, Watched, Unwatched

2. **Added filter state to LibraryPage** (src/ui/pages/library.rs:63-66)
   - `watch_status_filter: WatchStatus`
   - `watch_status_popover: Option<gtk::Popover>`
   - `watch_status_menu_button: Option<gtk::MenuButton>`

3. **Added UI components**
   - Menu button with "object-select-symbolic" icon in toolbar (src/ui/pages/library.rs:284-290)
   - Radio button popover for selecting All/Watched/Unwatched (src/ui/pages/library.rs:1537-1610)
   - Helper method `get_watch_status_label()` for button label (src/ui/pages/library.rs:1204-1210)

4. **Added input messages** (src/ui/pages/library.rs:138-141)
   - `SetWatchStatusFilter(WatchStatus)` - Change filter selection
   - `ClearWatchStatusFilter` - Reset to All

5. **Implemented filtering logic** (src/ui/pages/library.rs:720-812)
   - Fetches playback progress for all items when filter is active (optimization: only when not "All")
   - For TV shows: checks `watched_episode_count == total_episode_count` in metadata
   - For movies/episodes: uses `playback_progress.watched` field from database
   - Works alongside existing filters (text, genre, year, rating, media type)

6. **Added message handlers** (src/ui/pages/library.rs:1059-1099)
   - Handles SetWatchStatusFilter: updates UI and reloads filtered items
   - Handles ClearWatchStatusFilter: resets to All and reloads

## Technical Details

- Watch status indicators are already shown on media cards (unwatched glow dot)
- Playback progress is batch-fetched for performance
- Filter integrates seamlessly with existing filter chain
- Popover UI follows same pattern as other filter popovers (genre, year, rating)
<!-- SECTION:NOTES:END -->
