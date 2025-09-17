---
id: task-142
title: Extend Plex auth module to support Home users
status: To Do
assignee: []
created_date: '2025-09-17 15:30'
labels:
  - backend
  - plex
  - auth
dependencies: []
priority: high
---

## Description

Modify the existing Plex authentication module to support listing Home users, switching between profiles, and handling PIN authentication for protected profiles.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Add PlexHomeUser struct with id, name, isProtected, hasAdmin fields
- [ ] #2 Implement get_home_users() function to list available profiles
- [ ] #3 Implement switch_user() function with optional PIN parameter
- [ ] #4 Add token management for multiple user contexts
- [ ] #5 Handle PIN validation errors gracefully
- [ ] #6 Add unit tests for new authentication functions
- [ ] #7 Ensure backward compatibility with single-user flow
<!-- AC:END -->
