---
id: task-353
title: Implement proactive cache cleanup worker using Relm4 Worker pattern
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 14:24'
updated_date: '2025-10-03 14:33'
labels:
  - cache
  - worker
  - relm4
  - background-task
dependencies: []
priority: medium
---

## Description

Create a background Worker component following Relm4 best practices to perform periodic cache cleanup. Should run on a schedule to prune old/stale entries before hitting space limits, keeping the cache healthy. Use Worker pattern for isolation and MessageBroker for communication.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create CacheCleanupWorker component following Relm4 Worker pattern
- [x] #2 Implement periodic timer (configurable interval, default: hourly)
- [x] #3 Time-based expiration: remove entries older than TTL (default: 30 days)
- [x] #4 Proactive LRU cleanup: remove least-accessed entries when approaching 80% of limit
- [x] #5 Use MessageBroker to publish cleanup events/stats
- [x] #6 Add configuration for cleanup_interval, max_age_days, proactive_threshold
- [x] #7 Worker should start/stop with application lifecycle
- [x] #8 Log cleanup operations (entries removed, space freed)
<!-- AC:END -->


## Implementation Plan

1. Research how Workers are initialized and integrated in the application
2. Create CacheCleanupWorker struct with configuration
3. Implement Worker trait with Input/Output messages
4. Add periodic timer using tokio::time::interval
5. Implement time-based cleanup (remove old entries)
6. Implement proactive LRU cleanup at 80% threshold
7. Add MessageBroker integration for cleanup events
8. Integrate worker into application lifecycle
9. Add logging for cleanup operations
10. Test the worker manually


## Implementation Notes

Implemented CacheCleanupWorker as a Relm4 Worker component with the following features:

**Architecture:**
- Created src/workers/cache_cleanup_worker.rs with Worker trait implementation
- Added CleanupConfig for configurable settings (interval: 1h, max_age: 30 days, threshold: 80%)
- Integrated with application lifecycle in src/ui/main_window.rs

**Cleanup Logic:**
1. Time-based cleanup: Removes cache entries older than 30 days using CacheRepository::delete_old_entries()
2. Proactive LRU cleanup: Monitors cache size against dynamic limit (calculated from disk space), removes least-accessed entries when exceeding 80% threshold
3. Uses CacheRepository::get_entries_for_cleanup() for LRU selection

**MessageBroker Integration:**
- Added CacheMessage enum to src/ui/shared/broker.rs with CleanupStarted, CleanupCompleted, and CleanupFailed variants
- Broadcasts cleanup events to all subscribers
- Forwards messages to main window for toast notifications

**Worker Lifecycle:**
- Starts automatically with application
- Runs periodic cleanup every hour (configurable)
- Can be triggered manually via CacheCleanupInput::TriggerCleanup
- Logs all operations with entry counts and space freed

**Implementation Details:**
- Uses tokio::time::interval for periodic scheduling
- Spawns async tasks for cleanup operations to avoid blocking
- Properly handles Arc<DatabaseConnection> for repository access
- Calculates dynamic cache limits using FileCacheConfig::calculate_dynamic_cache_limit()
- Updates cache statistics after cleanup operations

**Files Modified:**
- src/workers/cache_cleanup_worker.rs (new)
- src/workers/mod.rs (added export)
- src/ui/shared/broker.rs (added CacheMessage)
- src/ui/main_window.rs (integrated worker)

**Configuration:**
Currently uses default FileCacheConfig. Future enhancement could integrate with ConfigService for runtime config updates.
