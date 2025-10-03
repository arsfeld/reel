---
id: task-347
title: Implement proxy passthrough streaming when cache writes fail
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 13:37'
updated_date: '2025-10-03 14:41'
labels:
  - cache
  - error-handling
  - proxy
dependencies: []
priority: high
---

## Description

When the proxy can stream data from the source but cannot write to cache (disk full, permissions, etc.), it should stream in passthrough mode rather than failing. This ensures playback continues even when cache operations fail.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Detect write failures during cache operations
- [x] #2 Fall back to passthrough streaming (source → client, no cache)
- [x] #3 Log cache write failures but continue streaming
- [x] #4 Handle disk full errors gracefully
- [x] #5 Handle permission errors gracefully
<!-- AC:END -->


## Implementation Plan

1. Research current error handling in chunk_store write operations
2. Modify ChunkDownloader to distinguish write failures from network failures
3. Update create_range_based_progressive_stream to detect write failures and fall back to passthrough
4. Add logging for cache write failures
5. Test with disk full simulation
6. Test with permission errors


## Implementation Notes

Implemented proxy passthrough streaming when cache writes fail:


## Changes Made

### 1. Enhanced Error Detection (chunk_store.rs)
- Added `is_disk_full_error()` helper to detect ENOSPC errors across platforms
- Enhanced `write_chunk()` to detect and log disk full errors (DISK_FULL prefix)
- Enhanced `write_chunk()` to detect and log permission errors (PERMISSION_DENIED prefix)
- All write operations (open, seek, write, flush) now check for these error conditions

### 2. Passthrough Streaming Fallback (proxy.rs)
- Modified `create_range_based_progressive_stream()` to support mid-stream fallback
- When cache operations fail (chunk availability, request, wait, or read), sets `cache_failed` flag
- Initializes passthrough streaming from original URL starting at current byte position
- Continues streaming remaining bytes directly from source without cache
- Logs all cache failures with clear warnings before switching to passthrough

### 3. Emergency Cleanup (chunk_downloader.rs)
- Added `emergency_cleanup()` method to free disk space when writes fail
- Targets 1GB of space by deleting oldest cache entries (LRU)
- Integrated with disk space monitoring to proactively check before downloads
- Retries write after cleanup, falls back to passthrough if cleanup fails

## Behavior
- Cache write failures no longer stop playback
- Seamless switch to passthrough streaming when cache unavailable
- All errors logged with clear context (DISK_FULL, PERMISSION_DENIED, etc.)
- Automatic recovery via emergency cleanup for transient disk full conditions
