---
id: task-149
title: Add Plex Home user API support to auth module
status: To Do
assignee: []
created_date: '2025-09-17 15:36'
labels:
  - backend
  - plex
  - auth
dependencies: []
priority: high
---

## Description

Extend the PlexAuth module with two new functions to support Home users during authentication: get_home_users() to list available profiles after initial auth, and switch_to_user() to obtain a token for a specific profile with optional PIN support. This is a minimal extension that only adds what's needed for profile selection at login time.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Add PlexHomeUser struct with id, name, is_protected, is_admin, thumb fields
- [ ] #2 Implement get_home_users() function to fetch Home users from Plex API
- [ ] #3 Implement switch_to_user() function with optional PIN parameter
- [ ] #4 Add proper error handling for invalid PINs and API failures
- [ ] #5 Add unit tests for new API functions
<!-- AC:END -->
