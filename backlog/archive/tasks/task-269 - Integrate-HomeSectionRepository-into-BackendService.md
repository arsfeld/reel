---
id: task-269
title: Integrate HomeSectionRepository into BackendService
status: To Do
assignee: []
created_date: '2025-09-26 18:05'
updated_date: '2025-09-26 18:06'
labels:
  - backend
  - database
dependencies: []
---

## Description

Update BackendService::get_home_sections_per_source to use HomeSectionRepository for persisting home sections to the new database schema instead of the current ad-hoc media item saving.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Replace current media repository usage with HomeSectionRepository
- [ ] #2 Store home section metadata (hub_identifier, title, type, position)
- [ ] #3 Create home_section_items relationships for media items
- [ ] #4 Use database transactions for atomic section updates
- [ ] #5 Implement proper error handling and logging
<!-- AC:END -->

## Implementation Notes

This task is now more critical as it needs to support offline-first architecture. The implementation should ensure BackendService always saves home sections to database during sync, not just during get_home_sections_per_source calls.
