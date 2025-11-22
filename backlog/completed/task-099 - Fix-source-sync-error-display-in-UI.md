---
id: task-099
title: Fix source sync error display in UI
status: Done
assignee: []
created_date: '2025-09-16 19:34'
updated_date: '2025-10-02 14:54'
labels: []
dependencies: []
priority: high
---

## Description

The sources page UI does not display sync errors or connection failures to users. When a source fails to sync (e.g., 404 errors, connection failures), the errors are logged but the UI remains unchanged, giving no visual feedback about the failure. Users need to know when their sources are having issues.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Add error state display to source list items in sources page
- [ ] #2 Show sync error messages in the UI when sync fails
- [ ] #3 Display connection status indicators (connected/disconnected/error)
- [ ] #4 Add retry button or action for failed syncs
- [ ] #5 Persist error state until successfully resolved or manually dismissed
- [ ] #6 Test with various failure scenarios (404, network timeout, auth failure)
<!-- AC:END -->
