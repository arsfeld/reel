---
id: task-465.03
title: Implement buffering overlay UI component
status: To Do
assignee: []
created_date: '2025-11-22 18:32'
updated_date: '2025-11-22 18:36'
labels: []
dependencies:
  - task-465.01
  - task-465.02
parent_task_id: task-465
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create a self-contained BufferingOverlay component as a separate module that handles all buffering UI logic internally.

The component should be completely independent with its own state management, requiring only buffering data to be passed in. It should NOT require modifications to PlayerPage internals - just instantiation and data binding.

Create as a new file in ui/pages/player/buffering_overlay.rs or ui/shared/buffering_overlay.rs with its own Relm4 Component or SimpleComponent implementation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 BufferingOverlay created as separate module/file (not in player.rs)
- [ ] #2 Implements Relm4 Component or SimpleComponent trait
- [ ] #3 Component manages its own visibility state internally
- [ ] #4 Accepts buffering percentage and cache stats via Input messages
- [ ] #5 Displays circular progress indicator or progress bar
- [ ] #6 Shows buffering percentage text (e.g., '42%')
- [ ] #7 Shows download speed in human-readable format
- [ ] #8 Shows total downloaded/total size or bytes cached

- [ ] #9 Component uses GTK Overlay or Box layout suitable for overlay
- [ ] #10 Styling matches player control bar aesthetic
- [ ] #11 Component is responsive to window resizing
- [ ] #12 Component can be instantiated with minimal setup code
<!-- AC:END -->
