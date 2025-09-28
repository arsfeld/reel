---
id: task-268
title: Add home sections to sync_worker.rs sync flow
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 18:05'
updated_date: '2025-09-26 18:45'
labels:
  - backend
  - sync
dependencies:
  - task-270
---

## Description

Integrate home sections fetching into the sync worker's perform_sync method. This ensures home sections are synced alongside libraries and media items during scheduled syncs, following proper offline-first architecture where sync happens in background workers, not UI.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 In perform_sync, fetch home sections after syncing libraries
- [x] #2 Call backend.get_home_sections() to fetch from API
- [x] #3 Save sections using HomeSectionRepository with transactions
- [x] #4 Add home_sections_synced to SyncProgress notifications
- [x] #5 Include sections count in SyncResult
- [x] #6 Send UI update notification when sections are refreshed
<!-- AC:END -->


## Implementation Plan

1. Analyze current sync worker flow and BackendService::sync_source method
2. Examine how home sections are fetched via backend.get_home_sections()
3. Update SyncProgress struct to include home_sections_synced field
4. Update SyncWorkerOutput to include sections count in SyncCompleted
5. Modify perform_sync method to fetch and save home sections after libraries
6. Add proper notification for UI updates when sections are refreshed
7. Test the integration end-to-end

## Implementation Notes

Implemented home sections synchronization in sync_worker.rs:

- Added sync_home_sections() method that fetches sections via backend.get_home_sections()
- Integrated section syncing into perform_sync() after library/media sync completes  
- Used HomeSectionRepository with transactions to save sections and their items atomically
- Updated SyncProgress struct to include home_sections_synced boolean flag
- Modified SyncWorkerOutput::SyncCompleted to include sections_synced count
- Updated main_window.rs to display sections count in sync completion toast
- Added automatic home page refresh when sections are synced (navigates to home)
- Properly converts HomeSectionType enum values to database string format
- Handles section item relationships by converting to database IDs

The implementation follows the offline-first pattern where sync happens in background workers and UI is notified of updates.
