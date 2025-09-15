---
id: task-003
title: Fix movie poster incorrect aspect ratio
status: To Do
assignee: []
created_date: '2025-09-15 01:40'
updated_date: '2025-09-15 01:47'
labels:
  - ui
  - media
  - bug
dependencies: []
priority: medium
---

## Description

Movie posters are displayed with incorrect aspect ratio in MediaCard component. Current dimensions are 130x195 (1:1.5 ratio) but standard movie posters use 27:40 ratio (1:1.48). The hardcoded dimensions in media_card.rs lines 60-61 need adjustment. Additionally, when the window is resized, posters only resize in width rather than maintaining aspect ratio or adding new columns to the grid. The aspect ratio is only respected at the window's minimum allowed size.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Movie posters display with correct 27:40 aspect ratio
- [ ] #2 Poster images are not stretched or distorted
- [ ] #3 MediaCard maintains consistent layout with new dimensions
<!-- AC:END -->
