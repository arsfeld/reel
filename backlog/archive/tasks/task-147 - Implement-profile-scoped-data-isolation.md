---
id: task-147
title: Implement profile-scoped data isolation
status: To Do
assignee: []
created_date: '2025-09-17 15:31'
labels:
  - backend
  - data
dependencies: []
priority: high
---

## Description

Ensure that watch history, playback progress, and user preferences are properly isolated between different Plex profiles.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Update DataService to include profile_id in queries
- [ ] #2 Modify cache keys to include profile identifier
- [ ] #3 Ensure playback progress saves with profile context
- [ ] #4 Update sync manager to sync per-profile data
- [ ] #5 Clear profile-specific cache on profile switch
- [ ] #6 Implement profile-aware media item queries
- [ ] #7 Add profile validation to prevent cross-profile data access
<!-- AC:END -->
