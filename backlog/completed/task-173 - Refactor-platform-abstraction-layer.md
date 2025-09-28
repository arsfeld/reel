---
id: task-173
title: Refactor platform abstraction layer
status: Done
assignee:
  - '@assistant'
created_date: '2025-09-18 14:12'
updated_date: '2025-09-18 14:41'
labels:
  - refactoring
  - architecture
dependencies: []
priority: high
---

## Description

Move the core Relm4 application files (app.rs, main_window.rs) from src/platforms/relm4/ to a more integrated location while maintaining platform abstraction capability.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Move src/platforms/relm4/app.rs to src/app/relm4_app.rs
- [x] #2 Move src/platforms/relm4/components/main_window.rs to src/ui/main_window.rs
- [x] #3 Create proper platform abstraction in src/app/ for future platform support
- [x] #4 Update main.rs to use new app structure
- [x] #5 Ensure application still launches correctly
- [x] #6 Verify platform detection still works
<!-- AC:END -->


## Implementation Plan

1. Create src/app/ directory structure
2. Move src/platforms/relm4/app.rs to src/app/relm4_app.rs
3. Create platform abstraction module in src/app/mod.rs
4. Update imports and module declarations
5. Update main.rs to use new app structure
6. Remove old platforms directory structure
7. Test that application compiles and runs

## Implementation Notes

Refactored platform abstraction layer:

1. Created src/app/ directory for platform abstraction
2. Moved src/platforms/relm4/app.rs to src/app/app.rs (renamed from relm4_app.rs per user request)
3. Created AppPlatform struct in src/app/mod.rs with run_relm4() and detect_and_run() methods
4. Updated main.rs to use new app::AppPlatform structure
5. Fixed include_str! paths for CSS files (details.css and sidebar.css)
6. Removed old src/platforms/ directory structure

The refactoring maintains platform abstraction capability while simplifying the structure. The AppPlatform::detect_and_run() method is ready for future platform-specific implementations.
