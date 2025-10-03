---
id: task-391.01
title: 'Phase 5: MediaService Integration'
status: Done
assignee:
  - '@claude'
created_date: '2025-10-04 01:24'
updated_date: '2025-10-04 02:04'
labels:
  - transcoding
  - phase-5
  - mediaservice
dependencies: []
parent_task_id: task-391
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Wire quality selection to backend and cache. Implement get_stream_with_quality() in MediaService, integrate with PlayerPage for quality switching during playback.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add get_stream_with_quality() to MediaService
- [x] #2 Update PlayerPage to use quality-aware stream fetching
- [x] #3 Handle quality changes during playback
- [x] #4 Add loading states for quality switching
- [x] #5 Quality selection triggers stream URL fetch
- [x] #6 Cache lookup uses correct quality key
- [x] #7 Player switches to new quality smoothly
- [x] #8 Loading indicator shows during quality change
- [x] #9 Files updated as per docs/transcode-plan.md Phase 5

- [x] #10 Create task for Phase 6: Remote Connection Handling

- [ ] #11 Create task for Phase 7: Testing & Polish (if required)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Review existing code structure and understand current flow
2. Add get_stream_with_quality() to BackendService
3. Update AppCommand to support quality parameter
4. Implement quality change handling in PlayerPage
5. Add loading state for quality switching
6. Test quality switching during playback
7. Create task for Phase 6: Remote Connection Handling
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

Successfully implemented Phase 5: MediaService Integration for quality selection during playback.

### Changes Made

1. **BackendService (src/services/core/backend.rs)**
   - Added `get_stream_with_quality()` method that:
     - Accepts media_id and QualityOption parameters
     - Queries ConnectionService cache for connection type (local vs remote)
     - Uses PlexBackend.get_stream_url_for_quality() for Plex sources
     - Falls back to quality.url for non-Plex backends

2. **AppCommand (src/ui/shared/commands.rs)**
   - Added `ChangeQuality` command variant with media_id and quality parameters
   - Added `QualityChanged` result variant
   - Implemented `change_quality()` function that:
     - Gets stream URL for selected quality from BackendService
     - Generates quality-aware cache key (e.g., "1080p", "original")
     - Uses FileCache.get_cached_stream_with_quality() for quality-specific caching

3. **PlayerPage (src/ui/pages/player.rs)**
   - Implemented quality change handler in `PlayerInput::QualityChanged`:
     - Captures current playback position
     - Sets loading state
     - Executes ChangeQuality command via oneshot_command
     - Loads new stream URL in player
     - Restores playback position
     - Resumes playback
   - Added `CommandResult::QualityChanged` to existing match statements

### Quality Selection Flow

1. User selects quality from dropdown
2. QualitySelector emits QualityChanged event
3. PlayerPage receives QualityChanged input
4. Shows loading state and captures position
5. Executes ChangeQuality command:
   - BackendService.get_stream_with_quality() gets URL
   - FileCache provides quality-specific cached stream
6. Player loads new URL and seeks to saved position
7. Playback resumes at same point with new quality

### Integration Points

- Uses existing decision endpoint (src/backends/plex/api/decision.rs)
- Uses existing get_stream_url_for_quality (src/backends/plex/api/streaming.rs)
- Integrates with FileCache quality-aware caching
- Leverages ConnectionService for local/remote detection

### Next Steps

Created task-391.02 for Phase 6: Remote Connection Handling to ensure proper connection type handling and fallback logic.
<!-- SECTION:NOTES:END -->
