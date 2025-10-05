---
id: task-124
title: Add predefined filter tabs to library view
status: Done
assignee:
  - '@claude-code'
created_date: '2025-09-17 02:54'
updated_date: '2025-10-04 23:43'
labels:
  - ui
  - filtering
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add horizontal tabs at the top of library view for quick access to common filters: All, Unwatched, Recently Added. These should be instant filters on cached data.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add tab bar component to library view
- [x] #2 Implement Unwatched filter using playback_progress data
- [x] #3 Implement Recently Added filter (last 30 days)
- [x] #4 Store last selected tab per library in preferences
- [x] #5 Ensure filters work with existing sort options
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Review existing filter tab implementation in library.rs
2. Add UiPreferences struct to config.rs for storing per-library preferences
3. Add library_filter_tabs HashMap<String, String> to store filter tab per library
4. Add ConfigService methods to get/set library filter tab preference
5. Update LibraryPage::init to load saved filter tab from config
6. Update LibraryPage::SetFilterTab handler to save tab selection to config
7. Test tab persistence by switching between libraries
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Completed filter tab persistence feature. Most functionality was already implemented:

- Tab bar UI with All, Unwatched, Recently Added, Genres, and Years tabs (lines 322-385 in library.rs)
- Unwatched filter using playback_progress data (existing implementation)
- Recently Added filter for last 30 days (existing implementation)
- Tab-specific filter logic working with existing sort options

Implemented missing AC #4 (tab persistence):
- Added UiPreferences struct to config.rs with library_filter_tabs HashMap
- Added ConfigService methods: get_library_filter_tab() and set_library_filter_tab()
- Updated LibraryPage::SetLibrary to load saved tab preference on library switch
- Updated LibraryPage::SetFilterTab to save tab selection to config

The selected filter tab is now persisted per library and restored when switching between libraries.
<!-- SECTION:NOTES:END -->
