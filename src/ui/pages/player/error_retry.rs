use crate::models::{MediaItemId, PlaylistContext};
use relm4::AsyncComponentSender;
use relm4::gtk::glib::{self, SourceId};
use std::time::Duration;
use tracing::info;

use super::PlayerInput;

/// Manages error state and retry logic with exponential backoff for playback failures.
pub struct ErrorRetryManager {
    /// Current error message, if any
    error_message: Option<String>,
    /// Number of retry attempts for the current error
    retry_count: u32,
    /// Maximum number of retries before giving up
    max_retries: u32,
    /// Active retry timer
    retry_timer: Option<SourceId>,
}

impl ErrorRetryManager {
    pub fn new(max_retries: u32) -> Self {
        Self {
            error_message: None,
            retry_count: 0,
            max_retries,
            retry_timer: None,
        }
    }

    /// Get the current error message
    pub fn get_error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    /// Check if there's an active error
    pub fn has_error(&self) -> bool {
        self.error_message.is_some()
    }

    /// Show an error message
    pub fn show_error(&mut self, message: String) {
        self.error_message = Some(message);
    }

    /// Clear the error message and reset retry state
    pub fn clear_error(&mut self) {
        self.error_message = None;
        self.retry_count = 0;
        if let Some(timer) = self.retry_timer.take() {
            let _ = timer.remove();
        }
    }

    /// Schedule a retry with exponential backoff (1s, 2s, 4s)
    /// Returns true if retry was scheduled, false if max retries exceeded
    pub fn schedule_retry(
        &mut self,
        media_id: MediaItemId,
        context: Option<PlaylistContext>,
        sender: &AsyncComponentSender<super::PlayerPage>,
    ) -> bool {
        // Clear any previous error
        self.error_message = None;

        // Check if we've exceeded max retries
        if self.retry_count >= self.max_retries {
            self.error_message = Some(
                "Failed to load media after multiple attempts. Please try again later.".to_string(),
            );
            self.retry_count = 0;
            return false;
        }

        // Increment retry count
        self.retry_count += 1;

        // Calculate delay with exponential backoff: 2^(retry_count - 1) seconds
        let delay = Duration::from_secs(2_u64.pow(self.retry_count - 1));

        // Cancel any existing retry timer
        if let Some(timer) = self.retry_timer.take() {
            let _ = timer.remove();
        }

        // Schedule the retry
        info!("Scheduling retry #{} after {:?}", self.retry_count, delay);

        let sender_clone = sender.clone();
        self.retry_timer = Some(glib::timeout_add_local(delay, move || {
            if let Some(context) = context.clone() {
                sender_clone.input(PlayerInput::LoadMediaWithContext {
                    media_id: media_id.clone(),
                    context,
                });
            } else {
                sender_clone.input(PlayerInput::LoadMedia(media_id.clone()));
            }
            glib::ControlFlow::Break
        }));

        true
    }

    /// Reset the retry count (called when a load succeeds)
    pub fn reset(&mut self) {
        self.retry_count = 0;
    }
}

impl Drop for ErrorRetryManager {
    fn drop(&mut self) {
        // Clean up retry timer
        if let Some(timer) = self.retry_timer.take() {
            let _ = timer.remove();
        }
    }
}
