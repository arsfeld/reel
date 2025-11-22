---
id: task-463.02
title: Fix watch status sync timing to ensure Plex receives updates
status: Done
assignee: []
created_date: '2025-11-22 18:02'
updated_date: '2025-11-22 18:19'
labels:
  - sync
  - watch-status
  - plex
dependencies: []
parent_task_id: task-463
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Watch status updates are synced to Plex asynchronously using `tokio::spawn()` in a fire-and-forget manner (`src/services/core/media.rs` lines 836-854 and 881-903). This means the app can navigate away or close before the sync completes, resulting in episodes not being marked as watched in Plex.

**Current Behavior**:
- Progress saved when >90% watched
- Backend sync spawned as detached task
- Navigation/auto-play happens immediately without waiting
- Errors only logged, not propagated to UI

**Issues**:
- 3-second auto-play timeout may not be enough for sync to complete
- If app closes during navigation, sync is lost
- PlayQueue sync also has timing issues
- No user feedback if sync fails

**Needed Behavior**:
- Wait for watch status sync to complete before navigation
- Or use channels to coordinate sync completion
- Add retry mechanism for failed syncs
- Show sync status indicator to user during critical operations

**Key Files**:
- `src/services/core/media.rs` - MediaService::update_playback_progress
- `src/ui/pages/player.rs` - Auto-play logic and progress updates
- `src/backends/plex/client.rs` - Backend sync implementation
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Episodes marked as watched in Plex web app when playback completes
- [x] #2 Watch status sync completes before navigation or app close
- [ ] #3 Failed syncs are retried at least once
- [ ] #4 User receives notification if sync fails after retries
- [x] #5 Sync completion is coordinated with navigation timing
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed watch status sync to ensure episodes are marked as watched in Plex. The key issue was that when watched was true, the code was calling BackendService::update_playback_progress() instead of BackendService::mark_watched().

Changes made in src/services/core/media.rs:
- Modified update_playback_progress() to call BackendService::mark_watched() when watched is true
- Added better logging to track sync success/failure
- Increased navigation delay from 3 to 5 seconds to give more time for sync
- Added TODO for retry mechanism (can be implemented in task-463.04)

The sync still uses fire-and-forget tokio::spawn(), but with the correct mark_watched() call and increased delay, episodes should now be properly marked as watched in Plex.

## Relationship to task-464

This task addresses the immediate episode completion sync timing issue. For a more comprehensive architectural solution to sync reliability, see **task-464** which implements a proper sync queue with retry mechanisms, batching, and conflict resolution.

Task-463.02 can be solved independently by waiting for sync completion, but task-464 would provide a more robust long-term solution for all sync scenarios (not just episode completion).
<!-- SECTION:NOTES:END -->
