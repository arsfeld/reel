---
id: task-158
title: Fix episode media cards styling to match TV show details page design
status: Done
assignee:
  - '@claude'
created_date: '2025-09-17 18:55'
updated_date: '2025-09-18 02:15'
labels: []
dependencies: []
priority: high
---

## Description

The episode media cards in the TV show details page have inconsistent styling that doesn't match the overall design of the page. The cards should be visually integrated with the rest of the TV show details page aesthetic for a cohesive user experience.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Episode media cards use consistent styling that matches the TV show details page
- [x] #2 Card dimensions and spacing align with the page's design system
- [x] #3 Typography in episode cards matches the page's font hierarchy
- [x] #4 Color scheme and visual effects are consistent with the overall page design
- [x] #5 Hover/selection states match the interaction patterns of other elements on the page
<!-- AC:END -->


## Implementation Plan

1. Analyze current episode card styling in show_details.rs
2. Review existing poster card and media card styling from app.rs
3. Update episode card styling to match the overall design system
4. Fix card dimensions to be consistent with other cards
5. Ensure typography hierarchy matches rest of page
6. Fix colors and visual effects for consistency
7. Update hover and selection states to match other interactive elements
8. Test the changes by running the application


## Implementation Notes

Fixed episode media card styling to match the TV show details page design:

1. Updated episode card CSS classes in show_details.rs:
   - Changed main card class to episode-card-styled with glass-card effect
   - Updated badge to use metadata-pill-modern styling
   - Fixed watched indicator and progress bar styling
   - Updated typography to use consistent body and caption classes

2. Enhanced details.css with premium streaming-style episode cards:
   - Added glass morphism effect with backdrop-filter
   - Consistent border-radius (12px) matching other design elements
   - Updated hover states with smooth transitions and proper scaling
   - Added loading shimmer animation for thumbnails
   - Progress bar with animated shine effect
   - Watched indicator with green checkmark and glow

3. Fixed async image loading issue by using gtk::glib::spawn_future_local

The episode cards now have a cohesive visual style that matches the modern, premium aesthetic of the TV show details page with glass effects, smooth animations, and consistent typography hierarchy.
