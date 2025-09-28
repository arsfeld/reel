---
id: task-219
title: Add spinner to sidebar sources during sync
status: Done
assignee:
  - '@claude'
created_date: '2025-09-22 18:22'
updated_date: '2025-09-22 18:29'
labels:
  - ui
  - sync
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Display a loading spinner indicator next to each source in the sidebar while that source is actively syncing to provide visual feedback to users
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Spinner displays next to source name when sync is active
- [x] #2 Spinner is hidden when sync is not active
- [x] #3 Spinner updates reactively based on sync state changes
- [x] #4 Visual indicator is non-intrusive and follows GNOME HIG
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Examine sidebar component and sync state management
2. Add is_syncing field to SourceGroup struct
3. Add sync state input messages (SourceSyncStarted/SourceSyncCompleted)
4. Add spinner widget to source header with visibility controlled by is_syncing
5. Connect broker sync messages to source sync state
6. Test with actual sync operations
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

Added spinner indicator to source headers in the sidebar that displays during sync operations.

### Changes Made:

1. **Added is_syncing field to SourceGroup struct** - Tracks whether the source is currently syncing
2. **Added new input messages** - SourceSyncStarted and SourceSyncCompleted to manage sync state
3. **Added GTK Spinner widget** - Positioned next to source name, visible only during sync
4. **Updated connection icon visibility** - Now hidden when syncing (spinner takes precedence)
5. **Connected broker messages** - SyncStarted, SyncCompleted, and SyncError messages now update source sync state

### Technical Details:

- The spinner is controlled reactively using Relm4's `#[watch]` attribute
- Sync state is managed per-source through the SourceGroup factory component
- When sync starts, the spinner appears and connection icon is hidden
- When sync completes (successfully or with error), spinner disappears and connection state is restored
- Visual design follows GNOME HIG with non-intrusive spinner placement

### File Modified:
- `src/ui/sidebar.rs` - Added sync state management and spinner widget to source headers
<!-- SECTION:NOTES:END -->
