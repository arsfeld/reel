---
id: task-018
title: Fix AuthProvider creation in Relm4 auth dialog
status: To Do
assignee: []
created_date: '2025-09-15 02:34'
labels:
  - relm4
  - auth
  - backend
dependencies: []
priority: high
---

## Description

The auth dialog sets auth_provider_id to None because AuthProvider creation is not implemented before setting the field

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 AuthProvider is created and saved before setting auth_provider_id,Source creation includes valid auth_provider_id reference,Authentication flow properly links source to auth provider
<!-- AC:END -->
