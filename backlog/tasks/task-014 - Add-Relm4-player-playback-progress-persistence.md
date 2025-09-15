---
id: task-014
title: Add Relm4 player playback progress persistence
status: To Do
assignee: []
created_date: '2025-09-15 02:11'
labels:
  - player
  - relm4
  - persistence
dependencies: []
priority: medium
---

## Description

The Relm4 player saves playback progress but doesn't resume from saved position when reopening media. Users should be able to continue watching from where they left off.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Load saved playback position when opening media
- [ ] #2 Show resume prompt if media has saved progress
- [ ] #3 Implement 'Resume' and 'Start from beginning' options
- [ ] #4 Auto-resume if configured in settings
- [ ] #5 Update progress more frequently (every 5-10 seconds)
- [ ] #6 Mark media as watched when reaching 90% completion
<!-- AC:END -->
