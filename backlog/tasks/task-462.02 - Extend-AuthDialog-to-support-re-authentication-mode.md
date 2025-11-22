---
id: task-462.02
title: Extend AuthDialog to support re-authentication mode
status: Done
assignee: []
created_date: '2025-11-20 23:42'
updated_date: '2025-11-21 00:00'
labels:
  - ui
  - authentication
  - dialog
dependencies: []
parent_task_id: task-462
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Modify the AuthDialog to support re-authenticating existing sources without creating new ones.

Changes needed:
- Add ReauthMode to AuthDialog with source_id and source_type
- Pre-populate dialog fields based on existing source (server name, type, etc.)
- Show different title/description for re-auth mode ("Re-authenticate [Source Name]")
- Add explanatory text about why re-authentication is needed
- On successful authentication, update existing source instead of creating new one
- Clear existing token from storage before starting re-auth flow
- Preserve source ID and all metadata during credential update

UI should clearly indicate this is updating existing source, not creating new one.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 AuthDialog can be opened in re-authentication mode
- [x] #2 Dialog shows appropriate title and description for re-auth
- [x] #3 Dialog is pre-populated with source information
- [x] #4 Successful re-auth updates existing source credentials
- [x] #5 Source ID remains unchanged after re-authentication
- [x] #6 All source metadata is preserved
- [x] #7 User can cancel re-authentication without affecting source
<!-- AC:END -->
