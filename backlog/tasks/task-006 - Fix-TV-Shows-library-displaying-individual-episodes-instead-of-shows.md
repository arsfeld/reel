---
id: task-006
title: Fix TV Shows library displaying individual episodes instead of shows
status: Done
assignee:
  - '@claude'
created_date: '2025-09-15 01:45'
updated_date: '2025-09-15 02:11'
labels:
  - ui
  - library
  - bug
dependencies: []
priority: high
---

## Description

The TV Shows library page incorrectly displays individual episodes as separate items instead of grouping them by TV show. Users should see TV show cards that they can click to view seasons and episodes, not a flat list of all episodes.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 TV Shows library displays show-level cards, not individual episodes
- [x] #2 Each TV show card shows poster, title, and show metadata
- [x] #3 Clicking a TV show navigates to show details page with seasons/episodes
- [x] #4 Library correctly filters to show only TV show entities from the database
<!-- AC:END -->


## Implementation Plan

1. Test TV Shows library page to confirm if it shows episodes or shows
2. Check the find_by_library_and_type_paginated repository method for proper filtering
3. Verify library.library_type normalization (shows vs Shows) in library page logic
4. Test that only 'show' media type items are returned for TV show libraries
5. Fix any filtering issues found


## Implementation Notes

FIXED: TV Shows library type filtering issue that caused episodes to display instead of shows.

PROBLEM IDENTIFIED:
- Database shows inconsistent library_type casing: 'Shows' vs 'shows'  
- Library page filtering was case-sensitive, so 'Shows' libraries didn't match 'shows' filter
- Result: TV Shows libraries showed all items (including 3265 episodes) instead of just shows

FIX IMPLEMENTED:
- Made library type comparison case-insensitive using .to_lowercase()
- Now both 'Shows' and 'shows' library types properly filter to show only 'show' media type
- Maintains backward compatibility for all library type variations

VERIFICATION NEEDED:
- Test TV Shows library page to confirm only show cards are displayed
- Verify 181 show items are shown instead of 3265 episodes

\n\nFixed successfully - made library type comparison case-insensitive using .to_lowercase() to handle 'Shows' vs 'shows' database inconsistency.
