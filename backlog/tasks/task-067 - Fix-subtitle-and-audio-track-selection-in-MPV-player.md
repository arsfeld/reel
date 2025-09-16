---
id: task-067
title: Fix subtitle and audio track selection in MPV player
status: To Do
assignee: []
created_date: '2025-09-16 04:07'
labels:
  - bug
  - player
  - mpv
  - critical
  - subtitles
dependencies: []
priority: high
---

## Description

The subtitle and audio track selection options are always disabled/grayed out in the MPV player, preventing users from changing subtitles or audio tracks during playback. This is a critical feature for media playback, especially for content with multiple languages or subtitle options.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Investigate MPV player implementation in mpv_player.rs
- [ ] #2 Check how subtitle and audio tracks are being enumerated from MPV
- [ ] #3 Verify MPV initialization and track loading configuration
- [ ] #4 Implement proper track enumeration when media is loaded
- [ ] #5 Enable subtitle and audio track menu items when tracks are available
- [ ] #6 Implement track switching functionality for subtitles
- [ ] #7 Implement track switching functionality for audio tracks
- [ ] #8 Add proper error handling for track switching operations
- [ ] #9 Test with media files containing multiple subtitle tracks
- [ ] #10 Test with media files containing multiple audio tracks
- [ ] #11 Ensure track selection persists during playback
- [ ] #12 Verify functionality works with both local files and streaming content
<!-- AC:END -->
