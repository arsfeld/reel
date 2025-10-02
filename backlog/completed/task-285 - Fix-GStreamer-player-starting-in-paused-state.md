---
id: task-285
title: Fix GStreamer player starting in paused state
status: Done
assignee:
  - '@claude'
created_date: '2025-09-27 21:00'
updated_date: '2025-09-27 21:06'
labels:
  - bug
  - player
  - gstreamer
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The GStreamer player backend currently starts in a paused state when loading media, requiring users to manually press play. This creates a poor user experience compared to the MPV backend which starts playing automatically. The player should automatically begin playback after loading media, matching the behavior of the MPV backend.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 GStreamer player automatically starts playback after loading media
- [x] #2 Player state correctly transitions from Loading to Playing without manual intervention
- [x] #3 Behavior matches MPV backend for consistent user experience
- [x] #4 Auto-play setting in preferences is respected
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Analyze where the issue occurs - after load_media completes
2. Check if auto-play logic should trigger after loading
3. Add automatic play() call after successful media load in GStreamer player
4. Test to ensure GStreamer matches MPV behavior
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed the GStreamer player starting in paused state issue by adding automatic playback after successful media load.

The issue was in the player page (src/ui/pages/player.rs) where after loading media, the code would get the state from the player but not automatically start playback. This left the GStreamer player in the Loading/Paused state.

The fix adds a play() call after successful media load in both LoadMedia and LoadMediaWithContext handlers, ensuring that GStreamer automatically starts playing media just like the MPV backend does.

Changes made:
- Modified src/ui/pages/player.rs to call player_handle.play() after successful media load
- Added error handling for the play() call with a warning log if it fails
- Applied the fix to both LoadMedia and LoadMediaWithContext input handlers

Note: There is no explicit auto-play preference setting for initial media playback. The auto-play functionality in the code refers to automatically playing the next episode in TV shows, which is controlled by the playlist context and works independently of this fix.
<!-- SECTION:NOTES:END -->
