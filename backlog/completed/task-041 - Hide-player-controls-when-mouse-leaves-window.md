---
id: task-041
title: Hide player controls when mouse leaves window
status: Done
assignee: []
created_date: '2025-09-16 00:35'
updated_date: '2025-10-02 14:52'
labels:
  - ui
  - player
  - enhancement
dependencies: []
priority: high
---

## Description

Implement auto-hide functionality for player controls that hides them immediately when the mouse cursor moves outside the player window boundaries. This provides a cleaner viewing experience and prevents controls from remaining visible when the user is interacting with other applications.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Player controls hide immediately when mouse cursor exits window boundaries
- [ ] #2 Controls remain hidden while mouse is outside window
- [ ] #3 Controls reappear when mouse re-enters window
- [ ] #4 Works correctly in both windowed and fullscreen modes
- [ ] #5 Behavior consistent across both MPV and GStreamer backends
<!-- AC:END -->
