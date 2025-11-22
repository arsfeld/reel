use crate::models::ChapterMarker;
use relm4::AsyncComponentSender;
use relm4::gtk::glib::{self, SourceId};
use std::time::Duration;
use tracing::debug;

use super::PlayerInput;

/// Manages skip intro and skip credits functionality including visibility,
/// auto-skip behavior, and timer management.
pub struct SkipMarkerManager {
    // Marker data
    intro_marker: Option<ChapterMarker>,
    credits_marker: Option<ChapterMarker>,

    // Visibility state
    skip_intro_visible: bool,
    skip_credits_visible: bool,

    // Auto-hide timers
    skip_intro_hide_timer: Option<SourceId>,
    skip_credits_hide_timer: Option<SourceId>,

    // Config values
    config_skip_intro_enabled: bool,
    config_skip_credits_enabled: bool,
    config_auto_skip_intro: bool,
    config_auto_skip_credits: bool,
    config_minimum_marker_duration_seconds: u64,
}

impl SkipMarkerManager {
    pub fn new(
        skip_intro_enabled: bool,
        skip_credits_enabled: bool,
        auto_skip_intro: bool,
        auto_skip_credits: bool,
        minimum_marker_duration_seconds: u64,
    ) -> Self {
        Self {
            intro_marker: None,
            credits_marker: None,
            skip_intro_visible: false,
            skip_credits_visible: false,
            skip_intro_hide_timer: None,
            skip_credits_hide_timer: None,
            config_skip_intro_enabled: skip_intro_enabled,
            config_skip_credits_enabled: skip_credits_enabled,
            config_auto_skip_intro: auto_skip_intro,
            config_auto_skip_credits: auto_skip_credits,
            config_minimum_marker_duration_seconds: minimum_marker_duration_seconds,
        }
    }

    /// Returns whether the skip intro button should be visible
    pub fn is_skip_intro_visible(&self) -> bool {
        self.skip_intro_visible
    }

    /// Returns whether the skip credits button should be visible
    pub fn is_skip_credits_visible(&self) -> bool {
        self.skip_credits_visible
    }

    /// Load new markers and trigger visibility update
    pub fn load_markers(&mut self, intro: Option<ChapterMarker>, credits: Option<ChapterMarker>) {
        self.intro_marker = intro;
        self.credits_marker = credits;
    }

    /// Clear all markers and hide buttons
    pub fn clear_markers(&mut self) {
        self.intro_marker = None;
        self.credits_marker = None;
        self.skip_intro_visible = false;
        self.skip_credits_visible = false;

        if let Some(timer) = self.skip_intro_hide_timer.take() {
            let _ = timer.remove();
        }
        if let Some(timer) = self.skip_credits_hide_timer.take() {
            let _ = timer.remove();
        }
    }

    /// Update skip button visibility based on current playback position
    pub fn update_visibility(
        &mut self,
        position: Duration,
        sender: &AsyncComponentSender<super::PlayerPage>,
    ) {
        self.update_intro_visibility(position, sender);
        self.update_credits_visibility(position, sender);
    }

    fn update_intro_visibility(
        &mut self,
        position: Duration,
        sender: &AsyncComponentSender<super::PlayerPage>,
    ) {
        if let Some(ref intro) = self.intro_marker {
            // Check if marker meets minimum duration threshold
            let marker_duration = intro.end_time.as_secs() - intro.start_time.as_secs();
            let meets_threshold = marker_duration >= self.config_minimum_marker_duration_seconds;

            // Check if we're in the intro time range
            let in_intro_range = position >= intro.start_time && position < intro.end_time;

            if in_intro_range && meets_threshold {
                // Auto-skip if configured
                if self.config_auto_skip_intro {
                    // Only skip once at the start of the intro
                    if !self.skip_intro_visible
                        && position < intro.start_time + Duration::from_secs(1)
                    {
                        debug!("Auto-skipping intro");
                        sender.input(PlayerInput::Seek(intro.end_time));
                    }
                } else if self.config_skip_intro_enabled {
                    // Show button if enabled
                    let was_visible = self.skip_intro_visible;
                    if !was_visible {
                        self.skip_intro_visible = true;

                        // Start auto-hide timer when button becomes visible
                        // Cancel any existing timer
                        if let Some(timer) = self.skip_intro_hide_timer.take() {
                            let _ = timer.remove();
                        }

                        // Start 5 second auto-hide timer
                        let sender_clone = sender.clone();
                        let timer = glib::timeout_add_seconds_local(5, move || {
                            sender_clone.input(PlayerInput::HideSkipIntro);
                            glib::ControlFlow::Break
                        });
                        self.skip_intro_hide_timer = Some(timer);
                    }
                }
            } else {
                self.skip_intro_visible = false;
            }
        } else {
            self.skip_intro_visible = false;
        }
    }

    fn update_credits_visibility(
        &mut self,
        position: Duration,
        sender: &AsyncComponentSender<super::PlayerPage>,
    ) {
        if let Some(ref credits) = self.credits_marker {
            // Check if marker meets minimum duration threshold
            let marker_duration = credits.end_time.as_secs() - credits.start_time.as_secs();
            let meets_threshold = marker_duration >= self.config_minimum_marker_duration_seconds;

            // Check if we're in the credits time range
            let in_credits_range = position >= credits.start_time && position < credits.end_time;

            if in_credits_range && meets_threshold {
                // Auto-skip if configured
                if self.config_auto_skip_credits {
                    // Only skip once at the start of the credits
                    if !self.skip_credits_visible
                        && position < credits.start_time + Duration::from_secs(1)
                    {
                        debug!("Auto-skipping credits");
                        sender.input(PlayerInput::Seek(credits.end_time));
                    }
                } else if self.config_skip_credits_enabled {
                    // Show button if enabled
                    let was_visible = self.skip_credits_visible;
                    if !was_visible {
                        self.skip_credits_visible = true;

                        // Start auto-hide timer when button becomes visible
                        // Cancel any existing timer
                        if let Some(timer) = self.skip_credits_hide_timer.take() {
                            let _ = timer.remove();
                        }

                        // Start 5 second auto-hide timer
                        let sender_clone = sender.clone();
                        let timer = glib::timeout_add_seconds_local(5, move || {
                            sender_clone.input(PlayerInput::HideSkipCredits);
                            glib::ControlFlow::Break
                        });
                        self.skip_credits_hide_timer = Some(timer);
                    }
                }
            } else {
                self.skip_credits_visible = false;
            }
        } else {
            self.skip_credits_visible = false;
        }
    }

    /// Handle the HideSkipIntro message
    pub fn hide_skip_intro(&mut self) {
        self.skip_intro_visible = false;
        // Clear the timer without calling remove() - it already fired and was removed by GLib
        self.skip_intro_hide_timer.take();
    }

    /// Handle the HideSkipCredits message
    pub fn hide_skip_credits(&mut self) {
        self.skip_credits_visible = false;
        // Clear the timer without calling remove() - it already fired and was removed by GLib
        self.skip_credits_hide_timer.take();
    }

    /// Handle manual skip intro button click
    pub fn skip_intro(&mut self, sender: &AsyncComponentSender<super::PlayerPage>) {
        if let Some(ref intro) = self.intro_marker {
            // Seek to the end of the intro marker
            sender.input(PlayerInput::Seek(intro.end_time));
            // Hide the button immediately
            self.skip_intro_visible = false;
            if let Some(timer) = self.skip_intro_hide_timer.take() {
                let _ = timer.remove();
            }
        }
    }

    /// Handle manual skip credits button click
    pub fn skip_credits(&mut self, sender: &AsyncComponentSender<super::PlayerPage>) {
        if let Some(ref credits) = self.credits_marker {
            // Seek to the end of the credits marker
            sender.input(PlayerInput::Seek(credits.end_time));
            // Hide the button immediately
            self.skip_credits_visible = false;
            if let Some(timer) = self.skip_credits_hide_timer.take() {
                let _ = timer.remove();
            }
        }
    }

    /// Update config values
    pub fn update_config(
        &mut self,
        skip_intro_enabled: bool,
        skip_credits_enabled: bool,
        auto_skip_intro: bool,
        auto_skip_credits: bool,
        minimum_marker_duration_seconds: u64,
    ) {
        self.config_skip_intro_enabled = skip_intro_enabled;
        self.config_skip_credits_enabled = skip_credits_enabled;
        self.config_auto_skip_intro = auto_skip_intro;
        self.config_auto_skip_credits = auto_skip_credits;
        self.config_minimum_marker_duration_seconds = minimum_marker_duration_seconds;
    }
}

impl Drop for SkipMarkerManager {
    fn drop(&mut self) {
        // Clean up timers
        if let Some(timer) = self.skip_intro_hide_timer.take() {
            let _ = timer.remove();
        }
        if let Some(timer) = self.skip_credits_hide_timer.take() {
            let _ = timer.remove();
        }
    }
}
