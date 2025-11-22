use gtk::glib::{self, SourceId};
use gtk::prelude::*;
use libadwaita as adw;
use relm4::gtk;
use relm4::prelude::*;
use tracing::debug;

use super::{PlayerInput, PlayerPage};

/// Control visibility state machine states
#[derive(Debug, PartialEq)]
pub(super) enum ControlState {
    /// Controls and cursor are completely hidden
    Hidden,
    /// Controls and cursor are visible with inactivity timer running
    Visible { timer_id: Option<SourceId> },
    /// Controls and cursor are visible because mouse is over controls
    Hovering,
}

/// Control visibility state machine implementation
impl PlayerPage {
    /// Transition to the Hidden state
    pub(super) fn transition_to_hidden(
        &mut self,
        _sender: AsyncComponentSender<Self>,
        from_timer: bool,
    ) {
        // Don't hide controls if a popover is open
        if *self.active_popover_count.borrow() > 0 {
            debug!("Popover is open, keeping controls visible");
            return;
        }
        // Only try to cancel timer if not called from the timer itself
        if !from_timer
            && let ControlState::Visible { timer_id } = &mut self.control_state
            && let Some(timer) = timer_id.take()
        {
            let _ = timer.remove();
        }

        self.control_state = ControlState::Hidden;

        // Hide cursor
        if let Some(surface) = self.window.surface() {
            if let Some(cursor) = gtk::gdk::Cursor::from_name("none", None) {
                surface.set_cursor(Some(&cursor));
            } else {
                surface.set_cursor(None);
            }
        }
    }

    /// Transition to the Visible state
    pub(super) fn transition_to_visible(&mut self, sender: AsyncComponentSender<Self>) {
        // Cancel any existing timer first
        if let ControlState::Visible { timer_id } = &mut self.control_state
            && let Some(timer) = timer_id.take()
        {
            let _ = timer.remove();
        }

        // Show cursor
        if let Some(surface) = self.window.surface()
            && let Some(cursor) = gtk::gdk::Cursor::from_name("default", None)
        {
            surface.set_cursor(Some(&cursor));
        }

        // Start inactivity timer
        let timeout_secs = self.inactivity_timeout_secs;
        let sender_clone = sender.clone();
        let timer_id = glib::timeout_add_seconds_local(timeout_secs as u32, move || {
            sender_clone.input(PlayerInput::HideControls);
            glib::ControlFlow::Break
        });

        self.control_state = ControlState::Visible {
            timer_id: Some(timer_id),
        };
    }

    /// Transition to the Hovering state
    pub(super) fn transition_to_hovering(&mut self, _sender: AsyncComponentSender<Self>) {
        // Cancel any existing timer
        if let ControlState::Visible { timer_id } = &mut self.control_state
            && let Some(timer) = timer_id.take()
        {
            let _ = timer.remove();
        }

        self.control_state = ControlState::Hovering;

        // Ensure cursor is visible
        if let Some(surface) = self.window.surface()
            && let Some(cursor) = gtk::gdk::Cursor::from_name("default", None)
        {
            surface.set_cursor(Some(&cursor));
        }
    }

    /// Check if controls should be visible
    pub(super) fn controls_visible(&self) -> bool {
        !matches!(self.control_state, ControlState::Hidden)
    }

    /// Check if mouse movement exceeds threshold
    pub(super) fn mouse_movement_exceeds_threshold(&self, x: f64, y: f64) -> bool {
        if let Some((last_x, last_y)) = self.last_mouse_position {
            let dx = (x - last_x).abs();
            let dy = (y - last_y).abs();
            let distance = (dx * dx + dy * dy).sqrt();
            distance >= self.mouse_move_threshold
        } else {
            true // First movement always exceeds threshold
        }
    }

    /// Check if mouse is over control widgets
    pub(super) fn is_mouse_over_controls(&self, _x: f64, y: f64) -> bool {
        // For now use the heuristic approach, but this should be replaced
        // with actual widget bounds checking when controls_overlay is properly set
        if let Some(controls) = &self.controls_overlay {
            // Get the height of the controls overlay
            let controls_height = controls.height() as f64;
            let window_height = self.window.height() as f64;

            // Check if y position is within control bounds
            // Controls are at the bottom of the window
            y >= (window_height - controls_height - 50.0) // Add some padding
        } else {
            // Fallback to heuristic: bottom 20% of window
            let window_height = self.window.height() as f64;
            y >= window_height * 0.8
        }
    }
}
