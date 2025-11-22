use gtk::prelude::*;
use relm4::AsyncComponentSender;
use relm4::gtk;
use std::time::Duration;

use super::{PlayerInput, PlayerPage, format_duration};

/// Manages seek bar widget and position/duration display.
/// Handles click and drag gestures for seeking, tooltip preview,
/// and position/duration label updates.
pub struct SeekBarManager {
    seek_bar: gtk::Scale,
    position_label: gtk::Label,
    duration_label: gtk::Label,
    is_seeking: bool,
}

impl SeekBarManager {
    /// Create new SeekBarManager with initialized widgets and gesture handlers
    pub fn new(sender: &AsyncComponentSender<PlayerPage>) -> Self {
        // Create seek bar widget
        let seek_bar = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
        seek_bar.set_draw_value(false);
        seek_bar.set_has_tooltip(true);

        // Add tooltip to show time at cursor position
        seek_bar.connect_query_tooltip(|scale, x, _y, _keyboard_mode, tooltip| {
            let adjustment = scale.adjustment();
            // x is already relative to the widget
            let width = scale.width() as f64;
            let ratio = x as f64 / width;
            let max = adjustment.upper();
            let value = ratio * max;
            // Clamp value to ensure it's non-negative
            let duration = Duration::from_secs_f64(value.max(0.0));
            tooltip.set_text(Some(&format_duration(duration)));
            true
        });

        // Create time labels
        let position_label = gtk::Label::new(Some("0:00"));
        let duration_label = gtk::Label::new(Some("0:00"));

        // Setup seek bar gesture handlers - handle clicks and drags for video seeking
        {
            let sender_start = sender.clone();
            let sender_end = sender.clone();
            let seek_bar_for_click = seek_bar.clone();

            // Handle direct clicks and drags for video seeking
            let click_gesture = gtk::GestureClick::new();
            click_gesture.set_button(gtk::gdk::BUTTON_PRIMARY);

            // Start seeking on press
            click_gesture.connect_pressed(move |_gesture, _n_press, x, _y| {
                sender_start.input(PlayerInput::StartSeeking);

                // Calculate position from click location
                let widget_width = seek_bar_for_click.width() as f64;
                let adjustment = seek_bar_for_click.adjustment();
                let range = adjustment.upper() - adjustment.lower();
                let value = adjustment.lower() + (x / widget_width) * range;

                // Update the scale value and seek
                seek_bar_for_click.set_value(value);
                sender_start.input(PlayerInput::Seek(Duration::from_secs_f64(value.max(0.0))));
            });

            // End seeking on release
            click_gesture.connect_released(move |_gesture, _n_press, _x, _y| {
                sender_end.input(PlayerInput::StopSeeking);
            });

            seek_bar.add_controller(click_gesture);

            // Handle dragging
            let sender_drag = sender.clone();
            let seek_bar_for_drag = seek_bar.clone();
            let drag_gesture = gtk::GestureDrag::new();
            drag_gesture.set_button(gtk::gdk::BUTTON_PRIMARY);

            drag_gesture.connect_drag_update(move |_gesture, offset_x, _offset_y| {
                // Calculate position from drag location
                let widget_width = seek_bar_for_drag.width() as f64;
                let adjustment = seek_bar_for_drag.adjustment();
                let range = adjustment.upper() - adjustment.lower();

                // Get current position and add offset
                let current_value = seek_bar_for_drag.value();
                let value_per_pixel = range / widget_width;
                let new_value = (current_value + offset_x * value_per_pixel)
                    .clamp(adjustment.lower(), adjustment.upper());

                // Update the scale value and seek
                seek_bar_for_drag.set_value(new_value);
                sender_drag.input(PlayerInput::Seek(Duration::from_secs_f64(
                    new_value.max(0.0),
                )));
            });

            seek_bar.add_controller(drag_gesture);
        }

        Self {
            seek_bar,
            position_label,
            duration_label,
            is_seeking: false,
        }
    }

    /// Get reference to seek bar widget
    pub fn get_seek_bar(&self) -> &gtk::Scale {
        &self.seek_bar
    }

    /// Get reference to position label widget
    pub fn get_position_label(&self) -> &gtk::Label {
        &self.position_label
    }

    /// Get reference to duration label widget
    pub fn get_duration_label(&self) -> &gtk::Label {
        &self.duration_label
    }

    /// Update position display and seek bar value
    /// Only updates seek bar if not currently seeking (prevents flicker during drag)
    pub fn update_position(&mut self, position: Duration) {
        self.position_label.set_text(&format_duration(position));

        // Only update seek bar position if we're not actively seeking
        // This prevents the bar from jumping around during drag operations
        if !self.is_seeking {
            self.seek_bar.set_value(position.as_secs_f64());
        }
    }

    /// Update duration display and seek bar range
    pub fn update_duration(&self, duration: Duration) {
        self.duration_label.set_text(&format_duration(duration));
        self.seek_bar.set_range(0.0, duration.as_secs_f64());
    }

    /// Set seeking state (true when user is dragging seek bar)
    pub fn set_seeking(&mut self, seeking: bool) {
        self.is_seeking = seeking;
    }

    /// Reset to initial state (0:00 position, unknown duration)
    pub fn reset(&mut self) {
        self.seek_bar.set_value(0.0);
        self.position_label.set_text("0:00");
        self.duration_label.set_text("--:--");
        self.is_seeking = false;
    }
}
