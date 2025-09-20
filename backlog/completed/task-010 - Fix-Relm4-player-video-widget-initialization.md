---
id: task-010
title: Fix Relm4 player video widget initialization
status: Done
assignee:
  - '@claude'
created_date: '2025-09-15 02:10'
updated_date: '2025-09-15 02:19'
labels:
  - player
  - relm4
  - video
dependencies: []
---

## Description

The Relm4 player shows 'Initializing player...' but doesn't properly display the video widget. The video container needs to be properly populated with the actual video widget from the player backend (MPV or GStreamer).

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Remove placeholder label after video widget is created
- [x] #2 Ensure video widget is properly attached to container
- [x] #3 Handle both MPV and GStreamer video widget creation
- [x] #4 Set proper widget expansion and alignment properties
- [x] #5 Verify video actually displays in the widget
<!-- AC:END -->


## Implementation Plan

1. Analyze current video widget initialization flow
2. Store reference to placeholder widget
3. Remove placeholder when video widget is created
4. Ensure proper widget expansion and alignment
5. Test with both MPV and GStreamer backends


## Implementation Notes

Fixed the video widget initialization issue by:
1. Added video_placeholder field to PlayerPage struct to track the placeholder label
2. Modified initialization to store reference to placeholder
3. Updated video widget creation code to remove placeholder before adding video widget
4. Set proper expansion and alignment properties on the video widget
5. Verified with both MPV and GStreamer backends - video widget successfully attaches
