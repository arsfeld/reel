---
id: task-453
title: Fix Mark Show as Watched and Mark Season buttons not updating watch status
status: To Do
assignee: []
created_date: '2025-10-23 02:26'
updated_date: '2025-10-23 02:26'
labels:
  - bug
  - ui
  - playback
  - watch-status
dependencies:
  - task-450
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The "Mark Show as Watched/Unwatched" and "Mark Season" buttons on the TV show details page are not working correctly. When clicked, they trigger a page reload (as evidenced by the "Show/Season updated" debug logs), but the watch status of episodes is not actually being updated.

**Observed behavior:**
1. User clicks "Mark Show as Watched" or "Mark Season" button
2. Log shows: `Show/Season updated for [ID], reloading show details and episodes`
3. Page reloads and displays the same data
4. Episodes still show as unwatched - watch status has not changed
5. Backend server is not receiving the mark watched API calls

**Expected behavior:**
1. Clicking "Mark Show as Watched" should mark ALL episodes in the show as watched in both database and backend
2. Clicking "Mark Season" should mark all episodes in the current season as watched
3. UI should update to show all episodes with the green checkmark indicator
4. Watch status should sync to Plex/Jellyfin backend
5. Changes should persist after app restart

**Root cause analysis needed:**
The buttons appear to be triggering the wrong commands or the commands are not being executed properly. The page reload suggests the UI is responding to the button click, but the actual watch status update logic (using MarkShowWatchedCommand or MarkSeasonWatchedCommand) may not be firing or may be failing silently.

Reference: These buttons were added in task-450, which integrated with the command infrastructure from task-447.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Clicking 'Mark Show as Watched' successfully marks all episodes in the show as watched
- [ ] #2 Clicking 'Mark Show as Unwatched' successfully clears watch status for all episodes in the show
- [ ] #3 Clicking 'Mark Season' button when unwatched marks all episodes in current season as watched
- [ ] #4 Clicking 'Mark Season' button when watched clears watch status for all episodes in current season
- [ ] #5 Watch status changes are persisted to the database (playback_progress table)
- [ ] #6 Watch status changes are synced to the backend server (Plex/Jellyfin API)
- [ ] #7 UI updates to show green checkmarks on watched episodes after marking as watched
- [ ] #8 UI removes checkmarks when marking as unwatched
- [ ] #9 Changes persist after app restart
- [ ] #10 Error handling displays user feedback if backend sync fails
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
**Debug Evidence:**
From the logs, clicking the buttons triggers `Show/Season updated for 109085, reloading show details and episodes` but no actual watch status changes occur.

**Investigation Starting Points:**
1. Check `src/ui/pages/show_details.rs` - Look at the button click handlers for `ToggleShowWatched` and `ToggleSeasonWatched`
2. Verify the commands are being spawned correctly: `MarkShowWatchedCommand`, `MarkShowUnwatchedCommand`, `MarkSeasonWatchedCommand`, `MarkSeasonUnwatchedCommand`
3. Check if the commands are actually executing or failing silently
4. Add debug logging to the command execution to see if they're being called
5. Verify the MediaService methods (`mark_show_watched`, `mark_season_watched`, etc.) are working correctly
6. Look for error logs related to backend sync or database updates
<!-- SECTION:NOTES:END -->
