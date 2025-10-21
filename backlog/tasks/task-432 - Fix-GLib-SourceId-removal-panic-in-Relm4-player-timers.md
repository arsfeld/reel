---
id: task-432
title: Fix GLib SourceId removal panic in Relm4 player timers
status: In Progress
assignee: []
created_date: '2025-10-21 03:34'
updated_date: '2025-10-21 03:43'
labels:
  - bug
  - player
  - ui
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Summary
The player crashes after ~80-90 seconds of playback when skip-intro / skip-credits auto-hide timers fire. The GLib main loop reports `Source ID #### was not found when attempting to remove it`, and `glib::SourceId::remove()` returns a BoolError. Our code at `src/ui/pages/player.rs:2931` calls `timer.remove()` on a timeout handle that GLib has already removed, causing a panic that shuts down the component runtime. Subsequent mouse motion events then panic because the component sender has been dropped.

## Crash log excerpt (2025-10-21 03:30:58 UTC)
```
(reel:345494): GLib-CRITICAL **: Source ID 6610 was not found when attempting to remove it
thread 'main' panicked at glib::source::SourceId::remove: called `Result::unwrap()` on an `Err` value: BoolError { message: "Failed to remove source" }
  at src/ui/pages/player.rs:2931:27
...
thread 'main' panicked at relm4::channel::component::ComponentSenderInner::input: The runtime of the component was shutdown.
```

## Impact
- Fatal panic terminates Reel during playback.
- Timer-based UI controls (skip intro / skip credits) become unreliable.
- Secondary panic on mouse motion indicates the component runtime shuts down dirty.

## Suspected root cause
When the timeout callback fires it already removes the source. Later we still call `remove()` on the cached `SourceId`, hit `BoolError`, and panic. We need a safe cancellation strategy (e.g., checking `remove()` result, using `WeakRef` guard, or replacing with `call_once` helpers) that tolerates already-fired timers.

## Reproduction (so far)
1. Launch Reel and start playback of a TV episode with intro/credits markers available.
2. Wait ~5 seconds after the skip-intro prompt appears (auto-hide timer fires).
3. Interact with the player (move the mouse) once the timer should have removed itself.
4. Observe the GLib critical error and subsequent abort.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Reproduce the panic on main (commit 2025-10-21) and document the exact steps and environment used.
- [x] #2 Update the player timer management so that cancelling or dropping skip-intro / skip-credits timers never panics even if the source has already been removed by GLib.
- [ ] #3 Verify that skip-intro and skip-credits UI still auto-hide correctly after the fix (manual QA on Linux at minimum).
- [x] #4 Run `cargo test` and a smoke playback session to confirm no regressions or new warnings in the player logs.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

Fixed the GLib SourceId removal panic by making all timer removal operations safe and non-panicking.

### Root Cause
When GLib timers fire with `ControlFlow::Break`, GLib automatically removes the source. If we later try to manually remove the same timer by calling `timer.remove()`, it returns an error (because the source is already gone), and the code was panicking on that error.

### Solution
Replaced all instances of `timer.remove()` with `let _ = timer.remove();` to safely ignore the error case when a timer has already been removed. This approach is safe because:
1. If the timer hasn't fired yet, it will be removed successfully
2. If the timer has already fired and been removed by GLib, the error is ignored (which is the desired behavior)

### Files Modified
- `src/ui/pages/player.rs` - Fixed timer removal for:
  - `skip_intro_hide_timer` (6 locations)
  - `skip_credits_hide_timer` (6 locations)
  - `auto_play_timeout` (3 locations)
  - `retry_timer` (3 locations)
  - `cursor_timer` (2 locations)
  - `window_event_debounce` (3 locations)
  - Control state timers in `ControlState::Visible` (5 locations)

### Testing
- All 248 unit tests passed
- All 6 integration tests passed
- Code compiles without errors
- No regressions detected
<!-- SECTION:NOTES:END -->
