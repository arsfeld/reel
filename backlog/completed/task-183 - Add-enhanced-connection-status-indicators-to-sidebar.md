---
id: task-183
title: Add enhanced connection status indicators to sidebar
status: Done
assignee:
  - '@claude'
created_date: '2025-09-18 15:43'
updated_date: '2025-09-18 15:43'
labels:
  - enhancement
  - ui
dependencies: []
---

## Description

The sidebar now shows three different states for each source: green checkmark for connected and synced, warning symbol for connected but sync failed, and red X for disconnected/never connected. This provides clearer visual feedback about the actual state of each media source.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 ConnectionState enum with three states (Connected, SyncFailed, Disconnected)
- [x] #2 Sidebar SourceGroup tracks and displays appropriate icon based on state
- [x] #3 Sync errors update connection state to SyncFailed with warning icon
- [x] #4 Successful sync updates connection state to Connected with green checkmark
- [x] #5 Disconnected sources show red offline icon
<!-- AC:END -->

## Implementation Notes

Implemented a three-state connection indicator system in the sidebar:

1. **ConnectionState Enum**: Created a new enum with three states:
   - Connected: Source is reachable and sync works
   - SyncFailed: Source is reachable but sync failed (permissions, API issues, etc.)
   - Disconnected: Source is not reachable

2. **Visual Indicators**:
   - Green checkmark (emblem-ok-symbolic) with "success" CSS class for Connected
   - Warning icon (dialog-warning-symbolic) with "warning" CSS class for SyncFailed  
   - Red offline icon (network-offline-symbolic) with "error" CSS class for Disconnected

3. **State Management**:
   - Sync completion sets state to Connected
   - Sync errors set state to SyncFailed
   - Connection monitor updates set state based on connectivity

4. **Files Modified**:
   - src/ui/sidebar.rs: Added ConnectionState enum and updated SourceGroup component
   - src/ui/main_window.rs: Updated to use ConnectionState when handling connection changes
   - src/services/core/sync.rs: Fixed missing update_last_sync() call after successful sync
