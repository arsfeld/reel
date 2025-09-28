---
id: task-253
title: App crashes when navigating to Home while already on Home page
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 14:15'
updated_date: '2025-09-26 17:42'
labels:
  - bug
  - navigation
  - crash
dependencies: []
priority: high
---

## Description

The application crashes with a panic when the user clicks on Home in the sidebar while already on the Home page. The crash occurs due to a component runtime shutdown error in the ViewportScrolled handler.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Prevent crash when navigating to same page (Home to Home)
- [x] #2 Ensure navigation components remain valid during re-navigation
- [x] #3 Add guard to check if already on target page before navigation
- [x] #4 Test navigation to same page for all sidebar items
<!-- AC:END -->


## Implementation Plan

1. Locate the home navigation code in main_window.rs to understand the crash
2. Add guard to check if already on home page before navigating
3. Test navigation to same page for all main pages (home, sources, library)
4. Verify crash is resolved by navigating to home while already on home


## Implementation Notes

Fixed the crash by improving the navigation guard in main_window.rs. The issue was that when already on Home page and clicking Home again, the code was incorrectly popping pages from the navigation stack, which could remove the current Home page itself. The fix checks if the visible page is already "Home" and if so, simply updates the header without any navigation stack manipulation.
