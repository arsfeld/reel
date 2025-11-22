use std::time::Duration;

/// Manages playback progress tracking state and configuration.
pub struct ProgressTracker {
    /// Last time progress was saved
    last_progress_save: std::time::Instant,
    /// Whether to auto-resume from saved position
    config_auto_resume: bool,
    /// Threshold in seconds below which we don't resume
    config_resume_threshold_seconds: u64,
    /// Interval in seconds between progress saves
    config_progress_update_interval_seconds: u64,
}

impl ProgressTracker {
    pub fn new(
        auto_resume: bool,
        resume_threshold_seconds: u64,
        progress_update_interval_seconds: u64,
    ) -> Self {
        Self {
            last_progress_save: std::time::Instant::now(),
            config_auto_resume: auto_resume,
            config_resume_threshold_seconds: resume_threshold_seconds,
            config_progress_update_interval_seconds: progress_update_interval_seconds,
        }
    }

    /// Check if progress should be saved based on time elapsed and watch status
    pub fn should_save_progress(&self, position: Duration, duration: Duration) -> bool {
        let elapsed = self.last_progress_save.elapsed().as_secs();
        let watched = position.as_secs_f64() / duration.as_secs_f64() > 0.9;

        watched || elapsed >= self.config_progress_update_interval_seconds
    }

    /// Update configuration values
    pub fn update_config(
        &mut self,
        auto_resume: bool,
        resume_threshold_seconds: u64,
        progress_update_interval_seconds: u64,
    ) {
        self.config_auto_resume = auto_resume;
        self.config_resume_threshold_seconds = resume_threshold_seconds;
        self.config_progress_update_interval_seconds = progress_update_interval_seconds;
    }

    /// Reset the save timer (e.g., when starting new media or after saving)
    pub fn reset_save_timer(&mut self) {
        self.last_progress_save = std::time::Instant::now();
    }

    /// Check if we should resume from saved position
    pub fn should_resume(&self, saved_position: Duration, duration: Duration) -> bool {
        if !self.config_auto_resume {
            return false;
        }

        let saved_secs = saved_position.as_secs();
        let duration_secs = duration.as_secs();

        // Don't resume if position is too close to the start
        if saved_secs < self.config_resume_threshold_seconds {
            return false;
        }

        // Don't resume if already watched (>90%)
        if saved_secs as f64 / duration_secs as f64 > 0.9 {
            return false;
        }

        true
    }

    /// Get the progress update interval in seconds
    pub fn get_progress_update_interval_seconds(&self) -> u64 {
        self.config_progress_update_interval_seconds
    }

    /// Get auto-resume configuration value
    pub fn get_auto_resume(&self) -> bool {
        self.config_auto_resume
    }

    /// Get resume threshold in seconds
    pub fn get_resume_threshold_seconds(&self) -> u64 {
        self.config_resume_threshold_seconds
    }
}
