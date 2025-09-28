---
id: task-301
title: Fix GStreamer seeking functionality
status: To Do
assignee: []
created_date: '2025-09-28 04:08'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
GStreamer seeking appears to succeed but immediately fails with internal data flow errors from curlhttpsrc and matroska demuxer. The seek operation completes successfully but then the stream encounters errors that suggest the HTTP source and demuxer cannot properly handle the seek position, leading to EOS without complete header and streaming errors.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Seeking completes without HTTP source errors
- [ ] #2 No matroska demuxer errors after seeking
- [ ] #3 Playback continues smoothly after seek operations
- [ ] #4 Network streams handle seeking without data flow interruptions
<!-- AC:END -->
