use gtk4::prelude::*;

/// Manages screensaver/idle inhibition during video playback.
///
/// Uses GTK's Application::inhibit() which talks to the appropriate
/// platform-specific mechanism (freedesktop ScreenSaver on Linux,
/// xdg-desktop-portal, etc.).
#[allow(dead_code)]
pub struct ScreensaverInhibitor {
    inhibit_cookie: Option<u32>,
}

#[allow(dead_code)]
impl ScreensaverInhibitor {
    pub fn new() -> Self {
        Self {
            inhibit_cookie: None,
        }
    }

    /// Inhibit the screensaver. No-op if already inhibited.
    pub fn inhibit(&mut self, window: &impl IsA<gtk4::Window>) {
        if self.inhibit_cookie.is_some() {
            return;
        }

        let app = window.application();
        let Some(app) = app else {
            return;
        };

        let cookie = app.inhibit(
            Some(window),
            gtk4::ApplicationInhibitFlags::IDLE,
            Some("Video playback in progress"),
        );
        if cookie != 0 {
            self.inhibit_cookie = Some(cookie);
        }
    }

    /// Release screensaver inhibition. No-op if not inhibited.
    pub fn uninhibit(&mut self, window: &impl IsA<gtk4::Window>) {
        let Some(cookie) = self.inhibit_cookie.take() else {
            return;
        };

        let Some(app) = window.application() else {
            return;
        };

        app.uninhibit(cookie);
    }

    pub fn is_inhibited(&self) -> bool {
        self.inhibit_cookie.is_some()
    }
}
