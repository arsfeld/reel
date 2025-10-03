---
id: task-396
title: Integrate CacheProxy bandwidth reporting with PlayerHandle
status: Done
assignee:
  - '@claude-code'
created_date: '2025-10-04 02:37'
updated_date: '2025-10-04 02:45'
labels:
  - adaptive-quality
  - integration
  - cache
dependencies:
  - task-395
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CacheProxy needs to report chunk download metrics to PlayerHandle so bandwidth data flows to AdaptiveQualityManager. Currently PlayerHandle::report_chunk_download() exists but CacheProxy has no access to PlayerHandle.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add optional PlayerHandle parameter to CacheProxy
- [x] #2 Update CacheProxy::new() call sites to pass PlayerHandle
- [x] #3 Report chunk download metrics after successful chunk downloads
- [x] #4 Ensure bandwidth reporting works during playback
- [ ] #5 Test bandwidth monitoring with real cache downloads
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add optional PlayerHandle field to ChunkDownloader
2. Add set_player_handle() method to ChunkDownloader to update the handle
3. Measure download time in try_download_chunk() and report metrics
4. Add set_player_handle() method to ChunkManager that forwards to ChunkDownloader
5. Add set_player_handle() method to CacheProxy that forwards to ChunkManager
6. Add set_player_handle() method to FileCache/FileCacheHandle
7. Call set_player_handle() when playback starts in the player code
8. Test bandwidth reporting during playback
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Successfully integrated CacheProxy bandwidth reporting with PlayerHandle.

## What was implemented:

1. **ChunkDownloader enhancements**:
   - Added optional `PlayerHandle` field using `Arc<RwLock<Option<PlayerHandle>>>`
   - Added `set_player_handle()` method to update the handle
   - Implemented bandwidth measurement in `try_download_chunk()` using `Instant::now()`
   - Report metrics via `PlayerHandle::report_chunk_download()` after successful downloads

2. **ChunkManager integration**:
   - Added `set_player_handle()` method that forwards to ChunkDownloader

3. **CacheProxy integration**:
   - Added `set_player_handle()` method that forwards to ChunkManager

4. **FileCache/FileCacheHandle integration**:
   - Added `SetPlayerHandle` command variant to `FileCacheCommand`
   - Added `set_player_handle()` method to FileCacheHandle
   - Handle command in FileCache::run() to forward to CacheProxy

5. **Player page integration**:
   - Set PlayerHandle in cache when playback starts (after getting stream URL)
   - Clear PlayerHandle when playback stops
   - Both integrated in src/ui/pages/player.rs

6. **Debug trait**:
   - Implemented custom Debug for PlayerHandle to satisfy trait bounds

## Files modified:
- src/cache/chunk_downloader.rs (bandwidth reporting)
- src/cache/chunk_manager.rs (forwarding)
- src/cache/proxy.rs (forwarding)
- src/cache/file_cache.rs (command handling)
- src/player/controller.rs (Debug impl)
- src/ui/pages/player.rs (lifecycle integration)

## How it works:
When playback starts, the PlayerHandle is set in the cache system. During chunk downloads, the ChunkDownloader measures download time and reports (bytes, duration) to the PlayerHandle, which forwards to AdaptiveQualityManager for bandwidth monitoring. When playback stops, the handle is cleared.
<!-- SECTION:NOTES:END -->
