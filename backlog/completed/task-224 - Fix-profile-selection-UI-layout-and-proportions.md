---
id: task-224
title: Fix profile selection UI layout and proportions
status: Done
assignee:
  - '@claude'
created_date: '2025-09-22 22:59'
updated_date: '2025-09-23 18:25'
labels:
  - plex
  - authentication
  - ui
  - frontend
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The profile selection screen has visual layout issues that affect usability. The title and avatar icon are disproportionately large, profile cards are too small, and there's excessive wasted space around the profile grid. The flowbox dimensions and spacing need adjustment for better visual balance and GNOME HIG compliance.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Profile selection title and avatar icon are properly sized relative to profile cards
- [x] #2 Profile cards have appropriate size for easy selection
- [x] #3 Profile grid layout uses space efficiently without excessive gaps
- [x] #4 Overall layout follows GNOME Human Interface Guidelines
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed all profile selection UI layout issues:

1. Replaced oversized StatusPage with simple labels (title-2 style)
2. Increased profile card dimensions from 140x120 to 160x180
3. Improved spacing with better margins and padding
4. Added card styling and proper centering
5. Removed fixed FlowBox dimensions for better adaptivity
6. Adjusted avatar size from 64px to 80px for better proportions
7. Fixed lock icon positioning for protected profiles
<!-- SECTION:NOTES:END -->
