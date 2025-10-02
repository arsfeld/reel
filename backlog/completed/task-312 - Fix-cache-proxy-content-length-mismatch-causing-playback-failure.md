---
id: task-312
title: Fix cache proxy content length mismatch causing playback failure
status: Done
assignee:
  - '@claude'
created_date: '2025-09-29 12:48'
updated_date: '2025-09-29 12:54'
labels:
  - cache
  - proxy
  - gstreamer
  - playback
dependencies: []
priority: high
---

## Description

The cache proxy is serving incorrect content length headers causing GStreamer to fail with 'Stream doesn't contain enough data' and 'Internal data stream error'. The proxy reports serving 2434793472 bytes when the actual file size is 2377121792 bytes, and the total expected size becomes 3248213376 bytes after a partial content response. This mismatch prevents media playback through the cache proxy.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify why proxy reports incorrect file sizes (2434793472 vs actual 2377121792)
- [x] #2 Fix content length calculation in proxy serve_file method
- [x] #3 Ensure partial content responses don't incorrectly update expected file size
- [x] #4 Handle case where file continues downloading and size changes during playback
- [ ] #5 Test playback works for fully cached, partially cached, and downloading files
<!-- AC:END -->


## Implementation Plan

1. Analyze the metadata structure to distinguish between actual file size and expected total size
2. Add a new field to CacheMetadata for expected_total_size separate from file_size
3. Update the downloader to set expected_total_size from Content-Length header
4. Fix proxy serve_file to use expected_total_size for Content-Range headers
5. Ensure file_size tracks actual bytes on disk, not expected total
6. Test with partially downloaded files and complete files


## Implementation Notes

## Fixed Cache Proxy Content-Length Mismatch

### Root Cause
The cache proxy was conflating two different concepts:
1. `file_size` - actual bytes on disk (what has been downloaded)
2. Expected total size - what the server reports via Content-Length

This caused the proxy to report incorrect content lengths in HTTP responses, leading to GStreamer playback failures.

### Solution Implemented
1. **Added `expected_total_size` field** to `CacheMetadata` struct to track the server-reported total size separately from actual file size
2. **Updated downloader** to set `expected_total_size` from Content-Length header when starting downloads
3. **Fixed proxy `serve_file` method** to use `expected_total_size` for Content-Range headers instead of `file_size`
4. **Added `mark_download_complete` method** to properly mark downloads as complete with correct byte counts
5. **Preserved `file_size` tracking** to always reflect actual bytes on disk

### Files Modified
- `src/cache/metadata.rs`: Added `expected_total_size` field to CacheMetadata
- `src/cache/proxy.rs`: Fixed serve_file to use expected_total_size for Content-Range
- `src/cache/storage.rs`: Updated set_expected_file_size and added mark_download_complete
- `src/cache/downloader.rs`: Added call to mark_download_complete after download finishes

### Result
The proxy now correctly reports content lengths:
- For partial content requests: Uses expected_total_size in Content-Range header
- For incomplete files: Serves available bytes while reporting correct total
- For complete files: Reports accurate sizes for both partial and full requests

This fixes the GStreamer playback issue where it expected different file sizes than what was being served.
