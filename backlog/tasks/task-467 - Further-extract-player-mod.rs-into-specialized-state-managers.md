---
id: task-467
title: Further extract player mod.rs into specialized state managers
status: To Do
assignee: []
created_date: '2025-11-22 19:05'
updated_date: '2025-11-22 19:06'
labels:
  - refactoring
  - player
  - architecture
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The player mod.rs is still 2,593 lines after the initial module extraction (task-466). Further decompose it by extracting stateful subsystems into dedicated manager structs. This follows the same pattern as SkipMarkerManager - encapsulating related state and behavior into focused, testable components.

Target extractions:
1. **Auto-play manager**: Auto-play countdown, timeout management, navigation triggers
2. **Error/retry manager**: Error state, retry attempts with exponential backoff, error recovery
3. **Progress tracker**: Periodic progress saves to DB, watch status sync to backend
4. **Volume manager**: Volume state, slider widget, volume up/down controls
5. **Seek bar manager**: Seek bar widget, position/duration labels, drag/click handling

Each manager will:
- Own its related state fields
- Provide clear public API to PlayerPage
- Handle its own timers and cleanup
- Be independently testable

Goal: Reduce mod.rs from ~2,600 lines to ~1,800-2,000 lines while improving code organization and testability.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 All code compiles without errors or warnings
- [ ] #2 All existing tests pass without modification
- [ ] #3 No behavior changes - only code movement and encapsulation
- [ ] #4 Each extracted manager has clear, focused responsibility
- [ ] #5 PlayerPage mod.rs is reduced from ~2,600 lines to ~1,800-2,000 lines
- [ ] #6 All managers properly clean up resources in Drop implementations
- [ ] #7 All state fields are properly encapsulated in managers
<!-- AC:END -->
