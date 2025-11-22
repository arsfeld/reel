---
id: task-444
title: Fix TV show details page season dropdown double background
status: Done
assignee: []
created_date: '2025-10-23 01:06'
updated_date: '2025-10-23 01:09'
labels:
  - ui
  - bug
  - tv-shows
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The season dropdown selector on the TV show details page currently displays with a double background, creating a visual artifact. This should be fixed to show a clean, single background consistent with the rest of the UI.

Location: `src/ui/pages/show_details.rs`

The issue is likely in the dropdown widget styling or the container it's placed in, where both the dropdown and its parent container have background colors applied.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Season dropdown displays with a single, clean background
- [x] #2 Dropdown styling is consistent with other UI elements
- [x] #3 No visual artifacts or double backgrounds visible
- [x] #4 Tested on GNOME/libadwaita with dark and light themes
<!-- AC:END -->
