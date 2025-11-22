---
id: task-383
title: Implement skip intro and skip credits UI buttons in player
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 18:08'
updated_date: '2025-10-03 18:25'
labels:
  - player
  - ui
  - markers
dependencies: []
priority: high
---

## Description

Add skip intro and skip credits button overlays to the player UI that appear during intro/credits sequences and allow users to skip forward. Buttons should auto-hide after timeout and follow existing OSD pill styling

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Skip intro button component added to player overlay
- [x] #2 Skip credits button component added to player overlay
- [x] #3 Buttons only visible when markers exist and playback position is within marker range
- [x] #4 Clicking skip intro jumps playback to intro_marker.end_time
- [x] #5 Clicking skip credits jumps playback to credits_marker.end_time (or stops if near end)
- [x] #6 Buttons auto-hide 5 seconds after appearing
- [x] #7 Buttons use existing .osd.pill CSS styling from player.css
- [x] #8 Works with both MPV and GStreamer playback backends
<!-- AC:END -->


## Implementation Plan

1. Read player.rs to understand current UI structure and message system
2. Design skip button component with visibility tracking
3. Add skip intro and skip credits button widgets to player overlay
4. Implement visibility logic (show when position is within marker range)
5. Add auto-hide timer (5 seconds)
6. Wire up click handlers to seek to end of markers
7. Apply .osd.pill CSS styling
8. Test with both MPV and GStreamer backends


## Implementation Notes

Implemented skip intro and skip credits button overlays in the player UI.

Key changes:
- Added intro_marker, credits_marker, and visibility state fields to PlayerPage struct
- Created LoadedMarkers input message to receive marker data from database
- Added marker loading logic in LoadMedia and LoadMediaWithContext handlers
- Implemented skip button widgets using existing .osd.pill CSS styling
- Added UpdateSkipButtonsVisibility logic that checks if playback position is within marker range
- Implemented auto-hide timer (5 seconds) for both skip buttons
- Added SkipIntro and SkipCredits handlers that seek to marker end_time
- Position checking triggered on every UpdatePosition event
- Works with both MPV and GStreamer backends (uses generic PlayerHandle::seek)

Files modified:
- src/ui/pages/player.rs - Added skip button UI, state management, and logic
