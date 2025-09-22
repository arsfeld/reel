---
id: task-214
title: Fix remaining non-standard header usage in Plex backend
status: Done
assignee:
  - '@claude'
created_date: '2025-09-22 15:19'
updated_date: '2025-09-22 15:24'
labels:
  - backend
  - plex
  - api
  - cleanup
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Several places in the Plex backend are still using individual headers instead of the standardized headers method created in task-213. This includes identity checks, auth module, and other API calls that should be using the standard_headers() method for consistency.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 All identity endpoint calls use standard headers
- [x] #2 Auth module uses standard headers where applicable
- [x] #3 Connection testing uses standard headers
- [x] #4 All Plex API calls consistently use standard_headers() method
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Search for all header usage in the Plex backend code
2. Identify places not using standard_headers() method
3. Replace individual header construction with standard_headers()
4. Test all modified endpoints
5. Verify consistency across all Plex API calls
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Created a new create_standard_headers() function for standalone usage outside of PlexApi instances.

Changes made:
1. Added create_standard_headers() function in api/client.rs that can be used without a PlexApi instance
2. Refactored PlexApi::standard_headers() to use the new function
3. Updated auth.rs to use create_standard_headers() for all API calls
4. Updated mod.rs to use create_standard_headers() for all identity endpoint calls
5. Verified all other API files already use standard_headers() method correctly

All tests pass and compilation succeeds with no errors.
<!-- SECTION:NOTES:END -->
