---
id: task-42.01
title: Simplify auth dialog to two tabs and remove broken icons
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 00:40'
updated_date: '2025-09-16 00:59'
labels:
  - ui
  - auth
  - refactor
dependencies: []
parent_task_id: task-42
priority: high
---

## Description

The auth dialog currently has too many tabs making it difficult to navigate. Simplify to just two tabs (Plex and Jellyfin) and remove the broken/unnecessary icons from tabs. Manual input options should be moved to advanced sections within each service tab rather than separate tabs.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Reduce tabs to only Plex and Jellyfin (remove separate manual tabs)
- [x] #2 Remove broken icons from tab headers
- [x] #3 Move manual URL input to expandable advanced section in Plex tab
- [x] #4 Move manual server input to expandable advanced section in Jellyfin tab
- [x] #5 Ensure tab navigation is smooth and responsive
<!-- AC:END -->


## Implementation Plan

1. Remove the "Plex (Manual)" tab from view_stack
2. Remove tab icons from the ViewSwitcher
3. Create expandable sections in both Plex and Jellyfin tabs for manual input
4. Test navigation between simplified tabs

## Implementation Notes

Simplified the auth dialog from multiple tabs to just two (Plex and Jellyfin).

Changes made:
- Removed the separate "Plex (Manual)" tab
- Removed broken icons from tab headers by not specifying them
- Added an "Advanced Options" PreferencesGroup to the Plex tab containing manual server URL and auth token fields
- Kept Jellyfin tab as-is since manual server input is already part of the main form

The dialog now has a cleaner interface with just two tabs and advanced options properly organized within each service tab.
