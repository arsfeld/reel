---
id: task-218
title: Expose shader upscaling controls in player UI
status: Done
assignee:
  - '@claude'
created_date: '2025-09-22 17:51'
updated_date: '2025-09-22 17:56'
labels:
  - ui
  - player
  - enhancement
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Connect the existing Video Quality menu button in the player controls to the MPV upscaling modes. The button should display a menu with options for None, High Quality, FSR, and Anime upscaling modes, allowing users to switch between different shader configurations during playback.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Video Quality button displays a menu with all available upscaling modes
- [x] #2 Clicking an upscaling mode immediately applies it to the current video
- [x] #3 Current upscaling mode is indicated with a checkmark in the menu
- [x] #4 Menu updates dynamically based on player backend (only show for MPV)
- [x] #5 Upscaling mode persists across playback sessions
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Examine the existing Video Quality button implementation in the player controls
2. Review the MPV upscaling mode implementation from task 217
3. Create a menu model with the upscaling options
4. Connect the menu to apply upscaling modes via MPV controller
5. Add state tracking to show current mode with checkmark
6. Implement persistence using the preferences system
7. Add backend detection to only show menu for MPV player
8. Test all upscaling modes during video playback
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

Successfully exposed shader upscaling controls in the player UI by:

1. **Added Configuration Support**:
   - Added `mpv_upscaling_mode` field to PlaybackConfig in config.rs
   - Implemented persistence of upscaling mode preference
   - Added default functions and serialization helpers

2. **Enhanced Player UI**:
   - Added quality_menu_button, current_upscaling_mode, and is_mpv_backend fields to PlayerPage
   - Implemented populate_quality_menu() method to dynamically create the upscaling menu
   - Connected menu to existing Video Quality button in player controls

3. **Implemented Menu Logic**:
   - Menu displays all 4 upscaling modes: None, High Quality, FSR, and Anime
   - Current mode is indicated with checkmark icon 
   - Menu is disabled for non-MPV backends with appropriate tooltip
   - Added SetUpscalingMode and UpdateQualityMenu input handlers

4. **Integrated with Player Backend**:
   - Menu actions call PlayerHandle::set_upscaling_mode() to apply changes immediately
   - Saved upscaling mode is applied when player initializes with MPV backend
   - Preference is saved to config whenever user changes the mode

The implementation follows the existing pattern used for audio/subtitle track selection menus, ensuring consistency in the UI. The quality menu is properly initialized on player startup and updated when tracks are loaded.
<!-- SECTION:NOTES:END -->
