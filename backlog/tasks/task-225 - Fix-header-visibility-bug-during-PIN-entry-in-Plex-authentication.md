---
id: task-225
title: Fix header visibility bug during PIN entry in Plex authentication
status: Done
assignee: []
created_date: '2025-09-22 22:59'
updated_date: '2025-09-23 18:26'
labels:
  - plex
  - authentication
  - ui
  - frontend
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
After selecting a protected profile and moving to the PIN entry screen, the 'Connect Your Plex Account' header incorrectly reappears. This is a visibility state management issue where the initial connection header is shown when it should remain hidden during the profile selection and PIN entry flow.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Connection header remains hidden during profile selection flow
- [x] #2 Connection header stays hidden during PIN entry screen
- [x] #3 Header visibility state is properly managed throughout authentication flow
- [x] #4 No visual flickering or incorrect header display during authentication steps
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed header visibility bug during PIN entry:

Added plex_pin_input_active check to the initial connection header visibility condition. This prevents the "Connect Your Plex Account" header from showing when the user is in the PIN entry flow.

Also improved the PIN entry UI to match the profile selection improvements with better layout and styling.
<!-- SECTION:NOTES:END -->
