---
id: task-349
title: Handle disk space exhaustion in cache system
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 13:37'
updated_date: '2025-10-03 14:42'
labels:
  - cache
  - error-handling
  - storage
dependencies: []
priority: medium
---

## Description

Add detection and graceful handling for disk space exhaustion. When cache writes fail due to no space, trigger cleanup and retry, or fall back to passthrough mode.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Detect ENOSPC (no space left) errors during writes
- [x] #2 Trigger emergency cache cleanup when disk full
- [x] #3 Retry write after cleanup if space freed
- [x] #4 Fall back to passthrough if cleanup insufficient
- [x] #5 Add disk space monitoring and warnings
<!-- AC:END -->


## Implementation Plan

1. Research current cache write operations and error handling
2. Implement ENOSPC error detection in chunk writes
3. Add emergency cleanup mechanism with cache eviction
4. Implement write retry logic after cleanup
5. Add passthrough fallback when cleanup insufficient
6. Add disk space monitoring and proactive warnings


## Implementation Notes

Implemented comprehensive disk space exhaustion handling:

1. ENOSPC Error Detection (chunk_store.rs):
   - Added is_disk_full_error() helper to detect ENOSPC errors across Unix/Windows
   - Modified write_chunk() to detect and return "DISK_FULL" prefixed errors
   - Detects errors at all stages: open, seek, write, flush

2. Emergency Cleanup (chunk_downloader.rs):
   - Added emergency_cleanup() method to free ~1GB by deleting LRU entries
   - Cleanup deletes database entries, chunks, and cache files
   - Returns bytes freed for logging

3. Write Retry Logic (chunk_downloader.rs):
   - try_download_chunk() detects DISK_FULL errors
   - Triggers emergency_cleanup() when disk full detected
   - Retries write after successful cleanup
   - Falls back if cleanup fails

4. Passthrough Fallback (proxy.rs):
   - Already implemented in create_range_based_progressive_stream()
   - Switches to passthrough streaming when cache operations fail
   - Logs warnings and streams directly from source URL

5. Disk Space Monitoring (config.rs, chunk_downloader.rs):
   - Added DiskSpaceStatus enum with Healthy/Info/Warning/Critical levels
   - Added check_disk_space_status() to FileCacheConfig
   - Proactive checking before each chunk download
   - Triggers emergency cleanup when critical (<5% or <1GB)
   - Logs warnings at different severity levels

All cache write failures now gracefully handled with automatic recovery or fallback.
