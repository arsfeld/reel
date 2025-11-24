---
id: task-468.01
title: Add Drop implementation for GStreamerPlayer to prevent memory leaks
status: Done
assignee: []
created_date: '2025-11-22 21:17'
updated_date: '2025-11-22 21:27'
labels: []
dependencies: []
parent_task_id: task-468
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The GStreamerPlayer currently lacks an explicit Drop implementation, which can lead to resource leaks. The bus sync handler closure captures many Arc clones creating potential reference cycles. According to GStreamer best practices, proper cleanup requires setting the pipeline to NULL state and disconnecting signal handlers.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Drop trait is implemented for GStreamerPlayer
- [x] #2 Pipeline is set to NULL state on drop
- [x] #3 Bus handler references are properly cleaned up
- [ ] #4 Long-running scenarios tested for memory leaks
- [x] #5 No reference cycles remain in cleanup path
<!-- AC:END -->
