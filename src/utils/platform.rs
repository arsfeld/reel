use gtk4::prelude::*;

/// Platform detection utilities for runtime styling
pub struct Platform;

impl Platform {
    /// Returns true if running on macOS
    pub fn is_macos() -> bool {
        cfg!(target_os = "macos")
    }

    /// Returns the current platform as a string
    pub fn current() -> &'static str {
        if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "linux"
        }
    }

    /// Applies platform-specific CSS classes to a widget
    /// Note: With WhiteSur theme on macOS, we don't need custom CSS classes
    pub fn apply_platform_classes(widget: &impl IsA<gtk4::Widget>) {
        // No longer applying platform-specific CSS classes
        // WhiteSur theme handles macOS styling
        _ = widget; // Suppress unused variable warning
    }

    /// Detects system dark mode preference
    pub fn prefers_dark_mode() -> bool {
        if let Some(settings) = gtk4::Settings::default() {
            settings.is_gtk_application_prefer_dark_theme()
        } else {
            false
        }
    }

    /// Returns the appropriate system font family for the platform
    pub fn system_font_family() -> &'static str {
        if cfg!(target_os = "macos") {
            "-apple-system, BlinkMacSystemFont, 'SF Pro Display', 'SF Pro Text'"
        } else if cfg!(target_os = "windows") {
            "'Segoe UI', 'Segoe UI Variable Display'"
        } else {
            "'Cantarell', 'Ubuntu', 'DejaVu Sans'"
        }
    }
}
