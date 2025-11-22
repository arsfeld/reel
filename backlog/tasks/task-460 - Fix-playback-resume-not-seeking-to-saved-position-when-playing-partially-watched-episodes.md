---
id: task-460
title: >-
  Fix playback resume not seeking to saved position when playing
  partially-watched episodes
status: In Progress
assignee: []
created_date: '2025-11-03 02:57'
updated_date: '2025-11-03 03:03'
labels:
  - bug
  - player
  - playback
  - resume
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When playing a TV show episode that has already been partially watched (e.g., 50% complete), the player does not automatically resume from the saved playback position. Instead, it starts from the beginning.

**Observed Behavior:**
- Episode 135555 has saved position of 1762504ms (~29 minutes into 58-minute episode)
- When user clicks to play the episode, it loads from the beginning (position 0)
- Player attempts to update position to 3025ms
- Playback repository correctly rejects this as "Suspicious position reset" and keeps existing position
- However, the player never actually seeks to the saved position (1762504ms)

**Log Evidence:**
```
2025-11-03T02:56:03.755313Z DEBUG reel::ui::pages::home: Media item selected: 135555
2025-11-03T02:56:04.533327Z  INFO reel::player::gstreamer_player: Loading media: http://127.0.0.1:50000/stream/...
2025-11-03T02:56:05.549735Z  INFO reel::player::gstreamer_player: Starting playback
2025-11-03T02:56:08.807016Z  WARN reel::db::repository::playback_repository: Suspicious position reset detected for media_id=135555: attempted to set position to 3025ms from 1762504ms (duration=3509472ms). Keeping existing position.
```

**Root Cause:**
The resume functionality implemented in task-361 is not working correctly. The player should retrieve the saved position from the database and seek to it after loading the media but before starting playback. This seek is not happening.

**Impact:**
Users lose their place when watching TV shows and must manually seek to where they left off, breaking the "Continue Watching" user experience.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Player retrieves saved playback position from database when loading partially-watched media
- [x] #2 Player automatically seeks to saved position after media loads but before playback starts
- [x] #3 Resume only occurs if position > 5 seconds and progress < 95%
- [x] #4 Playback repository's suspicious reset protection continues to work correctly
- [x] #5 Works for both TV episodes and movies
- [ ] #6 Manual testing confirms episode 135555 resumes at saved position (~29 minutes)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Root Cause

The bug was in the mismatch between how playback progress is saved vs. retrieved:

**When Saving** (`src/services/core/media.rs:817`):
- Uses `user_id = None` (comment: "Use None for user_id for now (single-user system)")
- Calls `repo.upsert_progress(media_id, None, position_ms, duration_ms)`

**When Retrieving** (`src/ui/pages/player.rs:2044-2050`):
- Uses `user_id = "default"`
- Calls `GetPlaybackProgressCommand` which uses `find_by_media_and_user`
- Database query filters by BOTH media_id AND user_id
- NULL != "default", so no results found

## Solution

Modified `src/services/core/playback.rs`:
1. Changed `get_progress()` to use `find_by_media_id()` instead of `find_by_media_and_user()`
2. This ignores the user_id parameter and queries by media_id only
3. Also fixed `get_playqueue_state()` to use `None` instead of `Some(user_id)` for consistency

This makes retrieval consistent with saving, and both now use user_id = None for the single-user system.

## Implementation Complete

All acceptance criteria except #6 (manual testing) have been met:

- ✅ #1-2: Player now retrieves saved position and seeks to it (by fixing the database query)
- ✅ #3: Resume thresholds already implemented in player.rs:2056-2063
- ✅ #4: Suspicious reset protection unchanged and working
- ✅ #5: Fix is generic and works for all media types (TV episodes and movies)
- ⏳ #6: Requires manual testing with episode 135555

## Testing

Code compiles successfully. Commit: 78ce43d

**Manual Testing Required:**
Run the application and play episode 135555 (which has saved position at ~29 minutes) to verify it automatically resumes at the correct position.
<!-- SECTION:NOTES:END -->
