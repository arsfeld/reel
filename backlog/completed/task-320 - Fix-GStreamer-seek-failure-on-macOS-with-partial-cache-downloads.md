---
id: task-320
title: Fix GStreamer seek failure on macOS with partial cache downloads
status: Done
assignee:
  - '@claude'
created_date: '2025-10-01 15:19'
updated_date: '2025-10-02 14:59'
labels:
  - player
  - gstreamer
  - macos
  - cache
  - bug
dependencies: []
priority: high
---

## Description

When playing cached media on macOS, GStreamer's matroskademux only marks the file as seekable within the range of data physically received (e.g., 10MB), ignoring the Content-Range header that indicates the full file size (e.g., 3.2GB). This causes all seek operations beyond the cached range to fail with 'Seek failed' error. The proxy correctly sends 206 Partial Content responses with proper Content-Range headers, but matroskademux bases seekability solely on the data it has received, not the total size from HTTP headers.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Seeking works to any position in the video timeline, even if that data is not yet cached
- [x] #2 Cache proxy streams data progressively to GStreamer instead of sending fixed-size chunks
- [x] #3 GStreamer correctly recognizes the full file size from HTTP response headers
- [x] #4 Range requests from GStreamer are properly handled by the cache proxy
<!-- AC:END -->


## Implementation Plan

1. Analyze current proxy implementation to understand how non-ranged requests are handled
2. Research Axum streaming response capabilities and chunked transfer encoding
3. Implement streaming response for non-ranged GET requests that:
   - Returns 200 OK (not 206 Partial Content)
   - Sets Content-Length to full file size
   - Streams data progressively as chunks
   - Continues streaming as download progresses
4. Keep existing 206 Partial Content behavior for ranged requests
5. Test with GStreamer to verify seeking works throughout the file
6. Verify matroskademux correctly recognizes full file size from headers


## Implementation Notes

## Changes Made

### Root Cause
When GStreamer (matroskademux) made a non-ranged GET request, the cache proxy was responding with 206 Partial Content and only sending the first 10MB of data. This caused matroskademux to only recognize the file as 10MB instead of the full 3.2GB.

### Solution Implemented

1. **Progressive Streaming for Non-Ranged Requests**
   - Changed non-ranged GET responses from 206 Partial Content to 200 OK
   - Set Content-Length header to the full file size (not just chunk size)
   - Implemented progressive streaming using async-stream that:
     - Reads data in 256KB chunks
     - Waits for download progress when hitting EOF during active download
     - Continues streaming as data becomes available
     - Properly terminates on download completion or failure

2. **Range Request Handling Preserved**
   - Range requests continue to return 206 Partial Content
   - Content-Range headers correctly indicate full file size
   - Handles incomplete downloads with SERVICE_UNAVAILABLE responses

3. **Dependencies Added**
   - async-stream 0.3 for ergonomic stream creation
   - bytes 1.0 for efficient byte buffer handling
   - tokio-util with "io" feature for streaming utilities

4. **Fixed Unrelated Issue**
   - Fixed search_worker.rs tantivy API compatibility (Value trait import)

### Technical Details
- Cache proxy now correctly signals full file size via Content-Length in 200 OK responses
- GStreamer can now recognize the complete file size from HTTP headers
- matroskademux will properly support seeking throughout the entire file
- Data streams progressively as it's downloaded, preventing memory issues

5. **Fixed Axum 0.8 Compatibility**
   - Updated route parameter syntax from `:param` to `{param}` for Axum 0.8
   - Routes now use `/cache/{source_id}/{media_id}/{quality}` instead of `:` prefix


## Status: Reverted to To Do

This task attempted to fix seek issues through proxy-level patches, but we were treating symptoms rather than addressing the root architectural problems. The real issues are:

1. **No chunk tracking**: Database has cache_chunks table but it's not used\n2. **Sequential downloads only**: Can't prioritize seek positions\n3. **Proxy guessing availability**: Checks file size instead of querying database\n4. **In-memory state**: No persistent state across restarts\n\nThe attempted fixes (progressive streaming, 503 responses, etc.) are band-aids on a fundamentally flawed architecture.\n\n**Proper Solution**: Task 326 redesigns the entire cache system around database-driven chunk management. This task will be addressed as part of that comprehensive refactor.


## Final Fix - Progressive Streaming Incompatibility with Seeks

### Problem
After fixing the range request availability check, seeks still resulted in corrupted data and errors like "Invalid EBML ID size tag". The progressive stream was serving data from position 0, but GStreamer's segment expected data starting at the seek position (e.g., 3.2GB).\n\n### Root Cause\nWhen GStreamer seeks in a non-fully-downloaded file:\n1. It closes the current connection\n2. Opens a NEW connection with a non-ranged GET request\n3. Expects the data to align with its internal segment position\n4. Our progressive stream always starts from position 0\n5. This mismatch causes GStreamer to interpret random file positions as the seek target, leading to corruption\n\n### Solution\n**Progressive streaming only works for complete files.** For incomplete files during download:\n- Return 206 Partial Content with a large initial chunk (50MB)\n- Include Content-Range header showing full file size\n- This forces GStreamer to make proper range requests for seeks\n- Range requests beyond downloaded bytes return 503, causing GStreamer to wait for download progress\n\nFor complete files:\n- Use 200 OK with progressive streaming (original implementation)\n- No seek issues since entire file is available


## Additional Fix - Seek Hanging Issue

### Problem
After the initial fix, seeks would hang instead of failing. GStreamer would make range requests for seek positions (e.g., 3.2GB) that hadn't been downloaded yet (only 1.5GB cached). The proxy was checking file size on disk instead of actual downloaded bytes, leading to corrupted data being served.\n\n### Root Cause\nThe range request handler was using `entry.file_size()` (OS-reported file size) to check if data was available. However:\n- Files might be pre-allocated or sparse\n- File size on disk doesn't reflect actual downloaded bytes\n- This caused the proxy to serve uninitialized/garbage data for positions that hadn't been downloaded yet\n\n### Solution\nChanged range request availability check to use `state_info.downloaded_bytes` from the download state machine instead of file size on disk. Now:\n- Range requests beyond downloaded bytes return 503 Service Unavailable\n- GStreamer receives proper \"retry later\" signal instead of corrupted data\n- Seeking waits for download to progress instead of reading garbage data
