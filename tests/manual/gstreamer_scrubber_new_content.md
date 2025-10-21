# Manual Test: GStreamer Scrubber Reset on New Content

## Issue
Task-434: The GStreamer player scrubber UI shows stale position information when switching between videos. When you finish watching one video and open a different one, the scrubber continues to display the previous video's position instead of resetting to the new video's state.

## Root Cause
When loading new media via `LoadMedia` or `LoadMediaWithContext` input handlers, the scrubber UI components (seek_bar, position_label, duration_label) were not being reset. They retained their values from the previous video until the first position update timer fired (every 1 second), causing a brief but noticeable display of stale information.

## Solution
Reset the scrubber UI components immediately when loading new media:
- Set seek_bar value to 0.0
- Set position_label to "0:00"
- Set duration_label to "--:--"

This ensures the UI immediately reflects the initial state of the new video, preventing any stale values from the previous video from being displayed.

## Test Procedure

### Prerequisites
- Reel application with GStreamer player backend (macOS or forced via settings)
- Access to a media library with multiple videos

### Test Case 1: Sequential Video Playback
1. Play a video and let it play for at least 30 seconds
2. Note the current position shown in the scrubber (e.g., "0:45 / 1:30:00")
3. Navigate to and open a different video
4. **Expected**: Scrubber immediately shows "0:00 / --:--" while loading
5. **Expected**: Once video loads, scrubber shows correct duration (e.g., "0:00 / 0:45:30")
6. **Expected**: No flash or display of the previous video's position

### Test Case 2: Episode Navigation in Playlist
1. Start playing an episode from a TV show
2. Let it play for 2-3 minutes
3. Click "Next Episode" button
4. **Expected**: Scrubber immediately resets to "0:00 / --:--"
5. **Expected**: Position from previous episode is not displayed
6. **Expected**: New episode's duration appears once loaded

### Test Case 3: Previous Episode Navigation
1. Play second episode of a series for 1-2 minutes
2. Click "Previous Episode" button
3. **Expected**: Scrubber immediately resets to "0:00 / --:--"
4. **Expected**: No remnants of second episode's position visible
5. **Expected**: First episode loads with correct initial state

### Test Case 4: Rapid Video Switching
1. Open a video and let it buffer
2. Immediately (within 1 second) navigate to a different video
3. Repeat several times
4. **Expected**: Each time, scrubber resets immediately
5. **Expected**: No stale positions from any previous video
6. **Expected**: No UI glitches or position jumps

### Test Case 5: Resume After Completion
1. Play a short video to completion
2. Scrubber should show full duration (e.g., "2:30 / 2:30")
3. Navigate to and play a different, longer video
4. **Expected**: Scrubber resets to "0:00 / --:--" immediately
5. **Expected**: No display of "2:30" from the completed video
6. **Expected**: New video plays from the beginning with correct scrubber state

## Success Criteria
- [ ] Scrubber always shows "0:00 / --:--" when new media starts loading
- [ ] No stale position information from previous video is ever displayed
- [ ] Scrubber updates smoothly to show actual position once playback starts
- [ ] Fix works consistently across all navigation methods (direct selection, next/previous buttons)
- [ ] No UI glitches, flashes, or position jumps when switching content

## Related Issues
- task-430: GStreamer scrubber position sticking after seeking (different issue, also fixed)
- task-296, task-310, task-320, task-327: Earlier scrubber-related issues

## Implementation Details
**Files Modified**: `src/ui/pages/player.rs`

**Changes**:
- Added scrubber UI reset in `PlayerInput::LoadMedia` handler (lines 1870-1873)
- Added scrubber UI reset in `PlayerInput::LoadMediaWithContext` handler (lines 2135-2138)

**Code Pattern**:
```rust
// Reset scrubber UI to prevent showing previous video's position
self.seek_bar.set_value(0.0);
self.position_label.set_text("0:00");
self.duration_label.set_text("--:--");
```

This is the proper root cause fix that addresses the issue at the UI state management level, ensuring clean state transitions when loading new content.
