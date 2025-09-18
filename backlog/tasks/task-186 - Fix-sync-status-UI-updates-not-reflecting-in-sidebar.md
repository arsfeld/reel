---
id: task-186
title: Fix sync status UI updates not reflecting in sidebar
status: In Progress
assignee:
  - '@claude'
created_date: '2025-09-18 16:52'
updated_date: '2025-09-18 19:49'
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
- [ ] #1 Ensure UI updates when connection state changes from Connected to SyncFailed
- [ ] #2 Fix the factory component view refresh mechanism for state changes
- [ ] #3 Verify warning icon appears immediately after sync failure
- [ ] #4 Test that all three states (Connected, SyncFailed, Disconnected) update visually
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Analyze why #[watch] attributes don't trigger view updates in factory components
2. Check if update_with_view needs to manually trigger view refresh
3. Implement proper view update mechanism after state changes
4. Test all connection states update visually in sidebar
<!-- SECTION:PLAN:END -->
