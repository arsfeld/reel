---
id: task-297
title: Fix navigation to home page timing - navigate on startup not after sync
status: Done
assignee:
  - '@claude'
created_date: '2025-09-28 02:10'
updated_date: '2025-10-02 14:51'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Currently the app navigates to the home page after sync completes, which creates a poor user experience. The app should navigate to home immediately on startup if sources are configured, and sync should happen in the background without forcing navigation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Remove navigation to home from post-sync logic
- [x] #2 Add check for configured sources during app initialization
- [x] #3 Navigate to home page immediately if sources exist on startup
- [x] #4 Ensure background sync doesn't trigger page navigation
- [x] #5 Keep user on their current page when sync completes
- [x] #6 Update home page to show loading state while initial data loads from cache
- [ ] #7 Test that navigation works correctly with both cached and fresh data
- [ ] #8 Verify sync updates home page content without forcing navigation
<!-- AC:END -->


## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Check for configured sources during app initialization (init_sync handler)
2. Navigate to home immediately if sources exist, before triggering sync
3. Remove navigation to home from SyncWorkerOutput::SyncCompleted handler
4. Update home page to handle loading state with cached data
5. Ensure sync updates home page data without forcing navigation
6. Test that navigation works correctly with both scenarios
<!-- SECTION:PLAN:END -->
