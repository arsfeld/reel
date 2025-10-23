---
id: task-453
title: Add Play Show and Play Season buttons to TV show details page
status: To Do
assignee: []
created_date: '2025-10-23 02:22'
updated_date: '2025-10-23 02:23'
labels:
  - feature
  - ui
  - playback
  - episode
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TV show details page currently has "Mark Show as Watched" and "Mark Season" buttons for changing watch status (from task-450), but lacks buttons to actually start playback of all episodes in a show or season.

Users expect to be able to:
- Click "Play Show" to start continuous playback from the first unwatched episode (or first episode if all watched)
- Click "Play Season" to start continuous playback from the first unwatched episode in the current season
- Have episodes automatically advance to the next episode when one finishes

This is different from the existing watch status buttons, which mark content as watched/unwatched without starting playback.

Expected behavior:
- "Play Show" button should:
  - Find the first unwatched episode across all seasons
  - If all episodes are watched, start from S1E1
  - Start playback with a playlist context containing all episodes in order
  
- "Play Season" button should:
  - Find the first unwatched episode in the current selected season
  - If all episodes in season are watched, start from first episode of season
  - Start playback with a playlist context containing all episodes in that season

This improves user experience by:
- Enabling binge-watching without manual episode selection
- Providing a clear call-to-action for starting show/season playback
- Matching functionality available in Plex, Jellyfin, and streaming services
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Show details page has a 'Play Show' button that starts playback from the first unwatched episode
- [ ] #2 Show details page has a 'Play Season' button that starts playback from the first unwatched episode in the selected season
- [ ] #3 Play Show button creates a playlist context with all episodes in the show in correct order
- [ ] #4 Play Season button creates a playlist context with all episodes in the current season in correct order
- [ ] #5 If all episodes are watched, Play Show starts from S1E1
- [ ] #6 If all episodes in season are watched, Play Season starts from first episode of that season
- [ ] #7 Buttons are clearly labeled to distinguish from 'Mark as Watched' buttons
- [ ] #8 Episode auto-advance works correctly when playing through show/season playlists
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
**Current State:** The existing 'Mark Show as Watched/Unwatched' and 'Mark Season' buttons (from task-450) only update watch status metadata - they do not start playback. Users clicking these buttons expecting playback to start will see the show reload but nothing plays.

**Implementation Hint:** Look at how movie_details.rs implements the Play button for reference. Episode playback likely needs playlist context from PlaylistContext model to enable auto-advance between episodes.
<!-- SECTION:NOTES:END -->
