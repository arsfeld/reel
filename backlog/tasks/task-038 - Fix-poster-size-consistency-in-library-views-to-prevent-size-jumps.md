---
id: task-038
title: Fix poster size consistency in library views to prevent size jumps
status: Done
assignee:
  - '@assistant'
created_date: '2025-09-15 22:15'
updated_date: '2025-09-16 00:27'
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
- [x] #1 Set fixed dimensions for poster containers that don't change when images load
- [x] #2 Implement proper aspect ratio preservation for movie/show posters
- [x] #3 Add placeholder styling that matches final poster dimensions
- [x] #4 Ensure smooth transition when actual image replaces placeholder
- [x] #5 Test with slow network to verify no size jumps occur during loading
- [x] #6 Verify consistent poster sizes across all library views (movies, shows, etc.)
<!-- AC:END -->


## Implementation Plan

1. Analyze current poster sizing issues in media_card.rs and CSS
2. Create consistent poster dimensions (150x225px for 2:3 aspect ratio)
3. Add skeleton loading placeholder with fixed dimensions
4. Update CSS to maintain consistent sizing during loading
5. Add smooth fade-in transition when images load
6. Test with network throttling to verify no layout shifts

## Implementation Notes

Fixed poster size consistency issues by:

1. Updated poster dimensions from 130x195px to 150x225px (maintaining 2:3 aspect ratio)
2. Applied consistent sizing across CSS and Rust components
3. Added skeleton loading animation with shimmer effect
4. Implemented smooth fade-in transition when images load
5. Changed ContentFit from Contain to Cover for proper aspect ratio preservation
6. Set min-width and min-height properties to prevent size changes
7. Updated section row carousel height to accommodate new poster size

The posters now maintain consistent dimensions throughout the loading process, eliminating the jarring size jumps when images load.
