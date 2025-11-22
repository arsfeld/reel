---
id: task-344
title: Fix Plex playback progress tracking to match OpenAPI spec
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 00:28'
updated_date: '2025-10-03 00:34'
labels:
  - plex
  - playback
  - api
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Current implementation works but has several deviations from the official Plex OpenAPI specification. These issues may cause problems with future Plex server versions, multi-client scenarios, and proper state tracking. The fixes will improve spec compliance, accuracy, and robustness.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Timeline endpoint uses POST instead of GET
- [x] #2 Scrobble endpoint uses PUT instead of GET
- [x] #3 PlayQueue version is tracked from responses instead of hardcoded to '1'
- [x] #4 Player controller sends state parameter (playing/paused/stopped/buffering) to update_progress
- [x] #5 Redundant 'playbackTime' parameter is removed from timeline requests
- [x] #6 Periodic timeline updates sent every 10 seconds during playback
- [x] #7 Timeline updates sent on pause, unpause, and stop events
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Fix HTTP methods for timeline and scrobble endpoints
   - Change timeline endpoint from GET to POST in progress.rs and playqueue.rs
   - Change scrobble endpoint from GET to PUT in progress.rs

2. Fix PlayQueue version tracking
   - Update update_play_queue_progress to accept version parameter
   - Pass actual version from PlayQueueInfo instead of hardcoded "1"
   - Update PlayQueueService::update_progress_with_queue to pass version

3. Remove redundant playbackTime parameter
   - Remove from timeline requests in progress.rs and playqueue.rs

4. Fix player state tracking and propagation
   - Update player.rs to send proper state (playing/paused/stopped/buffering)
   - Send timeline updates on play, pause, and stop events
   - Add immediate progress update on state changes

5. Implement periodic timeline updates during playback
   - Verify config setting is 10 seconds
   - Ensure updates only sent during active playback
   - Send state parameter with each update

6. Test all changes
   - Verify timeline uses POST
   - Verify scrobble uses PUT
   - Verify version is tracked correctly
   - Verify state changes trigger immediate updates
   - Verify periodic updates work during playback
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed Plex playback progress tracking to fully comply with OpenAPI spec:

**HTTP Methods Fixed:**
- Changed timeline endpoint from GET to POST in both progress.rs and playqueue.rs
- Changed scrobble endpoint from GET to PUT in progress.rs

**PlayQueue Version Tracking:**
- Added play_queue_version parameter to update_play_queue_progress()
- Updated PlayQueueState struct to include version field
- Now passes actual version from server responses instead of hardcoded "1"

**State Parameter Improvements:**
- Updated player.rs to map PlayerState to Plex state strings (playing/paused/stopped/buffering)
- Periodic updates now send correct state based on actual player state
- Added immediate timeline updates on play, pause, and stop events

**Other Improvements:**
- Removed redundant playbackTime parameter from timeline requests
- Changed default progress_update_interval to 10 seconds (was 5)
- All changes maintain backward compatibility

**Files Modified:**
- src/backends/plex/api/progress.rs
- src/backends/plex/api/playqueue.rs
- src/backends/plex/mod.rs
- src/services/core/playqueue.rs
- src/ui/pages/player.rs
- src/config.rs
<!-- SECTION:NOTES:END -->
