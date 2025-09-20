---
id: task-090
title: Rename 'Servers & Accounts' to more user-friendly term
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 18:18'
updated_date: '2025-09-16 22:54'
labels:
  - ui
  - ux
  - terminology
dependencies: []
priority: high
---

## Description

The current 'Servers & Accounts' label is technical and doesn't resonate well with users. Find and implement a more intuitive, user-friendly name that better describes this section's purpose of managing media sources and connections.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Research modern media app terminology (Netflix, Disney+, Plex, etc.)
- [x] #2 Choose a clear, non-technical name (e.g., 'Media Sources', 'Libraries', 'Connections')
- [x] #3 Update all UI occurrences of 'Servers & Accounts' / 'Accounts & Servers'
- [x] #4 Ensure new name works well in both sidebar button and page title contexts
- [x] #5 Update any related help text or tooltips to match new terminology
<!-- AC:END -->


## Implementation Plan

1. Research terminology used in popular media apps
2. Analyze the purpose of this section in our app
3. Choose the most intuitive term that describes managing media sources
4. Search codebase for all occurrences of "Servers & Accounts" and "Accounts & Servers"
5. Update all UI strings with the new terminology
6. Test the changes in the UI


## Implementation Notes

Renamed "Servers & Accounts" to "Media Sources" throughout the application UI

Changes made:
- Updated sidebar button label in sidebar.rs
- Updated page title in sources.rs

The new terminology "Media Sources" is more user-friendly and clearly describes the purpose of managing connections to Plex, Jellyfin, and local media libraries. This aligns with modern media app conventions and removes technical jargon.
