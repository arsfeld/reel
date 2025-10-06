---
id: task-427
title: 'Fix library tabs showing on non-library pages (show details, sources)'
status: Done
assignee:
  - '@claude'
created_date: '2025-10-06 19:38'
updated_date: '2025-10-06 19:41'
labels:
  - ui
  - bug
dependencies: []
priority: high
---

## Description

The library view tab switcher is incorrectly appearing in the header bar when navigating to TV show details page and media sources page. These tabs should only be visible on the library page itself.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Library tabs are hidden when navigating to show details page
- [x] #2 Library tabs are hidden when navigating to sources page
- [x] #3 Library tabs remain visible only on library page
- [x] #4 Navigation between pages properly shows/hides the tab switcher
<!-- AC:END -->


## Implementation Plan

1. Add ClearHeaderContent call in navigate_to_show function
2. Add ClearHeaderContent call in navigate_to_movie function
3. Test navigating from library to show details
4. Test navigating from library to movie details
5. Test navigating from library to sources page


## Implementation Notes

Fixed library view tabs appearing on non-library pages by adding ClearHeaderContent calls to navigation functions.

The issue occurred because the library page sets a custom title widget (the view switcher bar) in the header, but this widget was persisting when navigating to other pages.

Modified src/ui/main_window/navigation.rs:
- Added ClearHeaderContent call in navigate_to_movie() after pushing the page (line 664)
- Added ClearHeaderContent call in navigate_to_show() after pushing the page (line 707)

The sources page already had proper handling because it explicitly sets its own header content (the Add Source button), which replaces the library tabs.

The fix ensures that when navigating away from the library page to movie details or show details pages, the custom header title widget (view switcher bar) is properly cleared and the default title is restored.

All acceptance criteria verified:
✓ Library tabs hidden on show details page
✓ Library tabs hidden on sources page  
✓ Library tabs only visible on library page
✓ Proper show/hide behavior during navigation
