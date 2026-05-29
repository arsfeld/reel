/// Watch progress for a media item.
#[derive(Debug, Clone, PartialEq)]
pub struct WatchProgress {
    pub media_item_id: String,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub watched: bool,
    pub last_watched_at: String, // ISO 8601
}

#[allow(dead_code)]
impl WatchProgress {
    /// Progress as a fraction (0.0 to 1.0).
    pub fn progress_fraction(&self) -> f64 {
        if self.duration_seconds > 0.0 {
            (self.position_seconds / self.duration_seconds).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Whether to show a resume prompt for this item.
    /// Only show if not already watched, position > 30s, and not past 90%.
    pub fn should_show_resume(&self) -> bool {
        !self.watched && self.position_seconds > 30.0 && self.progress_fraction() < 0.90
    }

    /// Resume position with 10-second backtrack for context.
    pub fn resume_position(&self) -> f64 {
        (self.position_seconds - 10.0).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress(position: f64, duration: f64, watched: bool) -> WatchProgress {
        WatchProgress {
            media_item_id: "test:item:1".to_string(),
            position_seconds: position,
            duration_seconds: duration,
            watched,
            last_watched_at: "2026-03-14T12:00:00Z".to_string(),
        }
    }

    #[test]
    fn progress_fraction_normal() {
        let wp = progress(50.0, 100.0, false);
        assert!((wp.progress_fraction() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn progress_fraction_zero_duration() {
        let wp = progress(10.0, 0.0, false);
        assert!((wp.progress_fraction() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn progress_fraction_clamped_to_one() {
        let wp = progress(110.0, 100.0, false);
        assert!((wp.progress_fraction() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn progress_fraction_at_start() {
        let wp = progress(0.0, 100.0, false);
        assert!((wp.progress_fraction() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn should_show_resume_in_progress() {
        let wp = progress(300.0, 7200.0, false); // 5 min into 2-hour movie
        assert!(wp.should_show_resume());
    }

    #[test]
    fn should_show_resume_false_when_watched() {
        let wp = progress(300.0, 7200.0, true);
        assert!(!wp.should_show_resume());
    }

    #[test]
    fn should_show_resume_false_when_position_too_low() {
        let wp = progress(10.0, 7200.0, false); // 10s in
        assert!(!wp.should_show_resume());
    }

    #[test]
    fn should_show_resume_false_at_threshold() {
        let wp = progress(30.0, 7200.0, false); // exactly 30s
        assert!(!wp.should_show_resume());
    }

    #[test]
    fn should_show_resume_true_just_past_threshold() {
        let wp = progress(30.1, 7200.0, false);
        assert!(wp.should_show_resume());
    }

    #[test]
    fn should_show_resume_false_past_ninety_percent() {
        let wp = progress(6500.0, 7200.0, false); // ~90.3%
        assert!(!wp.should_show_resume());
    }

    #[test]
    fn should_show_resume_true_just_under_ninety_percent() {
        let wp = progress(6479.0, 7200.0, false); // ~89.99%
        assert!(wp.should_show_resume());
    }

    #[test]
    fn resume_position_backs_up_ten_seconds() {
        let wp = progress(300.0, 7200.0, false);
        assert!((wp.resume_position() - 290.0).abs() < f64::EPSILON);
    }

    #[test]
    fn resume_position_clamped_to_zero() {
        let wp = progress(5.0, 7200.0, false);
        assert!((wp.resume_position() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn resume_position_exactly_ten() {
        let wp = progress(10.0, 7200.0, false);
        assert!((wp.resume_position() - 0.0).abs() < f64::EPSILON);
    }
}
