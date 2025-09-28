---
id: task-170
title: Consolidate worker components from Relm4 and services
status: Done
assignee:
  - '@claude'
created_date: '2025-09-18 14:12'
updated_date: '2025-09-18 14:23'
labels:
  - refactoring
  - architecture
dependencies: []
priority: high
---

## Description

Merge src/platforms/relm4/components/workers/ and src/services/workers/ into a single unified workers directory. This eliminates duplication and creates a single source of truth for all worker implementations.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Move all workers from src/platforms/relm4/components/workers/ to src/workers/
- [x] #2 Move existing workers from src/services/workers/ to src/workers/
- [x] #3 Update all import paths in the codebase
- [x] #4 Ensure no functionality is lost during consolidation
- [x] #5 All tests pass after consolidation
<!-- AC:END -->


## Implementation Plan

1. Analyze current worker structure in both locations
2. Create consolidated src/workers/ directory structure
3. Move Relm4 workers (image_loader, search_worker, sync_worker)
4. Move service workers (connection_worker)
5. Update module declarations and imports
6. Update all references throughout codebase
7. Build and test


## Implementation Notes

Successfully consolidated all worker components from src/platforms/relm4/components/workers/ and src/services/workers/ into a unified src/workers/ directory.

Changes made:
- Created new src/workers/ directory
- Moved all Relm4 workers (connection_monitor, image_loader, search_worker, sync_worker)
- Removed unused service_connection_worker (as confirmed by user)
- Updated all import paths in pages (home.rs, library.rs, show_details.rs)
- Removed worker modules from both platforms/relm4/components and services
- Added workers module to main.rs
- Verified build compiles successfully
