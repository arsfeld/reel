---
id: task-462.01
title: Add authentication status tracking to Source model and database
status: Done
assignee: []
created_date: '2025-11-20 23:42'
updated_date: '2025-11-20 23:48'
labels:
  - database
  - models
  - authentication
dependencies: []
parent_task_id: task-462
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add fields to track authentication state for sources so we can distinguish between:
- Authenticated and connected
- Authenticated but disconnected (network/server issue)
- Not authenticated (token expired/invalid)

This requires:
- Add `auth_status` field to source entity (enum: Authenticated, AuthRequired, Unknown)
- Add `last_auth_check` timestamp to track when authentication was last verified
- Create migration to add new fields to sources table
- Update Source model to include authentication status

This is the foundation for displaying proper UI indicators and triggering re-authentication.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 auth_status field exists in sources table with proper enum values
- [x] #2 last_auth_check timestamp field exists in sources table
- [x] #3 Migration runs successfully without data loss
- [x] #4 Source entity includes AuthStatus enum
- [x] #5 Source model can be serialized/deserialized with new fields
<!-- AC:END -->
