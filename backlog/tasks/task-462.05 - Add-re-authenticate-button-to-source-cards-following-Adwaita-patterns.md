---
id: task-462.05
title: Add re-authenticate button to source cards following Adwaita patterns
status: Done
assignee: []
created_date: '2025-11-20 23:43'
updated_date: '2025-11-21 00:06'
labels:
  - ui
  - adwaita
  - sources-page
dependencies: []
parent_task_id: task-462
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add a "Re-authenticate" button to source list items when authentication is required, following Adwaita button guidelines.

Implementation:
- Add re-authenticate button to SourceListItem widget
- Only show button when auth_status is AuthRequired
- Use suggested action style for the re-authenticate button
- Position according to HIG (likely in actions area with Sync/Remove buttons)
- Button triggers SourceListItemInput::Reauth message
- Add appropriate icon (e.g., dialog-password-symbolic)
- Ensure button state (enabled/disabled) reflects current operation status

Follow Adwaita button patterns from HIG for action buttons in list items.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Re-authenticate button appears only when auth is required
- [x] #2 Button uses suggested action style appropriately
- [x] #3 Button is properly positioned and aligned
- [x] #4 Clicking button triggers re-authentication flow
- [x] #5 Button has appropriate icon and label
- [ ] #6 Button state reflects loading/disabled when operation in progress
<!-- AC:END -->
