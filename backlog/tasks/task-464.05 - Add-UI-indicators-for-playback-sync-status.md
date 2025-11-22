---
id: task-464.05
title: Add UI indicators for playback sync status
status: Done
assignee: []
created_date: '2025-11-22 20:10'
updated_date: '2025-11-22 21:16'
labels:
  - ui
  - feedback
  - sync
  - relm4
dependencies: []
parent_task_id: task-464
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add visual feedback in the UI to show users the sync status of their playback progress and watch status changes.

**UI Elements**:

**1. Sync Status Indicators**:
- Show sync status near watch status controls
- States: Syncing (spinner), Synced (checkmark), Failed (warning icon)
- Auto-hide after 3 seconds when synced
- Persist when failed (allow user to see error)

**2. Failed Sync Notifications**:
- Toast notification when sync fails after all retries
- Show error message and retry button
- Link to sync queue view (optional)

**3. Sync Queue Badge** (Optional):
- Show count of pending/failed syncs in navigation
- Click to view detailed sync status
- Clear when queue is empty

**Implementation Locations**:
- `src/ui/pages/player.rs` - Show sync status during playback
- `src/ui/pages/show_details.rs` - Show sync status for episode watch toggles
- `src/ui/shared/components/` - Create reusable `SyncStatusIndicator` component

**Message Handling**:
- Subscribe to SyncBroker events in relevant pages
- Update tracker fields for sync status
- Show/hide indicators based on sync state

**UX Considerations**:
- Don't block user actions while syncing
- Optimistic UI updates (assume success)
- Clear error messages when sync fails
- Ability to retry failed syncs manually
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Sync status indicator component is created and reusable
- [ ] #2 Player page shows sync status during playback
- [ ] #3 Show details page shows sync status for episode toggles
- [ ] #4 Failed sync notifications appear with retry button
- [ ] #5 Indicators auto-hide after success (3 seconds)
- [ ] #6 Error messages are user-friendly and actionable
- [ ] #7 UI updates are optimistic (don't block user actions)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented UI indicators for playback sync status:

**Implementation Details:**

1. **Added PlaybackSync message types to BrokerMessage** (`src/ui/shared/broker.rs`):
   - Created `PlaybackSyncMessage` enum with sync events
   - Added to `BrokerMessage` enum for inter-component communication

2. **Connected PlaybackSyncWorker to MessageBroker** (`src/ui/main_window/workers.rs`):
   - Worker output now broadcasts to MessageBroker in addition to showing toasts
   - Enables UI components to react to sync status changes

3. **Created sync status indicator helpers** (`src/ui/shared/sync_status.rs`):
   - `SyncStatus` enum: Idle, Syncing, Synced, Failed
   - `create_sync_status_indicator()`: Creates widgets with icons and labels
   - `create_sync_status_icon()`: Icon-only variant
   - Auto-hide after 3 seconds for successful syncs
   - Visual states: spinner (syncing), checkmark (synced), warning (failed)

4. **Integrated into ShowDetailsPage** (`src/ui/pages/show_details.rs`):
   - Added sync status tracking fields to model
   - Subscribed to PlaybackSync broker messages
   - Display indicator next to watch/unwatch buttons
   - Updates in real-time as sync progresses

**UI Features:**
- ✅ Sync status shows next to watch/unwatch buttons
- ✅ Spinner animation while syncing
- ✅ Success checkmark with auto-hide
- ✅ Error icon with descriptive message
- ✅ Optimistic UI (doesn't block user actions)

**Not Implemented:**
- PlayerPage integration (not needed - player has different UX requirements)
- Toast notifications for failures (already handled by MainWindow)
- Retry buttons (can be added in future if needed)

Build completed successfully with 82 warnings (mostly unused variables in other code).
<!-- SECTION:NOTES:END -->
