---
id: task-326.03
title: Implement ChunkManager for coordinating chunk operations
status: Done
assignee:
  - '@claude'
created_date: '2025-10-01 15:41'
updated_date: '2025-10-01 16:14'
labels:
  - cache
  - implementation
dependencies: []
parent_task_id: task-326
---

## Description

Implement ChunkManager that:

1. **Tracks chunk requests**: When proxy needs a chunk, registers the request
2. **Manages priorities**: Prioritizes chunks based on:
   - User-requested ranges (seeks, current playback)
   - Predictive prefetch (next N seconds of video)
   - Background completion
3. **Queries availability**: Checks database for which chunks are available
4. **Coordinates downloads**: Sends chunk download requests to ChunkDownloader
5. **Provides availability API**: For proxy to check if chunks exist

**Implementation Notes**:
- Use cache_chunks table for availability
- Use cache_download_queue table for pending downloads
- Priority queue for chunk requests
- Chunk size should be configurable (e.g., 1MB, 5MB)

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 ChunkManager can register chunk requests with priority
- [x] #2 ChunkManager queries database for chunk availability
- [x] #3 ChunkManager creates download queue entries for missing chunks
- [x] #4 ChunkManager provides availability check API for byte ranges
- [x] #5 ChunkManager handles chunk completion notifications
- [x] #6 Add comprehensive tests for priority handling
<!-- AC:END -->


## Implementation Plan

1. Review existing cache module structure
2. Create ChunkManager struct with priority queue and event system
3. Implement database query methods (has_chunk, has_byte_range)
4. Implement request/wait mechanism with priority handling
5. Add background queue processor
6. Write comprehensive tests for priority handling and availability checks


## Implementation Notes

## Chunk Completion Events

ChunkManager should provide an event system for chunk completion notifications:

```rust
// When chunk completes downloading
event_bus.publish(ChunkCompleted { 
    cache_key,
    start_byte,
    end_byte 
});

// Proxy can subscribe
let mut receiver = chunk_manager.subscribe_to_chunk(cache_key, start_byte, end_byte);
receiver.wait_for_completion(timeout).await?;
```

This allows proxy to efficiently wait for specific chunks without polling.

### Post-Implementation Improvements

**Chunk Size Configuration**
- Made chunk size configurable (removed hardcoded 2MB constant)
- Updated FileCacheConfig default: 10MB chunks (was 1MB)
- ChunkManager now accepts chunk_size_bytes parameter
- Tests use 2MB for manageable test data

**Benefits of 10MB chunks for large media files:**
- 4GB file: 400 chunks (vs 2,048 with 2MB)
- 10GB file: 1,000 chunks (vs 5,120 with 2MB)  
- 50GB 4K movie: 5,000 chunks (vs 25,600 with 2MB)
- Reduces database overhead by ~5x
- Still responsive for seeks (10MB downloads in <1 second on typical connections)

**Updated Methods:**
- `ChunkManager::new(repository, chunk_size_bytes)` - now requires size
- `chunk_size()` - getter for configured size
- `byte_to_chunk_index()`, `chunk_start_byte()`, `chunk_end_byte()` - now instance methods

**Files Modified:**
- `src/cache/config.rs` - Updated defaults (10MB chunks, 20MB initial buffer)
- `src/cache/chunk_manager.rs` - Configurable chunk size, all tests pass
- `src/cache/mod.rs` - Removed CHUNK_SIZE constant export


## Implementation Summary

Implemented ChunkManager as the central coordinator for chunk-based cache operations.

### Core Components

1. **Priority System**
   - CRITICAL (0): Chunks needed NOW for playback
   - HIGH (1): Next few chunks for smooth playback
   - MEDIUM (2): User-requested pre-cache
   - LOW (3): Background sequential fill
   - BinaryHeap-based priority queue ensures CRITICAL chunks are processed first

2. **Chunk Calculations**
   - Fixed 2MB chunk size (CHUNK_SIZE constant)
   - Byte-to-chunk-index conversion: `byte_offset / CHUNK_SIZE`
   - Handles file boundaries correctly (last chunk may be smaller)

3. **Database Integration**
   - Queries cache_chunks table for availability
   - Improved has_byte_range() checks for contiguous coverage across multiple chunks
   - Detects gaps in chunk coverage

4. **Event-Based Waiting**
   - Uses tokio::sync::Notify for chunk completion events
   - No polling - waiters are notified when chunks become available
   - Timeout support prevents indefinite waits

5. **Request Management**
   - request_chunk() adds to priority queue
   - request_chunks_for_range() handles byte range requests
   - cancel_requests() removes pending requests
   - Queue filtering based on entry_id and chunk_index

### Key Methods

- `has_chunk(entry_id, chunk_index)` - Check single chunk availability
- `has_byte_range(entry_id, start, end)` - Check range with contiguous coverage
- `request_chunk(entry_id, chunk_index, priority)` - Add to queue
- `wait_for_chunk(entry_id, chunk_index, timeout)` - Event-based wait
- `get_available_chunks(entry_id)` - List available chunks
- `cancel_requests(entry_id, chunks)` - Cancel pending

### Test Coverage

15 comprehensive tests covering:
- Chunk calculations and boundaries
- Priority ordering (BinaryHeap behavior)
- Single chunk availability
- Multi-chunk contiguous ranges
- Gap detection in chunks
- Request queueing with priorities
- Already-available chunk handling
- Range request expansion
- Request cancellation
- Available chunks listing
- Wait timeouts
- Wait for already-available chunks

### Integration Points

- Requires CacheRepository for database queries
- Exports Priority enum and CHUNK_SIZE constant
- Ready for integration with ChunkDownloader (task-326.04)
- Ready for integration with CacheProxy (task-326.05)

### Design Decisions

1. **In-Memory Priority Queue**: Following design doc recommendation to start without database persistence
2. **2MB Chunk Size**: Balances overhead vs responsiveness
3. **Contiguous Coverage Check**: More sophisticated than repository's has_byte_range()
4. **Event-Based Waiting**: Avoids polling overhead

### Files Modified

- `src/cache/chunk_manager.rs` - New file (888 lines)
- `src/cache/mod.rs` - Export ChunkManager, Priority, CHUNK_SIZE
