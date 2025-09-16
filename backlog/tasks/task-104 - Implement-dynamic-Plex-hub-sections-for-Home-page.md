---
id: task-104
title: Implement dynamic Plex hub sections for Home page
status: To Do
assignee: []
created_date: '2025-09-16 19:37'
labels:
  - feature
  - ui
  - plex
dependencies: []
priority: high
---

## Description

Replace hardcoded Home page sections with dynamic hub data from Plex API. Plex provides various hubs like 'Recently Added Movies', 'Popular This Week', 'Top Rated', etc. that should be displayed dynamically based on what the server provides.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Fetch hub data from Plex /hubs/home/refresh endpoint
- [ ] #2 Dynamically create sections based on hub response
- [ ] #3 Support all Plex hub types (movie, show, episode, mixed)
- [ ] #4 Handle hub-specific layouts (hero, shelf, grid)
- [ ] #5 Respect hub size limits and sorting
<!-- AC:END -->
