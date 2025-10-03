---
id: task-363
title: 'Fix playback failure: Failed to get cached stream'
status: In Progress
assignee:
  - '@claude'
created_date: '2025-10-03 14:49'
updated_date: '2025-10-03 15:02'
labels:
  - bug
  - player
  - cache
  - critical
dependencies: []
priority: high
---

## Description

Player fails to start playback with error 'Failed to get cached stream'. The cache system creates a new entry and makes a HEAD request to get Content-Length, but the player attempts to start playback before the cache initialization completes. This is a timing/async issue where the cache entry isn't fully ready when get_cached_stream is called.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify where get_cached_stream is called in the player code
- [x] #2 Ensure HEAD request completes and cache entry has total_size before returning stream URL
- [x] #3 Properly await cache initialization before starting playback
- [ ] #4 Player successfully starts playback without 'Failed to get cached stream' error
- [x] #5 Cache entry is properly initialized with Content-Length before use
<!-- AC:END -->


## Implementation Plan

1. Analyze the database transaction handling in cache repository
2. Check if database updates are properly committed before verification
3. Identify why verification query sees stale data (None/0 for expected_total_size)
4. Fix the timing issue - ensure database commits are visible before verification
5. Test playback to confirm the fix works


## Implementation Notes

Fixed database update issue in cache repository:

1. Root cause: When updating a CacheEntryModel via update_cache_entry(), the conversion to ActiveModel was marking all fields as Unchanged, preventing the update from actually modifying the database.

2. Solution: Added dedicated update_expected_total_size() method to CacheRepository that explicitly sets expected_total_size field as Set() in the ActiveModel.

3. Implementation:
   - Added update_expected_total_size() to CacheRepository trait
   - Implemented in CacheRepositoryImpl using explicit ActiveModel with Set() values
   - Updated ensure_database_entry() to use the new method

4. Additional issue discovered: Plex servers may not properly respond to HEAD requests with Content-Length header. This is a separate issue that needs investigation (using GET with Range header might be better).

Files modified:
- src/db/repository/cache_repository.rs: Added update_expected_total_size method
- src/cache/file_cache.rs: Updated to use new repository method

### Discovery: Plex HEAD Request Limitation

After adding detailed logging, discovered the root cause:
- Plex server returns **HTTP 500 Internal Server Error** when HEAD requests are made to video file URLs
- This is a Plex server limitation/bug - it doesn't properly support HEAD requests on media endpoints
- The error occurs at: https://[server]/library/parts/[id]/[timestamp]/file.mkv

### Recommended Solution: Use Range Requests Instead

Replace HEAD request with GET + Range header:
```rust
// Instead of: client.head(url)
// Use: client.get(url).header("Range", "bytes=0-0")
```

Benefits:
1. GET with Range: bytes=0-0 requests only first byte
2. Server responds with 206 Partial Content + Content-Range header
3. Content-Range provides total file size: "bytes 0-0/[total]"
4. More widely supported than HEAD requests
5. Only downloads 1 byte, so still efficient

This fix requires updating ensure_database_entry() in src/cache/file_cache.rs

### Final Implementation

Replaced HEAD request with GET + Range header in ensure_database_entry():
- Changed from client.head(url) to client.get(url).header("Range", "bytes=0-0")
- Handles both 206 Partial Content (parses Content-Range) and 200 OK (uses Content-Length)
- Added comprehensive error handling and logging
- Solution works around Plex server limitation that returns HTTP 500 for HEAD requests

The fix properly initializes cache entries with file size before playback starts.
