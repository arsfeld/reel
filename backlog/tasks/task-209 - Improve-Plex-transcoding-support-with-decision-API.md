---
id: task-209
title: Improve Plex transcoding support with decision API
status: To Do
assignee: []
created_date: '2025-09-22 14:19'
labels:
  - backend
  - plex
  - api
  - transcoding
dependencies:
  - task-206
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Enhance transcoding capabilities by implementing the Plex transcode decision API and subtitle handling endpoints. This will provide better quality selection, codec compatibility checking, and improved subtitle support.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Transcode decision endpoint (/{transcodeType}/:/transcode/universal/decision) is implemented
- [ ] #2 Subtitle transcoding endpoint (/{transcodeType}/:/transcode/universal/subtitles) handles subtitle streams
- [ ] #3 Quality selection logic uses decision API instead of hardcoded parameters
- [ ] #4 Codec compatibility checking prevents playback failures
- [ ] #5 Subtitle rendering works without color artifacts (addresses GStreamer issues)
<!-- AC:END -->
