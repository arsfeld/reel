---
id: task-410
title: Redesign filter pills with modern compact styling matching movie details page
status: Done
assignee:
  - '@claude'
created_date: '2025-10-06 12:05'
updated_date: '2025-10-06 12:08'
labels:
  - ui
  - design
  - filters
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The current filter pills in the library page advanced filter system need a visual refresh to match the sleek, modern appearance of the pills used in the movie details page (genres, cast, etc.). This will create visual consistency across the application and improve the overall polish of the filter UI.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Filter pills use compact padding and modern styling similar to movie details page pills
- [x] #2 Pill design maintains visual consistency with movie details page aesthetic
- [x] #3 Pills remain readable and accessible at their new compact size
- [x] #4 Filter pill styling integrates seamlessly with existing filter popover UI
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Examine movie details page pill styling (CSS and Rust code)
2. Locate library page filter pill rendering code
3. Update filter pills to use metadata-pill-modern class
4. Adjust pill layout margins to match movie details page
5. Test visual appearance and ensure consistency
6. Build and verify no errors
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Updated library page filter pills to use modern styling from movie details page:

- Changed CSS class from `pill` to `metadata-pill-modern`
- Added `interactive-element` class for hover effects
- Adjusted label margins to match movie details page (12px start/end, 6px top/bottom)
- Updated chip margins for better spacing (8px end, 6px bottom)

The filter pills now feature:
- Glass morphism effect with backdrop blur
- Smooth transitions and hover animations
- Consistent styling with genre pills on movie details page
- Modern, compact appearance while maintaining readability

Modified file: src/ui/pages/library.rs (lines 2684-2691)
<!-- SECTION:NOTES:END -->
