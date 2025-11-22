---
id: task-376
title: Display cast information in show details page
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 17:09'
updated_date: '2025-10-03 17:42'
labels:
  - ui
  - show-details
  - frontend
dependencies: []
priority: medium
---

## Description

Update show details page UI to display cast information. Show series regulars and recurring cast members with their character names. Include profile images where available.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add cast section to show details page layout
- [x] #2 Display actors with character names
- [x] #3 Show person profile images
- [x] #4 Handle missing profile images gracefully
- [x] #5 Test with shows that have cast data
<!-- AC:END -->


## Implementation Plan

1. Review existing cast display in movie_details.rs
2. Add cast_box field to ShowDetailsPage struct
3. Initialize cast_box in init method
4. Add cast section to view! macro
5. Populate cast_box in LoadDetails command
6. Build and test with show data

## Implementation Notes

Added cast display to show details page using shared person_card component.

Implementation:
- Added cast_box field to ShowDetailsPage struct
- Initialize cast_box in init with horizontal layout and stagger animation
- Added Cast section to view macro with visibility based on cast data
- Populate cast cards in LoadDetails command (limited to 10 members)
- Reuses create_person_card from shared module for consistency
- Displays actor names with character roles
- Shows profile images (120x120) with graceful fallback for missing images
- Matches movie details styling with cast-card-modern class
