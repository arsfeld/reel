---
id: task-462.08
title: Implement error handling with actionable error messages
status: Done
assignee: []
created_date: '2025-11-20 23:43'
updated_date: '2025-11-21 02:09'
labels:
  - ui
  - error-handling
  - user-experience
dependencies: []
parent_task_id: task-462
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add comprehensive error handling for re-authentication failures with user-friendly, actionable messages following GNOME HIG.

Error scenarios to handle:
- Network error during re-authentication
- Invalid credentials provided
- Server not reachable
- Token refresh failed
- Backend initialization failed after re-auth

For each error:
- Show Adwaita toast with clear message
- Provide actionable next steps (e.g., "Check network", "Try again")
- Use appropriate severity (error, warning, info)
- Log detailed error for debugging
- Don't expose technical details to users
- Offer retry option where appropriate

Follow GNOME HIG for error message writing and presentation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 All error scenarios have user-friendly messages
- [x] #2 Error messages are actionable with next steps
- [x] #3 Messages follow GNOME HIG writing style
- [x] #4 Technical details are logged but not shown to users
- [x] #5 Appropriate severity levels are used
- [x] #6 Toast notifications are non-blocking
- [x] #7 Retry options are available where sensible
<!-- AC:END -->
