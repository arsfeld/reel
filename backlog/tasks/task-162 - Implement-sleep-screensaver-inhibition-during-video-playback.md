---
id: task-162
title: Implement sleep/screensaver inhibition during video playback
status: To Do
assignee: []
created_date: '2025-09-18 01:44'
labels:
  - feature
  - player
  - system-integration
dependencies: []
priority: high
---

## Description

The system should prevent sleep mode and screensaver activation while a video is actively playing. This ensures uninterrupted viewing experience. The inhibition should only be active during playback and should be released when the video is paused, stopped, or the player is closed.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Implement sleep inhibition when video playback starts
- [ ] #2 Release sleep inhibition when video is paused
- [ ] #3 Release sleep inhibition when video is stopped
- [ ] #4 Release sleep inhibition when player window is closed
- [ ] #5 Use GTK/GNOME inhibit API for proper system integration
- [ ] #6 Handle inhibition state correctly when switching between videos
- [ ] #7 Ensure inhibition works on both X11 and Wayland
- [ ] #8 Test that system does not sleep during video playback
- [ ] #9 Test that system can sleep again after playback stops
<!-- AC:END -->
