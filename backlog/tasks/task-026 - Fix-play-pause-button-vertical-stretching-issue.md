---
id: task-026
title: Fix play/pause button vertical stretching issue
status: To Do
assignee: []
created_date: '2025-09-15 03:45'
labels:
  - ui
  - player
  - bug
dependencies: []
priority: high
---

## Description

The play/pause button in the player controls is still showing vertical stretching despite CSS fixes. The button should be perfectly circular but appears stretched vertically. Need to investigate alternative approaches to ensure the button maintains a 1:1 aspect ratio.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Investigate why current CSS rules are not working
- [ ] #2 Try using a container wrapper approach
- [ ] #3 Consider using fixed-size SVG icons
- [ ] #4 Test with different GTK button implementations
- [ ] #5 Ensure button is perfectly circular in all states
<!-- AC:END -->
