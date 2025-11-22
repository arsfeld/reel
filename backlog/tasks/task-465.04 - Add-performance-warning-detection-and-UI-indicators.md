---
id: task-465.04
title: Add performance warning detection and UI indicators
status: To Do
assignee: []
created_date: '2025-11-22 18:32'
updated_date: '2025-11-22 18:36'
labels: []
dependencies:
  - task-465.02
parent_task_id: task-465
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement performance warning detection logic as a separate module/utility that can be used by the BufferingOverlay component.

Create reusable warning detection functions that take current stats and return warning state, keeping all logic out of player.rs. The BufferingOverlay component will use these utilities internally.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Warning detection logic in separate module (e.g., player/buffering_warnings.rs)
- [ ] #2 Function to detect slow download: is_download_too_slow(speed, bitrate) -> bool
- [ ] #3 Function to detect stalled buffering: is_buffering_stalled(history) -> bool
- [ ] #4 Warning messages defined as constants or helper functions
- [ ] #5 Logic is pure/stateless where possible
- [ ] #6 BufferingOverlay can import and use these utilities

- [ ] #7 No coupling to PlayerPage internals
<!-- AC:END -->
