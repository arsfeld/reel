---
id: task-160
title: Fix TV show seasons and episode counts not persisting in database
status: To Do
assignee: []
created_date: '2025-09-17 19:40'
labels:
  - bug
  - database
  - sync
  - critical
dependencies: []
priority: high
---

## Description

ALL TV shows are displaying with 0 seasons and 0 episode count in the UI, even though episodes exist and are being synced. The root issue is that Show metadata (particularly the seasons array and total_episode_count) is not being properly persisted to the database during sync or updates. Episodes are syncing correctly but the parent Show entity retains empty/zero values for seasons and episode counts.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Investigate why Show seasons array is not persisting to database during sync
- [ ] #2 Debug the metadata serialization/deserialization for Show entities
- [ ] #3 Fix the database update mechanism to properly persist Show metadata
- [ ] #4 Ensure sync process correctly updates existing shows with season data
- [ ] #5 Verify Shows are saved with correct seasons data during initial library sync
- [ ] #6 Add comprehensive logging to track Show metadata through save/update cycle
- [ ] #7 Test that show_details page displays seasons and episode counts after fix
<!-- AC:END -->
