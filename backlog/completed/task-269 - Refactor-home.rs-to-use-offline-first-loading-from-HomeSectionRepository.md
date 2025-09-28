---
id: task-269
title: Refactor home.rs to use offline-first loading from HomeSectionRepository
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 18:11'
updated_date: '2025-09-26 18:34'
labels:
  - backend
  - offline
  - database
dependencies:
  - task-270
---

## Description

Modify the HomePage component to load sections directly from HomeSectionRepository (database cache) without triggering API calls. The UI should only read from cache and react to sync worker updates.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Replace get_cached_home_sections with HomeSectionRepository::get_by_source
- [x] #2 Return cached sections immediately (no await on API calls)
- [ ] #3 If online and cache stale (>5 min), trigger background refresh
- [ ] #4 Save fresh sections to DB using HomeSectionRepository during background refresh
- [x] #5 Preserve hub_identifier, ordering, and all metadata when persisting
- [x] #6 Handle empty cache gracefully on first run
- [ ] #7 Remove BackendService::get_home_sections_per_source call from LoadData handler
- [x] #8 Load sections directly from HomeSectionRepository::get_by_source
- [x] #9 Remove background API refresh logic from UI component
- [x] #10 Keep source loading states for UI feedback during sync
- [ ] #11 Subscribe to sync worker notifications for section updates
- [x] #12 Display cached data immediately on page load
<!-- AC:END -->


## Implementation Plan

1. Review current BackendService implementation and get_cached_home_sections method
2. Examine HomeSectionRepository interface and available methods
3. Refactor get_cached_home_sections to load from database first
4. Implement background refresh logic with 5-minute staleness check
5. Add proper error handling for empty cache on first run
6. Test the offline-first behavior and background refresh
