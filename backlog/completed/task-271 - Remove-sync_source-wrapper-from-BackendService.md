---
id: task-271
title: Remove sync_source wrapper from BackendService
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 18:42'
updated_date: '2025-09-26 19:07'
labels:
  - backend
  - refactoring
  - cleanup
dependencies: []
---

## Description

BackendService::sync_source is just a thin wrapper around SyncService::sync_source. Remove this unnecessary indirection and have callers use SyncService directly. This continues the cleanup of BackendService to be purely about backend creation and API operations.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Remove sync_source method from BackendService completely
- [x] #2 Update sync_worker.rs to call SyncService::sync_source directly
- [x] #3 Pass required parameters (db, backend, source_id) to SyncService
- [x] #4 Remove any other references to BackendService::sync_source
- [x] #5 Ensure no dead code or backward compatibility wrappers remain
<!-- AC:END -->


## Implementation Plan

1. Study the current BackendService::sync_source implementation
2. Check sync_worker.rs to understand how it calls BackendService::sync_source
3. Understand SyncService::sync_source signature and requirements
4. Update sync_worker.rs to call SyncService directly
5. Remove sync_source method from BackendService
6. Check for any other references and clean them up
7. Test the changes compile and work correctly


## Implementation Notes

Successfully removed the unnecessary sync_source wrapper from BackendService. The method was just passing through to SyncService::sync_source.

Changes made:
1. Updated sync_worker.rs to create the backend using BackendService::create_backend_for_source and then call SyncService::sync_source directly
2. Removed the sync_source method from BackendService in backend.rs
3. Verified no other code references exist (only documentation references remain)
4. Code compiles successfully

This completes the cleanup of BackendService to focus purely on backend creation and API operations, with SyncService handling all synchronization logic directly.
