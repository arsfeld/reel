---
id: task-464.02
title: Create PlaybackSyncWorker component for background sync processing
status: Done
assignee: []
created_date: '2025-11-22 20:10'
updated_date: '2025-11-22 20:24'
labels:
  - worker
  - background
  - sync
  - relm4
dependencies: []
parent_task_id: task-464
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement a Relm4 Worker component that processes the playback sync queue in the background, with retry logic and exponential backoff.

**Worker Responsibilities**:
- Poll sync queue at regular intervals (e.g., every 5 seconds)
- Batch pending changes by source for efficient API calls
- Execute sync operations with retry logic
- Update queue status (syncing → synced/failed)
- Emit progress events for UI feedback
- Handle connection status changes (pause when offline)

**Key Features**:
- Exponential backoff for retries (1s, 2s, 4s, 8s, 16s, max 60s)
- Max retry attempts (e.g., 5 attempts before marking as permanently failed)
- Batch deduplication (combine multiple position updates for same item)
- Connection awareness (subscribe to ConnectionBroker)
- Graceful shutdown (complete in-flight operations)

**Integration**:
- Create worker in `src/workers/playback_sync_worker.rs`
- Initialize in app startup
- Subscribe to ConnectionBroker for connection events
- Emit events to SyncBroker for UI updates

**Message Types**:
- Input: `ProcessQueue`, `RetryFailed`, `PauseSync`, `ResumeSync`
- Output: `SyncProgress`, `SyncCompleted`, `SyncFailed`
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Worker component processes queue at regular intervals
- [x] #2 Exponential backoff retry logic is implemented
- [x] #3 Batching combines multiple changes efficiently
- [ ] #4 Worker pauses when connection is lost and resumes when restored
- [x] #5 Worker gracefully handles shutdown without data loss
- [x] #6 Events are emitted for UI progress updates
- [ ] #7 Unit tests cover retry logic and batching
<!-- AC:END -->
