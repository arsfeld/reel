---
id: task-465.04
title: Add performance warning detection and UI indicators
status: Done
assignee: []
created_date: '2025-11-22 18:32'
updated_date: '2025-11-22 19:20'
labels: []
dependencies:
  - task-465.02
parent_task_id: task-465
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement performance warning detection logic as a separate module/utility that can be used by the BufferingOverlay component.

Create reusable warning detection functions that take current stats and return warning state, keeping all logic out of player.rs. The BufferingOverlay component will use these utilities internally.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Warning detection logic in separate module (e.g., player/buffering_warnings.rs)
- [ ] #2 Function to detect slow download: is_download_too_slow(speed, bitrate) -> bool
- [ ] #3 Function to detect stalled buffering: is_buffering_stalled(history) -> bool
- [ ] #4 Warning messages defined as constants or helper functions
- [ ] #5 Logic is pure/stateless where possible
- [ ] #6 BufferingOverlay can import and use these utilities

- [ ] #7 No coupling to PlayerPage internals
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Complete

Implemented performance warning detection as a separate, reusable module in `src/ui/pages/player/buffering_warnings.rs`.

### Features Implemented:
- ✅ Separate module with pure, stateless warning detection functions
- ✅ `is_download_too_slow()` - detects when download speed can't keep up with bitrate
- ✅ `is_buffer_critically_low()` - detects critically low buffer levels
- ✅ `is_buffering_stalled()` - detects when buffering makes no progress
- ✅ `detect_warnings()` - convenience function that checks all conditions
- ✅ Warning severity levels (Info, Warning, Critical)
- ✅ User-friendly warning messages and actionable recommendations
- ✅ BufferingOverlay component updated to use warning detection
- ✅ Warning UI added to BufferingOverlay with icons and messages
- ✅ No coupling to PlayerPage internals - fully reusable

### Files Created/Modified:
1. Created `src/ui/pages/player/buffering_warnings.rs` (394 lines)
   - `PerformanceWarning` enum with severity and message methods
   - Pure functions for warning detection
   - Comprehensive test coverage

2. Modified `src/ui/pages/player/buffering_overlay.rs`
   - Added warning detection integration
   - Added warning UI display (icon, message, recommendation)
   - Added `UpdateEstimatedBitrate` input message
   - Auto-updates warnings when state changes

3. Modified `src/ui/pages/player/mod.rs`
   - Added module declaration

4. Modified `src/styles/player.css`
   - Added `.performance-warnings` container styles
   - Added `.warning-icon` and `.warning-icon-critical` colors
   - Added `.warning-message` and `.warning-recommendation` text styles

### Warning Types:
1. **SlowDownload** - Download speed slower than required bitrate (with safety margin)
2. **CriticallyLowBuffer** - Buffer level below 15% threshold
3. **BufferingStalled** - No buffering progress for 10+ seconds
4. **NetworkUnstable** - Intermittent network issues detected

### UI Design:
- Warning icon changes based on severity (warning/error symbol)
- Icon color: yellow for warnings, red for critical
- Primary message from most severe warning
- Optional recommendation text below message
- Warnings section separated with subtle border

### Testing:
- ✅ Code compiles successfully
- ✅ All warning detection functions tested
- ✅ Message and recommendation formatting tested
- ✅ Component ready for integration in task 465.05

Warning detection is fully functional and ready to be used when BufferingOverlay is integrated into PlayerPage.
<!-- SECTION:NOTES:END -->
