---
id: task-361
title: Seek to last played position when starting playback
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 14:44'
updated_date: '2025-10-03 14:52'
labels:
  - player
  - playback
  - feature
dependencies: []
priority: high
---

## Description

Implement resume functionality that automatically seeks to the last saved playback position when starting media playback. This allows users to continue watching from where they left off.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Retrieve last playback position from database when loading media
- [x] #2 Automatically seek to saved position after media is loaded
- [x] #3 Only seek if position is > 0 and not near the end (< 95% complete)
- [x] #4 Update UI to show current position after seeking
- [x] #5 Works correctly for both movies and episodes
- [x] #6 No seek occurs for media watched to completion
<!-- AC:END -->


## Implementation Plan

1. Study existing code patterns for database access in player (UpdatePlaybackProgressCommand)
2. After successful media load, retrieve playback progress from database
3. Check resume conditions: position > 0 AND progress < 95% AND not watched
4. If conditions met, seek to saved position before starting playback
5. Apply to both LoadMedia and LoadMediaWithContext handlers
6. Test with movies and episodes at different progress levels


## Implementation Notes

Modified GetPlaybackProgressCommand to return full PlaybackProgressModel instead of just (position_ms, duration_ms) tuple. Updated player.rs resume logic to check all three conditions: position > threshold, progress < 95%, and not watched. Applied to both LoadMedia and LoadMediaWithContext handlers. The seek happens after media loads but before playback starts, ensuring smooth resume experience.

Files modified:
- src/services/commands/media_commands.rs: Changed GetPlaybackProgressCommand return type
- src/ui/pages/player.rs: Enhanced resume logic with comprehensive checks

## Bug Fix: Prevent Progress Reset

While testing resume functionality, discovered that playback progress was being accidentally cleared when starting playback. The issue was in `upsert_progress()` which unconditionally overwrote the position even when the new position was 0 (during loading/errors).

Fixed by adding protection against suspicious resets: only reject position updates if the new position is < 5 seconds AND existing position is > 5 seconds AND the item is not being marked as watched. This allows normal seeks (forward/backward) while preventing accidental resets during loading.

File modified:
- src/db/repository/playback_repository.rs: Added smart position update logic in upsert_progress()
