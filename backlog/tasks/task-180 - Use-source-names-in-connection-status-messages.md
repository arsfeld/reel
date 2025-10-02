---
id: task-180
title: Use source names in connection status messages
status: To Do
assignee: []
created_date: '2025-09-18 15:18'
updated_date: '2025-10-02 14:56'
labels:
  - ui
  - enhancement
dependencies: []
priority: medium
---

## Description

Connection status messages currently show source IDs like 'Source plex_123456 connected' which is not user-friendly. Should display the actual source name like 'Plex Server connected' instead.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Modify ConnectionStatusChanged handler to lookup source name from database
- [ ] #2 Pass source name along with status updates to UI components
- [ ] #3 Update status message formatting to use source names
- [ ] #4 Ensure source name is available in all connection status contexts
<!-- AC:END -->
