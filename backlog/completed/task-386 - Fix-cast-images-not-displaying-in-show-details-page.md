---
id: task-386
title: Fix cast images not displaying in show details page
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 18:56'
updated_date: '2025-10-03 18:58'
labels:
  - ui
  - bug
  - cast-crew
dependencies: []
priority: high
---

## Description

Cast images fail to display in show_details.rs even though they load correctly in movie_details.rs. The PersonImageLoaded command handler in show_details stores the texture but doesn't recreate the cast cards to display the loaded images, unlike movie_details which properly rebuilds cards when images arrive.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 PersonImageLoaded handler in show_details recreates cast cards with loaded textures
- [x] #2 Cast images display correctly when viewing show details
- [x] #3 Implementation matches the working pattern in movie_details.rs lines 648-693
- [x] #4 Verify cast images load and display for multiple shows
<!-- AC:END -->


## Implementation Plan

1. Read show_details.rs to understand current PersonImageLoaded handler
2. Read movie_details.rs lines 648-693 to see the working pattern
3. Update PersonImageLoaded handler in show_details.rs to recreate cast cards with loaded textures
4. Verify compilation


## Implementation Notes

Fixed cast images not displaying in show_details.rs by updating the PersonImageLoaded command handler to match the pattern from movie_details.rs.

The handler now:
1. Stores the loaded texture in person_textures HashMap
2. Clears all existing cast cards from cast_box
3. Recreates all cast cards with updated textures (loaded ones show images, others show placeholders)

This matches the implementation in movie_details.rs lines 648-693 and ensures cast images display correctly when they finish loading asynchronously.

Modified files:
- src/ui/pages/show_details.rs (lines 812-829): Updated PersonImageLoaded handler to recreate cast cards
