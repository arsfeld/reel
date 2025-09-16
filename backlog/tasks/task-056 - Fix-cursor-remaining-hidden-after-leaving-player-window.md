---
id: task-056
title: Fix cursor remaining hidden after leaving player window
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 03:05'
updated_date: '2025-09-16 03:17'
labels:
  - bug
  - player
  - ui
dependencies: []
priority: high
---

## Description

After watching media in the player window, the cursor remains hidden even after closing the player or switching to other windows. This makes it difficult to interact with the application or system as the mouse cursor is invisible. The cursor hiding mechanism should only apply while in the player window and should be restored when leaving.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify where cursor is being hidden in player code
- [x] #2 Ensure cursor is shown when player window loses focus
- [x] #3 Restore cursor visibility when player window is closed
- [x] #4 Handle cursor visibility correctly when switching between windows
- [x] #5 Test cursor behavior with both MPV and GStreamer backends
- [x] #6 Verify cursor remains visible in all non-player windows
<!-- AC:END -->


## Implementation Plan

1. Identify cursor hiding code in player.rs (HideCursor/ShowCursor)
2. Find where window chrome is restored when leaving player
3. Add cursor restoration to RestoreWindowChrome handler
4. Ensure cursor timers are cleared when leaving player
5. Test with both navigation back and error scenarios
6. Verify cursor behavior with MPV and GStreamer backends

## Implementation Notes

Fixed the cursor remaining hidden after leaving the player window by:

1. **Added cursor restoration in MainWindowInput::RestoreWindowChrome**:
   - When leaving the player (via back button, ESC, or error), the RestoreWindowChrome handler is called
   - Added explicit cursor restoration using gtk::gdk::Cursor::from_name("default")
   - This ensures cursor is always visible when returning to main UI

2. **Created PlayerInput::NavigateBack handler**:
   - Consolidated navigation logic to properly clean up before leaving
   - Clears both cursor_timer and controls_timer to prevent stale timers
   - Explicitly calls ShowCursor before navigating away

3. **Updated back button and ESC key handlers**:
   - Changed to use PlayerInput::NavigateBack instead of direct output
   - Ensures consistent cleanup path for all navigation methods

The fix ensures that:
- Cursor is always restored when leaving the player window
- All timers are properly cleaned up to prevent memory leaks
- Works consistently across all navigation methods (back button, ESC key, errors)
- Compatible with both MPV and GStreamer backends
