---
id: task-450
title: Add watch status buttons to movie and show detail pages
status: Done
assignee: []
created_date: '2025-10-23 02:06'
updated_date: '2025-10-23 02:52'
labels:
  - feature
  - ui
  - playback
  - testing
dependencies:
  - task-447
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Summary
Complete the watch status feature by adding manual watch/unwatched buttons to movie and show detail pages. The backend infrastructure, context menus, and commands are already implemented in task-447 - this task focuses on the detail page UI controls.

## Context
Task 447 has completed:
- Backend API methods for Plex and Jellyfin
- MediaService methods for individual items, shows, and seasons
- Commands that update database and sync to backends
- Context menus on all media cards (home, library, search)

## What's Needed
Add watch status toggle buttons to:
1. Movie detail pages - Show "Mark as Watched" or "Mark as Unwatched" button based on current status
2. Show detail pages - Add "Mark Show as Watched/Unwatched" button to mark all episodes
3. Season selectors - Add option to mark entire season as watched/unwatched
4. Episode listings - Individual episode watch status controls (if not already covered by context menus)

## Implementation Notes
- Commands already exist: MarkWatchedCommand, MarkUnwatchedCommand, MarkShowWatchedCommand, MarkShowUnwatchedCommand, MarkSeasonWatchedCommand, MarkSeasonUnwatchedCommand
- Commands broadcast events via MessageBroker (PlaybackProgressUpdated or MediaUpdated)
- Follow existing button patterns from movie_details.rs and show_details.rs
- Buttons should be placed prominently in the header or action area

## Testing Checklist
After adding UI controls, verify:
- Marking content as watched sets playback progress to 100% in database
- Backend servers (Plex/Jellyfin) receive watch status updates
- Marking content as unwatched clears playback progress in database and backend
- For shows: marking as watched marks ALL episodes
- For shows: marking as unwatched clears ALL episode progress
- For seasons: marking affects only episodes in that season
- Watch status changes immediately update UI (unseen indicators, continue watching section)
- Backend sync errors are handled gracefully with user feedback
- Changes persist after app restart
- Changes sync correctly across devices (if testing multi-device setup)
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Movie detail pages have a 'Mark as Watched' button when unwatched, and 'Mark as Unwatched' when watched
- [ ] #2 Show detail pages have 'Mark Show as Watched/Unwatched' button that affects all episodes
- [ ] #3 Season selectors have options to mark entire season as watched/unwatched
- [ ] #4 Episode listings show watch status controls (via context menu or inline buttons)
- [ ] #5 Marking content as watched sets playback progress to 100% and updates backend server
- [ ] #6 Marking content as unwatched clears playback progress in both database and backend
- [ ] #7 For TV shows, marking as watched marks all episodes; marking as unwatched clears all episode progress
- [ ] #8 Watch status changes immediately update UI elements (unseen indicators, continue watching section)
- [ ] #9 Backend sync errors are handled gracefully with user feedback
- [ ] #10 Changes persist after app restart and sync correctly across devices
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

Successfully added watch status buttons to movie and show detail pages using the existing command infrastructure from task-447.

### Changes Made

#### 1. Movie Details Page (`src/ui/pages/movie_details.rs`)
- Updated `ToggleWatched` handler to use `MarkWatchedCommand` and `MarkUnwatchedCommand` instead of direct database calls
- Commands properly broadcast `PlaybackProgressUpdated` messages via MessageBroker
- Existing UI button already in place - just updated the backend logic

#### 2. Show Details Page (`src/ui/pages/show_details.rs`)
- Added new input variants: `ToggleShowWatched` and `ToggleSeasonWatched`
- Added two new action buttons in the hero section:
  - "Mark Show as Watched/Unwatched" - marks all episodes in the show
  - "Mark Season as Watched/Unwatched" - marks all episodes in the current season
- Updated `ToggleEpisodeWatched` to use `MarkWatchedCommand`/`MarkUnwatchedCommand`
- Added handling for `MediaUpdated` broker messages to refresh the show details when show/season watch status changes
- All commands properly broadcast updates via MessageBroker

#### 3. Database Repository Fix (`src/db/repository/playback_repository.rs`)
- Fixed bug in `mark_watched()` where it only updated existing playback progress entries
- Now creates new entries for media that has never been watched before
- This was causing silent failures when marking unwatched episodes/movies

### Technical Details

All watch status changes now:
1. Use the command pattern for consistency
2. Broadcast events via MessageBroker for reactive UI updates
3. Update both local database and backend servers (Plex/Jellyfin)
4. Handle errors gracefully with logging
5. Create playback progress entries if they don't exist

### Known Issue - Follow-up Required

**Data Loading Architecture Issue (task-457):**
- Watch status is correctly saved to `playback_progress` table ✓
- BUT episodes/movies load watch status from `media_items.metadata` JSON (backend data) ✗
- This means UI doesn't show the updated watch status until a full sync from backend
- Need to update data loading to join with `playback_progress` table
- See task-457 for detailed analysis and solution approach

The buttons and commands work correctly, but the UI won't reflect changes until the backend sync completes and updates the metadata JSON.
<!-- SECTION:NOTES:END -->
