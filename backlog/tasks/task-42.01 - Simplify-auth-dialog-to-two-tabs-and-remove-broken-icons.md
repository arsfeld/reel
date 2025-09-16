---
id: task-42.01
title: Simplify auth dialog to two tabs and remove broken icons
status: To Do
assignee: []
created_date: '2025-09-16 00:40'
labels:
  - ui
  - auth
  - refactor
dependencies: []
parent_task_id: task-42
priority: high
---

## Description

The auth dialog currently has too many tabs making it difficult to navigate. Simplify to just two tabs (Plex and Jellyfin) and remove the broken/unnecessary icons from tabs. Manual input options should be moved to advanced sections within each service tab rather than separate tabs.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Reduce tabs to only Plex and Jellyfin (remove separate manual tabs)
- [ ] #2 Remove broken icons from tab headers
- [ ] #3 Move manual URL input to expandable advanced section in Plex tab
- [ ] #4 Move manual server input to expandable advanced section in Jellyfin tab
- [ ] #5 Ensure tab navigation is smooth and responsive
<!-- AC:END -->
