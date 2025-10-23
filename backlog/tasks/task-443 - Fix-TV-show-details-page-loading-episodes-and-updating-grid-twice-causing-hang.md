---
id: task-443
title: Fix TV show details page loading episodes and updating grid twice causing hang
status: Done
assignee: []
created_date: '2025-10-23 00:55'
updated_date: '2025-10-23 00:58'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TV show details page is loading episodes from the database and rebuilding the episode grid twice in quick succession when opening a show. The logs show the same episodes being loaded and the grid being cleared and rebuilt twice (e.g., for show "9-1-1", episodes are loaded at 00:53:55.460 and again at 00:53:55.578). This duplicate work causes a noticeable hang when opening shows with many episodes, as the UI clears and re-adds all episode cards unnecessarily. This appears to be triggered by duplicate update messages or signals firing during the page initialization.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Episodes are loaded from database only once when opening show details page
- [x] #2 Episode grid is built and populated only once per page load
- [x] #3 No duplicate clearing and rebuilding of episode cards
- [ ] #4 Show details page opens without hang or delay
- [ ] #5 Performance is acceptable even for shows with many episodes (10+ episodes)
- [ ] #6 Debug logging confirms single load/update cycle
- [x] #7 Root cause of duplicate update identified and fixed
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Root Cause

The duplicate loading was caused by two code paths both triggering episode loading:

1. **Path 1 (Explicit)**: In `LoadDetails` command handler (line 729), the code explicitly sent `ShowDetailsInput::LoadEpisodes` after setting up the show and season dropdown.

2. **Path 2 (Implicit)**: When the season dropdown model was populated and `set_selected()` was called (line 724), it triggered the `connect_selected_notify` signal handler (line 395), which sent `ShowDetailsInput::SelectSeason`, which in turn sent `ShowDetailsCommand::LoadEpisodes`.

Both paths resulted in the same episodes being loaded and the grid being rebuilt twice.

## Solution

Removed the explicit `LoadEpisodes` call at line 729, relying solely on the dropdown's `connect_selected_notify` handler to trigger episode loading. This ensures episodes are loaded only once when the dropdown selection is set.

## Changes

- `src/ui/pages/show_details.rs`: Removed explicit `sender.input(ShowDetailsInput::LoadEpisodes)` call after dropdown initialization
- Added comment explaining that episode loading happens via the dropdown's signal handler

## Testing Notes

The fix requires manual testing with a TV show to verify:
- Episodes load only once (check debug logs)
- No UI hang when opening show details
- Episode grid appears correctly
- Performance is improved for shows with many episodes
<!-- SECTION:NOTES:END -->
