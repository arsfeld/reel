---
id: task-445
title: Fix season number mismatch causing episodes to not load for some shows
status: To Do
assignee: []
created_date: '2025-10-23 01:17'
labels:
  - bug
  - tv-shows
  - episode-loading
dependencies:
  - task-442
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
After fixing task-442, some shows like "The Wheel of Time" (ID: 122654) still show 0 episodes due to a season number mismatch. The show's metadata shows "Season 0: number=3, episodes=8" but the UI queries for season=1. This suggests either:

1. The show's seasons array has incorrect indexing (using array index vs actual season number)
2. The UI is querying with the wrong season number (using a counter instead of the actual season_number field)
3. Episodes are being synced with different season numbers than what's in the show metadata

Examples:
- The Wheel of Time: metadata shows season number=3, but UI queries season=1
- Need to check if this affects other shows as well

This is blocking users from viewing episodes for affected shows even though the episodes may be correctly synced to the database.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Shows with non-sequential season numbers display episodes correctly
- [ ] #2 UI queries episodes using the correct season_number from show metadata
- [ ] #3 Season selector displays the correct season number to users
- [ ] #4 Episodes load successfully for shows like 'The Wheel of Time'
- [ ] #5 Verify fix works for shows with season 0 (specials), season gaps, or non-standard numbering
<!-- AC:END -->
