---
id: task-447
title: >-
  Add manual watch status controls to mark episodes and movies as
  watched/unwatched
status: In Progress
assignee: []
created_date: '2025-10-23 01:40'
updated_date: '2025-10-23 02:06'
labels:
  - feature
  - ui
  - playback
  - sync
dependencies:
  - task-446
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Summary
Users should be able to manually mark movies, episodes, and TV shows as watched or unwatched through UI controls. This allows users to maintain accurate watch status when they've watched content elsewhere, need to correct incorrect status, or want to re-watch content from scratch.

## User Value
- **Flexibility**: Mark content as watched even if it wasn't played through Reel (watched on mobile, web, or other device)
- **Corrections**: Fix incorrect watch status caused by accidental clicks or sync issues
- **Re-watching**: Reset watch status to experience content again without progress resuming
- **Completeness**: Mark entire shows or seasons as watched without playing each episode

## Context
Currently, watch status is only updated automatically during playback. There's no way for users to manually override or set watch status. This is a common feature in media players like Plex, Jellyfin, and streaming services.

## Integration with Context Menus
This feature should be integrated with the context menu system being implemented in task-446. Right-clicking on media items should provide quick access to "Mark as Watched" / "Mark as Unwatched" options in addition to other controls on detail pages.

## Technical Considerations
- Must update both local database (playback_progress table) and sync back to backend servers (Plex/Jellyfin)
- For TV shows: marking a show as watched should mark all episodes as watched; marking as unwatched should clear all episode progress
- For seasons: marking a season watched should mark all episodes in that season
- Watch status changes should trigger UI updates (unseen indicators, continue watching section, etc.)
- Should respect backend API for setting watched status
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Episode detail pages have a 'Mark as Watched' button when unwatched, and 'Mark as Unwatched' when watched
- [ ] #2 Movie detail pages have watch status toggle buttons
- [ ] #3 TV show detail pages have options to mark entire show as watched/unwatched
- [ ] #4 Season selectors on show details have options to mark entire season as watched/unwatched
- [x] #5 Context menu on home page and library page includes 'Mark as Watched'/'Mark as Unwatched' options for media items
- [ ] #6 Marking content as watched sets playback progress to 100% and updates the backend server
- [ ] #7 Marking content as unwatched clears playback progress in both database and backend
- [ ] #8 For TV shows, marking as watched marks all episodes; marking as unwatched clears all episode progress
- [ ] #9 Watch status changes immediately update UI elements (unseen indicators, continue watching section)
- [ ] #10 Backend sync errors are handled gracefully with user feedback

- [ ] #11 Changes persist after app restart and sync correctly across devices
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Progress

### Completed:
1. ✅ Added backend API methods:
   - Plex: `mark_watched()` and `mark_unwatched()` using `/:/scrobble` and `/:/unscrobble` endpoints
   - Jellyfin: `mark_watched()` and `mark_unwatched()` using `/Users/{UserId}/PlayedItems/{Id}` API

2. ✅ Added repository layer support:
   - PlaybackRepository already had `mark_watched()` and `mark_unwatched()` methods

3. ✅ Created service layer (MediaService):
   - `mark_watched()` - marks single item, syncs to backend
   - `mark_unwatched()` - marks single item unwatched, syncs to backend
   - `mark_show_watched()` - marks all episodes in a show
   - `mark_show_unwatched()` - clears all episode progress
   - `mark_season_watched()` - marks all episodes in a season
   - `mark_season_unwatched()` - clears all episode progress in season

4. ✅ Created commands (media_commands.rs):
   - `MarkWatchedCommand` - broadcasts PlaybackProgressUpdated
   - `MarkUnwatchedCommand` - broadcasts PlaybackProgressUpdated
   - `MarkShowWatchedCommand` - broadcasts MediaUpdated
   - `MarkShowUnwatchedCommand` - broadcasts MediaUpdated
   - `MarkSeasonWatchedCommand` - broadcasts MediaUpdated
   - `MarkSeasonUnwatchedCommand` - broadcasts MediaUpdated

5. ✅ Added context menu to MediaCard:
   - Added `MarkWatched` and `MarkUnwatched` to MediaCardOutput
   - Context menu shows "Mark as Watched" or "Mark as Unwatched" based on current status
   - Actions properly wired up

6. ✅ Updated HomePage:
   - Added `MarkWatched` and `MarkUnwatched` to HomePageInput
   - Forward MediaCardOutput to HomePageInput
   - Execute commands via spawn_command

### In Progress:
7. ⏳ Need to update remaining UI pages to handle new MediaCardOutput variants:
   - `src/ui/pages/library/mod.rs`
   - `src/ui/pages/search.rs`
   - `src/ui/factories/section_row.rs`

### Todo:
8. Add watch status controls to movie detail pages
9. Add watch status controls to show detail pages (show and season level)
10. Test that watch status changes:
    - Update database correctly
    - Sync to backend successfully
    - Broadcast events properly
    - Update UI to reflect changes
    - Persist after app restart
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Progress (Session 1)

### ✅ Completed:

**Backend Layer:**
- Added `mark_watched()` and `mark_unwatched()` methods to Plex API (using /:/scrobble and /:/unscrobble endpoints)
- Added `mark_watched()` and `mark_unwatched()` methods to Jellyfin API (using /Users/{UserId}/PlayedItems/{Id})
- Added public wrapper methods to PlexBackend and JellyfinBackend
- Added backend service methods to route calls to appropriate backend

**Service Layer:**
- MediaService methods: `mark_watched()`, `mark_unwatched()`, `mark_show_watched()`, `mark_show_unwatched()`, `mark_season_watched()`, `mark_season_unwatched()`
- All methods update database and sync to backend in fire-and-forget tasks
- Commands: MarkWatchedCommand, MarkUnwatchedCommand, MarkShowWatchedCommand, MarkShowUnwatchedCommand, MarkSeasonWatchedCommand, MarkSeasonUnwatchedCommand
- Commands broadcast appropriate events via MessageBroker (PlaybackProgressUpdated or MediaUpdated)

**UI Layer:**
- MediaCard factory: Added MarkWatched and MarkUnwatched to output enum
- Context menu: Shows "Mark as Watched" or "Mark as Unwatched" based on current status (AC #5 ✅)
- Updated HomePage, LibraryPage, SearchPage to forward watch status outputs
- Updated SectionRow factory to forward watch status outputs
- All pages execute commands via oneshot_command

**Code compiles successfully with no errors!**

### 🚧 Still Todo:
- AC #1: Add watch status button to episode detail pages
- AC #2: Add watch status button to movie detail pages
- AC #3: Add mark show watched/unwatched to show detail pages
- AC #4: Add mark season watched/unwatched to season selectors
- AC #6-11: End-to-end testing

### Next Steps:
1. Add watch status buttons to movie_details.rs and show_details.rs pages
2. Test the implementation end-to-end
3. Verify database updates, backend sync, and UI updates work correctly

## Task Split

This task has been partially completed and split into two parts:

**✅ Completed in task-447:**
- Backend API integration (Plex and Jellyfin)
- Service layer and commands
- Context menus on all media cards (home, library, search pages)
- AC #5 is complete

**🔜 Remaining work moved to task-450:**
- Detail page UI controls (movie details, show details, season selectors)
- AC #1, #2, #3, #4
- End-to-end testing (AC #6-11)

The foundation is solid and ready for the detail page implementation in task-450.
<!-- SECTION:NOTES:END -->
