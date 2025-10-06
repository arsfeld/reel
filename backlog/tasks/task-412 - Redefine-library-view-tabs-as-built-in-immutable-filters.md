---
id: task-412
title: Redefine library view tabs as built-in immutable filters
status: To Do
assignee: []
created_date: '2025-10-06 13:29'
labels:
  - ui
  - refactor
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Library view tabs (All, Unwatched, etc.) currently add filters to the UI, which is incorrect. Instead, they should represent pre-determined built-in views that pre-filter all items but don't show the filter in the UI. Each view is immutable and built into the system, but users can add additional filters on top of these base views.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Library view tabs pre-filter items without showing the filter in the UI
- [ ] #2 Switching to 'Unwatched' view filters to unwatched items without displaying an 'unwatched' filter pill
- [ ] #3 Each library view acts as an immutable built-in base filter
- [ ] #4 Users can still add additional filters on top of the active library view
- [ ] #5 Filter UI only shows user-added filters, not the built-in view filter
<!-- AC:END -->
