---
id: task-466
title: Break down player.rs into smaller focused modules
status: Done
assignee: []
created_date: '2025-11-22 18:33'
updated_date: '2025-11-22 19:00'
labels: []
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The `src/ui/pages/player.rs` file is currently 3,515 lines and handles too many responsibilities. This makes it difficult to understand, maintain, and test. Extract self-contained functionality into separate modules within a `player/` directory to improve code organization and maintainability.

The goal is to extract cohesive units of functionality without refactoring the actual logic. Each extraction should be a straightforward code movement that maintains the same behavior while improving file structure.

This work will create a new directory structure:
- `src/ui/pages/player/` (new directory)
  - `mod.rs` (main player component)
  - `controls_visibility.rs` (state machine)
  - `menu_builders.rs` (track/zoom/quality menus)
  - `sleep_inhibition.rs` (screensaver prevention)
  - `backend_manager.rs` (player backend lifecycle)
  - `skip_markers.rs` (intro/credits skip logic)
  - `playlist_navigation.rs` (prev/next episode logic)
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 All code compiles without errors or warnings
- [ ] #2 All existing tests pass without modification
- [ ] #3 No behavior changes - only code movement and module organization
- [ ] #4 Each extracted module has clear, focused responsibility
- [ ] #5 PlayerPage component in mod.rs is significantly smaller and easier to understand
- [ ] #6 All state fields are properly accessible to extracted modules
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Successfully extracted player.rs into smaller focused modules:

**File Size Reduction:**
- Original: ~3,515 lines in single file
- New mod.rs: 2,592 lines (26% reduction)
- Extracted into 6 focused modules totaling 1,130 lines

**Modules Created:**
1. sleep_inhibition.rs (50 lines) - Sleep/screensaver inhibition
2. menu_builders.rs (364 lines) - Audio/subtitle/zoom/quality menus
3. controls_visibility.rs (137 lines) - Control visibility state machine
4. backend_manager.rs (219 lines) - Player backend lifecycle
5. playlist_navigation.rs (94 lines) - Previous/next navigation
6. skip_markers.rs (266 lines) - Skip intro/credits logic

**Tasks Completed:**
- ✅ task-466.07: Created player/ directory structure
- ✅ task-466.01: Extracted sleep inhibition logic  
- ✅ task-466.02: Extracted menu builder logic
- ✅ task-466.03: Extracted control visibility state machine
- ✅ task-466.04: Extracted backend management logic
- ✅ task-466.06: Extracted playlist navigation logic
- ✅ task-466.05: Extracted skip intro/credits logic

**Result:**
Code compiles successfully with all functionality preserved. The player module is now significantly more maintainable with clear separation of concerns. All 6 planned modules have been successfully extracted.
<!-- SECTION:NOTES:END -->
