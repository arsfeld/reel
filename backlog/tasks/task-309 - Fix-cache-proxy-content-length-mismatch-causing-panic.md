---
id: task-309
title: Fix cache proxy content-length mismatch causing panic
status: Done
assignee:
  - '@claude'
created_date: '2025-09-29 12:27'
updated_date: '2025-09-29 12:41'
labels: []
dependencies: []
priority: high
---

## Description

The cache proxy is crashing with a content-length mismatch error when streaming partial content. The payload claims a different content-length than the custom header, causing Hyper to panic. This prevents media playback through the cache proxy.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify root cause of content-length mismatch between payload (479199232) and header (475004928)
- [x] #2 Fix partial content response handling in cache proxy
- [x] #3 Ensure proper content-length calculation for range requests
- [x] #4 Handle concurrent download and streaming without conflicts
- [x] #5 Add error handling to prevent panics in proxy server
- [x] #6 Test media playback through cache proxy with various range requests
<!-- AC:END -->


## Implementation Plan

1. Analyze proxy serve_file method to understand how content-length is calculated
2. Check if file is still being downloaded when serving partial content
3. Fix content-length calculation for partial/in-progress files
4. Add proper synchronization between downloader and proxy server
5. Add error handling to prevent panics
6. Test with various range requests during active downloads


## Implementation Notes

## Fixed Content-Length Mismatch in Cache Proxy

### Root Cause
The cache proxy was serving files that were still being downloaded. It calculated Content-Length based on the current file size on disk, but when serving partial content (range requests), it tried to read beyond what was actually downloaded, causing a mismatch between the declared Content-Length header and the actual payload size.

### Solution Implemented

1. **Separated actual vs expected file sizes**: Modified proxy to track both the actual downloaded size and the expected total size from metadata.

2. **Adjusted range calculations**: When serving partial content for incomplete files, the proxy now limits the served range to what is actually available on disk.

3. **Added proper error handling**: Implemented graceful handling of EOF errors when files are still downloading, with fallback to partial reads.

4. **Updated metadata tracking**: Added `set_expected_file_size()` method to properly track the total expected size during initial download.

### Key Changes

- `src/cache/proxy.rs`: Fixed `serve_file()` method to handle incomplete downloads correctly
- `src/cache/storage.rs`: Added `set_expected_file_size()` method for proper metadata tracking
- `src/cache/downloader.rs`: Updated to set expected file size when starting downloads

### Error Handling Improvements

- Returns HTTP 416 (Range Not Satisfiable) when requested range exceeds available data
- Returns HTTP 503 (Service Unavailable) with Retry-After header when data is not yet available
- Gracefully handles UnexpectedEof errors by serving whatever data is available
- Proper Content-Range headers that reflect actual served content vs total expected size

This fix ensures the cache proxy can safely stream media files while they are still being downloaded, without causing panics due to content-length mismatches.
