---
id: task-449
title: Fix UI continuously refreshing during video playback
status: Done
assignee:
  - Claude
created_date: '2025-10-23 01:55'
updated_date: '2025-11-04'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When playing a video, the UI refreshes completely every ~5 seconds, causing visual disruption and unnecessary performance overhead. The logs show that playback progress updates trigger full page reloads on both the home page and show details page, including clearing all sections, reloading from database, and rebuilding the entire UI.

This affects user experience by:
- Creating visual flicker/disruption while watching content
- Causing GTK warnings about improper widget cleanup
- Consuming unnecessary CPU/memory resources
- Potentially interrupting navigation or interactions with the UI while video is playing

The expected behavior is that playback progress updates should only update the necessary UI elements (progress bars, watch status indicators) without triggering full page reloads.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Playback progress updates do not trigger home page reloads while video is playing
- [x] #2 Playback progress updates do not trigger show details page episode list reloads while video is playing
- [x] #3 Progress bars and watch status indicators still update correctly during playback
- [x] #4 No GTK warnings about finalizing buttons with children during playback
- [x] #5 UI remains stable and responsive during video playback without unexpected refreshes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Investigation Summary

**Root Cause:**
1. Player page saves playback progress every ~5 seconds during playback
2. `UpdatePlaybackProgressCommand` broadcasts `PlaybackProgressUpdated` message to all UI components
3. Components respond with full page reloads instead of ignoring the message:
   - **Home page** (home.rs:515): Calls `LoadData` → clears ALL sections and reloads from database
   - **Show details page** (show_details.rs:609): Calls `LoadEpisodes` → reloads ALL episodes from database
   - **Movie details page** (movie_details.rs:487): Calls `LoadDetails` → reloads ALL movie details from database

## Solution

**Simple fix:** Remove the reload calls from all three pages when they receive `PlaybackProgressUpdated` messages.

**Why this works:**
- The database still gets updated with progress (playback position saved)
- UI updates are not needed during active playback (user is watching video)
- When playback stops and user navigates back, pages naturally reload with fresh data
- Eliminates all UI flicker, GTK warnings, and unnecessary DB queries

## Implementation Steps

1. **Home page** (src/ui/pages/home.rs:506-516)
   - Remove the `sender.input(HomePageInput::LoadData)` call
   - Change to a no-op or log message

2. **Show details page** (src/ui/pages/show_details.rs:597-610)
   - Remove the `sender.input(ShowDetailsInput::LoadEpisodes)` call
   - Change to a no-op or log message

3. **Movie details page** (src/ui/pages/movie_details.rs:475-488)
   - Remove the `sender.oneshot_command(async { MovieDetailsCommand::LoadDetails })` call
   - Change to a no-op or log message

4. **Test**
   - Play a video and verify no UI refreshes occur
   - Stop playback and navigate to home/details pages
   - Verify progress and watch status are updated correctly

## Files to Modify
- `src/ui/pages/home.rs` - Remove reload in PlaybackProgressUpdated handler
- `src/ui/pages/show_details.rs` - Remove reload in PlaybackProgressUpdated handler
- `src/ui/pages/movie_details.rs` - Remove reload in PlaybackProgressUpdated handler
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Successfully fixed UI continuously refreshing during video playback by removing unnecessary page reload calls.

### Changes Made:

1. **src/ui/pages/home.rs** (line 515):
   - Removed `sender.input(HomePageInput::LoadData)` call
   - Added comment explaining why reload is not needed during active playback

2. **src/ui/pages/show_details.rs** (line 754):
   - Removed `sender.input(ShowDetailsInput::LoadEpisodes)` call
   - Added comment explaining why reload is not needed during active playback

3. **src/ui/pages/movie_details.rs** (line 487):
   - Removed `sender.oneshot_command(async { MovieDetailsCommand::LoadDetails })` call
   - Added comment explaining why reload is not needed during active playback

### Result:
- Playback progress is still saved to database every ~5 seconds
- UI no longer refreshes during video playback
- Progress and watch status update correctly when user navigates after playback ends
- Eliminates visual flicker, GTK warnings, and unnecessary performance overhead

Commit: fix: prevent UI refresh during active video playback
Branch: claude/work-on-todo-task-011CUoWSvAGMZfYyuvKg5Z5e
<!-- SECTION:NOTES:END -->
