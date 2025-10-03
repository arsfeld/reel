---
id: task-366
title: Add UI warning when using remote Plex connection
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 16:09'
updated_date: '2025-10-03 16:48'
labels:
  - feature
  - ui
  - networking
dependencies: []
priority: medium
---

## Description

When the application falls back to remote (relay/cloud) connections instead of local connections, users should be warned. Remote connections have higher latency, use internet bandwidth, and may not support direct play for large files. Add a toast notification and/or status indicator when remote connections are active.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Detect when active connection is remote vs local
- [x] #2 Show toast notification on first remote connection: 'Using remote connection - local connection unavailable'
- [x] #3 Add visual indicator in UI (sidebar or status bar) showing connection type
- [x] #4 Indicator persists while remote connection is active
- [x] #5 Toast and indicator only appear when remote connection is used
- [x] #6 Users can clearly see when they're on remote connection
<!-- AC:END -->


## Implementation Plan

1. Study ConnectionService to understand where connection type is determined
2. Update ConnectionStatusChanged message to include connection type
3. Add connection type tracking in MainWindow to detect transitions to remote
4. Implement toast notification on first remote connection
5. Add visual indicator in sidebar showing connection type (local/remote/relay)
6. Test with different connection scenarios


## Implementation Notes

Implemented remote/relay connection warning system:

## Changes Made:

### 1. Data Layer
- Added `connection_quality` field to `ConnectionInfo` struct
- Database field `connection_quality` now populated in Source model via `From` conversion

### 2. Connection Type Detection
- Updated `ConnectionMonitorOutput` to include `ConnectionType` enum (Local/Remote/Relay)
- `ConnectionService` stores connection type in cache and database
- `ConnectionMonitor` detects and reports connection types via `ConnectionChanged` and `ConnectionRestored` messages

### 3. Toast Notifications
- MainWindow tracks connection types per source in HashMap
- Shows toast when transitioning from local to remote/relay:
  - Remote: "Using remote connection - Local connection unavailable"
  - Relay: "Using relay connection - Direct connection unavailable"
- No toast for local connections (expected good state)

### 4. Visual Warning Indicator
- Warning icon (dialog-warning-symbolic) appears next to source name in sidebar
- Only shown for remote/relay connections (not local)
- Tooltip explains the issue:
  - Remote: "Using remote connection - Local connection unavailable. This may have higher latency."
  - Relay: "Using relay connection - Direct connection unavailable. This will have higher latency and limited bandwidth."
- Icon persists while remote/relay connection is active

### 5. Architecture
- ConnectionMonitor (worker) checks connections and emits status with type
- MainWindow receives status and forwards to sidebar via UpdateSourceConnectionStatus
- Sidebar loads initial connection_quality from database on startup
- SourceGroup manually updates warning icon widget (factory components dont support #[watch])

## Files Modified:
- src/workers/connection_monitor.rs
- src/services/core/mod.rs
- src/services/core/connection_cache.rs
- src/models/auth_provider.rs
- src/ui/main_window/mod.rs
- src/ui/main_window/workers.rs
- src/ui/sidebar.rs
- src/services/core/auth.rs
- src/services/core/backend.rs
