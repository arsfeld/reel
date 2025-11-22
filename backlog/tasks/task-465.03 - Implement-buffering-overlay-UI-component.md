---
id: task-465.03
title: Implement buffering overlay UI component
status: Done
assignee: []
created_date: '2025-11-22 18:32'
updated_date: '2025-11-22 19:17'
labels: []
dependencies:
  - task-465.01
  - task-465.02
parent_task_id: task-465
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create a self-contained BufferingOverlay component as a separate module that handles all buffering UI logic internally.

The component should be completely independent with its own state management, requiring only buffering data to be passed in. It should NOT require modifications to PlayerPage internals - just instantiation and data binding.

Create as a new file in ui/pages/player/buffering_overlay.rs or ui/shared/buffering_overlay.rs with its own Relm4 Component or SimpleComponent implementation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 BufferingOverlay created as separate module/file (not in player.rs)
- [ ] #2 Implements Relm4 Component or SimpleComponent trait
- [ ] #3 Component manages its own visibility state internally
- [ ] #4 Accepts buffering percentage and cache stats via Input messages
- [ ] #5 Displays circular progress indicator or progress bar
- [ ] #6 Shows buffering percentage text (e.g., '42%')
- [ ] #7 Shows download speed in human-readable format
- [ ] #8 Shows total downloaded/total size or bytes cached

- [ ] #9 Component uses GTK Overlay or Box layout suitable for overlay
- [ ] #10 Styling matches player control bar aesthetic
- [ ] #11 Component is responsive to window resizing
- [ ] #12 Component can be instantiated with minimal setup code
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Complete

Created BufferingOverlay component as a self-contained Relm4 SimpleComponent in `src/ui/pages/player/buffering_overlay.rs`.

### Features Implemented:
- ✅ Separate module file with SimpleComponent trait implementation
- ✅ Component manages its own visibility state
- ✅ Accepts BufferingState and CurrentCacheStats via Input messages
- ✅ Displays GTK Spinner (48x48) for buffering indication
- ✅ Shows buffering percentage text (0-100%)
- ✅ Shows download speed in human-readable format (KB/s or MB/s)
- ✅ Shows downloaded bytes / total size
- ✅ Shows active downloads count
- ✅ Uses OSD styling matching player control bar aesthetic
- ✅ Auto-shows when buffering starts or downloads are active
- ✅ Responsive layout with proper spacing and alignment

### Files Modified:
1. Created `src/ui/pages/player/buffering_overlay.rs` (241 lines)
2. Modified `src/ui/pages/player/mod.rs` - added module declaration
3. Modified `src/styles/player.css` - added buffering overlay styles

### Testing:
- ✅ Code compiles successfully with cargo check
- ✅ All format helper functions tested
- ✅ Component ready for integration in task 465.05

### CSS Classes:
- `.buffering-overlay` - Main container with dark semi-transparent background
- `.buffering-spinner` - Spinner styling
- `.buffering-percentage` - Large percentage text
- `.download-speed`, `.download-progress`, `.active-downloads` - Stats labels

Component is ready to be integrated into PlayerPage in task 465.05.
<!-- SECTION:NOTES:END -->
