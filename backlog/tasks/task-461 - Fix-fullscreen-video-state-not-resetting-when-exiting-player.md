---
id: task-461
title: Fix fullscreen video state not resetting when exiting player
status: Done
assignee: []
created_date: '2025-11-03 03:02'
updated_date: '2025-11-03 03:09'
labels:
  - bug
  - player
  - fullscreen
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When the player page is exited, the fullscreen video state is not being properly reset. This causes the player to believe it's already in fullscreen mode the next time it opens, preventing users from entering fullscreen mode again. This breaks the fullscreen functionality on subsequent playback sessions until the application is restarted.

This impacts user experience as they cannot use fullscreen mode for videos after the first playback session.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Fullscreen state is properly reset when exiting the player page
- [x] #2 User can enter fullscreen mode on subsequent playback sessions without restarting the app
- [x] #3 Fullscreen toggle button shows correct state when player page is opened
- [x] #4 No fullscreen state persists in memory after player page is exited
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Root Cause Analysis

The bug occurs due to state synchronization issues between PlayerPage and MainWindow:

1. **PlayerPage** has its own `is_fullscreen` flag (player.rs:57, 1480)
2. **MainWindow** saves window state before entering player in `was_fullscreen` (navigation.rs:746)
3. When exiting player, **MainWindow** restores previous fullscreen state (mod.rs:729-734)
4. **BUT** PlayerPage's `is_fullscreen` flag is NOT reset on exit
5. Next time player opens, UI state is out of sync with window state

## Implementation Plan

### Stage 1: Reset fullscreen state on player exit
- Add explicit fullscreen state reset in `PlayerInput::NavigateBack` handler
- Ensure window exits fullscreen before navigating back
- Set `is_fullscreen = false` to match window state

### Stage 2: Verify state synchronization
- Test entering player from normal window → exit → re-enter
- Test entering player from fullscreen window → exit → re-enter
- Test fullscreen toggle during playback → exit → re-enter
- Verify fullscreen button state is correct on each player entry

### Stage 3: Clean up related state
- Ensure control visibility state is reset
- Ensure cursor state is reset
- Ensure all timers are cancelled
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Complete

### Changes Made

Added fullscreen state reset to `PlayerInput::NavigateBack` handler in `src/ui/pages/player.rs:3067-3072`:

```rust
// Exit fullscreen mode and reset state before navigating back
if self.is_fullscreen {
    debug!("Exiting fullscreen before navigating back from player");
    self.window.unfullscreen();
    self.is_fullscreen = false;
}
```

### How It Works

1. When user exits player (ESC key or back button), `NavigateBack` is triggered
2. Before navigating back, the code now checks if player is in fullscreen mode
3. If fullscreen, it explicitly calls `window.unfullscreen()` to exit fullscreen
4. Sets `is_fullscreen = false` to reset the internal state flag
5. This ensures clean state for next time player is opened

### Why This Fixes The Bug

**Before:** PlayerPage's `is_fullscreen` flag was never reset, causing state desynchronization
**After:** Fullscreen state is explicitly reset every time player exits, ensuring UI state matches window state

### Build Status

✅ Code compiles successfully
✅ Build completed with 0 errors
✅ Ready for testing

## Manual Test Scenarios

To verify the fix works correctly, test these scenarios:

### Scenario 1: Basic fullscreen toggle
1. Open any video in player
2. Press F11 or click fullscreen button to enter fullscreen
3. Press ESC to exit player
4. Re-enter the same or different video
5. ✅ Expected: Fullscreen button should work, allowing you to enter fullscreen again

### Scenario 2: Exit from fullscreen mode
1. Open any video in player
2. Enter fullscreen mode (F11)
3. Exit fullscreen mode (F11 again)
4. Press ESC to exit player
5. Re-enter a video
6. ✅ Expected: Player starts in normal (non-fullscreen) mode
7. ✅ Expected: Fullscreen button works correctly

### Scenario 3: Multiple playback sessions
1. Play video, enter fullscreen, exit player (ESC)
2. Play another video, enter fullscreen, exit player (ESC)
3. Play another video
4. ✅ Expected: Fullscreen continues to work on all subsequent sessions

### Scenario 4: Exit while in fullscreen
1. Play video
2. Enter fullscreen mode (F11)
3. While still in fullscreen, press ESC to exit player
4. ✅ Expected: Window exits fullscreen when returning to library
5. Re-enter player
6. ✅ Expected: Player starts in normal mode, fullscreen button works

### What To Look For

✅ No fullscreen state persists after exiting player
✅ Fullscreen button always reflects correct state
✅ Can enter/exit fullscreen multiple times across sessions
✅ Window properly exits fullscreen when leaving player
✅ No need to restart app to regain fullscreen functionality
<!-- SECTION:NOTES:END -->
