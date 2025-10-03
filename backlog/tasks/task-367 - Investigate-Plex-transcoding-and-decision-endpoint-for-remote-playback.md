---
id: task-367
title: Investigate Plex transcoding and decision endpoint for remote playback
status: To Do
assignee: []
created_date: '2025-10-03 16:09'
labels:
  - research
  - plex
  - streaming
  - documentation
dependencies: []
priority: high
---

## Description

Remote Plex connections cannot directly stream media files - they require going through the Plex transcoding/decision endpoint even for direct play scenarios. Research how Plex's /video/:/transcode/universal/decision endpoint works, what parameters it requires, and how to properly request direct play streams through it.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Document Plex decision endpoint API structure and required parameters
- [ ] #2 Identify difference between local direct URLs and transcode endpoint URLs
- [ ] #3 Research how Plex clients request direct play vs transcoded streams
- [ ] #4 Document how to construct proper decision endpoint requests for direct play
- [ ] #5 Identify what response format the decision endpoint returns
- [ ] #6 Create plan for implementing decision endpoint support in stream URL generation
<!-- AC:END -->
