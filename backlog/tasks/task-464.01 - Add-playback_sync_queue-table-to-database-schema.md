---
id: task-464.01
title: Add playback_sync_queue table to database schema
status: Done
assignee: []
created_date: '2025-11-22 20:09'
updated_date: '2025-11-22 20:18'
labels:
  - database
  - schema
  - migration
  - sync
dependencies: []
parent_task_id: task-464
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create a new database table to track pending playback progress and watch status changes that need to be synced to backends.

**Table Schema**:
- `id` - Primary key
- `media_item_id` - Foreign key to media_items
- `source_id` - Foreign key to sources
- `user_id` - User identifier
- `change_type` - Enum: 'progress_update' | 'mark_watched' | 'mark_unwatched'
- `position_ms` - Playback position (for progress updates)
- `completed` - Boolean (for watch status)
- `created_at` - When the change was created
- `last_attempt_at` - Last sync attempt timestamp
- `attempt_count` - Number of sync attempts
- `error_message` - Last error if sync failed
- `status` - Enum: 'pending' | 'syncing' | 'synced' | 'failed'

**Migration**:
- Create SeaORM migration in `src/db/migrations/`
- Add entity definition in `src/db/entities/`
- Add indexes for efficient querying (media_item_id, source_id, status)

**Repository**:
- Create `PlaybackSyncRepository` with methods:
  - `enqueue_change()` - Add new change to queue
  - `get_pending()` - Get all pending changes
  - `mark_syncing()` - Update status to syncing
  - `mark_synced()` - Mark change as successfully synced
  - `mark_failed()` - Mark change as failed with error
  - `get_failed_retryable()` - Get failed changes that can be retried
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Migration creates playback_sync_queue table with all required fields
- [x] #2 Entity definition exists with proper relations to media_items and sources
- [x] #3 Indexes are created for efficient querying (media_item_id, source_id, status)
- [x] #4 PlaybackSyncRepository implements all CRUD operations
- [ ] #5 Repository methods are tested with unit tests
- [x] #6 Migration runs successfully on clean database
- [x] #7 Migration is reversible (down migration works)
<!-- AC:END -->
