---
id: task-152
title: Add optional profile name to source display
status: To Do
assignee: []
created_date: '2025-09-17 15:36'
labels:
  - backend
  - database
  - ui
dependencies: []
priority: low
---

## Description

Add a small enhancement to store and display which profile was selected during authentication. This helps users identify which profile a Plex source is connected to without adding complexity to the data model.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Add optional profile_name column to sources table via migration
- [ ] #2 Store selected profile name during source creation
- [ ] #3 Display profile name in sources list UI if available
- [ ] #4 Update source display card to show 'Plex (ProfileName)' format
<!-- AC:END -->
