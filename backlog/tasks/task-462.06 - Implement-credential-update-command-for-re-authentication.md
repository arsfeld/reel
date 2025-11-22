---
id: task-462.06
title: Implement credential update command for re-authentication
status: Done
assignee: []
created_date: '2025-11-20 23:43'
updated_date: '2025-11-21 00:00'
labels:
  - backend
  - authentication
  - commands
dependencies: []
parent_task_id: task-462
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create a new command to update credentials for an existing source without creating duplicates.

Implementation:
- Create UpdateSourceCredentialsCommand in auth_commands.rs
- Command takes source_id, new credentials, and auth_provider
- Update auth_provider in database for the source
- Update keyring/file storage with new token
- Re-initialize backend with new credentials
- Emit broker message for UI updates
- Ensure atomic operation (rollback on failure)

This command ensures credential updates are handled properly without data loss.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 UpdateSourceCredentialsCommand exists and is tested
- [x] #2 Command updates database auth_provider correctly
- [x] #3 Command updates keyring/file storage with new token
- [x] #4 Backend is re-initialized with new credentials
- [ ] #5 Operation is atomic (all or nothing)
- [ ] #6 Broker message is emitted on success
- [ ] #7 Error handling covers all failure scenarios
<!-- AC:END -->
