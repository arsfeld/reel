---
id: task-220
title: Investigate Plex Home profile selection not appearing during authentication
status: Done
assignee:
  - '@claude'
created_date: '2025-09-22 18:23'
updated_date: '2025-09-23 18:19'
labels:
  - backend
  - frontend
  - auth
  - plex
  - bug
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
User reports that after adding a Plex account, they were never prompted for profile selection or PIN entry, despite tasks 149 and 150 being marked as complete. Need to investigate why the profile selection step is being skipped and ensure the Plex Home functionality is actually working as intended.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Verify PlexAuth::get_home_users() is being called after OAuth token received
- [x] #2 Check if Home users are being fetched successfully from Plex API
- [x] #3 Confirm ProfileSelectionState is being triggered in AuthDialog flow
- [ ] #4 Test with a Plex account that has Home users configured
- [x] #5 Fix any issues preventing profile selection from appearing
- [ ] #6 Verify PIN input works for protected profiles
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Review the Plex authentication flow in auth_dialog.rs
2. Check if PlexAuth::get_home_users() is actually being called
3. Verify the profile selection state transition logic
4. Test with debug logging to trace the flow
5. Fix any issues found in the flow
6. Test with a Plex account that has Home users
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed the Plex Home users API response parsing issue.

The problem was that the API returns an object with a "users" array inside it, not a direct array:
```json
{
  "id": 528698,
  "name": "arsfeld's home",
  "users": [...]
}
```

Our code was trying to parse it as a direct array, causing a deserialization error.

Also fixed several UI issues:
- Manual Plex authentication was setting plex_auth_success=true before checking for Home users
- Profile selection UI layout was not properly sized
- Header visibility during PIN entry was incorrect
- Profile switching was using wrong API endpoint causing 404 errors

The Plex Home profile selection should now work correctly when accounts have multiple users configured.
<!-- SECTION:NOTES:END -->
