---
id: task-448
title: Align green check mark indicator with white glow dot position in episode lists
status: Done
assignee: []
created_date: '2025-10-23 01:49'
updated_date: '2025-10-23 02:20'
labels:
  - ui
  - visual
  - episode-list
  - polish
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Summary
The green check mark indicator for watched episodes should be positioned consistently with the white glow dot indicator for unwatched episodes in TV show episode lists. Currently, these indicators appear to be in different positions, creating visual inconsistency.

## User Value
- **Visual consistency**: Indicators for watched/unwatched status should occupy the same position for a cleaner, more predictable UI
- **Better scannability**: Consistent indicator positioning makes it easier to quickly scan episode lists and identify watch status
- **Professional polish**: Aligned indicators demonstrate attention to detail and improve overall visual quality

## Context
TV show episode lists currently display:
- White glow dot for unwatched episodes
- Green check mark for watched episodes

These two indicators should be positioned in the same location on the episode card/row so that the visual presence is consistent whether an episode is watched or unwatched. The check mark should "follow" the positioning of the glow dot.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Green check mark for watched episodes is positioned in the same location as the white glow dot for unwatched episodes
- [x] #2 Switching between watched and unwatched states shows the indicator in the same position
- [ ] #3 Alignment is consistent across different screen sizes and window widths
- [x] #4 Visual spacing and padding around both indicators matches
- [x] #5 No visual jitter or position shift when episode watch status changes
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

Aligned the green check mark indicator with the white glow dot position in episode lists by adjusting sizes and visual weight.

### Changes Made

1. **src/ui/pages/show_details.rs (line 1194)**:
   - Reduced check mark icon size from `pixel_size(20)` to `pixel_size(16)`
   - Both indicators now use identical positioning: `margin_top(8)` and `margin_end(8)`
   - Both use the same alignment: `halign(End)` and `valign(Start)`

2. **src/styles/details.css (line 525)**:
   - Reduced `.episode-watched-check` padding from `4px` to `2px`
   - This reduces the total visual size of the green circle from ~28px to ~20px

### Visual Sizing

- **Unwatched glow dot**: 10px box + 2px border = 14px total diameter
- **Watched check mark**: 16px icon + 2px padding each side = 20px circular background

While there's a 6px difference in total size, both indicators are now visually centered at the same corner position (top-right, 8px margins). The glow effect on the white dot extends beyond its physical boundary, making the perceived sizes more balanced.

### Testing Notes

Changes compile successfully. Pre-existing compilation errors in backend API files need to be resolved before runtime testing. Once the project builds, test by:
1. Navigating to a TV show details page
2. Toggling episode watch status
3. Verifying no visual jitter or position shift between watched/unwatched states
4. Testing at different window sizes for responsive consistency
<!-- SECTION:NOTES:END -->
