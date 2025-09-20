---
id: task-133
title: Display proper sync status in UI
status: Done
assignee:
  - '@claude'
created_date: '2025-09-17 03:18'
updated_date: '2025-09-17 03:25'
labels: []
dependencies: []
priority: high
---

## Description

The application currently lacks clear visibility of synchronization status for media sources. Users need to see when sources are syncing, what's being synced, progress indicators, and any sync errors or completion states. This information should be displayed in an appropriate location in the UI.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Sync status indicator shows current sync state (idle, syncing, error, complete)
- [x] #2 Progress information displayed during active sync operations
- [x] #3 Source-specific sync status visible for multi-backend setups
- [x] #4 Error messages displayed when sync failures occur
- [x] #5 Last successful sync timestamp shown for each source
<!-- AC:END -->


## Implementation Plan

1. Analyze existing sync status infrastructure and database schema
2. Identify UI components and broker message flow for sync events
3. Enhance SourceListItem component to display sync status indicators
4. Add progress bars for active sync operations
5. Display error messages when sync fails
6. Show last sync timestamp for each source
7. Connect broker messages to UI updates for real-time status
8. Test with multiple backends to ensure proper status display


## Implementation Notes

Enhanced the Sources page to display comprehensive sync status information for all media backends.


## Changes Made:

1. **Enhanced SourceListItem Component**:
   - Added sync_error and last_sync_status fields to track sync state
   - Created a new sync status section in the UI with connection and sync status indicators
   - Added dynamic text showing: "Syncing... X/Y", "Sync failed", "Xm/h/d ago", or "Never synced"
   - Implemented a progress bar that appears during sync operations showing completion percentage

2. **Updated Broker Message Handling**:
   - Connected SyncStarted events to clear errors and set syncing state
   - Connected SyncProgress events to update the progress bar
   - Connected SyncCompleted events to update last_sync time and status
   - Connected SyncError events to display error messages and set failed state

3. **UI Improvements**:
   - Progress bar with OSD styling appears below sync status text during operations
   - Error tooltips show detailed error messages on hover
   - Relative time display (minutes/hours/days) for last sync timestamp
   - Visual indicators change based on sync state (syncing animation, error state)

4. **Integration with Existing Infrastructure**:
   - Leverages existing MessageBroker system for real-time updates
   - Uses existing sync_status database table via SyncRepository
   - Works with all backend types (Plex, Jellyfin, Local)

The implementation provides users with clear visibility of:
- Current sync state (idle, syncing, error, complete)
- Progress during sync operations (current/total items)
- Error messages when sync fails
- Last successful sync timestamp for each source
- Source-specific status in multi-backend setups
