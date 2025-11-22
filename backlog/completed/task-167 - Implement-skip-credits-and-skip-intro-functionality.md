---
id: task-167
title: Implement skip credits and skip intro functionality
status: Done
assignee: []
created_date: '2025-09-18 13:49'
updated_date: '2025-10-05 23:07'
labels:
  - player
  - ui
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add Netflix-style skip intro and skip credits buttons that appear at appropriate times during playback, allowing users to quickly jump past opening sequences and end credits
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Skip intro button appears during intro sequences
- [x] #2 Skip credits button appears during end credits
- [x] #3 Buttons have configurable detection thresholds
- [x] #4 Buttons auto-hide after timeout period
- [x] #5 Skip functionality works with both MPV and GStreamer players
- [x] #6 User preferences for auto-skip are persisted
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
This high-level task has been broken down into specific implementation tasks:
- task-381 (database schema) ✅ Done
- task-382 (marker fetching) ✅ Done  
- task-383 (UI implementation) ✅ Done
- task-403 (user preferences) ✅ Done

All subtasks are complete. This parent task can now be marked as Done after verifying all acceptance criteria.
<!-- SECTION:NOTES:END -->
