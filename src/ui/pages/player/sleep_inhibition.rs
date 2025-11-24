use gtk4::prelude::*;
use libadwaita as adw;
use tracing::debug;

/// Manages sleep and screensaver inhibition during video playback
pub struct SleepInhibitor {
    inhibit_cookie: Option<u32>,
}

impl SleepInhibitor {
    /// Create a new sleep inhibitor
    pub fn new() -> Self {
        Self {
            inhibit_cookie: None,
        }
    }

    /// Set up sleep inhibition when playback starts
    pub fn setup(&mut self, window: &adw::ApplicationWindow) {
        // Only set up inhibition if not already active
        if self.inhibit_cookie.is_some() {
            return;
        }

        if let Some(app) = window.application() {
            if let Ok(gtk_app) = app.downcast::<adw::Application>() {
                use gtk4::ApplicationInhibitFlags;

                let flags = ApplicationInhibitFlags::IDLE | ApplicationInhibitFlags::SUSPEND;
                let cookie = gtk_app.inhibit(Some(window), flags, Some("Playing video"));

                self.inhibit_cookie = Some(cookie);
                debug!("Sleep inhibition enabled (cookie: {})", cookie);
            }
        }
    }

    /// Release sleep inhibition when playback stops/pauses
    pub fn release(&mut self, window: &adw::ApplicationWindow) {
        if let Some(cookie) = self.inhibit_cookie.take() {
            if let Some(app) = window.application() {
                if let Ok(gtk_app) = app.downcast::<adw::Application>() {
                    gtk_app.uninhibit(cookie);
                    debug!("Sleep inhibition disabled (cookie: {})", cookie);
                }
            }
        }
    }
}
