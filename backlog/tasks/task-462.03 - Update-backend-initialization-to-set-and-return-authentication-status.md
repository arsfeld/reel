---
id: task-462.03
title: Update backend initialization to set and return authentication status
status: Done
assignee: []
created_date: '2025-11-20 23:43'
updated_date: '2025-11-21 02:01'
labels:
  - backend
  - authentication
  - architecture
dependencies: []
parent_task_id: task-462
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Modify MediaBackend::initialize() to properly detect and report authentication status instead of just returning Ok(None).

Changes needed:
- Update MediaBackend trait to return Result&lt;AuthenticationResult&gt; instead of Result&lt;Option&lt;User&gt;&gt;
- AuthenticationResult enum: Authenticated(User), AuthRequired, NetworkError
- Update PlexBackend::initialize() to set auth_status based on token refresh/validation results
- Update JellyfinBackend::initialize() similarly
- Update BackendService to store auth status in database after initialization
- Add logic to periodically re-check authentication status (e.g., on connection failures)

This enables proper detection of when re-authentication is needed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 MediaBackend trait returns detailed authentication status
- [x] #2 PlexBackend correctly identifies when token is expired vs network error
- [x] #3 JellyfinBackend correctly identifies authentication failures
- [x] #4 Authentication status is persisted to database
- [x] #5 Connection failures trigger auth status re-check
<!-- AC:END -->
