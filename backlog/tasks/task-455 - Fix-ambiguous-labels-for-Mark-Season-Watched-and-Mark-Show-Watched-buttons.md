---
id: task-455
title: Fix ambiguous labels for Mark Season Watched and Mark Show Watched buttons
status: Done
assignee: []
created_date: '2025-10-23 02:31'
updated_date: '2025-10-23 02:33'
labels:
  - ui
  - usability
  - show-details
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Currently the buttons to mark a season as watched and mark an entire show as watched have the same icon and the same text, making it impossible to distinguish which button does what.

The buttons should have:
- Distinct labels (e.g., "Mark Season as Watched" vs "Mark Show as Watched")
- Potentially different icons or visual indicators to make their scope clear
- Clear indication of whether they affect just the current season or the entire show

This affects the TV show details page where both buttons are present.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed the ambiguous button labels in the show details page by:

1. **Updated button labels** to be more specific:
   - Show button: "Mark Show as Watched" / "Mark Show as Unwatched"
   - Season button: "Mark Season as Watched" / "Mark Season as Unwatched"

2. **Changed icons** to be visually distinct:
   - Show button: Uses `media-playlist-consecutive-symbolic` (unwatched) and `view-list-symbolic` (watched)
   - Season button: Uses `folder-symbolic` (unwatched) and `folder-open-symbolic` (watched)

These changes make it immediately clear which button affects the entire show versus just the current season, both through the label text and the icon choice.

File modified: src/ui/pages/show_details.rs:324-341, 355-368
<!-- SECTION:NOTES:END -->
