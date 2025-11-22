use crate::models::PlaylistContext;
use relm4::AsyncComponentSender;
use relm4::gtk::glib::{self, SourceId};
use std::time::Duration;
use tracing::{debug, info};

use super::{PlayerInput, PlayerOutput};

/// Manages auto-play functionality including countdown timers and automatic
/// navigation to the next episode or back to the library.
pub struct AutoPlayManager {
    /// Whether auto-play has been triggered for the current media
    auto_play_triggered: bool,
    /// Active timeout for auto-play countdown
    auto_play_timeout: Option<SourceId>,
}

impl AutoPlayManager {
    pub fn new() -> Self {
        Self {
            auto_play_triggered: false,
            auto_play_timeout: None,
        }
    }

    /// Check if auto-play should trigger based on playback position
    /// Auto-play triggers when playback reaches 95% completion
    pub fn check_auto_play(
        &mut self,
        position: Duration,
        duration: Duration,
        context: &Option<PlaylistContext>,
        sender: &AsyncComponentSender<super::PlayerPage>,
    ) {
        let progress = position.as_secs_f64() / duration.as_secs_f64();
        let should_auto_play = progress > 0.95;

        if should_auto_play && !self.auto_play_triggered {
            self.auto_play_triggered = true;

            // Check if we have a playlist context with auto-play enabled
            if let Some(context) = context {
                if context.is_auto_play_enabled() {
                    if context.has_next() {
                        info!("Auto-play triggered, loading next episode");

                        // Load next item after a short delay to let current one finish
                        let sender_clone = sender.clone();
                        let timeout_id = glib::timeout_add_seconds_local(3, move || {
                            // Clear the timeout reference before it's auto-removed by GLib
                            sender_clone.input(PlayerInput::ClearAutoPlayTimeout);
                            sender_clone.input(PlayerInput::Next);
                            glib::ControlFlow::Break
                        });

                        // Store timeout ID in case we need to cancel (e.g., user manually navigates)
                        self.auto_play_timeout = Some(timeout_id);
                    } else {
                        info!("Episode ending without next episode, will navigate back");

                        // No next episode available - navigate back after a delay
                        // This delay allows watch status to be saved and synced before navigation
                        let sender_clone = sender.clone();
                        let timeout_id = glib::timeout_add_seconds_local(5, move || {
                            // Clear the timeout reference before it's auto-removed by GLib
                            sender_clone.input(PlayerInput::ClearAutoPlayTimeout);
                            sender_clone.input(PlayerInput::NavigateBack);
                            glib::ControlFlow::Break
                        });

                        // Store timeout ID in case we need to cancel (e.g., user manually navigates)
                        self.auto_play_timeout = Some(timeout_id);

                        // Show toast notification to user
                        sender
                            .output(PlayerOutput::ShowToast("End of season".to_string()))
                            .unwrap();
                    }
                } else {
                    debug!(
                        "Episode ending with auto-play disabled, letting video finish naturally"
                    );
                    // Auto-play is disabled, let the video finish naturally
                    // User can manually navigate back when they're ready
                }
            } else {
                debug!("Episode ending without playlist context, letting video finish naturally");
                // No playlist context - this is a standalone video
                // Let it finish naturally, user can manually navigate back
            }
        }
    }

    /// Cancel pending auto-play countdown and reset triggered state
    /// Call this when user manually navigates or stops playback
    pub fn cancel(&mut self) {
        self.auto_play_triggered = false;
        if let Some(timeout) = self.auto_play_timeout.take() {
            let _ = timeout.remove();
        }
    }

    /// Reset triggered state (called from ClearAutoPlayTimeout message)
    pub fn reset(&mut self) {
        self.auto_play_timeout = None;
    }
}

impl Drop for AutoPlayManager {
    fn drop(&mut self) {
        // Clean up timeout
        if let Some(timer) = self.auto_play_timeout.take() {
            let _ = timer.remove();
        }
    }
}
