---
id: task-013
title: Implement Relm4 player playlist navigation
status: To Do
assignee: []
created_date: '2025-09-15 02:10'
labels:
  - player
  - relm4
  - navigation
dependencies: []
priority: medium
---

## Description

The Previous/Next buttons in the Relm4 player need to properly navigate through playlists (TV show episodes, movie collections). The playlist context system exists but needs proper integration with the UI controls.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Enable Previous button when not at first item in playlist
- [ ] #2 Enable Next button when not at last item in playlist
- [ ] #3 Load previous/next media while maintaining playlist context
- [ ] #4 Update current index in playlist context on navigation
- [ ] #5 Show current position in playlist (e.g., 'Episode 3 of 10')
- [ ] #6 Handle edge cases (first/last item in playlist)
<!-- AC:END -->
