---
id: task-405
title: Move filter tabs to header bar as immutable view switchers
status: Done
assignee:
  - '@assistant'
created_date: '2025-10-06 02:52'
updated_date: '2025-10-06 03:17'
labels:
  - library
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Tabs should be real tabs in the window header bar (replacing 'Library' text), not filter toggles. Each tab provides an immutable view of a specific subset of library items without modifying filters.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Tabs appear in window header bar where 'Library' currently displays
- [x] #2 Tabs are proper view switchers (All, Unwatched, Recently Added, etc.)
- [x] #3 Tabs do NOT modify or interact with filters - they provide distinct views
- [x] #4 Active tab is visually distinct in header bar
- [x] #5 Tab selection persists per library across sessions
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add new MainWindowInput message for setting title widget
2. Modify MainWindow to handle custom title widgets
3. Create ViewStack and ViewSwitcher in LibraryPage
4. Send title widget to MainWindow when LibraryPage initializes
5. Handle view mode changes to update filtered content
6. Clean up old filter tab buttons from sidebar
7. Persist view mode selection per library
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented view switcher tabs in header bar:

1. Added SetTitleWidget message to MainWindowInput for custom title widgets
2. Updated MainWindow handlers to set/clear title widget
3. Renamed FilterTab to ViewMode to reflect immutable view concept
4. Added ViewStack and ViewSwitcher widgets to LibraryPage
5. Populated ViewStack with 5 pages (All, Unwatched, Recently Added, Genres, Years)
6. Connected ViewSwitcher to ViewStack for tab navigation
7. Send ViewSwitcher to MainWindow header on library load
8. Removed old filter tab toggle buttons from sidebar
9. View mode selection persists per library using existing config service
10. Fixed existing spawn() calls to use spawn_local() for non-Send types

Files modified:
- src/ui/main_window/mod.rs: Added SetTitleWidget message and handler
- src/ui/main_window/navigation.rs: Forward SetHeaderTitleWidget output
- src/ui/pages/library.rs: Implemented ViewStack/ViewSwitcher, removed filter tabs

Updates after user feedback:
- Removed Genres and Years tabs (only meaningful views: All, Unwatched, Recently Added)
- Replaced ViewSwitcher/ViewStack with TabBar/TabView to eliminate broken icon issue
- Prevented tab closing by connecting to close-page signal and returning Propagation::Stop
<!-- SECTION:NOTES:END -->
