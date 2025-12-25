use gtk::prelude::*;
use relm4::gtk;
use relm4::prelude::*;

use crate::cache::stats::CurrentCacheStats;
#[cfg(feature = "gstreamer")]
use crate::player::BufferingState;

use super::buffering_warnings::{PerformanceWarning, WarningSeverity, detect_warnings};

/// Input messages for the BufferingOverlay component
/// Note: Some variants are defined as public API for future use
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum BufferingOverlayInput {
    /// Update buffering state from player
    #[cfg(feature = "gstreamer")]
    UpdateBufferingState(BufferingState),
    /// Update cache statistics
    UpdateCacheStats(CurrentCacheStats),
    /// Update estimated bitrate for warning detection
    UpdateEstimatedBitrate(u64),
    /// Show the overlay
    Show,
    /// Hide the overlay
    Hide,
}

/// The BufferingOverlay component displays buffering progress and download statistics
pub struct BufferingOverlay {
    /// Whether the overlay is currently visible
    visible: bool,
    /// Current buffering percentage (0-100)
    buffering_percentage: i32,
    /// Whether buffering is active
    is_buffering: bool,
    /// Current cache statistics
    cache_stats: CurrentCacheStats,
    /// Estimated bitrate for the current media (bytes per second)
    estimated_bitrate_bps: Option<u64>,
    /// Active performance warnings
    warnings: Vec<PerformanceWarning>,
}

#[allow(unused_assignments)]
#[relm4::component(pub)]
impl SimpleComponent for BufferingOverlay {
    type Init = ();
    type Input = BufferingOverlayInput;
    type Output = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_halign: gtk::Align::Center,
            set_valign: gtk::Align::Center,
            set_spacing: 16,
            #[watch]
            set_visible: model.visible && (model.is_buffering || model.cache_stats.active_downloads > 0),
            add_css_class: "osd",
            add_css_class: "buffering-overlay",

            // Buffering spinner and percentage
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_halign: gtk::Align::Center,
                set_spacing: 12,

                // Spinner
                gtk::Spinner {
                    set_spinning: true,
                    set_width_request: 48,
                    set_height_request: 48,
                    add_css_class: "buffering-spinner",
                },

                // Buffering percentage
                gtk::Label {
                    #[watch]
                    set_label: &if model.is_buffering {
                        format!("{}%", model.buffering_percentage)
                    } else {
                        "Loading...".to_string()
                    },
                    add_css_class: "title-2",
                    add_css_class: "buffering-percentage",
                },
            },

            // Download statistics
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_halign: gtk::Align::Center,
                set_spacing: 4,
                #[watch]
                set_visible: model.cache_stats.download_speed_bps > 0 || model.cache_stats.bytes_downloaded > 0,

                // Download speed
                gtk::Label {
                    #[watch]
                    set_label: &format_download_speed(model.cache_stats.download_speed_bps),
                    add_css_class: "dim-label",
                    add_css_class: "download-speed",
                },

                // Downloaded bytes / total size
                gtk::Label {
                    #[watch]
                    set_label: &format_downloaded_bytes(
                        model.cache_stats.bytes_downloaded,
                        model.cache_stats.total_size
                    ),
                    add_css_class: "dim-label",
                    add_css_class: "download-progress",
                },

                // Active downloads count
                gtk::Label {
                    #[watch]
                    set_label: &format_active_downloads(model.cache_stats.active_downloads),
                    #[watch]
                    set_visible: model.cache_stats.active_downloads > 0,
                    add_css_class: "dim-label",
                    add_css_class: "active-downloads",
                },
            },

            // Performance warnings
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_halign: gtk::Align::Center,
                set_spacing: 8,
                #[watch]
                set_visible: !model.warnings.is_empty(),
                add_css_class: "performance-warnings",

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_halign: gtk::Align::Center,
                    set_spacing: 8,
                    #[watch]
                    set_visible: !model.warnings.is_empty(),

                    // Warning icon
                    gtk::Image {
                        #[watch]
                        set_icon_name: Some(if model.warnings.iter().any(|w| w.severity() == WarningSeverity::Critical) {
                            "dialog-error-symbolic"
                        } else {
                            "dialog-warning-symbolic"
                        }),
                        set_pixel_size: 16,
                        #[watch]
                        add_css_class: if model.warnings.iter().any(|w| w.severity() == WarningSeverity::Critical) {
                            "warning-icon-critical"
                        } else {
                            "warning-icon"
                        },
                    },

                    // Warning message
                    gtk::Label {
                        #[watch]
                        set_label: &get_primary_warning_message(&model.warnings),
                        add_css_class: "warning-message",
                        set_wrap: true,
                        set_max_width_chars: 40,
                        set_justify: gtk::Justification::Center,
                    },
                },

                // Recommendation (if available)
                gtk::Label {
                    #[watch]
                    set_label: &get_warning_recommendation(&model.warnings),
                    #[watch]
                    set_visible: !get_warning_recommendation(&model.warnings).is_empty(),
                    add_css_class: "dim-label",
                    add_css_class: "warning-recommendation",
                    set_wrap: true,
                    set_max_width_chars: 40,
                    set_justify: gtk::Justification::Center,
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = BufferingOverlay {
            visible: false,
            buffering_percentage: 0,
            is_buffering: false,
            cache_stats: CurrentCacheStats::empty(),
            estimated_bitrate_bps: None,
            warnings: Vec::new(),
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            #[cfg(feature = "gstreamer")]
            BufferingOverlayInput::UpdateBufferingState(state) => {
                self.is_buffering = state.is_buffering;
                self.buffering_percentage = state.percentage;
                // Auto-show when buffering starts
                if state.is_buffering {
                    self.visible = true;
                }
                // Update warnings when buffering state changes
                self.update_warnings();
            }
            BufferingOverlayInput::UpdateCacheStats(stats) => {
                self.cache_stats = stats;
                // Update warnings when cache stats change
                self.update_warnings();
            }
            BufferingOverlayInput::UpdateEstimatedBitrate(bitrate) => {
                self.estimated_bitrate_bps = Some(bitrate);
                // Update warnings when bitrate changes
                self.update_warnings();
            }
            BufferingOverlayInput::Show => {
                self.visible = true;
            }
            BufferingOverlayInput::Hide => {
                self.visible = false;
            }
        }
    }
}

impl BufferingOverlay {
    /// Update performance warnings based on current state
    fn update_warnings(&mut self) {
        self.warnings = detect_warnings(
            self.cache_stats.download_speed_bps,
            self.estimated_bitrate_bps,
            self.buffering_percentage,
        );
    }
}

/// Get the primary warning message to display
fn get_primary_warning_message(warnings: &[PerformanceWarning]) -> String {
    if warnings.is_empty() {
        return String::new();
    }

    // Show the most severe warning first
    let mut sorted_warnings = warnings.to_vec();
    sorted_warnings.sort_by_key(|w| match w.severity() {
        WarningSeverity::Critical => 0,
        WarningSeverity::Warning => 1,
        WarningSeverity::Info => 2,
    });

    sorted_warnings[0].message()
}

/// Get the warning recommendation to display
fn get_warning_recommendation(warnings: &[PerformanceWarning]) -> String {
    if warnings.is_empty() {
        return String::new();
    }

    // Show recommendation from the most severe warning
    let mut sorted_warnings = warnings.to_vec();
    sorted_warnings.sort_by_key(|w| match w.severity() {
        WarningSeverity::Critical => 0,
        WarningSeverity::Warning => 1,
        WarningSeverity::Info => 2,
    });

    sorted_warnings[0].recommendation().unwrap_or_default()
}

/// Format download speed in human-readable format (KB/s or MB/s)
fn format_download_speed(bytes_per_second: u64) -> String {
    if bytes_per_second == 0 {
        return String::new();
    }

    let kbps = bytes_per_second as f64 / 1024.0;
    if kbps < 1024.0 {
        format!("{:.1} KB/s", kbps)
    } else {
        let mbps = kbps / 1024.0;
        format!("{:.1} MB/s", mbps)
    }
}

/// Format downloaded bytes and total size
fn format_downloaded_bytes(downloaded: u64, total: Option<u64>) -> String {
    if downloaded == 0 {
        return String::new();
    }

    let downloaded_mb = downloaded as f64 / (1024.0 * 1024.0);

    if let Some(total_bytes) = total {
        let total_mb = total_bytes as f64 / (1024.0 * 1024.0);
        format!("{:.1} MB / {:.1} MB", downloaded_mb, total_mb)
    } else {
        format!("{:.1} MB", downloaded_mb)
    }
}

/// Format active downloads count
fn format_active_downloads(count: usize) -> String {
    if count == 0 {
        String::new()
    } else if count == 1 {
        "1 active download".to_string()
    } else {
        format!("{} active downloads", count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_download_speed() {
        assert_eq!(format_download_speed(0), "");
        assert_eq!(format_download_speed(1024), "1.0 KB/s");
        assert_eq!(format_download_speed(1024 * 512), "512.0 KB/s");
        assert_eq!(format_download_speed(1024 * 1024), "1.0 MB/s");
        assert_eq!(format_download_speed(1024 * 1024 * 5), "5.0 MB/s");
    }

    #[test]
    fn test_format_downloaded_bytes() {
        assert_eq!(format_downloaded_bytes(0, None), "");
        assert_eq!(format_downloaded_bytes(1024 * 1024 * 10, None), "10.0 MB");
        assert_eq!(
            format_downloaded_bytes(1024 * 1024 * 10, Some(1024 * 1024 * 100)),
            "10.0 MB / 100.0 MB"
        );
    }

    #[test]
    fn test_format_active_downloads() {
        assert_eq!(format_active_downloads(0), "");
        assert_eq!(format_active_downloads(1), "1 active download");
        assert_eq!(format_active_downloads(5), "5 active downloads");
    }
}
