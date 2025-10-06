---
id: task-404
title: Investigate and unify confusing filter system in library page
status: Done
assignee:
  - '@claude-code'
created_date: '2025-10-06 02:46'
updated_date: '2025-10-06 02:55'
labels:
  - ui
  - ux
  - library
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The library page has multiple conflicting filter mechanisms that create a confusing user experience:

1. Sidebar filter panel that doesn't always show and has no way to hide it when visible
2. Filter tabs (All, Unwatched, Recently Added, Genres, Years) that appear to add filters instead of replacing the view
3. Unclear behavior for Genres and Years tabs
4. Pill-shaped filter chips on top of the display with unclear relationship to other filters
5. The sidebar filter panel looks bad visually

All these mechanisms need to be unified into a coherent, intuitive filtering system.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Analyze current filter implementation and document all filter mechanisms and their interactions
- [x] #2 Identify conflicts and redundancies in the current filter system
- [x] #3 Design a unified filter strategy that eliminates confusion
- [x] #4 Propose UI/UX improvements for filter discoverability and clarity
- [x] #5 Document the proposed solution with clear rationale
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read library page implementation to understand current filter mechanisms
2. Document all filter types and their behavior
3. Identify conflicts and redundancies
4. Analyze UX issues and interaction patterns
5. Design unified filter strategy
6. Document proposed solution with rationale
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Investigation Summary

Analyzed the library page filter system and identified multiple conflicting mechanisms:

### Current Filter Mechanisms
1. **Filter Tabs** (All, Unwatched, Recently Added, Genres, Years) - act as filters not views
2. **Sidebar Filter Panel** - toggleable 280px panel with genre/year/rating/watch status
3. **Active Filter Chips** - pills showing all active filters
4. **Media Type Filter** - for mixed libraries
5. **Text Search** - in toolbar

### Key Problems Identified
- Duplicate "Unwatched" functionality (tab + panel filter)
- Genres/Years tabs just open panel instead of switching views
- Filter state loss when switching tabs
- No way to close sidebar panel once opened
- Tabs treated as removable filters in chip display
- Inconsistent behavior (Recently Added changes sort + filters)

### Solution: Created Follow-up Tasks
- **task-405**: Move tabs to header bar as immutable view switchers
- **task-406**: Remove confusing sidebar filter panel entirely  
- **task-407**: Implement clean popover-based advanced filter system with filter pills

### Design Rationale
- **Tabs = Views**: Immutable subsets of library (header bar)
- **Filters = Refinement**: User-controlled filtering via popover + pills
- **Clear Separation**: Views don't modify filters, filters don't change views
- **Progressive Disclosure**: Simple (search) → Advanced (popover filters)
<!-- SECTION:NOTES:END -->
