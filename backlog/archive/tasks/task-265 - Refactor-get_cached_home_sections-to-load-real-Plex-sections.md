---
id: task-265
title: Refactor get_cached_home_sections to load real Plex sections
status: To Do
assignee: []
created_date: '2025-09-26 17:49'
updated_date: '2025-09-26 18:05'
labels: []
dependencies:
  - task-264
---

## Description

Replace the current hardcoded section generation in get_cached_home_sections with actual loading of Plex home sections from the database. This provides true offline-first functionality with real Plex data.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Remove all hardcoded section creation (Continue Watching, Recently Added, etc.)
- [ ] #2 Use HomeSectionRepository to load saved sections from database
- [ ] #3 Maintain the same return type for compatibility
- [ ] #4 Load section items with proper relationships
- [ ] #5 Handle empty database gracefully (first run scenario)
- [ ] #6 Preserve section ordering from Plex
<!-- AC:END -->
