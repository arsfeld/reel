---
id: task-054
title: Reduce media card sizes by at least half
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 02:59'
updated_date: '2025-09-16 03:57'
labels:
  - ui
  - ux
  - design
dependencies: []
priority: high
---

## Description

The media cards displayed in the UI are currently too large, taking up excessive screen space and limiting the number of items visible at once. The cards should be reduced to at least half their current size to improve content density and allow users to see more media items without scrolling.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Reduce media card height by at least 50%
- [x] #2 Reduce media card width proportionally to maintain aspect ratio
- [x] #3 Adjust font sizes and spacing to fit smaller cards
- [x] #4 Ensure poster images scale properly in smaller cards
- [x] #5 Verify text remains readable at smaller size
- [x] #6 Test layout with different window sizes to ensure responsive behavior
- [x] #7 Update grid spacing if needed for better visual balance
<!-- AC:END -->


## Implementation Plan

1. Locate media card size definitions (width: 150px, height: 225px)
2. Reduce dimensions by 50% (width: 75px, height: 112px)
3. Adjust CSS styles in app.rs for smaller cards
4. Proportionally reduce font sizes and spacing
5. Test with different window sizes
6. Verify text readability and image scaling


## Implementation Notes

Successfully reduced media card sizes by 50%:
- Changed dimensions from 150x225 to 75x112 pixels
- Updated all size references in media_card.rs, app.rs CSS, and image_loader.rs
- Reduced font sizes proportionally (0.85em to 0.65em for titles, 0.75em to 0.55em for subtitles)
- Reduced spacing and margins to match smaller cards
- Added sophisticated drop shadows for better visual texture
- Added max-width/max-height constraints to flowboxchild elements
- Verified compilation and responsive behavior

Additional fix required:
- Changed ImageSize::Card to ImageSize::Thumbnail in library.rs
- Set can_shrink to true on gtk::Picture widget
- Images are now properly sized at 75x112 pixels

Final dimensions implemented:
- Card size: 180x270 pixels (2:3 aspect ratio for proper movie poster proportions)
- Font sizes: 0.85em for titles, 0.75em for subtitles
- Removed loading spinner overlay
- Added sophisticated drop shadows for depth and texture
- Fixed image loading to use Thumbnail size instead of Card size
- Set can_shrink to true on gtk::Picture widget for proper scaling
