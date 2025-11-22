---
id: task-462
title: Implement re-authentication flow for expired/invalid credentials
status: Done
assignee: []
created_date: '2025-11-20 23:41'
updated_date: '2025-11-21 02:12'
labels:
  - ui
  - authentication
  - user-experience
  - adwaita
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When authentication tokens expire or become invalid, users currently need to manually delete and re-add sources to re-authenticate. This creates a poor user experience and potential data loss (watch history, progress).

We need a proper re-authentication flow that:
- Detects when sources need re-authentication (token expired/invalid)
- Provides clear UI indicators showing authentication status
- Allows users to re-authenticate without deleting the source
- Preserves all source data, history, and progress
- Follows GNOME HIG and Adwaita design patterns

This will improve reliability and user experience when dealing with authentication failures, especially for Plex local connections that are more prone to token expiration issues.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Sources that need re-authentication are visually distinct with clear status indicators
- [x] #2 Re-authenticate button appears on disconnected sources following Adwaita button patterns
- [x] #3 Clicking re-authenticate opens auth dialog pre-configured for that source
- [x] #4 Re-authentication updates existing source credentials without creating duplicates
- [x] #5 All source data (watch history, progress, library cache) is preserved during re-authentication
- [x] #6 UI shows appropriate loading states during re-authentication process
- [x] #7 Error messages are clear and actionable following GNOME HIG guidelines
- [x] #8 Re-authentication works for all backend types (Plex, Jellyfin)
- [x] #9 Connection status updates in real-time after successful re-authentication
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Successfully implemented complete re-authentication flow with all subtasks completed:

✓ task-462.01 - Authentication status tracking in database
✓ task-462.02 - AuthDialog extended for re-authentication mode
✓ task-462.03 - Backend initialization returns auth status
✓ task-462.04 - Visual status indicators with GNOME HIG-compliant icons and colors
✓ task-462.05 - Re-authenticate button with Adwaita patterns
✓ task-462.06 - Credential update command implementation
✓ task-462.07 - Loading states with spinners and disabled buttons
✓ task-462.08 - User-friendly error messages for all scenarios
✓ task-462.09 - Real-time auth status monitoring via ConnectionMonitor
✓ task-462.10 - End-to-end flow verification

Implementation highlights:
- Authentication status shown with appropriate icons (emblem-ok, dialog-warning, network-offline)
- Status colors follow GNOME HIG (success=green, warning=yellow, error=red)
- Re-authenticate button only visible when auth required, with loading state during operation
- Error messages hide technical details but log them for debugging
- ConnectionMonitor tracks auth status changes and emits notifications
- Toast notifications inform users when re-authentication is needed
- All source data (watch history, progress, library cache) preserved during re-auth
- Works for both Plex and Jellyfin backends

All code compiles successfully with no errors.
<!-- SECTION:NOTES:END -->
