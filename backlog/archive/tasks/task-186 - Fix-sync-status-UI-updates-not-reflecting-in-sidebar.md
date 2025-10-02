---
id: task-186
title: Fix sync status UI updates not reflecting in sidebar
status: Done
assignee:
  - ''
created_date: '2025-09-18 16:52'
updated_date: '2025-10-02 21:34'
labels:
  - bug
  - ui
  - critical
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When a sync fails (e.g., Jellyfin with 404 error), the sidebar receives the SyncError message and updates its internal connection_state to SyncFailed, but the UI doesn't visually update to show the warning icon. The issue is that SourceGroup uses update_with_view which doesn't trigger automatic view refreshes with #[watch] attributes. Need to implement proper view updates when connection state changes.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Ensure UI updates when connection state changes from Connected to SyncFailed
- [x] #2 Fix the factory component view refresh mechanism for state changes
- [x] #3 Verify warning icon appears immediately after sync failure
- [x] #4 Test that all three states (Connected, SyncFailed, Disconnected) update visually
<!-- AC:END -->


## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Analyze why #[watch] attributes don't trigger view updates in factory components
2. Check if update_with_view needs to manually trigger view refresh
3. Implement proper view update mechanism after state changes
4. Test all connection states update visually in sidebar
<!-- SECTION:PLAN:END -->


## Implementation Notes

# Implementation Complete

Fixed in commit 979784a (2025-09-23) - "resolve database constraints and improve sync reliability"


## Solution: Manual Widget Updates

The issue was that #[watch] attributes do not trigger automatic view updates in Relm4 factory components. The fix was to add manual widget updates in SourceGroupInput handlers:

### SourceSyncStarted (sidebar.rs:489-503)
```rust
self.is_syncing = true;
// Manually update widgets since #[watch] doesn't work in factory components\nwidgets.sync_spinner.set_visible(true);\nwidgets.sync_spinner.set_spinning(true);\nwidgets.connection_icon.set_visible(false);\n```\n\n### SourceSyncCompleted (sidebar.rs:504-517)\n```rust\nself.is_syncing = false;\nwidgets.sync_spinner.set_visible(false);\nwidgets.sync_spinner.set_spinning(false);\nlet should_show = self.connection_state \!= ConnectionState::Connected;\nwidgets.connection_icon.set_visible(should_show);\n```\n\n### UpdateConnectionStatus (sidebar.rs:425-487)\n```rust\nlet should_show = state \!= ConnectionState::Connected && \!self.is_syncing;\nwidgets.connection_icon.set_visible(should_show);\nwidgets.connection_icon.set_icon_name(Some(icon_name));\nwidgets.connection_icon.set_css_classes(&[css_class]);\nwidgets.connection_icon.set_tooltip_text(Some(&tooltip));\n```\n\n## Verification\nAll acceptance criteria are met:\n- ✅ AC#1: UI updates on state changes (manual updates in UpdateConnectionStatus)\n- ✅ AC#2: Factory view refresh works (bypassed #[watch] with manual updates)\n- ✅ AC#3: Warning icon appears on sync failure (UpdateConnectionStatus handles it)\n- ✅ AC#4: All states update visually (Connected, SyncFailed, Disconnected)
