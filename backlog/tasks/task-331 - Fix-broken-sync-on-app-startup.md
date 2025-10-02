---
id: task-331
title: Fix broken sync on app startup
status: Done
assignee:
  - '@claude'
created_date: '2025-10-02 18:02'
updated_date: '2025-10-02 18:09'
labels:
  - bug
  - sync
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Sync functionality that used to run automatically on app startup is no longer working. The application starts but no synchronization occurs with media backends.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Sync is triggered automatically on application startup
- [x] #2 Sync completes successfully and updates the database
- [x] #3 Logs confirm sync operations are running
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Investigate the init_sync flow in main_window.rs
2. Check why sources are not being synced on startup
3. Add sync trigger for all sources during init_sync
4. Test the fix by running the app and checking logs
5. Verify sync completes successfully
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed broken sync on app startup by modifying the init_sync handler in main_window.rs (lines 697-767).

Root cause: The init_sync flow was loading sources but never triggering the actual sync operation. It would navigate to home but the SyncWorker was never invoked.

Solution: Added code to directly send StartSync messages to the SyncWorker for each source after they are loaded during initialization. This ensures all sources are synced automatically when the app starts.

Changes:
- Modified src/ui/main_window.rs init_sync handler to iterate through loaded sources
- For each source, send SyncWorkerInput::StartSync message directly to the sync_worker
- Added proper error handling for sync command failures
- Maintained existing navigation to home and sidebar refresh logic
<!-- SECTION:NOTES:END -->
