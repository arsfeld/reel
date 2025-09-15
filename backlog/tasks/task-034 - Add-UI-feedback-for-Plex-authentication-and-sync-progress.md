---
id: task-034
title: Add UI feedback for Plex authentication and sync progress
status: To Do
assignee: []
created_date: '2025-09-15 15:35'
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
- [ ] #1 Display sync progress indicator when Plex sync starts after authentication
- [ ] #2 Update sidebar navigation dynamically when new libraries are discovered
- [ ] #3 Show notification or toast when Plex authentication completes successfully
- [ ] #4 Add sync status indicator in Sources page showing current sync operation
- [ ] #5 Refresh UI components (sidebar, homepage) when sync completes
<!-- AC:END -->
