---
id: task-398
title: Fix 4 failing Plex backend runtime tests
status: Done
assignee:
  - '@claude-code'
created_date: '2025-10-04 12:34'
updated_date: '2025-10-04 12:42'
labels:
  - testing
  - bugfix
  - plex
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Four Plex backend tests are failing at runtime: test_error_handling_server_error, test_movie_fetching, test_progress_update, and test_show_fetching. These need to be investigated and fixed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Investigate why test_error_handling_server_error is failing
- [x] #2 Investigate why test_movie_fetching is failing
- [x] #3 Investigate why test_progress_update is failing
- [x] #4 Investigate why test_show_fetching is failing
- [x] #5 Fix all identified issues
- [x] #6 All 4 tests pass successfully
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Run all 4 failing tests to capture error details
2. Analyze each failure and identify root causes
3. Fix each issue one by one
4. Verify all tests pass
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed all 4 failing Plex backend tests by updating mock server configurations.

Root Cause:
The tests were failing because mockito was returning 501 Not Implemented instead of the expected responses. This happened because the mock URLs did not match the actual API requests:
- API requests include query parameters: /library/sections/{id}/all?includeExtras=1&includeRelated=1&includePopularLeaves=1&includeGuids=1
- Test mocks only specified: /library/sections/{id}/all

Changes Made:
1. test_movie_fetching: Added query parameters to mock URL
2. test_show_fetching: Added query parameters to mock URL
3. test_error_handling_server_error: Added query parameters to mock URL
4. test_progress_update: Changed HTTP method from GET to POST and removed incorrect playbackTime parameter

All tests now pass successfully (18/18 Plex tests passing).
<!-- SECTION:NOTES:END -->
