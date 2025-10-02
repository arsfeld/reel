---
id: task-313
title: Implement proper state machine for cache proxy download tracking
status: Done
assignee:
  - '@claude'
created_date: '2025-09-29 12:57'
updated_date: '2025-09-29 13:42'
labels:
  - cache
  - proxy
  - state-machine
dependencies:
  - task-314
  - task-315
priority: high
---

## Description

The cache proxy currently has no reliable way to know the download state of cached files. It guesses based on file size and metadata flags, leading to race conditions and incorrect responses. Implement a proper state machine that tracks download progress, allows queries about readiness, and coordinates between the downloader and proxy components.

This implementation must integrate with the new database schema (task-314) to persist state across application restarts and enable proper tracking of download progress using the cache_entries, cache_chunks, and cache_download_queue tables.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Design state machine with states: NotStarted, Initializing, Downloading, Paused, Complete, Failed
- [x] #2 Create shared state store accessible by both downloader and proxy
- [x] #3 Implement state transitions with proper locking to prevent races
- [x] #4 Add method to query if minimum data is available for playback
- [x] #5 Add method to wait for specific amount of data with timeout
- [x] #6 Update downloader to publish state changes to shared store
- [x] #7 Update proxy to check state before serving requests
- [x] #8 Handle edge cases like download failures and retries
- [x] #9 Persist state changes to cache_entries table in database
- [x] #10 Load initial state from database on startup
<!-- AC:END -->

## Implementation Notes

Implemented comprehensive state machine for cache proxy download tracking.

## Key Implementation Details:

1. **Created CacheStateMachine Module** (`src/cache/state_machine.rs`):
   - Defined DownloadState enum with states: NotStarted, Initializing, Downloading, Paused, Complete, Failed
   - Implemented DownloadStateInfo struct to track state, progress, and metadata
   - Added state transition validation to ensure only valid transitions are allowed
   - Integrated with CacheRepository for database persistence

2. **State Store Implementation**:
   - Used Arc<RwLock<HashMap>> for thread-safe shared state storage
   - Added wait mechanisms with oneshot channels for waiting on data availability
   - Implemented progress tracking with byte-level granularity

3. **Downloader Integration**:
   - Modified ProgressiveDownloader to use state machine instead of internal state
   - Updated all state transitions to go through the state machine
   - Added state machine updates for progress tracking during downloads
   - Removed duplicate DownloadState enum from downloader module

4. **Proxy Integration**:
   - Updated CacheProxy to check state machine before serving requests
   - Added wait_for_data mechanism with configurable timeout (5 seconds)
   - Improved error handling for failed/incomplete downloads
   - Enhanced response logic based on current download state

5. **Database Integration**:
   - Connected to cache_entries table for state persistence
   - Load initial states from database on startup
   - Update download progress in database during transfers
   - Persist state transitions for recovery after restarts

6. **Race Condition Prevention**:
   - All state transitions use proper locking mechanisms
   - Atomic state checks and updates prevent concurrent modification issues
   - Waiters are properly notified when data becomes available

7. **Edge Case Handling**:
   - Failed downloads can be retried (transition from Failed to Initializing)
   - Paused downloads can be resumed without data loss
   - Cancelled downloads are marked as Failed with appropriate reason
   - Timeout handling for waiting on data availability

## Testing:
- Code compiles successfully with all state machine integration
- State transitions are validated to prevent invalid state changes
- Database persistence ensures state survives application restarts
