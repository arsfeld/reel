---
id: task-467.01
title: Extract auto-play logic to AutoPlayManager
status: Done
assignee: []
created_date: '2025-11-22 19:05'
updated_date: '2025-11-22 21:55'
labels:
  - refactoring
  - player
dependencies: []
parent_task_id: task-467
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extract auto-play functionality from player mod.rs into a dedicated `auto_play.rs` module with an AutoPlayManager struct.

State to extract:
- `auto_play_triggered: bool`
- `auto_play_timeout: Option<SourceId>`

Logic to extract:
- Auto-play trigger detection (>95% completion)
- Countdown timer management (5 second delay)
- Navigation to next episode
- Toast notification for "No more episodes"
- Timeout cancellation on manual navigation

AutoPlayManager API:
- `new() -> Self`
- `check_auto_play(position, duration, context, sender)` - Check if auto-play should trigger
- `cancel()` - Cancel pending auto-play
- `reset()` - Reset triggered state
- Drop impl for cleanup
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Create src/ui/pages/player/auto_play.rs with AutoPlayManager struct
- [ ] #2 Extract auto-play state fields from PlayerPage
- [ ] #3 Move auto-play trigger logic to manager
- [ ] #4 Move countdown and navigation logic to manager
- [ ] #5 Code compiles without errors
- [ ] #6 Auto-play works correctly with 5 second countdown
<!-- AC:END -->
