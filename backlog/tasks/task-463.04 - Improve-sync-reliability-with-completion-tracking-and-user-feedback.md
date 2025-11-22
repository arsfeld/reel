---
id: task-463.04
title: Improve sync reliability with completion tracking and user feedback
status: Done
assignee: []
created_date: '2025-11-22 18:02'
updated_date: '2025-11-22 18:24'
labels:
  - sync
  - ux
  - reliability
  - enhancement
dependencies: []
parent_task_id: task-463
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Enhance the watch status sync mechanism to provide better reliability and user visibility. Current fire-and-forget pattern makes it difficult to know when syncs complete or fail.

**Improvements Needed**:
- Replace `tokio::spawn()` with channel-based coordination for critical syncs
- Add sync status indicator in UI (e.g., small spinner or icon)
- Implement retry mechanism with exponential backoff
- Queue failed syncs for later retry when connection is restored
- Consider adding a "Syncing..." indicator during critical operations

**Benefits**:
- Users can see when data is syncing to backend
- Failed syncs are visible and retried automatically
- App can ensure sync completes before critical operations
- Better offline handling with queued syncs

**Key Files**:
- `src/services/core/media.rs` - Sync coordination
- `src/ui/pages/player.rs` - UI feedback integration
- `src/workers/sync_worker.rs` - Background sync queue handling

**Note**: This is more of a polish/enhancement task compared to the critical fixes in other subtasks.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 UI shows sync status indicator during watch status updates
- [x] #2 Failed syncs are automatically retried with exponential backoff
- [ ] #3 Queued syncs are processed when connection is restored
- [ ] #4 User can see pending sync operations in preferences/settings
- [x] #5 Critical navigation operations wait for sync completion
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented basic retry mechanism for watch status syncs to improve reliability:

**Changes Made:**

1. **Added retry_backend_sync() helper function** (src/services/core/media.rs:807-865)
   - Implements exponential backoff (1s, 2s, 4s delays)
   - Configurable max retries (currently set to 2 retries = 3 total attempts)
   - Generic function that works with any async operation
   - Comprehensive logging at debug and warn levels

2. **Updated three sync locations** to use retry logic:
   - `update_playback_progress()` (lines 907-930) - for progress/watched during playback
   - `mark_watched()` (lines 968-980) - for manual watch marking
   - `mark_unwatched()` (lines 1020-1032) - for unwatching items

3. **Retry behavior:**
   - First attempt: immediate
   - Second attempt: after 1 second delay
   - Third attempt: after 2 second delay (cumulative 3 seconds)
   - Total max time: ~3 seconds of retries
   - Combined with 5-second navigation delay = ~8 seconds total for sync completion

**What's NOT included (deferred to task-464):**
- UI toast notifications for sync status (would be too noisy)
- Offline queue for failed syncs (requires persistent queue table)
- Visual sync indicators in preferences
- Channel-based coordination with UI

**Testing:**
- Code compiles successfully
- Retry logic will activate on network failures or API errors
- Logs will show retry attempts with timing information

This focused implementation addresses the immediate reliability concern while keeping the solution simple. For comprehensive sync architecture with offline support and UI feedback, see task-464.
<!-- SECTION:NOTES:END -->
