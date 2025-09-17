---
id: task-145
title: Update MediaBackend trait for profile support
status: To Do
assignee: []
created_date: '2025-09-17 15:31'
labels:
  - backend
  - architecture
dependencies: []
priority: high
---

## Description

Extend the MediaBackend trait and Plex implementation to support profile-aware operations, ensuring all API calls use the correct user token.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Add get_profiles() method to MediaBackend trait
- [ ] #2 Add switch_profile() method with PIN parameter
- [ ] #3 Add current_profile field to backend state
- [ ] #4 Update all Plex API calls to use profile-specific tokens
- [ ] #5 Implement profile context in get_libraries() method
- [ ] #6 Update get_playback_progress() to be profile-aware
- [ ] #7 Ensure sync operations respect current profile
- [ ] #8 Add profile parameter to authenticate() method
<!-- AC:END -->
