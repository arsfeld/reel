---
id: task-038
title: Fix poster size consistency in library views to prevent size jumps
status: To Do
assignee: []
created_date: '2025-09-15 22:15'
labels:
  - ui
  - library
  - performance
  - ux
dependencies: []
priority: high
---

## Description

Library posters currently display at a very small size initially, then jump to a much larger size when the actual image loads. This creates a jarring visual experience and layout shift. Posters should maintain a consistent, medium size throughout the loading process, with proper aspect ratio reservation to prevent layout shifts.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Set fixed dimensions for poster containers that don't change when images load
- [ ] #2 Implement proper aspect ratio preservation for movie/show posters
- [ ] #3 Add placeholder styling that matches final poster dimensions
- [ ] #4 Ensure smooth transition when actual image replaces placeholder
- [ ] #5 Test with slow network to verify no size jumps occur during loading
- [ ] #6 Verify consistent poster sizes across all library views (movies, shows, etc.)
<!-- AC:END -->
