---
id: task-465.02
title: Create cache statistics aggregation for real-time metrics
status: Done
assignee: []
created_date: '2025-11-22 18:32'
updated_date: '2025-11-22 19:12'
labels: []
dependencies: []
parent_task_id: task-465
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The cache system has DownloaderStats and ProxyStats but they're not exposed for UI consumption. We need a way to query current download metrics in real-time.

This involves creating a method to aggregate and expose download speed, bytes downloaded, and active transfer information for the currently playing media.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 CacheProxy or ChunkManager exposes get_current_stats() method
- [x] #2 Method returns current download speed in bytes/sec
- [x] #3 Method returns total bytes downloaded for current media
- [x] #4 Method returns active download count
- [x] #5 Stats update at least every second
- [x] #6 Method is thread-safe and can be called from UI thread
<!-- AC:END -->
