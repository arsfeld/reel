---
id: task-384
title: Add user preferences for skip intro/credits auto-skip behavior
status: To Do
assignee: []
created_date: '2025-10-03 18:08'
labels:
  - settings
  - preferences
  - player
dependencies: []
priority: medium
---

## Description

Allow users to configure whether intro and credits should be skipped automatically or require manual button clicks. Preferences should be persisted and applied during playback

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Settings page has toggle for 'Auto-skip intro'
- [ ] #2 Settings page has toggle for 'Auto-skip credits'
- [ ] #3 Preferences stored in app configuration/database
- [ ] #4 When auto-skip intro enabled, playback automatically jumps to intro_marker.end_time
- [ ] #5 When auto-skip credits enabled, playback automatically jumps to credits_marker.end_time or next episode
- [ ] #6 Toast notification shown when auto-skip occurs
- [ ] #7 Manual skip buttons still visible even when auto-skip enabled
<!-- AC:END -->
