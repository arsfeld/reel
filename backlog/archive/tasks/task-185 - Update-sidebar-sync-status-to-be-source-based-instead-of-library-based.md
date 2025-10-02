---
id: task-185
title: Update sidebar sync status to be source-based instead of library-based
status: Done
assignee: []
created_date: '2025-09-18 16:46'
updated_date: '2025-10-02 21:32'
labels:
  - ui
  - sidebar
  - sync
dependencies: []
priority: high
---

## Description

The sidebar currently updates sync status on a per-library basis, but it should update on a per-source basis. Additionally, remove the green checkmark indicators and only show error states in the sidebar for a cleaner interface.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Refactor sidebar sync status updates to track at source level instead of library level
- [x] #2 Remove green checkmark indicators from sidebar
- [x] #3 Only display error states in sidebar sync status
- [x] #4 Ensure sync progress and completion update the entire source section
<!-- AC:END -->


## Implementation Notes

# Implementation Review

All acceptance criteria are already implemented:


## AC#1: Source-level tracking ✅
- Connection state managed at SourceGroup level (sidebar.rs:77)
- sync_spinner and connection_icon are per-source, not per-library (sidebar.rs:233-244)

## AC#2: Green checkmarks removed ✅
- connection_icon only visible when state != Connected (sidebar.rs:246)
- #[watch] attribute ensures it hides when Connected

## AC#3: Error-only display ✅
- SyncFailed shows warning icon (dialog-warning-symbolic)
- Disconnected shows error icon (network-offline-symbolic)
- Connected state shows no icon at all

## AC#4: Source section updates ✅
- SourceSyncStarted/SourceSyncCompleted messages handle source-level updates
- Spinner shows during source sync (sidebar.rs:489-516)
- Connection status updates on sync completion (sidebar.rs:953-1049)

Note: Library-level messages still exist but are for granular progress tracking, not main status indicator.
