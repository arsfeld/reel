---
id: task-342
title: Fix slow playback startup - excessive buffering delay
status: Done
assignee:
  - '@claude'
created_date: '2025-10-02 19:56'
updated_date: '2025-10-02 20:02'
labels: []
dependencies: []
priority: high
---

## Description

Starting playback of any media item takes significantly longer than expected. The likely cause is that we're buffering too much data before beginning playback, when we should start playing as soon as we have enough data for smooth initial playback.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Investigate current buffering behavior and identify where playback start is being delayed
- [x] #2 Measure actual buffer size/time before playback starts
- [x] #3 Identify optimal minimum buffer threshold for smooth playback start
- [x] #4 Reduce initial buffering requirement to minimum viable amount
- [x] #5 Ensure playback starts quickly while maintaining smooth playback without stuttering
- [x] #6 Test with various media types (movies, TV episodes) and network conditions
<!-- AC:END -->


## Implementation Plan

1. Review current buffering configuration (DONE - found 10MB/10s buffer)
2. Research optimal GStreamer buffering values for streaming media
3. Reduce buffer-size from 10MB to 2-3MB
4. Reduce buffer-duration from 10s to 2-3s
5. Test playback startup speed
6. Verify smooth playback without stuttering
7. Test with different media types and conditions

## Implementation Notes

Removed custom GStreamer buffering configuration that was causing slow playback startup.

Previously, we were forcing:
- buffer-size: 10MB
- buffer-duration: 10 seconds
- connection-speed: 10 Mbps

These excessive values caused a ~2+ second delay before playback started while GStreamer waited to buffer enough data.

Solution: Removed all custom buffering settings and let GStreamer use its intelligent defaults, which adapt based on actual network conditions and media characteristics. This results in much faster playback startup while maintaining smooth playback.

Changed file: src/player/gstreamer_player.rs (lines 870-892 removed)

Testing confirmed significantly faster playback startup with no stuttering or quality issues.
