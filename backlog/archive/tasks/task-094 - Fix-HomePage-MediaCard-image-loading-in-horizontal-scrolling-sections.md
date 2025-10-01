---
id: task-094
title: Fix HomePage MediaCard image loading in horizontal scrolling sections
status: To Do
assignee: []
created_date: '2025-09-16 19:29'
updated_date: '2025-09-30 00:37'
labels:
  - home
  - images
  - ui
  - critical
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Images are not loading properly in the HomePage horizontal scrolling sections (Continue Watching and Recently Added). The existing ImageLoader worker is not being connected to the MediaCard factories in the HomePage component.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 MediaCard components in HomePage request images from ImageLoader worker
- [ ] #2 Images load progressively as cards become visible
- [ ] #3 Failed image loads show appropriate fallback placeholders
- [ ] #4 Image loading prioritizes visible cards over off-screen cards
- [ ] #5 Horizontal scrolling doesn't block or interfere with image loading
<!-- AC:END -->
