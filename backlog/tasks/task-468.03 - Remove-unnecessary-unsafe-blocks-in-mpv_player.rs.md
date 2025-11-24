---
id: task-468.03
title: Remove unnecessary unsafe blocks in mpv_player.rs
status: Done
assignee: []
created_date: '2025-11-24 19:57'
updated_date: '2025-11-24 20:11'
labels:
  - cleanup
dependencies: []
parent_task_id: task-468
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Remove 3 unnecessary nested unsafe blocks in src/player/mpv_player.rs:
- Line 696: Nested unsafe inside outer unsafe block (glGetIntegerv)
- Line 727: Nested unsafe inside outer unsafe block (glViewport)  
- Line 762: Nested unsafe inside outer unsafe block (glFlush)

These are all inside an outer unsafe block starting at line 663, so the inner unsafe blocks are redundant.
<!-- SECTION:DESCRIPTION:END -->
