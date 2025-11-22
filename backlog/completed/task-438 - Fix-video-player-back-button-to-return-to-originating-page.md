---
id: task-438
title: Fix video player back button to return to originating page
status: Done
assignee: []
created_date: '2025-10-23 00:21'
updated_date: '2025-10-23 00:28'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The video player's back button currently doesn't navigate back to where the user came from. When a user starts playing a video from the show details page or home page, clicking the back button should return them to that exact page, but it currently doesn't work as expected. This creates a frustrating user experience where navigation context is lost.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 When playing a video from the home page, clicking back returns to the home page
- [x] #2 When playing a video from show details page, clicking back returns to show details page
- [x] #3 When playing a video from library page, clicking back returns to library page
- [x] #4 Navigation state is preserved across different entry points to the player
- [x] #5 Back button behavior is consistent regardless of video type (movie, episode, etc.)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

Fixed a double-pop navigation bug that prevented the video player's back button from returning to the originating page.

### Root Cause
The issue was in the player output handling flow:
1. When `PlayerOutput::NavigateBack` was sent
2. It triggered `RestoreWindowChrome` (which popped the player page)
3. Then triggered `Navigate("back")` (which popped again)
4. Result: Navigation skipped the intended destination page

### Solution
1. **Added navigation context tracking** (`previous_page_before_player` field in MainWindow)
   - Captures the current page title before entering player
   - Provides logging for debugging navigation flow

2. **Fixed double-pop bug**
   - Removed the pop operation from `RestoreWindowChrome` handler
   - Let `navigate_back` handle the single pop operation
   - Ensures proper cleanup and header restoration

3. **Enhanced both player navigation paths**
   - Updated `navigate_to_player` to save previous page
   - Updated `navigate_to_player_with_context` to save previous page
   - Both functions now properly track navigation context

### Files Modified
- `src/ui/main_window/mod.rs`: Added tracking field and fixed RestoreWindowChrome
- `src/ui/main_window/navigation.rs`: Enhanced navigation functions with context tracking

### Testing
The fix correctly handles navigation from:
- Home page → Player → Back to Home
- Library page → Player → Back to Library  
- Show Details → Player → Back to Show Details
- Movie Details → Player → Back to Movie Details

All navigation paths now correctly maintain the page history.
<!-- SECTION:NOTES:END -->
