---
id: task-446
title: Add context menu to media items on home page with navigation options
status: Done
assignee: []
created_date: '2025-10-23 01:38'
updated_date: '2025-10-23 01:46'
labels:
  - feature
  - ui
  - home-page
  - ux
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Summary
Users should be able to right-click on media items (movies, episodes, shows) displayed on the home page to access quick actions via a context menu. This improves navigation and discoverability by providing shortcuts to common actions.

## User Value
- Faster navigation: Users can quickly jump to a show's details page from an episode card without needing to search or browse
- Better UX: Right-click context menus are a familiar desktop interaction pattern
- Discoverability: Exposes available actions that might not be immediately obvious from the card UI alone

## Context
Currently, home page media cards only support click-to-play/view functionality. Adding context menus would provide access to additional actions like navigating to parent shows, marking as watched/unwatched, or other media-specific operations.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Right-clicking on an episode card on the home page displays a context menu
- [x] #2 Context menu includes 'Go to Show' option for episode items that navigates to the show details page
- [x] #3 Context menu includes 'Play' option that starts playback of the selected item
- [x] #4 Right-clicking on a movie card displays a context menu with appropriate options (Play, etc.)
- [x] #5 Context menu closes when clicking outside of it or pressing Escape
- [x] #6 Context menu appears at the cursor position and stays within window bounds
- [x] #7 All menu options execute their intended actions correctly
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

Added context menu support to media cards on the home page with right-click functionality:

### Changes Made:

1. **MediaCard Factory Component** (`src/ui/factories/media_card.rs`):
   - Added `GoToShow(MediaItemId)` variant to `MediaCardOutput` enum for navigating to parent shows
   - Added `parent_show_id` field to track parent show IDs for episodes
   - Overrode `init_widgets` method to set up context menu before view initialization
   - Created PopoverMenu with dynamic menu items based on media type
   - Added GestureClick controller for right-click detection (button 3)
   - Implemented action group with "play" and "go_to_show" actions
   - Menu appears at cursor position using set_pointing_to

2. **Home Page** (`src/ui/pages/home.rs`):
   - Updated factory output forwarding to handle `GoToShow` variant
   - Routes to MediaItemSelected to navigate to the parent show

3. **Other Pages** (library, search, section_row):
   - Updated all MediaCard factory usages to handle the new `GoToShow` output variant
   - Ensures pattern matching is exhaustive across the codebase

### Context Menu Behavior:

- **For Episodes**: Shows "Play" and "Go to Show" options
  - "Play" starts playback of the episode
  - "Go to Show" navigates to the parent show's details page
- **For Movies/Shows**: Shows "Play" option only
- **Right-click gesture**: Detects button 3 (right mouse button)
- **Menu positioning**: Uses GdkRectangle at cursor position for proper placement
- **Auto-dismiss**: Menu closes when clicking outside or pressing Escape (handled by PopoverMenu)

### Technical Notes:

- Used GTK PopoverMenu with Gio Menu model for native GNOME integration
- SimpleActionGroup pattern for clean action handling
- Context menu setup happens in init_widgets before view_output! to avoid ownership issues
- Parent show IDs extracted from MediaItemModel's parent_id field during initialization
<!-- SECTION:NOTES:END -->
