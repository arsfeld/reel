---
id: task-102
title: Fix Continue Watching section missing items
status: Done
assignee: []
created_date: '2025-09-16 19:36'
updated_date: '2025-10-02 14:54'
labels:
  - bug
  - ui
dependencies: []
priority: high
---

## Description

The Continue Watching section is not showing all items that should be there. Items with in-progress playback are missing or not being properly filtered/sorted.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Continue Watching shows all items with playback progress > 0 and < 90%
- [ ] #2 Items are sorted by last watched date (most recent first)
- [ ] #3 Both movies and TV episodes appear in Continue Watching
- [ ] #4 Playback progress is accurately reflected
<!-- AC:END -->
