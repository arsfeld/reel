---
id: task-058
title: 'Redesign player OSD for cleaner, more minimalistic interface'
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 03:19'
updated_date: '2025-09-16 03:29'
labels:
  - ui
  - player
  - design
dependencies: []
priority: high
---

## Description

The player's on-screen display (OSD) controls need a visual redesign to be more clean, slick, and minimalistic. The current design may be too busy or cluttered. Create a modern, streamlined interface that provides essential controls without visual noise, following contemporary media player design patterns.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Use subtle, modern icons with consistent visual weight
- [x] #2 Implement semi-transparent background with blur effect for controls
- [x] #3 Add smooth fade in/out animations for control visibility
- [x] #4 Use minimal color palette with subtle accent colors
- [x] #5 Ensure controls are visually unobtrusive during playback
- [x] #6 Maintain accessibility with appropriate contrast ratios
- [x] #7 Simplify player control buttons to have cleaner visual design
- [x] #8 Keep all existing functionality while improving visual presentation
<!-- AC:END -->


## Implementation Plan

1. Analyze current OSD design and identify areas for visual simplification
2. Redesign play/pause button to be larger and more prominent
3. Reorganize controls layout for cleaner appearance while keeping all features
4. Implement subtle glassmorphism effect for controls background
5. Refine progress bar styling with sleeker design
6. Enhance fade animations for smoother control visibility transitions
7. Update color palette to be more minimal and elegant
8. Adjust spacing and sizing for better visual hierarchy
9. Test all changes to ensure controls remain functional and accessible

## Implementation Notes

Redesigned the player OSD with an ultra-minimalistic approach:

- Reduced all control sizes for a more compact, sleek appearance
- Implemented subtle glassmorphism with minimal backdrop blur
- Made progress bar ultra-thin (2px default, 4px on hover)
- Simplified color palette with reduced opacity values
- Removed unnecessary borders and reduced shadows
- Made play/pause button 36x36px instead of oversized
- Reduced padding and margins throughout for tighter layout
- Implemented faster, subtler fade animations
- Kept all functionality while dramatically reducing visual weight

The new design is much more minimal and unobtrusive during playback while maintaining good usability and accessibility.
