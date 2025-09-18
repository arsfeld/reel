---
id: task-171
title: Move Relm4 UI components to src/ui/
status: Done
assignee:
  - '@claude'
created_date: '2025-09-18 14:12'
updated_date: '2025-09-18 14:33'
labels:
  - refactoring
  - ui
  - architecture
dependencies: []
priority: high
---

## Description

Extract UI components from the isolated src/platforms/relm4/components/ directory to a top-level src/ui/ directory. This reduces platform isolation and makes the UI layer more accessible.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create src/ui/ directory structure
- [x] #2 Move src/platforms/relm4/components/pages/ to src/ui/pages/
- [x] #3 Move src/platforms/relm4/components/dialogs/ to src/ui/dialogs/
- [x] #4 Move src/platforms/relm4/components/factories/ to src/ui/factories/
- [x] #5 Move src/platforms/relm4/components/shared/ to src/ui/shared/
- [x] #6 Update all import paths and module declarations
- [x] #7 Verify all UI components still work correctly
<!-- AC:END -->


## Implementation Plan

1. Analyze current structure and component dependencies
2. Create src/ui/ directory structure
3. Move each component subdirectory one by one
4. Update module declarations in mod.rs files
5. Update all import paths across the codebase
6. Build and test to ensure everything works


## Implementation Notes

Successfully moved all Relm4 UI components from src/platforms/relm4/components/ to src/ui/.

Changes made:
- Created src/ui/ directory structure
- Moved pages/, dialogs/, factories/, and shared/ subdirectories
- Moved main_window.rs and sidebar.rs to src/ui/
- Created src/ui/mod.rs to export all modules
- Added ui module to src/main.rs
- Updated all import paths throughout the codebase:
  - Changed crate::platforms::relm4::components:: to crate::ui::
  - Fixed path to player.css stylesheet
- Removed empty src/platforms/relm4/components/ directory
- Updated src/platforms/relm4/mod.rs to remove components module

The build completes successfully with only unrelated warnings about unused imports.
