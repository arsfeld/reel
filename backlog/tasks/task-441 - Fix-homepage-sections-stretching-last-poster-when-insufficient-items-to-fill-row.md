---
id: task-441
title: >-
  Fix homepage sections stretching last poster when insufficient items to fill
  row
status: In Progress
assignee:
  - Claude
created_date: '2025-10-23 00:42'
updated_date: '2025-10-23 00:54'
labels: []
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The homepage continue watching section and other horizontal scrolling sections display an overly wide/stretched movie poster when there aren't enough items to fill the available horizontal space. Instead of leaving empty space at the end of the row, the last item expands to fill the remaining space, which distorts the poster aspect ratio and looks unprofessional. The layout should maintain consistent poster sizes and leave empty space at the end when there are fewer items than the row can hold. This same issue also affects the TV show details page episode list, so any fix should be backported there as well.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Movie posters maintain consistent aspect ratio regardless of number of items
- [ ] #2 Last poster in a row does not stretch to fill remaining space
- [ ] #3 Empty space appears at end of row when insufficient items to fill width
- [ ] #4 Behavior is consistent across all homepage sections (continue watching, recommendations, etc.)
- [ ] #5 Poster sizing remains correct when window is resized
- [ ] #6 Layout works correctly with 1 item, few items, and many items

- [ ] #7 Fix is backported to TV show details page episode list to prevent episode stretching
<!-- AC:END -->
