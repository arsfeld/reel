---
id: task-464
title: Implement reliable bidirectional sync for playback progress and watch status
status: To Do
assignee: []
created_date: '2025-11-22 18:18'
labels:
  - sync
  - playback
  - watch-status
  - reliability
  - architecture
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Currently, local changes to playback progress and watch status use a fire-and-forget sync pattern that's unreliable. When users mark episodes as watched or update playback position locally, the changes are synced to the backend using detached `tokio::spawn()` tasks with no retry mechanism, change tracking, or user feedback.

**Current State**:
- Local → Backend sync exists but uses fire-and-forget pattern (`src/services/core/media.rs` lines 837, 912)
- No tracking of which changes succeeded/failed
- No retry mechanism if backend is offline or sync fails
- No conflict resolution when both local and backend change
- Each change triggers individual API call (no batching)
- User has no visibility into sync status

**Critical Gaps**:
1. No `playback_sync_queue` table to track pending changes
2. No "dirty" flag to identify unsynced records
3. Missing retry logic with exponential backoff
4. No conflict resolution (what if position differs between local/remote?)
5. No batching or deduplication for rapid changes
6. No UI feedback showing sync status

**Impact**:
- Changes made offline may be silently lost
- Rapid marking of multiple episodes causes many individual API calls
- Users don't know if their changes synced successfully
- Backend may have stale data if syncs fail silently

**Solution Architecture**:
- Add `playback_sync_queue` table with change tracking
- Create `PlaybackSyncWorker` to process queue in background
- Implement batching and deduplication
- Add conflict resolution logic (last-write-wins or local-progressive)
- Show sync status in UI (pending/syncing/synced/failed)
- Support offline queue that syncs when connection restored
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Local playback changes are tracked in sync queue table
- [ ] #2 Failed syncs are automatically retried with exponential backoff
- [ ] #3 Batching prevents rapid API calls for bulk operations
- [ ] #4 Conflict resolution handles local vs remote differences
- [ ] #5 UI shows sync status (syncing/synced/failed) for watch status changes
- [ ] #6 Offline changes are queued and synced when connection restored
- [ ] #7 Sync queue persists across app restarts
- [ ] #8 Error messages are user-friendly and actionable
<!-- AC:END -->
