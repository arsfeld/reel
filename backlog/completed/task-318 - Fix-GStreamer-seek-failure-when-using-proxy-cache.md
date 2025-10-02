---
id: task-318
title: Fix GStreamer seek failure when using proxy cache
status: Done
assignee:
  - '@arosenfeld'
created_date: '2025-09-29 13:57'
updated_date: '2025-09-29 14:02'
labels:
  - bug
  - cache
  - proxy
  - gstreamer
  - playback
dependencies: []
priority: high
---

## Description

GStreamer fails to seek when playing media through the cache proxy. The logs show 'seek_simple failed, trying full seek API' followed by 'Both seek methods failed', even though the media reports as seekable with a valid range. This causes playback to continue at the current position instead of jumping to the requested time. The issue appears to be related to how the proxy serves partial content or how it responds to range requests during seeking operations.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Analyze GStreamer's seek requirements and how it uses HTTP range requests for seeking
- [x] #2 Debug the proxy's handling of range requests during seek operations
- [x] #3 Implement proper HTTP 206 Partial Content response headers for seek support
- [x] #4 Ensure Content-Range and Accept-Ranges headers are correctly set
- [x] #5 Handle GStreamer's specific byte-range request patterns during seeks
- [x] #6 Test seeking works correctly with various seek positions
- [x] #7 Verify seeking works with both cached and actively downloading content
- [x] #8 Add logging to track seek-related range requests in the proxy
<!-- AC:END -->


## Implementation Plan

1. Research GStreamer's HTTP seeking requirements and how it uses range requests
2. Analyze current proxy implementation for range request handling
3. Test current seeking behavior with debug logging
4. Fix Content-Range and Accept-Ranges header implementation
5. Handle edge cases for partial content when file is still downloading
6. Test seeking with both cached and actively downloading content
7. Add comprehensive logging for debugging seek operations


## Implementation Notes

## Summary

Fixed GStreamer seek failure when playing media through the cache proxy by improving HTTP range request handling.


## Changes Made

1. **Fixed parse_range_header function**:
   - Removed incorrect validation that rejected valid ranges for still-downloading files
   - Properly handle open-ended ranges (e.g., "bytes=500-")
   - Allow serving partial content for files being downloaded

2. **Enhanced HTTP response headers**:
   - Always include Accept-Ranges: bytes header to indicate range support
   - Properly format Content-Range headers for partial responses
   - Added Cache-Control headers to prevent caching issues
   - Use HTTP 206 status for partial content even when file is incomplete

3. **Improved handling of seek requests during download**:
   - Return 503 Service Unavailable with Retry-After when data not yet available
   - Distinguish between temporary unavailability (downloading) and permanent errors
   - Better handling of edge cases when requested range exceeds current file size

4. **Added comprehensive debug logging**:
   - Log all range request parsing with details
   - Track range vs full requests in stats
   - Log detailed information about served ranges and file states

## Technical Details

GStreamer uses HTTP range requests for seeking in media streams. It expects:
- Accept-Ranges: bytes header to know seeking is supported
- Proper HTTP 206 Partial Content responses with Content-Range headers
- Correct handling of open-ended ranges for progressive download

The proxy now correctly implements the HTTP/1.1 range request specification, allowing GStreamer to seek properly in both fully cached and actively downloading files.
