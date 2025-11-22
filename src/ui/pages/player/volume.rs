use gtk::prelude::*;
use relm4::AsyncComponentSender;
use relm4::gtk;

use super::{PlayerInput, PlayerPage};

/// Manages volume control widget and volume adjustment logic.
/// Handles volume slider widget, volume up/down operations,
/// and synchronization with player backend.
pub struct VolumeManager {
    volume_slider: gtk::Scale,
    volume: f64,
}

impl VolumeManager {
    /// Create new VolumeManager with initialized widget and handler
    pub fn new(sender: &AsyncComponentSender<PlayerPage>) -> Self {
        // Create volume slider widget
        let volume_slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.01);
        volume_slider.set_value(1.0);
        volume_slider.set_draw_value(false);

        // Setup volume slider change handler
        {
            let sender = sender.clone();
            let volume_slider_clone = volume_slider.clone();
            volume_slider_clone.connect_value_changed(move |scale| {
                sender.input(PlayerInput::SetVolume(scale.value()));
            });
        }

        Self {
            volume_slider,
            volume: 1.0,
        }
    }

    /// Get reference to volume slider widget
    pub fn get_volume_slider(&self) -> &gtk::Scale {
        &self.volume_slider
    }

    /// Get current volume level (0.0 - 1.0)
    pub fn get_volume(&self) -> f64 {
        self.volume
    }

    /// Set volume level (0.0 - 1.0)
    /// Updates both internal state and slider widget
    pub fn set_volume(&mut self, volume: f64) {
        self.volume = volume.clamp(0.0, 1.0);
        self.volume_slider.set_value(self.volume);
    }

    /// Increase volume by 10%, capped at 100%
    pub fn volume_up(&mut self) -> f64 {
        self.volume = (self.volume + 0.1).min(1.0);
        self.volume_slider.set_value(self.volume);
        self.volume
    }

    /// Decrease volume by 10%, capped at 0%
    pub fn volume_down(&mut self) -> f64 {
        self.volume = (self.volume - 0.1).max(0.0);
        self.volume_slider.set_value(self.volume);
        self.volume
    }

    /// Sync volume state from player backend
    /// Updates internal state without triggering change events
    pub fn sync_from_player(&mut self, volume: f64) {
        self.volume = volume.clamp(0.0, 1.0);
        // Update slider value without triggering value_changed signal
        self.volume_slider.set_value(self.volume);
    }
}
