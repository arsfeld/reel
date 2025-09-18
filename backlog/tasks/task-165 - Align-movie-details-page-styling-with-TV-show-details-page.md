---
id: task-165
title: Align movie details page styling with TV show details page
status: Done
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
- [x] #3 Movie overview text has same typography and spacing as TV shows
- [x] #4 Movie metadata (year, rating, duration) follows TV show layout pattern
- [x] #5 Both pages use consistent spacing and visual hierarchy
- [x] #6 CSS styles are shared between both pages where appropriate
<!-- AC:END -->


## Implementation Plan

1. Examine TV show details page layout and styling
2. Examine movie details page current implementation
3. Identify styling differences between the two pages
4. Extract common styling patterns to shared CSS classes
5. Update movie details page to match TV show layout
6. Test both pages for visual consistency


## Implementation Notes

Aligned movie details page styling with TV show details page for visual consistency:

- Changed hero section height from 600px to 480px to match TV shows
- Updated poster CSS class from poster-premium to poster-styled (shared class)
- Adjusted margins to match TV shows (24px with 16px bottom margin)
- Moved overview text from separate content section to hero section below title
- Added overview-hero CSS class for consistent typography
- Set same text properties for overview (wrap mode, max chars, ellipsize, 3 lines)
- Removed redundant overview section from content area
- Reduced content section top margin from 32px to 12px
- Updated all spacing values to match TV show page exactly

Both pages now share the same visual hierarchy and layout patterns for a unified experience.
