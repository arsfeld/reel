---
id: task-073
title: Restore unwatched indicators with glow effect
status: To Do
assignee: []
created_date: '2025-09-17 02:46'
labels:
  - ui
  - css
  - animation
dependencies: []
priority: high
---

## Description

Implement glowing unwatched indicators on media cards to visually highlight content that hasn't been watched, restoring functionality lost in GTK to Relm4 transition

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Unwatched indicator appears as glowing dot in top-right corner of media cards
- [ ] #2 Indicator shows unwatched count badge for shows with unwatched episodes
- [ ] #3 CSS animation creates subtle glow pulse effect
- [ ] #4 Indicator uses theme-aware colors (bright blue or green)
- [ ] #5 Indicator only shows for truly unwatched content based on playback_progress data
- [ ] #6 Animation performance is smooth and doesn't impact scrolling
<!-- AC:END -->
