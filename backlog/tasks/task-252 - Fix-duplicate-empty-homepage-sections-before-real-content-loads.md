---
id: task-252
title: Fix duplicate empty homepage sections before real content loads
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 14:13'
updated_date: '2025-09-26 17:39'
labels:
  - ui
  - bug
  - homepage
dependencies: []
priority: high
---

## Description

The homepage initially displays empty placeholder sections (Continue Watching, Recently Added, Movies, TV Shows) which are then followed by the actual sections with proper content. This creates a confusing UI where users see duplicate section headers - first empty, then populated. The empty sections should either be hidden until data is loaded or replaced entirely when real data arrives.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify why empty placeholder sections are created before real data loads
- [x] #2 Prevent empty sections from being displayed in the UI
- [x] #3 Ensure sections are only shown once with their actual content
- [x] #4 Verify smooth loading experience without duplicate section headers
- [x] #5 Test with multiple backends to ensure consistency
<!-- AC:END -->


## Implementation Plan

1. Analyze the duplicate section issue - sections are added twice when cache exists then API loads
2. Track section UI containers by source_id to enable proper removal
3. Modify clear_source_sections to properly remove UI elements
4. Test with multiple backends to ensure sections update correctly
5. Clean up any unnecessary state tracking

## Implementation Notes

Fixed the duplicate empty homepage sections issue by properly tracking and removing UI containers when updating sections from cache to API data. Added a new HashMap to track section UI containers by section_id, updated the display_source_sections method to store references, and modified clear_source_sections to properly remove both the UI elements and associated data. This ensures sections are cleanly replaced when API data arrives after cached data is displayed.
