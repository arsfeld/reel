---
id: task-095
title: Add dynamic HomeSection rendering for all Plex/Jellyfin section types
status: Done
assignee: []
created_date: '2025-09-16 19:29'
updated_date: '2025-10-02 14:53'
labels:
  - home
  - ui
  - sections
  - critical
dependencies: []
---

## Description

The HomePage UI is hardcoded to only show Continue Watching and Recently Added sections. It needs to dynamically render any number of sections returned by the backend's get_home_sections() API, including library-specific collections like Top Rated, Trending, Popular, etc.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 HomePage component dynamically creates section UI based on HomeSection data
- [ ] #2 Each section displays with proper title and type-specific styling
- [ ] #3 Sections support different HomeSectionType variants (TopRated, Trending, Custom, etc.)
- [ ] #4 Empty sections are hidden from the UI
- [ ] #5 Section order matches the backend priority/order
- [ ] #6 Each section has proper horizontal scrolling with appropriate media card display
<!-- AC:END -->
