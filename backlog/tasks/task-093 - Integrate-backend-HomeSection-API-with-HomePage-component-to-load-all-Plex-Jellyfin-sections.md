---
id: task-093
title: >-
  Integrate backend HomeSection API with HomePage component to load all
  Plex/Jellyfin sections
status: In Progress
assignee:
  - '@claude'
created_date: '2025-09-16 19:29'
updated_date: '2025-09-16 19:30'
labels:
  - home
  - plex
  - jellyfin
  - critical
dependencies: []
---

## Description

The HomePage component currently only loads Continue Watching and Recently Added from the database, but ignores the rich HomeSection API that Plex and Jellyfin backends provide. This causes most home sections to be missing from the UI.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 HomePage calls get_home_sections() from active backends on load
- [ ] #2 All returned home sections are displayed dynamically in the UI
- [ ] #3 Each section shows the correct title from the backend
- [ ] #4 Sections properly handle empty results and error states
- [ ] #5 Multiple backend sections are merged properly without conflicts
<!-- AC:END -->

## Implementation Plan

1. Research how to access backend services from HomePage component
2. Identify the proper service/coordinator to get active backends
3. Modify HomePage to call get_home_sections() from backends
4. Update HomePage data structures to handle dynamic sections
5. Test with both Plex and Jellyfin backends
6. Verify section merging works correctly with multiple backends
