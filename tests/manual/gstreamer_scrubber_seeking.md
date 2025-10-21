# GStreamer Scrubber Position After Seeking - Manual Test

## Test: task-430 - Fix GStreamer scrubber position after seeking

### Purpose
Verify that the video scrubber position updates correctly after seeking in the GStreamer backend and continues tracking playback without getting stuck.

### Prerequisites
- Reel built with GStreamer support
- Video file or media server (Plex/Jellyfin) with playable content
- GStreamer backend enabled (default on macOS, or set via preferences on Linux)

### Test Procedure

#### Test 1: Basic Scrubbing
1. Launch Reel and start playing any video with GStreamer backend
2. Wait for playback to start (verify video is playing)
3. Click on the scrubber bar to jump to a different position (e.g., 50% through)
4. **Expected**: Scrubber immediately moves to the clicked position
5. **Expected**: Scrubber continues updating as playback progresses from the new position
6. **Expected**: Scrubber does NOT jump back to the old position

#### Test 2: Rapid Scrubbing
1. While video is playing, rapidly click different positions on the scrubber
2. Perform 5-10 rapid seeks in succession
3. **Expected**: Scrubber follows each seek immediately
4. **Expected**: After the last seek, scrubber continues tracking from the final position
5. **Expected**: No jumping or stuttering of the scrubber position indicator

#### Test 3: Drag Scrubbing
1. While video is playing, click and drag the scrubber handle
2. Drag slowly across the scrubber bar
3. Release the scrubber at a new position
4. **Expected**: Scrubber position updates smoothly during drag
5. **Expected**: After release, scrubber continues tracking from release position
6. **Expected**: Position label and time display match the scrubber position

#### Test 4: Keyboard Seeking
1. While video is playing, press the Right arrow key (seek forward 5s)
2. Wait 2 seconds and observe scrubber position
3. Press Left arrow key (seek backward 5s)
4. Wait 2 seconds and observe scrubber position
5. **Expected**: Scrubber jumps immediately with each keypress
6. **Expected**: Scrubber continues tracking correctly after each seek
7. **Expected**: No jumping back to old positions

#### Test 5: Pause and Seek
1. Pause the video playback
2. Click on different positions in the scrubber bar
3. Resume playback
4. **Expected**: Scrubber stays at the sought position while paused
5. **Expected**: Scrubber resumes tracking from the correct position when playback resumes

### MPV Regression Test

Since MPV already works correctly, verify no regressions:

1. Switch to MPV backend (on Linux systems)
2. Repeat Test 1, Test 2, and Test 4 above
3. **Expected**: All tests pass with MPV as they did before
4. **Expected**: No new issues or changes in MPV seeking behavior

### Pass Criteria

All tests must pass with:
- ✓ Immediate scrubber position updates after seeking
- ✓ Continuous position tracking after seeks
- ✓ No jumping back to old positions
- ✓ Smooth user experience during seeking operations
- ✓ MPV behavior unchanged (no regressions)

### Notes

The fix implements a 200ms window where the seek target position is cached and returned instead of querying GStreamer. This gives the pipeline time to stabilize after a FLUSH seek operation.

- GStreamer may return stale position values immediately after seeking
- The cache prevents the UI from showing these stale values
- After 200ms, the player switches back to querying actual position
- This matches the approach used by the MPV player implementation
