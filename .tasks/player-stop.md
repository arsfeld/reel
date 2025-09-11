# Player Stop Fix Implementation Plan

## Problem Statement

When navigating away from the player page (back button), MPV playback never stops. Audio continues playing while the user has returned to the previous page, creating a broken user experience.

## Root Cause Analysis

### Primary Issue: Race Condition in Navigation Flow

**Location:** `src/platforms/gtk/ui/main_window.rs:1375-1417`

**Problem:** The back button handler spawns an async stop operation but immediately proceeds with UI navigation:

```rust
// PROBLEMATIC CODE:
glib::spawn_future_local(async move {
    player_page.stop().await;  // Runs in background
});

// Navigation happens IMMEDIATELY, not after stop completes
window.imp().content_header.set_visible(true);
// ... rest of navigation
```

**Result:** Player continues running while user sees the previous page.

### Secondary Issues

1. **MPV Stop Implementation Gaps** (`src/player/mpv_player.rs:720-747`)
   - No immediate audio muting
   - Audio buffers may continue draining
   - Missing forced audio output disable

2. **Complex Retry Logic** (`src/platforms/gtk/ui/pages/player.rs:1215-1294`)
   - Nested async operations with delays
   - Multiple fallback paths that may fail
   - Original caller doesn't wait for completion

3. **No Immediate Silence**
   - Volume is not immediately muted during stop
   - No forced audio output termination

## Implementation Plan

### Stage 1: Fix Navigation Race Condition
**Goal:** Ensure navigation waits for player stop completion
**File:** `src/platforms/gtk/ui/main_window.rs`
**Line:** ~1370 (back button handler)

**Changes:**
1. Modify back button callback to await stop completion
2. Move navigation logic inside the async block
3. Add loading indicator during stop operation
4. Ensure proper error handling

**Implementation:**
```rust
player_page.set_on_back_clicked(move || {
    if let Some(window) = window_weak.upgrade() {
        let player_page = window.imp().player_page.borrow().as_ref().unwrap().clone();
        let window_clone = window.clone();
        
        // Show loading/stopping state
        // TODO: Add stopping indicator
        
        glib::spawn_future_local(async move {
            // FIRST: Stop player and wait for completion
            if let Err(e) = player_page.stop().await {
                eprintln!("Failed to stop player: {}", e);
            }
            
            // THEN: Execute navigation on main thread
            glib::idle_add_local_once(move || {
                // All the navigation logic moved here...
                window_clone.imp().content_header.set_visible(true);
                // ... rest of restoration
            });
        });
    }
});
```

**Success Criteria:**
- [ ] Back button shows loading state during stop
- [ ] Navigation only occurs after stop completes
- [ ] Audio stops immediately when back is pressed
- [ ] No race conditions in navigation

### Stage 2: Improve MPV Stop Implementation  
**Goal:** Ensure immediate audio cessation
**File:** `src/player/mpv_player.rs`
**Line:** ~720 (stop method)

**Changes:**
1. Immediately mute volume before stop
2. Disable audio output to force silence
3. Add verification of stop completion
4. Simplify error handling

**Implementation:**
```rust
pub async fn stop(&self) -> Result<()> {
    debug!("MpvPlayer::stop() - Stopping playback with immediate audio cut");

    if let Some(ref mpv) = *self.inner.mpv.borrow() {
        // IMMEDIATE: Mute volume for instant silence
        if let Err(e) = mpv.command("set", &["volume", "0"]) {
            warn!("Failed to mute volume during stop: {:?}", e);
        }
        
        // IMMEDIATE: Disable audio output completely  
        if let Err(e) = mpv.command("set", &["ao", "null"]) {
            warn!("Failed to disable audio output: {:?}", e);
        }

        // THEN: Stop media playback
        mpv.command("stop", &[])
            .map_err(|e| anyhow::anyhow!("Failed to stop: {:?}", e))?;
            
        mpv.command("playlist-clear", &[])
            .map_err(|e| anyhow::anyhow!("Failed to clear playlist: {:?}", e))?;

        mpv.command("set", &["idle", "yes"])
            .map_err(|e| anyhow::anyhow!("Failed to set idle mode: {:?}", e))?;

        // Update internal state
        let mut state = self.inner.state.write().await;
        *state = PlayerState::Stopped;
        
        info!("MpvPlayer::stop() - Playback stopped with immediate audio termination");
    }
    Ok(())
}
```

**Success Criteria:**
- [ ] Audio stops immediately when stop() is called
- [ ] No audio buffer draining delays
- [ ] MPV enters clean stopped state
- [ ] Method completes quickly (< 100ms)

### Stage 3: Simplify PlayerPage Stop Logic
**Goal:** Remove complex retry mechanisms
**File:** `src/platforms/gtk/ui/pages/player.rs` 
**Line:** ~1207 (stop method)

**Changes:**
1. Remove complex lock retry logic
2. Simplify to: mute → stop → recreate if needed
3. Add immediate UI feedback
4. Ensure synchronous completion

**Implementation:**
```rust
pub async fn stop(&self) -> Result<()> {
    debug!("PlayerPage::stop() - Starting stop sequence");

    // Step 1: Stop ViewModel (events, state updates)
    self.view_model.stop().await;

    // Step 2: Immediate audio mute through player
    if let Ok(player) = self.player.read().await {
        // Force immediate silence
        let _ = player.set_volume(0.0).await;
        
        // Stop playback
        if let Err(e) = player.stop().await {
            error!("PlayerPage::stop() - Failed to stop player: {}", e);
        }
        
        player.clear_video_widget_state();
    }

    // Step 3: UI cleanup (always execute)
    self.cleanup_ui().await;

    // Step 4: Recreate player for clean state (optional)
    if self.should_recreate_player() {
        self.recreate_player().await;
    }

    info!("PlayerPage::stop() - Stop sequence completed");
    Ok(())
}
```

**Success Criteria:**
- [ ] Stop completes in < 200ms
- [ ] No nested async operations
- [ ] Audio stops immediately via volume=0
- [ ] Clean error handling without complex retries

### Stage 4: Add User Feedback
**Goal:** Provide visual feedback during stop operation
**Files:** UI templates and PlayerPage

**Changes:**
1. Add "Stopping..." overlay in player
2. Show loading state on back button
3. Disable navigation during stop
4. Clear feedback when complete

**Implementation Details:**
- Add stopping overlay to player template
- Show spinner during stop operation
- Disable back button until stop completes
- Toast notification if stop fails

**Success Criteria:**  
- [ ] User sees "Stopping playback..." indicator
- [ ] Back button shows loading state
- [ ] No duplicate clicks during stop
- [ ] Clear error messages if stop fails

### Stage 5: Testing & Validation
**Goal:** Ensure fix works across all scenarios

**Test Cases:**
1. **Basic Navigation**
   - [ ] Play video → press back → audio stops immediately
   - [ ] No audio continues after navigation
   - [ ] UI transitions smoothly

2. **Edge Cases**
   - [ ] Rapid back button clicks (debouncing)
   - [ ] Stop during MPV loading state
   - [ ] Stop during seek operations
   - [ ] Network stream vs local file

3. **Error Scenarios**
   - [ ] MPV backend failure during stop
   - [ ] Lock contention during GLArea operations  
   - [ ] Player recreation failure handling

4. **Performance**
   - [ ] Stop completes in < 200ms
   - [ ] No UI blocking during stop
   - [ ] Memory cleanup after stop

**Validation Methods:**
- Audio monitoring during navigation
- Log analysis for race conditions
- UI responsiveness testing
- Memory leak detection

## Implementation Notes

### Critical Requirements
- **NEVER** navigate before stop completes
- **ALWAYS** mute audio immediately when stopping
- **SIMPLIFY** complex retry logic
- **PROVIDE** user feedback during operations

### Risk Mitigation
- Test with both MPV and GStreamer backends
- Ensure fallback if stop fails completely
- Monitor for new race conditions
- Validate across different media types

### Success Metrics
1. **Functional:** Audio stops within 100ms of back button press
2. **UX:** Smooth navigation with appropriate feedback  
3. **Reliability:** No race conditions or hanging states
4. **Performance:** Stop operation completes quickly

## Dependencies

- No external dependencies required
- Uses existing MPV command interface
- Leverages current async/await patterns
- Compatible with reactive ViewModel system

## Timeline Estimate

- **Stage 1:** 2-3 hours (navigation fix)
- **Stage 2:** 1-2 hours (MPV stop improvements)  
- **Stage 3:** 2-3 hours (PlayerPage simplification)
- **Stage 4:** 1-2 hours (user feedback)
- **Stage 5:** 2-4 hours (testing & validation)

**Total:** 8-14 hours

## Rollback Plan

If issues arise:
1. Revert navigation changes (Stage 1) - most critical
2. Keep existing MPV stop implementation
3. Restore complex retry logic if needed
4. Remove user feedback additions

The fix is designed to be incremental - each stage can be validated independently.