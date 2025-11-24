---
id: task-468.02
title: Switch from sync bus handler to async bus watch with glib integration
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
The current implementation uses set_sync_handler which blocks the thread that posted messages. For GTK4 applications, GStreamer recommends using add_watch() which marshals messages to the main thread via glib main loop. This prevents thread blocking and integrates properly with the GTK event loop.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Bus handling uses add_watch instead of set_sync_handler
- [x] #2 Messages are marshaled to main thread via glib
- [x] #3 Bus watch ID is stored for cleanup in Drop
- [x] #4 All bus message handling works correctly in async context
- [x] #5 No thread blocking occurs during message handling
<!-- AC:END -->
