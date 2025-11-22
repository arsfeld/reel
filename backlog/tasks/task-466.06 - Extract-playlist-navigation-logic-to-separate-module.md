---
id: task-466.06
title: Extract playlist navigation logic to separate module
status: Done
assignee: []
created_date: '2025-11-22 18:34'
updated_date: '2025-11-22 18:52'
labels: []
dependencies:
  - task-466.07
parent_task_id: task-466
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extract the playlist navigation functionality from player.rs into a dedicated `playlist_navigation.rs` module. This handles previous/next episode navigation and playlist context updates.

Current location: Lines 829-868 and 2594-2632 in player.rs
Code to extract:
- `update_playlist_position_label()` method
- Previous/Next episode navigation logic
- Playlist context management

Required state fields:
- `playlist_context: Option<PlaylistContext>`
- `can_go_previous: bool`
- `can_go_next: bool`
- `playlist_position_label: gtk::Label`
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 New file created: src/ui/pages/player/playlist_navigation.rs
- [ ] #2 Playlist navigation methods moved to new module
- [ ] #3 Playlist position label updates properly encapsulated
- [ ] #4 Code compiles without errors
- [ ] #5 Previous/next navigation works correctly for TV show episodes and play queues
<!-- AC:END -->
