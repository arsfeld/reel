---
id: task-302
title: Hide scaling options in player UI
status: Done
assignee:
  - '@claude'
created_date: '2025-09-28 17:46'
updated_date: '2025-10-02 18:00'
labels:
  - ui
  - player
  - mpv
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The scaling options in the player don't work with the current MPV embed stack and should be hidden from users
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Remove or hide scaling options from player UI
- [x] #2 Ensure player UI remains functional without scaling controls
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Locate the quality_menu_button in the player UI view macro (around line 1019)
2. Hide the button by setting it invisible or removing it from the UI
3. Test that player UI still works without the scaling controls
4. Build and verify no compilation errors
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Hidden the video quality/upscaling menu button in the player UI by setting set_visible: false on the quality_menu_button in src/ui/pages/player.rs:1023.

The upscaling options (None, High Quality, FSR, Anime) do not work with the current MPV embed stack, so hiding them prevents user confusion. The button widget is still created but is invisible, maintaining code structure while removing non-functional UI elements.

Tested with cargo check - build completes successfully with no errors.
<!-- SECTION:NOTES:END -->
