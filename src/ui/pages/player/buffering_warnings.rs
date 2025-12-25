/// Performance warning detection utilities for buffering and download monitoring
///
/// This module provides pure, stateless functions to detect performance issues
/// during media playback. All functions are reusable and can be used by UI components
/// without coupling to PlayerPage internals.

/// Warning message constants
pub mod messages {
    pub const BUFFERING_STALLED: &str = "Buffering appears to be stalled";
    pub const NETWORK_ISSUE: &str = "Network connection may be unstable";
}

/// Warning severity levels
/// Note: Some variants are defined for API completeness
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum WarningSeverity {
    /// Informational warning (performance may be affected)
    Info,
    /// Warning (user experience is degraded)
    Warning,
    /// Critical (playback will likely fail or stall)
    Critical,
}

/// Performance warning types
/// Note: Some variants are defined for API completeness and future detection
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PerformanceWarning {
    /// Download speed is slower than required bitrate
    SlowDownload {
        download_speed_bps: u64,
        required_bitrate_bps: u64,
    },
    /// Buffer level is critically low (below safe threshold)
    CriticallyLowBuffer { percentage: i32 },
    /// Buffering has stalled (no progress for extended period)
    BufferingStalled,
    /// Network appears unstable (intermittent issues)
    NetworkUnstable,
}

impl PerformanceWarning {
    /// Get the severity level of this warning
    pub fn severity(&self) -> WarningSeverity {
        match self {
            PerformanceWarning::SlowDownload { .. } => WarningSeverity::Warning,
            PerformanceWarning::CriticallyLowBuffer { percentage } => {
                if *percentage < 10 {
                    WarningSeverity::Critical
                } else {
                    WarningSeverity::Warning
                }
            }
            PerformanceWarning::BufferingStalled => WarningSeverity::Critical,
            PerformanceWarning::NetworkUnstable => WarningSeverity::Warning,
        }
    }

    /// Get a user-friendly message for this warning
    pub fn message(&self) -> String {
        match self {
            PerformanceWarning::SlowDownload {
                download_speed_bps,
                required_bitrate_bps,
            } => {
                let download_mbps = *download_speed_bps as f64 / (1024.0 * 1024.0);
                let required_mbps = *required_bitrate_bps as f64 / (1024.0 * 1024.0);
                format!(
                    "Download speed ({:.1} Mbps) is slower than required ({:.1} Mbps)",
                    download_mbps, required_mbps
                )
            }
            PerformanceWarning::CriticallyLowBuffer { percentage } => {
                format!("Buffer level critically low ({}%)", percentage)
            }
            PerformanceWarning::BufferingStalled => messages::BUFFERING_STALLED.to_string(),
            PerformanceWarning::NetworkUnstable => messages::NETWORK_ISSUE.to_string(),
        }
    }

    /// Get an actionable recommendation for the user
    pub fn recommendation(&self) -> Option<String> {
        match self {
            PerformanceWarning::SlowDownload { .. } => {
                Some("Try pausing playback briefly to allow more buffering".to_string())
            }
            PerformanceWarning::CriticallyLowBuffer { .. } => {
                Some("Playback may stall soon. Consider pausing to buffer.".to_string())
            }
            PerformanceWarning::BufferingStalled => {
                Some("Check your network connection or try reloading".to_string())
            }
            PerformanceWarning::NetworkUnstable => {
                Some("Your network connection appears unstable".to_string())
            }
        }
    }
}

/// Check if download speed is too slow for the required bitrate
///
/// Returns true if the download speed is significantly slower than the required
/// bitrate, indicating that buffering may not keep up with playback.
///
/// # Arguments
/// * `download_speed_bps` - Current download speed in bytes per second
/// * `required_bitrate_bps` - Required bitrate for smooth playback in bytes per second
/// * `safety_margin` - Safety margin multiplier (e.g., 1.2 = 20% margin)
pub fn is_download_too_slow(
    download_speed_bps: u64,
    required_bitrate_bps: u64,
    safety_margin: f64,
) -> bool {
    if download_speed_bps == 0 || required_bitrate_bps == 0 {
        return false;
    }

    let required_with_margin = (required_bitrate_bps as f64 * safety_margin) as u64;
    download_speed_bps < required_with_margin
}

/// Check if buffer level is critically low
///
/// Returns true if the buffer percentage is below the critical threshold,
/// indicating that playback may stall soon.
///
/// # Arguments
/// * `buffer_percentage` - Current buffer level (0-100)
/// * `critical_threshold` - Threshold below which buffer is considered critical (default: 15)
pub fn is_buffer_critically_low(buffer_percentage: i32, critical_threshold: i32) -> bool {
    buffer_percentage > 0 && buffer_percentage < critical_threshold
}

/// Check if buffering appears to be stalled
///
/// Returns true if the buffer percentage hasn't changed for an extended period,
/// indicating the stream may be stuck.
///
/// # Arguments
/// * `current_percentage` - Current buffer level (0-100)
/// * `previous_percentage` - Previous buffer level (0-100)
/// * `seconds_unchanged` - How long the percentage has been unchanged
/// * `stall_threshold_secs` - Threshold in seconds to consider stalled (default: 10)
#[allow(dead_code)]
pub fn is_buffering_stalled(
    current_percentage: i32,
    previous_percentage: i32,
    seconds_unchanged: u64,
    stall_threshold_secs: u64,
) -> bool {
    // Not stalled if at 0% (not started) or 100% (complete)
    if current_percentage == 0 || current_percentage == 100 {
        return false;
    }

    // Stalled if same percentage for threshold time
    current_percentage == previous_percentage && seconds_unchanged >= stall_threshold_secs
}

/// Detect all active performance warnings
///
/// This is a convenience function that checks all warning conditions and returns
/// a list of active warnings.
///
/// # Arguments
/// * `download_speed_bps` - Current download speed in bytes per second
/// * `estimated_bitrate_bps` - Estimated playback bitrate in bytes per second (optional)
/// * `buffer_percentage` - Current buffer level (0-100)
pub fn detect_warnings(
    download_speed_bps: u64,
    estimated_bitrate_bps: Option<u64>,
    buffer_percentage: i32,
) -> Vec<PerformanceWarning> {
    let mut warnings = Vec::new();

    // Check slow download (if we know the bitrate)
    if let Some(bitrate) = estimated_bitrate_bps {
        if is_download_too_slow(download_speed_bps, bitrate, 1.2) {
            warnings.push(PerformanceWarning::SlowDownload {
                download_speed_bps,
                required_bitrate_bps: bitrate,
            });
        }
    }

    // Check critically low buffer
    if is_buffer_critically_low(buffer_percentage, 15) {
        warnings.push(PerformanceWarning::CriticallyLowBuffer {
            percentage: buffer_percentage,
        });
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_download_too_slow() {
        // 1 Mbps download, 2 Mbps required, 1.2x margin = needs 2.4 Mbps
        assert!(is_download_too_slow(
            1024 * 1024,     // 1 MB/s
            2 * 1024 * 1024, // 2 MB/s required
            1.2
        ));

        // 3 Mbps download, 2 Mbps required, 1.2x margin = needs 2.4 Mbps
        assert!(!is_download_too_slow(
            3 * 1024 * 1024, // 3 MB/s
            2 * 1024 * 1024, // 2 MB/s required
            1.2
        ));

        // Zero values should return false
        assert!(!is_download_too_slow(0, 1024 * 1024, 1.2));
        assert!(!is_download_too_slow(1024 * 1024, 0, 1.2));
    }

    #[test]
    fn test_is_buffer_critically_low() {
        assert!(is_buffer_critically_low(10, 15)); // 10% < 15% threshold
        assert!(is_buffer_critically_low(14, 15)); // 14% < 15% threshold
        assert!(!is_buffer_critically_low(15, 15)); // 15% == 15% threshold (not critical)
        assert!(!is_buffer_critically_low(20, 15)); // 20% > 15% threshold
        assert!(!is_buffer_critically_low(0, 15)); // 0% is ignored (not started)
    }

    #[test]
    fn test_is_buffering_stalled() {
        // Stalled: same percentage for 10+ seconds
        assert!(is_buffering_stalled(50, 50, 10, 10));
        assert!(is_buffering_stalled(50, 50, 15, 10));

        // Not stalled: percentage changed
        assert!(!is_buffering_stalled(51, 50, 10, 10));

        // Not stalled: not enough time elapsed
        assert!(!is_buffering_stalled(50, 50, 5, 10));

        // Not stalled: at 0% or 100%
        assert!(!is_buffering_stalled(0, 0, 15, 10));
        assert!(!is_buffering_stalled(100, 100, 15, 10));
    }

    #[test]
    fn test_detect_warnings() {
        // Slow download + low buffer
        let warnings = detect_warnings(
            1024 * 1024,           // 1 MB/s download
            Some(2 * 1024 * 1024), // 2 MB/s required
            10,                    // 10% buffer
        );
        assert_eq!(warnings.len(), 2);
        assert!(matches!(
            warnings[0],
            PerformanceWarning::SlowDownload { .. }
        ));
        assert!(matches!(
            warnings[1],
            PerformanceWarning::CriticallyLowBuffer { .. }
        ));

        // No warnings
        let warnings = detect_warnings(
            3 * 1024 * 1024,       // 3 MB/s download
            Some(2 * 1024 * 1024), // 2 MB/s required
            50,                    // 50% buffer
        );
        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn test_warning_severity() {
        let warning = PerformanceWarning::SlowDownload {
            download_speed_bps: 1024 * 1024,
            required_bitrate_bps: 2 * 1024 * 1024,
        };
        assert_eq!(warning.severity(), WarningSeverity::Warning);

        let warning = PerformanceWarning::CriticallyLowBuffer { percentage: 5 };
        assert_eq!(warning.severity(), WarningSeverity::Critical);

        let warning = PerformanceWarning::CriticallyLowBuffer { percentage: 12 };
        assert_eq!(warning.severity(), WarningSeverity::Warning);

        let warning = PerformanceWarning::BufferingStalled;
        assert_eq!(warning.severity(), WarningSeverity::Critical);
    }

    #[test]
    fn test_warning_messages() {
        let warning = PerformanceWarning::SlowDownload {
            download_speed_bps: 1024 * 1024,
            required_bitrate_bps: 2 * 1024 * 1024,
        };
        let msg = warning.message();
        assert!(msg.contains("1.0"));
        assert!(msg.contains("2.0"));
        assert!(msg.contains("Mbps"));

        let warning = PerformanceWarning::CriticallyLowBuffer { percentage: 8 };
        assert_eq!(warning.message(), "Buffer level critically low (8%)");

        let warning = PerformanceWarning::BufferingStalled;
        assert!(!warning.message().is_empty());
    }

    #[test]
    fn test_warning_recommendations() {
        let warning = PerformanceWarning::SlowDownload {
            download_speed_bps: 1024 * 1024,
            required_bitrate_bps: 2 * 1024 * 1024,
        };
        assert!(warning.recommendation().is_some());

        let warning = PerformanceWarning::BufferingStalled;
        let rec = warning.recommendation();
        assert!(rec.is_some());
        let rec_str = rec.unwrap();
        assert!(rec_str.contains("network") || rec_str.contains("reload"));
    }
}
