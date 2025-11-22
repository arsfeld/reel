---
id: task-420.06
title: Update library.rs to use new module structure
status: Done
assignee:
  - '@claude-code'
created_date: '2025-10-06 17:36'
updated_date: '2025-10-06 18:11'
labels: []
dependencies: []
parent_task_id: task-420
---

## Description

Update the main library.rs file to declare the new modules and re-export necessary types. Update all internal references to use the new module structure.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add mod declarations for all new modules (types, messages, filters, ui_builders, data)
- [x] #2 Add pub use statements to re-export public types
- [x] #3 Update library.rs to only contain LibraryPage struct, Debug impl, Drop impl, and AsyncComponent trait implementation
- [x] #4 Update all internal method calls to reference module paths
- [x] #5 Remove moved code from library.rs
- [x] #6 Code compiles without errors
<!-- AC:END -->


## Implementation Plan

1. Agent created library/mod.rs with module structure
2. Add missing #[relm4::component(pub async)] attribute
3. Add explicit view! macro import
4. Verify all modules are declared and types are re-exported
5. Verify compilation


## Implementation Notes

Converted src/ui/pages/library.rs to library/mod.rs with module structure:

- Created library/mod.rs with module declarations for: types, messages, filters, ui_builders, data
- Added pub use statements to re-export public types
- Kept LibraryPage struct and Debug impl in mod.rs
- Kept AsyncComponent implementation with view! macro in mod.rs
- Added missing #[relm4::component(pub async)] attribute (was causing compilation errors)
- Added explicit `use relm4::view;` macro import for proper macro resolution in module files
- Old library.rs file replaced with library/ directory structure
- All 5 submodules working correctly with pub(super) visibility
- Code compiles without errors
