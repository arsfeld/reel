---
id: task-120
title: Add predefined filter tabs to library view
status: To Do
assignee: []
created_date: '2025-09-17 02:50'
labels:
  - ui
  - filtering
  - tabs
dependencies: []
priority: high
---

## Description

Implement horizontal filter tabs (All, Unwatched, Recently Added, Genres, Years) at the top of library views for quick content filtering without server queries

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Five filter tabs render horizontally: All, Unwatched, Recently Added, Genres, Years
- [ ] #2 All tab shows complete library content (default state)
- [ ] #3 Unwatched tab filters to show only unwatched content
- [ ] #4 Recently Added tab shows content from last 30 days
- [ ] #5 Genres and Years tabs show placeholder UI for future implementation
- [ ] #6 Selected tab state persists per library in user preferences
- [ ] #7 Tab switching is instant (<100ms) using cached data
- [ ] #8 Tabs work in combination with existing sort options
<!-- AC:END -->
