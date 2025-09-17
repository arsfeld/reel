---
id: task-150
title: Add profile selection step to Plex authentication flow
status: To Do
assignee: []
created_date: '2025-09-17 15:36'
labels:
  - frontend
  - ui
  - auth
dependencies: []
priority: high
---

## Description

Modify the AuthDialog component to add an intermediate profile selection step after successful OAuth authentication. When Home users are available, display them for selection before completing the authentication process. This keeps profile selection contained within the existing auth flow without requiring any database or backend changes.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Add ProfileSelectionState to AuthDialog state machine
- [ ] #2 Fetch Home users after receiving initial OAuth token
- [ ] #3 Create profile selection UI with user cards showing name and avatar
- [ ] #4 Add visual indicator for PIN-protected profiles
- [ ] #5 Handle profile selection and proceed to PIN input if needed
- [ ] #6 Skip profile selection if no Home users exist (single user account)
- [ ] #7 Pass selected profile token to source creation
<!-- AC:END -->
