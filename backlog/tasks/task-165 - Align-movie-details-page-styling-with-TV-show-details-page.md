---
id: task-165
title: Align movie details page styling with TV show details page
status: In Progress
assignee:
  - '@claude'
created_date: '2025-09-18 02:33'
updated_date: '2025-09-18 02:38'
labels:
  - ui
  - movie
  - consistency
dependencies: []
priority: high
---

## Description

The movie details page needs to match the TV show details page in terms of visual styling and layout. This includes proper poster positioning, backdrop display, and overall visual consistency between the two pages. The TV show details page has a better layout that should be applied to movies for a unified experience.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Movie poster displays with same size and positioning as TV show poster
- [x] #2 Movie backdrop image displays properly behind content like TV shows
- [ ] #3 Movie overview text has same typography and spacing as TV shows
- [x] #4 Movie metadata (year, rating, duration) follows TV show layout pattern
- [x] #5 Both pages use consistent spacing and visual hierarchy
- [ ] #6 CSS styles are shared between both pages where appropriate
<!-- AC:END -->


## Implementation Plan

1. Examine TV show details page layout and styling
2. Examine movie details page current implementation
3. Identify styling differences between the two pages
4. Extract common styling patterns to shared CSS classes
5. Update movie details page to match TV show layout
6. Test both pages for visual consistency
