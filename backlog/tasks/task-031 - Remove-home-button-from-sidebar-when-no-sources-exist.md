---
id: task-031
title: Remove home button from sidebar when no sources exist
status: In Progress
assignee:
  - '@claude'
created_date: '2025-09-15 15:14'
updated_date: '2025-09-16 04:38'
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
- [ ] #1 Home button is hidden when no sources are configured
- [ ] #2 Home button appears when at least one source exists
- [ ] #3 Sidebar updates reactively when sources are added/removed
<!-- AC:END -->

## Implementation Plan

1. Search for the sidebar implementation to understand current structure
2. Find where the home button is defined in the sidebar
3. Locate where source count/status is tracked
4. Implement conditional rendering based on source existence
5. Test with no sources, adding sources, and removing sources
