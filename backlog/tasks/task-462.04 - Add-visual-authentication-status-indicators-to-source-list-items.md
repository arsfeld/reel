---
id: task-462.04
title: Add visual authentication status indicators to source list items
status: Done
assignee: []
created_date: '2025-11-20 23:43'
updated_date: '2025-11-21 02:05'
labels:
  - ui
  - adwaita
  - sources-page
dependencies: []
parent_task_id: task-462
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Update the Sources page UI to show clear authentication status following GNOME HIG and Adwaita patterns.

UI changes:
- Add status indicator (icon + text) showing "Connected", "Authentication Required", "Disconnected"
- Use Adwaita status colors: success (green), warning (yellow), error (red)
- Add tooltip explaining the status
- Position status indicator according to HIG (likely in subtitle or trailing section)
- Use appropriate Adwaita icons (e.g., dialog-warning-symbolic for auth required)
- Ensure status updates reactively when authentication state changes

Reference GNOME HIG for status indicators and color usage.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Status indicator is visually clear and follows Adwaita design patterns
- [x] #2 Uses appropriate icons from icon-naming-spec
- [x] #3 Status colors match GNOME HIG guidelines
- [x] #4 Tooltips provide helpful context
- [x] #5 Status updates in real-time when backend auth status changes
- [x] #6 UI is accessible (proper ARIA labels, keyboard navigation)
<!-- AC:END -->
