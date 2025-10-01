---
id: task-311
title: Add periodic stats reporting for cache downloader and proxy
status: Done
assignee:
  - '@claude'
created_date: '2025-09-29 12:45'
updated_date: '2025-09-29 13:02'
labels:
  - cache
  - observability
dependencies: []
priority: high
---

## Description

Add configurable periodic statistics reporting for the cache downloader and proxy components to provide visibility into their operation without spamming the console. Stats should be emitted at regular intervals (e.g., every 30-60 seconds) and include key metrics like active downloads, download speeds, cache hits/misses, and proxy request counts.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Stats are emitted at configurable intervals (default 30-60 seconds)
- [x] #2 Downloader stats include: active downloads, total downloaded bytes, average speed, queue size
- [x] #3 Proxy stats include: total requests served, cache hit rate, active streams, bytes served
- [x] #4 Stats use appropriate log levels (info or debug) to avoid console spam
- [x] #5 Stats reporting can be disabled via configuration
- [x] #6 Stats format is concise and readable
<!-- AC:END -->


## Implementation Plan

1. Define statistics structures for downloader and proxy
2. Add stats tracking fields to ProgressiveDownloader and CacheProxy
3. Implement periodic stats reporting using tokio intervals
4. Add configuration options for stats reporting interval and enable/disable
5. Format stats output in concise, readable format
6. Test stats reporting with various scenarios

## Implementation Notes

## Implementation Summary

Added comprehensive periodic stats reporting for both the cache downloader and proxy components:

### Changes Made:

1. **Created new stats module** (`src/cache/stats.rs`):
   - `DownloaderStats` struct with atomic counters for download metrics
   - `ProxyStats` struct with atomic counters for proxy metrics
   - Formatted output methods for concise, readable stats reports

2. **Updated configuration** (`src/cache/config.rs`):
   - Added `enable_stats` flag (default: true)
   - Added `stats_interval_secs` parameter (default: 30 seconds)

3. **Enhanced ProgressiveDownloader** (`src/cache/downloader.rs`):
   - Integrated DownloaderStats tracking
   - Added periodic stats reporting via tokio interval
   - Tracks: downloads started/completed/failed, bytes downloaded, active/queued counts
   - Shows top 3 active downloads with progress and speed

4. **Enhanced CacheProxy** (`src/cache/proxy.rs`):
   - Integrated ProxyStats tracking
   - Added periodic stats reporting
   - Tracks: requests served, cache hits/misses, bytes served, active streams
   - Differentiates between range and full requests

5. **Updated FileCache** (`src/cache/file_cache.rs`):
   - Passes stats configuration to proxy initialization

### Stats Output Examples:

Downloader stats:
```
📊 Downloader Stats [0h 1m 30s] | Started: 5 | Completed: 3 | Failed: 1 | Total: 245.3 MB | Avg: 2.72 MB/s
   Active: 1 | Queued: 2
   Downloads:
     • source1:movie123 [45%] @ 1024.5 KB/s
```

Proxy stats:
```
🌐 Proxy Stats [0h 1m 30s] | Requests: 150 | Hit Rate: 78.3% | Active: 3 | Served: 1.45 GB | Avg: 16.11 MB/s | Range: 120 | Full: 30
```

### Key Features:
- Non-blocking atomic counters for thread-safe stats collection
- Configurable reporting interval and enable/disable flag
- Concise single-line format with optional details for active operations
- Uses info log level for visibility without spam
- Automatic cleanup when components shut down
