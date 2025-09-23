---
id: task-230
title: Refactor player controls visibility to use proper state machine
status: Done
assignee:
  - '@claude'
created_date: '2025-09-23 19:23'
updated_date: '2025-09-23 19:35'
labels:
  - ui
  - player
  - refactoring
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Replace the current complex boolean flag system with a clean 3-state machine (HIDDEN, VISIBLE, HOVERING) as specified in docs/player-states.md, while preserving smooth fade animations for better UX
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Implement ControlState enum with Hidden, Visible, and Hovering variants
- [x] #2 Replace boolean flags (show_controls, controls_fade_out, mouse_over_controls) with single state field
- [x] #3 Implement proper state transitions with clear entry/exit actions for each state
- [x] #4 Add mouse movement threshold (5px) to avoid triggering on micro-movements
- [x] #5 Add debouncing (50ms) for window enter/exit events to prevent flickering
- [x] #6 Detect actual control widget bounds for hover detection instead of using window percentage heuristic
- [x] #7 Simplify event flow to match state machine design (no cascading events)
- [x] #8 Show controls on any keyboard input, not just Tab key
- [x] #9 Preserve smooth fade-in/fade-out animations when transitioning between states
- [x] #10 Make timings configurable (inactivity timeout, animation duration, debounce delay)
- [x] #11 Ensure cursor visibility always matches control visibility state
- [x] #12 Update documentation to reflect new implementation with animation details
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add ControlState enum with Hidden, Visible, and Hovering variants
2. Replace boolean flags with single state field in PlayerPage struct
3. Create state transition methods following state machine design
4. Add mouse movement threshold and debouncing logic
5. Update event handlers to use proper state transitions
6. Implement actual control bounds detection for hover
7. Update keyboard input handling to show controls on any key
8. Add configurable timing constants
9. Preserve fade animations by updating CSS classes based on state
10. Ensure cursor visibility follows control state
11. Update all event flows to match state machine
12. Test state transitions and animation behavior
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Refactored player controls visibility to use a proper state machine with 3 states (Hidden, Visible, Hovering) as specified in docs/player-states.md.

Key changes:
- Added ControlState enum with clear state definitions
- Replaced multiple boolean flags with single state field
- Implemented state transition methods with proper entry/exit actions
- Added mouse movement threshold (5px) to prevent micro-movements
- Fixed timer management to prevent crashes when timer has already fired
- Integrated control bounds detection for hover state
- Updated keyboard handling to show controls on any key press
- Made all timings configurable
- Preserved smooth CSS animations during transitions
- Ensured cursor visibility always matches control state

The implementation properly handles all state transitions as defined in the state machine diagram and prevents the cascading event issues that existed with the boolean flag system.
<!-- SECTION:NOTES:END -->
