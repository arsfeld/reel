use gtk4::gdk;

/// Player actions triggered by keyboard shortcuts.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum PlayerAction {
    TogglePause,
    SeekForward(f64),
    SeekBackward(f64),
    VolumeUp(f64),
    VolumeDown(f64),
    ToggleMute,
    ToggleFullscreen,
    ExitFullscreen,
    SpeedUp,
    SpeedDown,
    SpeedReset,
}

/// Skip interval configuration for keyboard shortcuts.
pub struct SkipIntervals {
    pub short_secs: f64,
    pub long_secs: f64,
}

impl Default for SkipIntervals {
    fn default() -> Self {
        Self {
            short_secs: 10.0,
            long_secs: 60.0,
        }
    }
}

/// Map a key press to a player action. Returns None if not a shortcut.
///
/// Pure function - fully testable without GTK runtime.
#[allow(dead_code)]
pub fn map_key_to_action(
    key: gdk::Key,
    modifiers: gdk::ModifierType,
    is_text_input_focused: bool,
) -> Option<PlayerAction> {
    map_key_to_action_with_intervals(
        key,
        modifiers,
        is_text_input_focused,
        &SkipIntervals::default(),
    )
}

/// Map a key press to a player action using configurable skip intervals.
#[allow(dead_code)]
pub fn map_key_to_action_with_intervals(
    key: gdk::Key,
    modifiers: gdk::ModifierType,
    is_text_input_focused: bool,
    intervals: &SkipIntervals,
) -> Option<PlayerAction> {
    if is_text_input_focused {
        return None;
    }

    let no_mods = modifiers.is_empty();
    let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK)
        && !modifiers.contains(gdk::ModifierType::CONTROL_MASK);

    match key {
        gdk::Key::space if no_mods => Some(PlayerAction::TogglePause),
        gdk::Key::Right if no_mods => Some(PlayerAction::SeekForward(intervals.short_secs)),
        gdk::Key::Left if no_mods => Some(PlayerAction::SeekBackward(intervals.short_secs)),
        gdk::Key::Right if shift => Some(PlayerAction::SeekForward(intervals.long_secs)),
        gdk::Key::Left if shift => Some(PlayerAction::SeekBackward(intervals.long_secs)),
        gdk::Key::Up if no_mods => Some(PlayerAction::VolumeUp(5.0)),
        gdk::Key::Down if no_mods => Some(PlayerAction::VolumeDown(5.0)),
        gdk::Key::m if no_mods => Some(PlayerAction::ToggleMute),
        gdk::Key::F11 => Some(PlayerAction::ToggleFullscreen),
        gdk::Key::Escape => Some(PlayerAction::ExitFullscreen),
        gdk::Key::bracketright if no_mods => Some(PlayerAction::SpeedUp),
        gdk::Key::bracketleft if no_mods => Some(PlayerAction::SpeedDown),
        gdk::Key::BackSpace if no_mods => Some(PlayerAction::SpeedReset),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: gdk::ModifierType = gdk::ModifierType::empty();
    const SHIFT: gdk::ModifierType = gdk::ModifierType::SHIFT_MASK;

    // --- Basic shortcuts ---

    #[test]
    fn space_toggles_pause() {
        assert_eq!(
            map_key_to_action(gdk::Key::space, NONE, false),
            Some(PlayerAction::TogglePause)
        );
    }

    #[test]
    fn right_arrow_seeks_forward_10s() {
        assert_eq!(
            map_key_to_action(gdk::Key::Right, NONE, false),
            Some(PlayerAction::SeekForward(10.0))
        );
    }

    #[test]
    fn left_arrow_seeks_backward_10s() {
        assert_eq!(
            map_key_to_action(gdk::Key::Left, NONE, false),
            Some(PlayerAction::SeekBackward(10.0))
        );
    }

    #[test]
    fn up_arrow_volume_up() {
        assert_eq!(
            map_key_to_action(gdk::Key::Up, NONE, false),
            Some(PlayerAction::VolumeUp(5.0))
        );
    }

    #[test]
    fn down_arrow_volume_down() {
        assert_eq!(
            map_key_to_action(gdk::Key::Down, NONE, false),
            Some(PlayerAction::VolumeDown(5.0))
        );
    }

    #[test]
    fn m_toggles_mute() {
        assert_eq!(
            map_key_to_action(gdk::Key::m, NONE, false),
            Some(PlayerAction::ToggleMute)
        );
    }

    #[test]
    fn f11_toggles_fullscreen() {
        assert_eq!(
            map_key_to_action(gdk::Key::F11, NONE, false),
            Some(PlayerAction::ToggleFullscreen)
        );
    }

    #[test]
    fn escape_exits_fullscreen() {
        assert_eq!(
            map_key_to_action(gdk::Key::Escape, NONE, false),
            Some(PlayerAction::ExitFullscreen)
        );
    }

    #[test]
    fn bracket_right_speeds_up() {
        assert_eq!(
            map_key_to_action(gdk::Key::bracketright, NONE, false),
            Some(PlayerAction::SpeedUp)
        );
    }

    #[test]
    fn bracket_left_slows_down() {
        assert_eq!(
            map_key_to_action(gdk::Key::bracketleft, NONE, false),
            Some(PlayerAction::SpeedDown)
        );
    }

    #[test]
    fn backspace_resets_speed() {
        assert_eq!(
            map_key_to_action(gdk::Key::BackSpace, NONE, false),
            Some(PlayerAction::SpeedReset)
        );
    }

    // --- Shift modifiers ---

    #[test]
    fn shift_right_seeks_forward_60s() {
        assert_eq!(
            map_key_to_action(gdk::Key::Right, SHIFT, false),
            Some(PlayerAction::SeekForward(60.0))
        );
    }

    #[test]
    fn shift_left_seeks_backward_60s() {
        assert_eq!(
            map_key_to_action(gdk::Key::Left, SHIFT, false),
            Some(PlayerAction::SeekBackward(60.0))
        );
    }

    // --- F11/Escape work with any modifiers ---

    #[test]
    fn f11_with_shift_still_works() {
        assert_eq!(
            map_key_to_action(gdk::Key::F11, SHIFT, false),
            Some(PlayerAction::ToggleFullscreen)
        );
    }

    #[test]
    fn escape_with_shift_still_works() {
        assert_eq!(
            map_key_to_action(gdk::Key::Escape, SHIFT, false),
            Some(PlayerAction::ExitFullscreen)
        );
    }

    // --- Text input focus blocks all shortcuts ---

    #[test]
    fn space_blocked_in_text_input() {
        assert_eq!(map_key_to_action(gdk::Key::space, NONE, true), None);
    }

    #[test]
    fn arrows_blocked_in_text_input() {
        assert_eq!(map_key_to_action(gdk::Key::Right, NONE, true), None);
        assert_eq!(map_key_to_action(gdk::Key::Left, NONE, true), None);
        assert_eq!(map_key_to_action(gdk::Key::Up, NONE, true), None);
        assert_eq!(map_key_to_action(gdk::Key::Down, NONE, true), None);
    }

    #[test]
    fn m_blocked_in_text_input() {
        assert_eq!(map_key_to_action(gdk::Key::m, NONE, true), None);
    }

    #[test]
    fn f11_blocked_in_text_input() {
        assert_eq!(map_key_to_action(gdk::Key::F11, NONE, true), None);
    }

    // --- Unknown keys return None ---

    #[test]
    fn unknown_key_returns_none() {
        assert_eq!(map_key_to_action(gdk::Key::a, NONE, false), None);
    }

    #[test]
    fn number_key_returns_none() {
        assert_eq!(map_key_to_action(gdk::Key::_1, NONE, false), None);
    }

    #[test]
    fn enter_returns_none() {
        assert_eq!(map_key_to_action(gdk::Key::Return, NONE, false), None);
    }

    // --- Configurable intervals ---

    #[test]
    fn custom_short_skip_interval() {
        let intervals = SkipIntervals {
            short_secs: 5.0,
            long_secs: 30.0,
        };
        assert_eq!(
            map_key_to_action_with_intervals(gdk::Key::Right, NONE, false, &intervals),
            Some(PlayerAction::SeekForward(5.0))
        );
        assert_eq!(
            map_key_to_action_with_intervals(gdk::Key::Left, NONE, false, &intervals),
            Some(PlayerAction::SeekBackward(5.0))
        );
    }

    #[test]
    fn custom_long_skip_interval() {
        let intervals = SkipIntervals {
            short_secs: 5.0,
            long_secs: 30.0,
        };
        assert_eq!(
            map_key_to_action_with_intervals(gdk::Key::Right, SHIFT, false, &intervals),
            Some(PlayerAction::SeekForward(30.0))
        );
        assert_eq!(
            map_key_to_action_with_intervals(gdk::Key::Left, SHIFT, false, &intervals),
            Some(PlayerAction::SeekBackward(30.0))
        );
    }
}
