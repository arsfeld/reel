---
id: task-463.01
title: Add automatic navigation when episode ends without next episode
status: Done
assignee: []
created_date: '2025-11-22 18:02'
updated_date: '2025-11-22 18:17'
labels:
  - player
  - navigation
  - ux
dependencies: []
parent_task_id: task-463
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The auto-play logic in `src/ui/pages/player.rs` (lines 3298-3319) only handles cases where a next episode exists. When playback reaches >95% completion AND there is no next episode available, the code silently does nothing, leaving the user in the player view with no clear indication or automatic navigation back.

**Current Behavior**:
- Auto-play checks `context.has_next()`
- If true, schedules next episode load
- If false, nothing happens - no fallback action

**Needed Behavior**:
- Detect when episode finishes AND no next episode exists
- Automatically navigate back to show details/episode list
- Show toast notification indicating end of season/show
- Ensure navigation happens after watch status is saved

**Key Files**:
- `src/ui/pages/player.rs` - Auto-play logic
- `src/ui/main_window/navigation.rs` - Navigation handler
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 When final episode of season ends, user is automatically returned to episode list
- [x] #2 Toast notification shows "End of season" or similar message
- [x] #3 Navigation occurs after watch status is saved locally
- [x] #4 Manual navigation still works if user clicks back before auto-play timeout
- [x] #5 Behavior is consistent across both MPV and GStreamer backends
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented automatic navigation when episode ends without next episode. The player now:
- Detects when playback reaches >95% completion AND there is no next episode
- Schedules a NavigateBack after 5 seconds to allow watch status to sync
- Shows an "End of season" toast notification to the user
- Handles edge cases when auto-play is disabled or playlist context is missing

Changes made in src/ui/pages/player.rs:
- Added else branch in auto-play logic to handle case when context.has_next() is false
- Navigation occurs 5 seconds after episode ends to ensure watch status sync completes
- Added logging for debugging completion scenarios
<!-- SECTION:NOTES:END -->
