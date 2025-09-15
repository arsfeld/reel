---
id: task-025
title: Optimize Plex server discovery to reduce playback startup delay
status: To Do
assignee: []
created_date: '2025-09-15 03:43'
labels:
  - performance
  - plex
  - backend
dependencies: []
priority: high
---

## Description

When playing a Plex media item, there's a significant delay (5+ seconds) while the system tries the saved URL, fails, then discovers servers again. This happens even when the server is available, just at a different address. The delay occurs between clicking play and the video actually starting.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Cache discovered server URLs with appropriate TTL
- [ ] #2 Implement parallel connection testing instead of sequential
- [ ] #3 Store multiple working URLs and try them concurrently
- [ ] #4 Add background server discovery to keep URLs fresh
- [ ] #5 Reduce connection timeout for faster failover
- [ ] #6 Skip discovery if recent successful connection exists
<!-- AC:END -->
