---
id: task-301
title: Fix GStreamer seeking functionality
status: Done
assignee:
  - '@claude'
created_date: '2025-09-28 04:08'
updated_date: '2025-09-29 02:19'
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
- [x] #1 Seeking completes without HTTP source errors
- [x] #2 No matroska demuxer errors after seeking
- [x] #3 Playback continues smoothly after seek operations
- [x] #4 Network streams handle seeking without data flow interruptions
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Research GStreamer HTTP source seeking requirements and matroska demuxer behavior
2. Add source-setup signal handler to configure curlhttpsrc for better seeking
3. Implement seek recovery mechanism when errors occur
4. Improve seek flags and buffering configuration
5. Test with various media formats and network conditions
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed GStreamer seeking failures by changing seek flags from ACCURATE to KEY_UNIT.

Root Cause:
- ACCURATE seeks try to position to exact byte positions in HTTP streams
- This causes curlhttpsrc to request arbitrary positions in Matroska files
- The demuxer receives incomplete/wrong data and fails to parse (Element 0xAB is SeekID)

Solution:
- Changed seek flags from FLUSH|ACCURATE to FLUSH|KEY_UNIT in seek() and frame_step_forward()
- KEY_UNIT ensures seeking only to valid cluster boundaries in Matroska format
- This allows HTTP range requests to work properly with the container format

Files modified:
- src/player/gstreamer_player.rs: Updated seek flags in two locations
<!-- SECTION:NOTES:END -->
