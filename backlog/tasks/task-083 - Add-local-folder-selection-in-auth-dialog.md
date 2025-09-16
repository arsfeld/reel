---
id: task-083
title: Add local folder selection in auth dialog
status: To Do
assignee: []
created_date: '2025-09-16 17:40'
updated_date: '2025-09-16 17:51'
labels:
  - ui
  - auth
  - local-files
dependencies: []
priority: medium
---

## Description

Enable users to add local media folders through the existing auth dialog. Add a simple folder picker to the Local Files tab for directory selection.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Add folder selection button to Local Files auth tab
- [ ] #2 Use native file dialog to select directories
- [ ] #3 Store selected path in SourceType::LocalFolder
- [ ] #4 Display selected folder path in the UI
- [ ] #5 Allow adding folder without authentication (local doesn't need auth)
<!-- AC:END -->
