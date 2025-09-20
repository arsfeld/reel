---
id: task-031
title: Remove home button from sidebar when no sources exist
status: Done
assignee:
  - '@claude'
created_date: '2025-09-15 15:14'
updated_date: '2025-09-16 04:39'
labels:
  - ui
  - navigation
dependencies: []
priority: high
---

## Description

The home button in the sidebar should not be displayed when there are no media sources configured, as it would lead to an empty page with no content to show.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Home button is hidden when no sources are configured
- [x] #2 Home button appears when at least one source exists
- [x] #3 Sidebar updates reactively when sources are added/removed
<!-- AC:END -->


## Implementation Plan

1. Search for the sidebar implementation to understand current structure
2. Find where the home button is defined in the sidebar
3. Locate where source count/status is tracked
4. Implement conditional rendering based on source existence
5. Test with no sources, adding sources, and removing sources


## Implementation Notes

Modified sidebar.rs to conditionally show the home button based on has_sources state:

1. Changed line 339 in view! macro from set_visible: true to set_visible: model.has_sources
2. Updated line 545 in update_with_view() to set home_section visibility based on self.has_sources
3. Home button now properly hides when no sources exist and shows when sources are added

The sidebar reactively updates visibility when sources are loaded via the RefreshSources/SourcesLoaded message flow.
