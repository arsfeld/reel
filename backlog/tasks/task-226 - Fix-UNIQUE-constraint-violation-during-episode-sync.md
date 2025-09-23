---
id: task-226
title: Fix UNIQUE constraint violation during episode sync
status: To Do
assignee: []
created_date: '2025-09-23 18:26'
labels:
  - backend
  - sync
  - database
  - bug
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
During sync, episodes are failing with 'UNIQUE constraint failed: media_items.parent_id, media_items.season_number, media_items.episode_number'. This happens when trying to insert duplicate episodes for the same show/season/episode combination. The sync process needs to handle existing episodes properly by updating instead of inserting.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Identify why duplicate episodes are being inserted
- [ ] #2 Implement upsert logic for episodes based on parent_id, season_number, and episode_number
- [ ] #3 Handle edge cases where episodes might be re-synced or updated
- [ ] #4 Ensure sync progress continues despite individual episode conflicts
- [ ] #5 Add proper error recovery so one show's failure doesn't stop the entire sync
<!-- AC:END -->
