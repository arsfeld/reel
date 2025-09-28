---
id: task-263
title: Implement HomeSectionRepository
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 17:48'
updated_date: '2025-09-26 18:08'
labels: []
dependencies: []
---

## Description

Create a repository layer for managing home sections in the database. This repository will handle CRUD operations for home sections and their associated media items.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create home_section_repository.rs with HomeSectionRepository trait
- [x] #2 Implement save_sections method to store/update sections from API
- [x] #3 Implement find_by_source method to retrieve sections for offline display
- [x] #4 Implement clear_sections_for_source for refresh operations
- [x] #5 Handle junction table operations for section items
- [x] #6 Add transaction support for atomic updates
- [x] #7 Write unit tests for repository methods
<!-- AC:END -->


## Implementation Plan

1. Study existing repository patterns from media_repository.rs
2. Create home_section_repository.rs with trait and implementation
3. Implement core CRUD operations with transaction support
4. Implement junction table operations for home_section_items
5. Add helper methods for API data transformation
6. Write comprehensive unit tests
7. Update mod.rs to export the new repository

## Implementation Notes

Created HomeSectionRepository with comprehensive functionality:

- Implemented trait with methods for CRUD operations and section management
- Added transaction support for atomic updates when saving sections and items
- Created save_sections() to atomically replace all sections for a source
- Implemented find_by_source() with proper ordering by position
- Added find_by_source_with_items() for complete section/items retrieval
- Implemented junction table operations for managing section items
- Added helper methods: mark_sections_stale(), has_sections(), find_by_hub_identifier()
- Fixed auto-increment issues by using ActiveValue::NotSet for IDs
- Created comprehensive unit tests with proper test database setup
- Updated mod.rs to export the new repository

All tests pass successfully.
