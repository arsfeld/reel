---
id: task-264
title: Update BackendService to persist home sections
status: To Do
assignee: []
created_date: '2025-09-26 17:49'
updated_date: '2025-09-26 18:05'
labels: []
dependencies:
  - task-263
---

## Description

Modify the BackendService::get_home_sections_per_source method to store the fetched Plex home sections in the database using the HomeSectionRepository. This enables offline access to the exact Plex sections.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Import and initialize HomeSectionRepository in get_home_sections_per_source
- [ ] #2 After fetching sections from Plex API, save them to database
- [ ] #3 Preserve hub_identifier, original titles, and section ordering
- [ ] #4 Handle section items relationship properly
- [ ] #5 Update error handling to log but not fail on save errors
- [ ] #6 Ensure transactional consistency when saving sections
<!-- AC:END -->

## Implementation Notes

Blocked by task-263: HomeSectionRepository must be completed first before this task can be implemented.
