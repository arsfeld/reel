---
id: task-151
title: Implement PIN input dialog for protected profiles
status: To Do
assignee: []
created_date: '2025-09-17 15:36'
labels:
  - frontend
  - ui
  - auth
dependencies: []
priority: medium
---

## Description

Create a simple PIN input dialog that appears when a user selects a PIN-protected profile during authentication. This dialog should handle numeric PIN entry, validation, and error states for incorrect PINs.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Create PinInputDialog component with numeric entry field
- [ ] #2 Implement PIN validation with switch_to_user API call
- [ ] #3 Show error message for incorrect PIN with retry option
- [ ] #4 Add cancel option to go back to profile selection
- [ ] #5 Ensure PIN field is masked for security
- [ ] #6 Auto-focus PIN field and support Enter key for submission
<!-- AC:END -->
