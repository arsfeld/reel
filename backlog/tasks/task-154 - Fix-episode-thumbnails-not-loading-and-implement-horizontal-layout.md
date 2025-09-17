---
id: task-154
title: Fix episode thumbnails not loading and implement horizontal layout
status: To Do
assignee: []
created_date: '2025-09-17 15:44'
labels:
  - bug
  - ui
  - frontend
dependencies: []
priority: high
---

## Description

Episodes in the TV show details page are not loading their thumbnail images and should be displayed in a horizontal scrollable layout instead of the current vertical list. This affects the visual presentation and usability of the show details page.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Debug why episode thumbnails are not loading from the backend
- [ ] #2 Ensure episode thumbnail URLs are properly fetched and stored
- [ ] #3 Convert episode list from vertical to horizontal scrollable layout
- [ ] #4 Implement lazy loading for episode thumbnails in horizontal scroll
- [ ] #5 Add episode number and title overlay on thumbnails
- [ ] #6 Ensure horizontal scroll works with keyboard navigation
- [ ] #7 Test with shows that have many episodes per season
<!-- AC:END -->
