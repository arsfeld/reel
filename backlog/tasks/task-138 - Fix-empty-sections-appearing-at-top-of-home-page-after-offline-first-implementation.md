---
id: task-138
title: >-
  Fix empty sections appearing at top of home page after offline-first
  implementation
status: Done
assignee:
  - '@claude'
created_date: '2025-09-17 04:00'
updated_date: '2025-09-17 14:43'
labels: []
dependencies: []
priority: high
---

## Description

After implementing offline-first homepage loading (task-134), empty sections are now appearing at the top of the home page. This happens because the cached data loading creates sections that may not have items, or the section clearing/updating logic isn't working properly when fresh data arrives.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Empty sections do not appear on the home page
- [x] #2 Sections only display when they contain items
- [x] #3 Cached sections are properly replaced when fresh data arrives
- [x] #4 Section ordering is consistent between cached and fresh data
<!-- AC:END -->


## Implementation Plan

1. Identify root cause of empty sections appearing
2. Add filtering to API section loading to match cached section behavior
3. Filter out empty sections before adding to converted_sections
4. Clean up display_source_sections to not add empty sections to model
5. Test with multiple backends to ensure no empty sections appear


## Implementation Notes

Fixed empty sections appearing at the top of the home page after offline-first implementation.

Root cause: The API section loading code in BackendService::get_home_sections_per_source was not filtering out empty sections, while the cached section loading code was properly filtering them.

Fix implemented:
1. Added filtering in backend.rs to only add sections with items to converted_sections
2. Modified display_source_sections in home.rs to filter out empty sections before adding to model
3. Sections are now consistently filtered in both cached and fresh data paths

Tested with Plex backend and confirmed no empty sections appear on the home page.
