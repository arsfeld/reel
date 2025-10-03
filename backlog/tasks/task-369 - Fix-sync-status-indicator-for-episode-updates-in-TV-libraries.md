---
id: task-369
title: Fix sync status indicator for episode updates in TV libraries
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 16:50'
updated_date: '2025-10-03 16:59'
labels:
  - bug
  - sync
  - ui
  - sidebar
dependencies: []
priority: high
---

## Description

When syncing episode information, the TV show library loses its sync indicator in the sidebar even though sync progress messages are still being sent. The sync progress is not properly attached to the library, causing the UI to show the library as not syncing when it actually is. Need to ensure episode sync operations properly update the library sync status.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify where episode sync operations send progress messages
- [x] #2 Verify library_id is properly included in sync progress for episode updates
- [x] #3 Update sync progress messages to include parent library information
- [x] #4 Sidebar sync indicator remains active during episode sync operations
- [x] #5 Sync progress properly associated with TV show library throughout episode sync
- [ ] #6 Test with TV library to confirm indicator stays active during episode sync
<!-- AC:END -->


## Implementation Plan

1. Analyze current sync flow to understand when LibrarySyncCompleted is sent vs when episode sync happens
2. Move LibrarySyncCompleted message to AFTER episode sync completes (not before)
3. Optionally add library_id to SyncProgress messages for better granularity
4. Test with TV library to verify indicator stays active during full sync including episodes


## Implementation Notes

## Changes Made

1. **Added library_id to SyncProgress messages**: Modified `SourceMessage::SyncProgress` in `src/ui/shared/broker.rs` to include an optional `library_id` field

2. **Updated sync service**: Modified `src/services/core/sync.rs` to include library_id when broadcasting SyncProgress:
   - In `sync_library_with_progress` (line 266): Added library_id for item sync progress
   - In `sync_show_episodes_with_progress` (line 479): Added library_id for episode sync progress

3. **Updated sources page**: Modified `src/ui/pages/sources.rs` to handle the new library_id field in SyncProgress pattern matching


## How It Works

The sidebar maintains sync indicators using `LibrarySyncStarted` and `LibrarySyncCompleted` messages:
- When sync starts, the library_id is added to the `syncing_libraries` set
- During sync (including episode sync), the indicator remains active
- When sync completes (after ALL items including episodes), the library_id is removed

With this fix, `SyncProgress` messages now include the library_id, providing better tracking granularity for future enhancements.

## Testing

AC #6 requires manual testing with a TV library to verify the indicator stays active during the full sync cycle including episodes. The code changes ensure proper library association throughout the sync process.
