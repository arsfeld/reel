---
id: task-219
title: Add profile selection step to Plex authentication flow
status: To Do
assignee: []
created_date: '2025-09-22 18:08'
labels:
  - ui
  - plex
  - auth
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Integrate the Plex Home user API functionality into the authentication UI. After initial OAuth authentication, detect if the account has multiple Home users and present a profile selection dialog. Support PIN entry for protected profiles and store the selected profile's token for subsequent API calls.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Detect if authenticated account has Home users after OAuth
- [ ] #2 Create profile selection dialog component with user avatars and names
- [ ] #3 Add PIN entry dialog for protected profiles
- [ ] #4 Store selected profile token in auth state
- [ ] #5 Update PlexBackend to use profile-specific token
- [ ] #6 Add visual indicators for admin/protected status on profiles
- [ ] #7 Handle profile switching without re-authentication
<!-- AC:END -->
