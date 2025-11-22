---
id: task-110
title: Create filter sidebar panel for library page
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 23:09'
updated_date: '2025-10-04 22:06'
labels: []
dependencies:
  - task-119
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Design and implement a collapsible filter sidebar that houses all filter controls in an organized, accessible layout. Should slide in/out from the side of the library view.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create FilterPanel component with collapsible sidebar design
- [x] #2 Add toggle button to show/hide filter panel
- [x] #3 Implement slide-in/out animation for panel
- [x] #4 Organize filter controls in logical sections
- [x] #5 Add clear all filters button
- [x] #6 Show active filter count badge
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Research libadwaita components for sidebar panel (OverlaySplitView, Clamp, etc.)
2. Create FilterPanel component structure
3. Move existing filter controls from toolbar to FilterPanel
4. Add toggle button to toolbar
5. Implement slide-in/out animation
6. Add "Clear all filters" button
7. Add active filter count badge to toggle button
8. Test the functionality
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented a collapsible filter sidebar panel for the library page using GTK Revealer widget.

Key changes:
- Added filter_panel_visible state field to track sidebar visibility
- Restructured library page layout with horizontal box containing:
  - Revealer widget for filter panel (280px width, SlideRight animation)
  - Vertical separator
  - Main scrolled window with media grid
- Moved all filter controls from toolbar into organized sidebar sections:
  - Genre filter section (visible when genres available)
  - Year range filter section (visible when year data available)
  - Rating filter section
  - Watch status filter section
- Added filter toggle button to toolbar with:
  - Funnel icon
  - Active filter count badge (displays number of active filters)
- Implemented Clear All Filters functionality that:
  - Clears all active filters (genre, year, rating, watch status, text)
  - Updates all filter UI components
  - Reloads library items
- Added get_active_filter_count() helper method
- Added ToggleFilterPanel and ClearAllFilters input messages

The sidebar provides a clean, organized interface for all filtering options with smooth slide-in/out animations and clear visual feedback on active filters.
<!-- SECTION:NOTES:END -->
