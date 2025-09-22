---
id: task-150
title: Add profile selection step to Plex authentication flow
status: Done
assignee:
  - '@claude'
created_date: '2025-09-17 15:36'
updated_date: '2025-09-22 18:17'
labels:
  - frontend
  - ui
  - auth
dependencies:
  - task-149
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Modify the AuthDialog component to add an intermediate profile selection step after successful OAuth authentication. When Home users are available, display them for selection before completing the authentication process. This keeps profile selection contained within the existing auth flow without requiring any database or backend changes.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add ProfileSelectionState to AuthDialog state machine
- [x] #2 Fetch Home users after receiving initial OAuth token
- [x] #3 Create profile selection UI with user cards showing name and avatar
- [x] #4 Add visual indicator for PIN-protected profiles
- [x] #5 Handle profile selection and proceed to PIN input if needed
- [x] #6 Skip profile selection if no Home users exist (single user account)
- [x] #7 Pass selected profile token to source creation
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add ProfileSelectionState enum variant to AuthDialogInput
2. Add state fields for profile selection (home users list, selected profile)
3. After receiving PlexTokenReceived, fetch Home users
4. Create profile selection UI with user cards in view macro
5. Add PIN input dialog for protected profiles
6. Handle profile selection and switch to user if needed
7. Pass final token to source creation
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

Successfully integrated profile selection into the Plex authentication flow within the AuthDialog component. The implementation adds a seamless intermediate step after OAuth authentication that presents available Home users for selection.

### Key Changes:

1. **State Management**: Added new input enum variants and state fields to handle profile selection:
   - PlexHomeUsersReceived, SelectPlexProfile, PlexPinRequested, PlexProfileTokenReceived
   - plex_home_users, plex_selected_profile, plex_primary_token fields

2. **UI Components**: Created profile selection UI with:
   - FlowBox containing user cards with avatars
   - Visual lock icon indicator for PIN-protected profiles
   - Admin label for admin users
   - "Use Primary Account" option to skip profile selection

3. **PIN Input Dialog**: Added separate PIN input dialog for protected profiles with password entry

4. **Authentication Flow**:
   - After OAuth token received, fetch Home users via PlexAuth::get_home_users()
   - Show profile selection if Home users exist, otherwise skip to source creation
   - Handle profile switching with PlexAuth::switch_to_user()
   - Support PIN authentication for protected profiles

5. **Source Creation**: Extracted source creation logic to helper method proceed_with_plex_source_creation() that handles both primary and profile tokens

The implementation maintains backward compatibility with single-user Plex accounts by automatically skipping profile selection when no Home users exist.
<!-- SECTION:NOTES:END -->
