---
id: task-465.01
title: Extend GStreamer buffering event handling to expose metrics
status: Done
assignee: []
created_date: '2025-11-22 18:32'
updated_date: '2025-11-22 19:07'
labels: []
dependencies: []
parent_task_id: task-465
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The GStreamer bus handler currently receives buffering events but only logs the percentage. We need to expose buffering state and percentage to the UI layer.

This involves extending the bus message handler to track buffering events and propagate them through the player controller to consumers.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 BufferingState added to PlayerState enum or separate state tracking
- [x] #2 Buffering percentage exposed via PlayerHandle getter
- [x] #3 Bus handler processes MessageView::Buffering and updates state
- [x] #4 Buffering state transitions: NotBuffering -> Buffering -> NotBuffering
- [x] #5 MPV player has equivalent buffering support or graceful no-op
<!-- AC:END -->
