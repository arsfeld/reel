---
id: task-270
title: Refactor BackendService to separate API from cache concerns
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 18:21'
updated_date: '2025-09-26 18:31'
labels:
  - backend
  - refactoring
dependencies: []
---

## Description

Clean up BackendService to only handle API operations. Remove cache-related methods like get_cached_home_sections and move that responsibility to repositories. BackendService should be stateless and only fetch from remote APIs.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Remove get_cached_home_sections method entirely
- [x] #2 Rename get_home_sections_per_source to get_home_sections
- [x] #3 Ensure get_home_sections only fetches from API, no cache reads
- [x] #4 Remove any cache writing from get_home_sections (sync worker handles that)
- [x] #5 Keep method focused on API transformation to HomeSectionWithModels
<!-- AC:END -->

## Implementation Notes

Successfully refactored BackendService to be stateless and only handle API operations:\n\n1. Removed get_cached_home_sections() method completely\n2. Renamed get_home_sections_per_source() to get_home_sections()\n3. Removed all database read/write operations from get_home_sections()\n4. Updated HomePage to only read from HomeSectionRepository cache\n5. Removed API calls from HomePage that weren't persisting data\n\nThe architecture is now cleaner with proper separation of concerns:\n- BackendService: Stateless, API-only operations\n- HomeSectionRepository: Handles all cache persistence\n- HomePage: Only reads from cache (offline-first)\n- Sync Worker: Will be responsible for fetching from API and persisting (future work)
