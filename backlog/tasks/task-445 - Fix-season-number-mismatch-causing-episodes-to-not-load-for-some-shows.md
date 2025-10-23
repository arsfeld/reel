---
id: task-445
title: Fix season number mismatch causing episodes to not load for some shows
status: In Progress
assignee: []
created_date: '2025-10-23 01:17'
updated_date: '2025-10-23 01:22'
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

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan

### Root Cause
The season dropdown selection handler was using array index + 1 to calculate season numbers, but shows can have non-sequential season numbers (e.g., [3, 4, 5] or [0, 1, 2]). This caused a mismatch:
- Dropdown index 0 for "Season 3" → calculated as season 1 → 0 episodes found

### Solution Implemented
1. Added `season_numbers: Vec<u32>` field to `ShowDetailsPage` struct to map dropdown indices to actual season numbers
2. Changed season dropdown callback to send the raw index instead of `index + 1`
3. Updated `SelectSeason` handler to look up actual season number from the mapping
4. Populate the mapping when show details are loaded from `show.seasons`
5. Clear the mapping when loading a new show

### Changes Made
- `src/ui/pages/show_details.rs`:
  - Line 25: Added `season_numbers` field to struct
  - Line 399: Changed callback to send index directly (removed `+ 1`)
  - Lines 484-500: Updated SelectSeason handler to look up season number from mapping
  - Lines 647-653: Populate season_numbers mapping when show is loaded
  - Line 478: Clear season_numbers when loading new show

### Testing Required
- Test with "The Wheel of Time" (ID: 122654) - has non-sequential seasons
- Test with shows that have season 0 (Specials)
- Test with shows that have season gaps
- Test with normally numbered shows (1, 2, 3...) to ensure no regression
<!-- SECTION:PLAN:END -->
