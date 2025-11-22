---
id: task-465.05
title: Integrate buffering overlay into PlayerPage
status: To Do
assignee: []
created_date: '2025-11-22 18:32'
updated_date: '2025-11-22 18:36'
labels: []
dependencies:
  - task-465.01
  - task-465.02
  - task-465.03
  - task-465.04
parent_task_id: task-465
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Integrate the BufferingOverlay component into PlayerPage with MINIMAL changes to player.rs.

The integration should be limited to:
1. Adding BufferingOverlay as a child component
2. Forwarding buffering events/stats to the component via simple message passing
3. Adding the overlay widget to the video container

All buffering logic, state management, and UI should remain in the BufferingOverlay component itself.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 BufferingOverlay added as Relm4 child component of PlayerPage
- [ ] #2 Integration adds <20 lines of code to player.rs
- [ ] #3 PlayerPage forwards buffering state from PlayerHandle to overlay via message
- [ ] #4 PlayerPage forwards cache stats to overlay (polled at 1-second intervals)
- [ ] #5 Overlay widget added to video container or overlay stack
- [ ] #6 No buffering logic added to PlayerPage update() or init()
- [ ] #7 Stats polling uses existing GLib timeout pattern from PlayerPage
- [ ] #8 Manual testing confirms overlay appears during media load

- [ ] #9 Manual testing confirms stats update in real-time
- [ ] #10 Overlay shows/hides automatically based on buffering state
<!-- AC:END -->
