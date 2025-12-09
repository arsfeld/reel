---
id: task-472.01
title: Add fetched_at timestamp to media_items table
status: Done
assignee: []
created_date: '2025-12-09 18:51'
updated_date: '2025-12-09 19:13'
labels:
  - database
  - migration
dependencies: []
parent_task_id: task-472
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

Add a `fetched_at` timestamp field to track when metadata was last fetched from the backend. This is separate from `updated_at` which tracks local modifications.

## Implementation

1. **Create migration** in `src/db/migrations/`:
   - Add `fetched_at: Option<DateTime>` to `media_items` table
   - Default to NULL for existing rows (will be populated on next fetch)

2. **Update entity** in `src/db/entities/media_items.rs`:
   - Add `fetched_at: Option<DateTimeUtc>` field

3. **Update MediaRepository**:
   - Set `fetched_at = now()` when saving items from backend
   - Add method `find_stale_items(ttl: Duration)` to query items older than TTL

4. **Update MediaService**:
   - Update `save_media_item()` and `save_media_items_batch()` to set `fetched_at`

## Files to Modify
- `src/db/migrations/` - new migration file
- `src/db/entities/media_items.rs`
- `src/db/repository/media_repository.rs`
- `src/services/core/media.rs`

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Migration adds `fetched_at` column
- [x] #2 Entity reflects new field
- [x] #3 `fetched_at` is set when saving items from backend sync
- [x] #4 Query method exists to find stale items
<!-- SECTION:DESCRIPTION:END -->

<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Completed: Added fetched_at column via migration m20251209_000001_add_fetched_at.rs, updated media_items entity, repository (insert/update/bulk_insert now set fetched_at), and added find_stale_items method. Also added index for efficient TTL queries. Fixed pre-existing tracing import issues in player modules.
<!-- SECTION:NOTES:END -->
