use objc2::rc::Retained;
use objc2_app_kit::NSColor;

/// Theme colors for the Reel application
pub struct ReelColors;

impl ReelColors {
    /// Primary background color
    pub fn background() -> Retained<NSColor> {
        unsafe { NSColor::windowBackgroundColor() }
    }

    /// Secondary background color for cards/containers
    pub fn card_background() -> Retained<NSColor> {
        unsafe { NSColor::controlBackgroundColor() }
    }

    /// Primary text color
    pub fn text() -> Retained<NSColor> {
        unsafe { NSColor::labelColor() }
    }

    /// Secondary text color
    pub fn secondary_text() -> Retained<NSColor> {
        unsafe { NSColor::secondaryLabelColor() }
    }

    /// Tertiary text color
    pub fn tertiary_text() -> Retained<NSColor> {
        unsafe { NSColor::tertiaryLabelColor() }
    }

    /// Accent color for interactive elements
    pub fn accent() -> Retained<NSColor> {
        unsafe { NSColor::controlAccentColor() }
    }

    /// Selection color
    pub fn selection() -> Retained<NSColor> {
        unsafe { NSColor::selectedContentBackgroundColor() }
    }

    /// Separator color
    pub fn separator() -> Retained<NSColor> {
        unsafe { NSColor::separatorColor() }
    }

    /// Success color (green)
    pub fn success() -> Retained<NSColor> {
        unsafe { NSColor::systemGreenColor() }
    }

    /// Warning color (yellow)
    pub fn warning() -> Retained<NSColor> {
        unsafe { NSColor::systemYellowColor() }
    }

    /// Error color (red)
    pub fn error() -> Retained<NSColor> {
        unsafe { NSColor::systemRedColor() }
    }

    /// Sidebar background
    pub fn sidebar_background() -> Retained<NSColor> {
        unsafe { NSColor::underPageBackgroundColor() }
    }

    /// Player controls background
    pub fn player_controls_background() -> Retained<NSColor> {
        unsafe { NSColor::colorWithCalibratedWhite_alpha(0.0, 0.8) }
    }

    /// Overlay background for modals
    pub fn overlay_background() -> Retained<NSColor> {
        unsafe { NSColor::colorWithCalibratedWhite_alpha(0.0, 0.5) }
    }
}
