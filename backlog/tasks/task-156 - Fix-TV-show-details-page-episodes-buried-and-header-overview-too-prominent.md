---
id: task-156
title: 'Fix TV show details page: episodes buried and header/overview too prominent'
status: Done
assignee:
  - '@claude'
created_date: '2025-09-17 18:45'
updated_date: '2025-09-18 01:40'
labels: []
dependencies: []
priority: high
---

## Description

Episodes are currently buried at the bottom of the TV show details page and are barely visible. The header is too large and the overview section has too many visual effects (glassy appearance) that distract from the main content. Need to restructure the layout to prioritize episode visibility.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Episodes section is prominently displayed and easily accessible (not buried at bottom)
- [x] #2 TV show header is reduced in size to be more proportional
- [x] #3 Overview section is redesigned to be smaller and more subtle (remove glassy effects)
- [x] #4 Overview is relocated to a less prominent position in the layout
- [x] #5 Visual hierarchy prioritizes content (episodes) over decorative elements
<!-- AC:END -->


## Implementation Plan

1. Analyze current layout structure and identify issues
2. Reduce hero section height from 600px to something more proportional
3. Remove or simplify glass-card effects from overview section
4. Restructure layout to move episodes section higher in the visual hierarchy
5. Move overview to a less prominent position (potentially as a collapsible section or after episodes)
6. Adjust CSS to reduce visual weight of decorative elements
7. Test with actual TV show data to ensure good user experience


## Implementation Notes

## Implementation Summary

Restructured the TV show details page to prioritize episode visibility and reduce visual clutter:

### Changes Made:

1. **Reduced Hero Section Height**: Changed from 600px to 400px for better proportion
2. **Simplified Poster Styling**: 
   - Reduced poster size from 300x450 to 200x300
   - Removed excessive shadow effects and 3D transforms
3. **Reorganized Layout**:
   - Moved episodes section to the top of the content area (previously was at bottom)
   - Relocated overview section below episodes with simpler styling
4. **Removed Glass Effects**:
   - Replaced glass-card class with simpler overview-section styling
   - Removed backdrop-filter and excessive blur effects
5. **Typography Adjustments**:
   - Reduced title size from 48px to 36px
   - Added new title-3 class for episode section heading
   - Simplified text shadows and decorative elements

### Files Modified:
- `src/platforms/relm4/components/pages/show_details.rs`: Layout restructuring
- `src/platforms/relm4/styles/details.css`: Visual styling simplification

### Result:
Episodes are now prominently displayed immediately below the header, overview section has been de-emphasized with simpler styling, and the overall visual hierarchy now prioritizes content over decorative elements.
