---
id: task-352
title: Implement dynamic cache size limits based on available disk space
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 14:07'
updated_date: '2025-10-03 14:34'
labels:
  - cache
  - storage
  - configuration
dependencies: []
priority: high
---

## Description

Replace fixed cache size limit with dynamic sizing based on available disk space. Cache should use the SMALLER of: fixed maximum (e.g., 10GB) OR (total disk space - minimum free space reserve). This prevents cache from consuming all disk space while allowing flexibility on larger drives.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Get actual disk space information (total, free, available)
- [x] #2 Calculate dynamic cache limit: min(fixed_max, total - min_free_reserve)
- [x] #3 Make fixed_max configurable (default: 10GB)
- [x] #4 Make min_free_reserve configurable as percentage (default: 5%) and bytes
- [x] #5 Trigger cleanup when cache size approaches dynamic limit (e.g., 90% of limit)
- [x] #6 Update cache stats to show dynamic limit vs static limit
- [x] #7 Log when dynamic limit is lower than configured fixed_max
<!-- AC:END -->


## Implementation Plan

1. Research existing cache size management in codebase
2. Research Rust libraries for disk space information
3. Implement disk space query functionality
4. Implement dynamic cache limit calculation
5. Add configuration options for fixed_max and min_free_reserve
6. Integrate with existing cache cleanup logic
7. Update cache stats to show dynamic vs static limits
8. Add logging for when dynamic limit is lower than fixed_max
9. Test implementation


## Implementation Notes

Implemented dynamic cache size limits based on available disk space.


## Implementation Details

**New Configuration Fields:**
- `max_size_mb`: Fixed maximum cache size (default: 10GB)
- `min_free_reserve_mb`: Optional absolute minimum free space to reserve
- `min_free_reserve_percent`: Percentage of total disk to reserve (default: 5%)
- `cleanup_threshold_percent`: Threshold to trigger cleanup (default: 90%)

**New Functions in FileCacheConfig:**
- `get_disk_space_info()`: Queries disk space using sysinfo crate
- `calculate_dynamic_cache_limit()`: Calculates effective limit using formula: min(fixed_max, total_disk - min_free_reserve)
- Returns `DynamicCacheLimit` with all computed values

**Integration:**
- Updated `CacheCleanupWorker` to use dynamic limits for LRU cleanup
- Updated `DownloaderStats::format_report()` to show cache limits in stats
- Logs warning when disk space constrains the cache below fixed maximum

**Files Modified:**
- `src/cache/config.rs`: Core implementation
- `src/cache/stats.rs`: Stats reporting
- `src/workers/cache_cleanup_worker.rs`: Cleanup integration
- `Cargo.toml`: Added sysinfo = "0.35" dependency

**Design:**
The system prioritizes disk space by ensuring minimum free space is always preserved, while allowing the full fixed maximum on larger drives. This prevents the cache from filling up small drives while utilizing available space on larger ones.
