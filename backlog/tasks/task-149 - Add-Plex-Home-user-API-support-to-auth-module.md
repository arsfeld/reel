---
id: task-149
title: Add Plex Home user API support to auth module
status: Done
assignee:
  - '@claude'
created_date: '2025-09-17 15:36'
updated_date: '2025-09-22 18:04'
labels:
  - backend
  - plex
  - auth
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extend the PlexAuth module with two new functions to support Home users during authentication: get_home_users() to list available profiles after initial auth, and switch_to_user() to obtain a token for a specific profile with optional PIN support. This is a minimal extension that only adds what's needed for profile selection at login time.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add PlexHomeUser struct with id, name, is_protected, is_admin, thumb fields
- [x] #2 Implement get_home_users() function to fetch Home users from Plex API
- [x] #3 Implement switch_to_user() function with optional PIN parameter
- [x] #4 Add proper error handling for invalid PINs and API failures
- [x] #5 Add unit tests for new API functions
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Research Plex Home users API endpoints in plex.tv documentation
2. Add PlexHomeUser struct with required fields (id, name, is_protected, is_admin, thumb)
3. Implement get_home_users() to fetch list of available Home users after auth
4. Implement switch_to_user() with PIN support for switching to a specific user
5. Add error handling for invalid PINs and API failures
6. Write unit tests for both new functions
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented Plex Home user API support in the auth module:

- Added PlexHomeUser struct with all required fields (id, name, is_protected, is_admin, thumb)
- Implemented get_home_users() function to fetch Home users from Plex API endpoint /api/v2/home/users
- Implemented switch_to_user() function with optional PIN support for switching to specific users
- Added proper error handling for invalid PINs (401), missing PINs (403), and other API failures
- Created comprehensive unit tests for both functions with mockito
- Added test-specific versions of functions (_with_url) to avoid environment variable usage in tests
- Exported PlexHomeUser struct from the plex module for external use

The implementation follows existing patterns in the auth module and maintains consistency with the current API design.
<!-- SECTION:NOTES:END -->
