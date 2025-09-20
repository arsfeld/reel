---
id: task-040
title: Implement MPV keyboard shortcuts in player page for both backends
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 00:34'
updated_date: '2025-09-16 04:54'
labels:
  - ui
  - player
  - enhancement
dependencies: []
priority: high
---

## Description

Add comprehensive MPV-style keyboard shortcuts to the player page that work with both GStreamer and MPV backends. The shortcuts should mimic MPV's default keybindings to provide a familiar experience for users accustomed to MPV's controls.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Implement seek shortcuts (arrow keys for 5s, Shift+arrows for 1s, Ctrl+arrows for 10s)
- [x] #2 Implement playback speed controls ([ and ] for speed down/up, Backspace to reset)
- [x] #3 Implement volume controls (9/0 or mouse wheel for volume, m for mute)
- [x] #4 Implement frame stepping (. for next frame, , for previous frame when paused)
- [x] #5 Implement subtitle controls (v for cycle subtitles, j/J for cycle subtitle tracks)
- [x] #6 Implement audio track cycling (# key or Shift+3)
- [x] #7 Implement fullscreen toggle (f key)
- [x] #8 Implement on-screen controller visibility toggle (Tab key)
- [x] #9 Implement quit/stop playback (q or ESC key)
- [x] #10 Ensure all shortcuts work identically for both MPV and GStreamer backends
<!-- AC:END -->


## Implementation Plan

1. Review current keyboard handling infrastructure
2. Add missing backend methods for speed control and frame stepping
3. Extend PlayerInput enum with new shortcut commands
4. Update keyboard event handler to capture all MPV shortcuts
5. Implement handlers for each shortcut in update() method
6. Add necessary PlayerCommand variants for new functionality
7. Implement backend-specific methods in MPV and GStreamer players
8. Test all shortcuts with both backends


## Implementation Notes

Implemented comprehensive MPV-style keyboard shortcuts for the player page with support for both MPV and GStreamer backends.

Key changes:
1. Added new methods to player backends (MPV and GStreamer) for:
   - Playback speed control (set_playback_speed, get_playback_speed)
   - Frame stepping (frame_step_forward, frame_step_backward)
   - Mute toggle (toggle_mute, is_muted)
   - Track cycling (cycle_subtitle_track, cycle_audio_track)

2. Extended PlayerInput enum with new commands:
   - Speed controls (SpeedUp, SpeedDown, SpeedReset)
   - Frame stepping (FrameStepForward, FrameStepBackward)
   - Volume controls (VolumeUp, VolumeDown, ToggleMute)
   - Track cycling (CycleSubtitleTrack, CycleAudioTrack)
   - Control visibility (ToggleControlsVisibility)
   - Relative seeking (SeekRelative)

3. Updated keyboard event handler to capture all MPV shortcuts with proper modifier key handling

4. Implemented handlers for each shortcut in the player page update() method

5. Added corresponding PlayerCommand variants and handlers in the controller

All shortcuts now work consistently across both MPV and GStreamer backends, providing users with familiar MPV-style controls regardless of the chosen playback engine.
