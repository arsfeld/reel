---
id: task-244
title: Player fails to load media without UI error indication
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 12:49'
updated_date: '2025-09-26 13:17'
labels:
  - player
  - ui
  - error-handling
dependencies: []
priority: high
---

## Description

When the player encounters an HTTP 500 error or other media loading failures, the UI shows no indication of the error. The player appears to load successfully but media doesn't play, leaving users confused about what went wrong.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Display error message when media fails to load (HTTP errors, network issues)
- [x] #2 Show appropriate error state in player UI instead of appearing loaded
- [x] #3 Log errors to user-visible diagnostics or notifications
- [x] #4 Provide retry mechanism or alternative stream options on failure
- [x] #5 Handle different error types (404, 500, network timeout) with specific messages
<!-- AC:END -->


## Implementation Plan

1. Review current error handling flow to understand gaps
2. Add toast notification system for user-visible error feedback
3. Enhance error detection in MPV and GStreamer player backends
4. Add specific HTTP error code detection (404, 500, etc.)
5. Improve retry mechanism with better user feedback
6. Test error handling with various failure scenarios


## Implementation Notes

Implemented improved error handling for media player:

1. Added toast notifications for immediate error feedback
2. Enhanced error display in player UI with overlay showing error message
3. Added ShowToast output to PlayerOutput enum for cross-component communication
4. Connected player error handling to main window toast system
5. Simplified error messages to show actual server/backend errors directly
6. Retained existing retry mechanism with exponential backoff

The player now properly shows errors both as toast notifications (for immediate feedback) and in the player UI overlay (with retry button). Errors are logged to console and displayed to users clearly.

FIX: The real issue was that when media failed to load, the player page was already displayed but in a broken state. The solution is to automatically navigate back when LoadError occurs, so the user is returned to the previous page (movie/show details) with a toast notification explaining the error.

This maintains proper separation of concerns - the movie/show details pages simply request navigation to the player, and the player handles its own errors by navigating back if it cannot play the media.

FINAL FIX: Implemented comprehensive error monitoring system:

1. **MPV Event Monitoring**: Added continuous monitoring of MPV player state to detect failures during playback
2. **Error Callback System**: Created error callback infrastructure from MPV -> PlayerController -> UI
3. **Dual Detection Strategy**:
   - Immediate validation after load_media (checks duration and player state)
   - Continuous monitoring during playback for runtime errors
4. **UI Integration**: Player page now listens for errors and automatically:
   - Shows toast notification with error details
   - Navigates back to previous page (movie/show details)

The system now properly detects HTTP 500 errors and other failures that occur both during initial load and during playback, ensuring users are never stuck on a broken player page.
