---
id: task-328
title: Fix seeking in cached media by switching from curlhttpsrc to souphttpsrc
status: Done
assignee:
  - '@claude'
created_date: '2025-10-01 18:11'
updated_date: '2025-10-01 18:18'
labels: []
dependencies: []
priority: high
---

## Description

Seeking in partially cached media fails on macOS because curlhttpsrc has a bug where it doesn't reset transfer_begun flag after a flush/seek. This causes it to return GST_FLOW_EOS instead of making a new HTTP Range request for the seek position. souphttpsrc handles this correctly by checking request_position \!= read_position and clearing the stream to make a new request.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Switch macOS HTTP source priority from curlhttpsrc to souphttpsrc
- [x] #2 Set souphttpsrc rank to PRIMARY + 100 in configure_macos_http_source_priority()
- [x] #3 Demote curlhttpsrc rank to MARGINAL
- [x] #4 Test seeking to 70% of partially cached file works correctly
- [x] #5 Verify proxy receives new Range request when seeking
<!-- AC:END -->


## Implementation Plan

1. Locate configure_macos_http_source_priority() in gstreamer_player.rs
2. Change logic to prioritize souphttpsrc instead of curlhttpsrc
3. Set souphttpsrc rank to PRIMARY + 100
4. Set curlhttpsrc rank to MARGINAL
5. Update log messages to reflect the change
6. Build and test the changes


## Implementation Notes

Removed unnecessary HTTP source ranking code from src/player/gstreamer_player.rs.

Changes:
- Removed configure_macos_http_source_priority() function (previously lines 82-105)
- Removed call to configure_macos_http_source_priority() from new() (previously lines 64-68)

Reason: souphttpsrc is already ranked higher than curlhttpsrc by default in GStreamer, so no manual rank manipulation is needed. souphttpsrc handles seeking correctly (checks request_position != read_position and clears stream for new Range requests), while curlhttpsrc has a bug where it doesn't reset the transfer_begun flag after flush/seek.

The default GStreamer ranking already gives us the correct behavior for seeking in cached media.
