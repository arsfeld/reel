/// State for the controls overlay visibility logic.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct OverlayState {
    pub visible: bool,
    pub is_fullscreen: bool,
    pub is_paused: bool,
    pub popover_open: bool,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            visible: true,
            is_fullscreen: false,
            is_paused: false,
            popover_open: false,
        }
    }
}

/// Actions that can affect overlay visibility.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum OverlayAction {
    MouseMoved,
    TimeoutFired,
    PauseChanged(bool),
    FullscreenChanged(bool),
    PopoverOpened,
    PopoverClosed,
}

/// Compute whether the overlay should be visible after an action.
///
/// Rules:
/// - Windowed mode: always visible
/// - Fullscreen + paused: always visible
/// - Fullscreen + popover open: visible
/// - Fullscreen + playing + mouse moved: visible (caller should restart timer)
/// - Fullscreen + playing + timeout fired: hidden
///
/// Pure function - fully testable.
#[allow(dead_code)]
pub fn compute_overlay_visibility(state: &OverlayState, action: &OverlayAction) -> bool {
    // Apply the action to determine new sub-states
    let is_fullscreen = match action {
        OverlayAction::FullscreenChanged(fs) => *fs,
        _ => state.is_fullscreen,
    };

    let is_paused = match action {
        OverlayAction::PauseChanged(p) => *p,
        _ => state.is_paused,
    };

    let popover_open = match action {
        OverlayAction::PopoverOpened => true,
        OverlayAction::PopoverClosed => false,
        _ => state.popover_open,
    };

    // Windowed mode: always visible
    if !is_fullscreen {
        return true;
    }

    // Fullscreen + paused: always visible
    if is_paused {
        return true;
    }

    // Fullscreen + popover open: visible
    if popover_open {
        return true;
    }

    // Fullscreen + playing: depends on the action
    match action {
        OverlayAction::MouseMoved => true,          // Show, restart timer
        OverlayAction::TimeoutFired => false,       // Hide
        OverlayAction::PauseChanged(false) => true, // Just resumed, show briefly
        OverlayAction::FullscreenChanged(true) => true, // Just entered fullscreen, show briefly
        _ => state.visible,                         // Keep current
    }
}

/// Whether the mouse cursor should be hidden.
/// Only hide in fullscreen when overlay is hidden.
#[allow(dead_code)]
pub fn should_hide_cursor(is_fullscreen: bool, overlay_visible: bool) -> bool {
    is_fullscreen && !overlay_visible
}

#[cfg(test)]
mod tests {
    use super::*;

    fn windowed_playing() -> OverlayState {
        OverlayState {
            visible: true,
            is_fullscreen: false,
            is_paused: false,
            popover_open: false,
        }
    }

    fn fullscreen_playing() -> OverlayState {
        OverlayState {
            visible: true,
            is_fullscreen: true,
            is_paused: false,
            popover_open: false,
        }
    }

    fn fullscreen_paused() -> OverlayState {
        OverlayState {
            visible: true,
            is_fullscreen: true,
            is_paused: true,
            popover_open: false,
        }
    }

    // --- Windowed mode: always visible ---

    #[test]
    fn windowed_always_visible_on_timeout() {
        let state = windowed_playing();
        assert!(compute_overlay_visibility(
            &state,
            &OverlayAction::TimeoutFired
        ));
    }

    #[test]
    fn windowed_always_visible_on_mouse() {
        let state = windowed_playing();
        assert!(compute_overlay_visibility(
            &state,
            &OverlayAction::MouseMoved
        ));
    }

    #[test]
    fn windowed_always_visible_on_pause() {
        let state = windowed_playing();
        assert!(compute_overlay_visibility(
            &state,
            &OverlayAction::PauseChanged(true)
        ));
    }

    // --- Fullscreen + paused: always visible ---

    #[test]
    fn fullscreen_paused_visible_on_timeout() {
        let state = fullscreen_paused();
        assert!(compute_overlay_visibility(
            &state,
            &OverlayAction::TimeoutFired
        ));
    }

    #[test]
    fn fullscreen_paused_visible_on_mouse() {
        let state = fullscreen_paused();
        assert!(compute_overlay_visibility(
            &state,
            &OverlayAction::MouseMoved
        ));
    }

    // --- Fullscreen + playing: auto-hide ---

    #[test]
    fn fullscreen_playing_visible_on_mouse() {
        let state = fullscreen_playing();
        assert!(compute_overlay_visibility(
            &state,
            &OverlayAction::MouseMoved
        ));
    }

    #[test]
    fn fullscreen_playing_hidden_on_timeout() {
        let state = fullscreen_playing();
        assert!(!compute_overlay_visibility(
            &state,
            &OverlayAction::TimeoutFired
        ));
    }

    #[test]
    fn fullscreen_playing_visible_when_popover_open() {
        let mut state = fullscreen_playing();
        state.popover_open = true;
        assert!(compute_overlay_visibility(
            &state,
            &OverlayAction::TimeoutFired
        ));
    }

    #[test]
    fn fullscreen_playing_hidden_after_popover_closed_then_timeout() {
        let state = OverlayState {
            visible: true,
            is_fullscreen: true,
            is_paused: false,
            popover_open: true,
        };
        // Close popover
        let visible = compute_overlay_visibility(&state, &OverlayAction::PopoverClosed);
        assert!(visible); // Still visible (keeps current)

        // Then timeout fires
        let state2 = OverlayState {
            popover_open: false,
            ..state
        };
        assert!(!compute_overlay_visibility(
            &state2,
            &OverlayAction::TimeoutFired
        ));
    }

    // --- State transitions ---

    #[test]
    fn entering_fullscreen_shows_overlay() {
        let state = windowed_playing();
        assert!(compute_overlay_visibility(
            &state,
            &OverlayAction::FullscreenChanged(true)
        ));
    }

    #[test]
    fn exiting_fullscreen_shows_overlay() {
        let state = fullscreen_playing();
        assert!(compute_overlay_visibility(
            &state,
            &OverlayAction::FullscreenChanged(false)
        ));
    }

    #[test]
    fn pausing_shows_overlay() {
        let state = fullscreen_playing();
        assert!(compute_overlay_visibility(
            &state,
            &OverlayAction::PauseChanged(true)
        ));
    }

    #[test]
    fn resuming_shows_overlay_briefly() {
        let state = fullscreen_paused();
        assert!(compute_overlay_visibility(
            &state,
            &OverlayAction::PauseChanged(false)
        ));
    }

    #[test]
    fn opening_popover_prevents_hide() {
        let state = fullscreen_playing();
        assert!(compute_overlay_visibility(
            &state,
            &OverlayAction::PopoverOpened
        ));
    }

    // --- Cursor hiding ---

    #[test]
    fn cursor_hidden_fullscreen_overlay_hidden() {
        assert!(should_hide_cursor(true, false));
    }

    #[test]
    fn cursor_visible_fullscreen_overlay_visible() {
        assert!(!should_hide_cursor(true, true));
    }

    #[test]
    fn cursor_visible_windowed_overlay_hidden() {
        assert!(!should_hide_cursor(false, false));
    }

    #[test]
    fn cursor_visible_windowed_overlay_visible() {
        assert!(!should_hide_cursor(false, true));
    }

    // --- Default state ---

    #[test]
    fn default_state_is_visible_windowed() {
        let state = OverlayState::default();
        assert!(state.visible);
        assert!(!state.is_fullscreen);
        assert!(!state.is_paused);
    }
}
