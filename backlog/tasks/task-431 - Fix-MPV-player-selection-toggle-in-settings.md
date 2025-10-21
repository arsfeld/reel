---
id: task-431
title: Fix MPV player selection toggle in settings
status: To Do
assignee: []
created_date: '2025-10-21 03:11'
labels:
  - bug
  - player
  - settings
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
In playback settings the player backend selector ignores changes: choosing MPV still leaves GStreamer active (and vice versa). Ensure the selected engine takes effect immediately or after a documented restart, and the UI reflects the active backend.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Reproduce the issue where changing the player backend selection has no effect and record current behaviour.
- [ ] #2 Implement logic so that selecting MPV switches playback to the MPV backend (and selecting GStreamer restores that backend).
- [ ] #3 Update the settings UI to reflect the currently active backend and communicate any restart requirements.
- [ ] #4 Add regression coverage (automated or manual checklist) ensuring backend selection persists and applies on both Linux and macOS builds.
<!-- AC:END -->
