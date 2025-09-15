---
id: task-024
title: Fix player crash when converting negative float seconds to Duration
status: To Do
assignee: []
created_date: '2025-09-15 03:38'
labels:
  - bug
  - player
  - relm4
  - critical
dependencies: []
priority: high
---

## Description

The Relm4 player crashes with a panic when attempting to convert negative float seconds to Duration during playback. This occurs after media is successfully loaded and hardware decoding is initialized.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Identify the source of negative duration values in the player code
- [ ] #2 Add validation to prevent negative values from being converted to Duration
- [ ] #3 Handle edge cases where seek position or playback time might be negative
- [ ] #4 Ensure player gracefully handles invalid time values without crashing
- [ ] #5 Add error recovery mechanism for duration conversion failures
- [ ] #6 Test with various media files to ensure no regressions
<!-- AC:END -->
