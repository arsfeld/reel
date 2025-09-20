---
id: task-034
title: Add UI feedback for Plex authentication and sync progress
status: Done
assignee:
  - '@claude'
created_date: '2025-09-15 15:35'
updated_date: '2025-09-15 22:55'
labels:
  - ui
  - plex
  - sync
  - feedback
dependencies: []
priority: high
---

## Description

After authenticating with Plex, users receive minimal feedback. Only the Sources page shows a 'Connected' checkmark, but there's no visible sync progress indicator or sidebar updates to reflect the newly available content. Users are left wondering if the sync is happening and when their content will be available.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Display sync progress indicator when Plex sync starts after authentication
- [x] #2 Update sidebar navigation dynamically when new libraries are discovered
- [x] #3 Show notification or toast when Plex authentication completes successfully
- [x] #4 Add sync status indicator in Sources page showing current sync operation
- [x] #5 Refresh UI components (sidebar, homepage) when sync completes
<!-- AC:END -->


## Implementation Plan

1. Add toast/notification system to app.rs for global notifications
2. Implement sync progress overlay component showing current sync status
3. Connect auth dialog completion to toast notifications
4. Enhance Sources page with detailed sync progress indicators
5. Add MessageBroker handlers to sidebar for dynamic library updates
6. Test with Plex authentication and verify all feedback mechanisms work


## Implementation Notes

Implemented toast notifications:
- Added adw::ToastOverlay to MainWindow for global toast display
- Added ShowToast input message to MainWindow
- Modified auth dialog completion to trigger toast notification
- Updated SyncSource handler to show toasts for sync start, completion, and errors
- Toast messages display sync progress with item counts

Dynamic sidebar updates:
- Added MessageBroker subscription to sidebar component
- Sidebar now listens to sync events (SyncStarted, SyncCompleted, SyncError)
- Sidebar refreshes sources automatically when sync completes
- Sync status spinner shows during active syncs

Sync progress indicators:
- Sources page now displays real-time sync progress (e.g., "Syncing 10/100")
- Sync buttons show loading animation during sync
- Progress updates via MessageBroker from sync service

UI components auto-refresh:
- Homepage refreshes via existing MainWindow navigation logic
- Sidebar refreshes when sync completes (RefreshSources triggered)
- Sources page reloads data after sync completion

Implementation complete. All acceptance criteria met:
1. ✅ Sync progress indicators display during Plex sync
2. ✅ Sidebar updates dynamically when libraries are discovered
3. ✅ Toast notifications show on authentication completion
4. ✅ Sources page shows detailed sync status and progress
5. ✅ UI components refresh automatically after sync

The implementation provides comprehensive feedback throughout the authentication and sync process, addressing the original user concern about lack of visibility into sync operations.
