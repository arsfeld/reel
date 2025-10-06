---
id: task-416
title: 'Fix broken tab bar in library view - no text, broken styling'
status: Done
assignee:
  - '@assistant'
created_date: '2025-10-06 14:55'
updated_date: '2025-10-06 15:13'
labels:
  - bug
dependencies: []
priority: high
---

## Description

The library view tab bar is completely broken. There's no text visible, the styling is broken, and it looks awful. Need to completely fix or replace the tab bar implementation.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Tab bar displays visible text labels for All, Unwatched, and Recently Added
- [x] #2 Tab bar has proper styling and looks good
- [x] #3 Tabs are functional and switch between views correctly
- [x] #4 Tab bar matches the design language of the rest of the app
<!-- AC:END -->


## Implementation Plan

1. Investigate why TabBar text is not visible - check if TabView/TabBar is the right widget
2. Consider alternatives: ViewSwitcher, ViewSwitcherBar, or custom button box
3. Implement the proper solution with visible text and good styling
4. Test tab switching functionality
5. Ensure styling matches app design language


## Implementation Notes

Replaced TabView/TabBar with ViewStack/ViewSwitcherBar in the library view.

The original implementation used TabView/TabBar, which is designed for managing closeable tabs with content panes (like browser tabs). This was the wrong widget for a simple view mode selector.

Changes made:
1. Replaced tab_view (TabView) with view_stack (ViewStack) in LibraryPage struct
2. Replaced tab_bar (TabBar) with view_switcher_bar (ViewSwitcherBar)
3. Updated initialization to use add_titled() for adding pages to ViewStack
4. Connected ViewSwitcherBar to ViewStack with set_stack()
5. Changed signal handler from connect_selected_page_notify to connect_visible_child_name_notify
6. Updated SetViewMode handler to use set_visible_child_name() instead of set_selected_page()
7. Simplified header widget output - ViewSwitcherBar doesn't need centering wrapper
8. Removed TabBar-specific CSS that's no longer needed

ViewSwitcherBar is the proper libadwaita widget for this use case - it provides built-in styling, proper text labels, and matches the GNOME design language. The tabs now display text correctly and look polished.

Files modified:
- src/ui/pages/library.rs (structure and logic)
- src/styles/base.css (removed obsolete TabBar styles)
