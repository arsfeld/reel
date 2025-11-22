---
id: task-462.07
title: Add loading states and progress indicators during re-authentication
status: Done
assignee: []
created_date: '2025-11-20 23:43'
updated_date: '2025-11-21 02:07'
labels:
  - ui
  - adwaita
  - user-experience
dependencies: []
parent_task_id: task-462
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement proper loading states following Adwaita patterns for re-authentication operations.

UI requirements:
- Show spinner on re-authenticate button while operation is in progress
- Disable button during re-authentication to prevent double-clicks
- Show loading state on source card while re-authenticating
- Use Adwaita spinner widget with appropriate size
- Add timeout handling (show error if operation takes too long)
- Clear loading state on success or failure
- Show temporary success indicator after successful re-auth

Follow GNOME HIG guidelines for progress and loading indicators.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Loading spinner appears during re-authentication
- [x] #2 Button is disabled during operation
- [x] #3 Loading state is visible but doesn't block UI
- [x] #4 Timeout handling prevents infinite loading
- [x] #5 Success state is briefly shown after completion
- [x] #6 Loading state is cleared on error
- [x] #7 UI remains responsive during operation
<!-- AC:END -->
