---
id: task-013
title: Implement Relm4 player playlist navigation
status: Done
assignee:
  - '@claude'
created_date: '2025-09-15 02:10'
updated_date: '2025-09-16 00:45'
labels:
  - player
  - relm4
  - navigation
dependencies: []
priority: medium
---

## Description

The Previous/Next buttons in the Relm4 player need to properly navigate through playlists (TV show episodes, movie collections). The playlist context system exists but needs proper integration with the UI controls.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Enable Previous button when not at first item in playlist
- [x] #2 Enable Next button when not at last item in playlist
- [x] #3 Load previous/next media while maintaining playlist context
- [x] #4 Update current index in playlist context on navigation
- [x] #5 Show current position in playlist (e.g., 'Episode 3 of 10')
- [x] #6 Handle edge cases (first/last item in playlist)
<!-- AC:END -->


## Implementation Plan

1. Find where Previous/Next buttons are defined in the player UI
2. Add state tracking for button sensitivity based on playlist context
3. Update button sensitivity when playlist context changes
4. Add position indicator (e.g., "Episode 3 of 10") to the UI
5. Test with TV show episodes to verify navigation works properly


## Implementation Notes

Implemented playlist navigation with button sensitivity control:

1. Added can_go_previous and can_go_next state fields to PlayerPage
2. Updated Previous/Next buttons to be disabled when at first/last item using #[watch] set_sensitive
3. Added playlist position label showing "Show - SxEx - Episode X of Y"
4. Updated LoadMedia and LoadMediaWithContext handlers to properly set navigation state
5. Navigation maintains playlist context through LoadMediaWithContext

The buttons are now properly enabled/disabled based on playlist position, and users can see their current position in the playlist.
