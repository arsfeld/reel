---
id: task-047
title: Simplify Plex auth tab by removing redundant content and improving layout
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 02:24'
updated_date: '2025-09-16 17:40'
labels:
  - ui
  - auth
  - ux
dependencies: []
priority: high
---

## Description

The Plex authentication tab has unnecessary visual clutter with a large icon and explanatory paragraph. Streamline the interface by removing the 'Connect to Plex' section, renaming the sign-in button to 'Automatic Login', and hiding advanced options by default to create a cleaner, more focused authentication experience.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Remove the large Plex icon from the authentication tab
- [x] #2 Remove the explanatory paragraph starting with 'Authenticate with your Plex account...'
- [x] #3 Rename 'Sign in with Plex' button to 'Automatic Login'
- [x] #4 Hide Advanced Options section by default (make it collapsed/expandable)
- [x] #5 Ensure Advanced Options can be expanded when needed by users
- [x] #6 Maintain all existing authentication functionality
<!-- AC:END -->

## Implementation Notes

Successfully simplified the Plex auth tab by:
1. Removed the large Plex icon (adw::StatusPage with network-server-symbolic icon)
2. Removed the descriptive text 'Authenticate with your Plex account to access your media libraries'
3. Renamed the authentication button from 'Sign in with Plex' to 'Automatic Login'
4. Converted the Advanced Options section to use an adw::ExpanderRow within an adw::PreferencesGroup
5. The Advanced Options are now collapsed by default and can be expanded to show manual configuration fields
6. All authentication functionality has been maintained - both OAuth and manual token authentication still work
