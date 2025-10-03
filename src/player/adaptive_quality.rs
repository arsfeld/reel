use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use super::PlayerState;
use crate::models::{QualityOption, Resolution};

/// Mode for adaptive quality control
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdaptiveMode {
    /// Automatic quality adjustment based on network conditions
    Auto,
    /// Manual quality selection by user
    Manual,
}

/// Playback health status
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackHealth {
    /// Playing smoothly without issues
    Healthy,
    /// Currently buffering
    Buffering,
    /// Frequent buffering (>3 in 60s)
    Unstable,
    /// Playback failed completely
    Failed,
}

/// Playback metrics for quality decisions
#[derive(Debug, Clone)]
pub struct PlaybackMetrics {
    pub health: PlaybackHealth,
    pub buffer_count: u32,
    pub average_buffer_duration: Duration,
    pub time_since_last_buffer: Duration,
    pub playback_errors: u32,
}

impl Default for PlaybackMetrics {
    fn default() -> Self {
        Self {
            health: PlaybackHealth::Healthy,
            buffer_count: 0,
            average_buffer_duration: Duration::ZERO,
            time_since_last_buffer: Duration::ZERO,
            playback_errors: 0,
        }
    }
}

/// Bandwidth trend indicator
#[derive(Debug, Clone, PartialEq)]
pub enum BandwidthTrend {
    Increasing,
    Stable,
    Decreasing,
}

/// Bandwidth metrics for quality decisions
#[derive(Debug, Clone)]
pub struct BandwidthMetrics {
    pub current_speed_bps: u64,
    pub average_speed_bps: u64,
    pub trend: BandwidthTrend,
    pub estimated_available_bps: u64,
}

impl Default for BandwidthMetrics {
    fn default() -> Self {
        Self {
            current_speed_bps: 0,
            average_speed_bps: 0,
            trend: BandwidthTrend::Stable,
            estimated_available_bps: 0,
        }
    }
}

/// Quality change decision
#[derive(Debug, Clone)]
pub enum QualityDecision {
    /// Keep current quality
    Maintain,
    /// Switch to lower quality
    Decrease(QualityOption),
    /// Switch to higher quality
    Increase(QualityOption),
    /// Emergency: playback failed, retry with lower quality
    Recover(QualityOption),
}

/// Buffering event record
#[derive(Debug, Clone)]
struct BufferEvent {
    timestamp: Instant,
    duration: Duration,
}

/// Bandwidth measurement
#[derive(Debug, Clone)]
struct SpeedMeasurement {
    timestamp: Instant,
    bytes_per_second: u64,
}

/// Consolidated adaptive quality manager with built-in monitoring
pub struct AdaptiveQualityManager {
    // Configuration
    mode: AdaptiveMode,
    available_qualities: Vec<QualityOption>,
    current_quality_index: usize,

    // Quality change control
    last_quality_change: Option<Instant>,
    quality_change_cooldown: Duration,

    // Playback monitoring state
    current_player_state: PlayerState,
    buffer_history: VecDeque<BufferEvent>,
    buffer_start_time: Option<Instant>,
    playback_metrics: PlaybackMetrics,

    // Bandwidth monitoring state
    speed_measurements: VecDeque<SpeedMeasurement>,
    bandwidth_metrics: BandwidthMetrics,

    // Communication channels
    state_rx: mpsc::UnboundedReceiver<PlayerState>,
    bandwidth_rx: mpsc::UnboundedReceiver<(u64, Duration)>,
    decision_tx: mpsc::UnboundedSender<QualityDecision>,
}

impl AdaptiveQualityManager {
    /// Create a new adaptive quality manager
    pub fn new(
        available_qualities: Vec<QualityOption>,
        current_quality_index: usize,
        state_rx: mpsc::UnboundedReceiver<PlayerState>,
        bandwidth_rx: mpsc::UnboundedReceiver<(u64, Duration)>,
        decision_tx: mpsc::UnboundedSender<QualityDecision>,
    ) -> Self {
        Self {
            mode: AdaptiveMode::Auto,
            available_qualities,
            current_quality_index,
            last_quality_change: None,
            quality_change_cooldown: Duration::from_secs(10),
            current_player_state: PlayerState::Idle,
            buffer_history: VecDeque::new(),
            buffer_start_time: None,
            playback_metrics: PlaybackMetrics::default(),
            speed_measurements: VecDeque::new(),
            bandwidth_metrics: BandwidthMetrics::default(),
            state_rx,
            bandwidth_rx,
            decision_tx,
        }
    }

    /// Set adaptive mode (Auto or Manual)
    pub fn set_mode(&mut self, mode: AdaptiveMode) {
        info!("Adaptive quality mode changed to: {:?}", mode);
        self.mode = mode;
    }

    /// Get current mode
    pub fn mode(&self) -> &AdaptiveMode {
        &self.mode
    }

    /// Manually set quality (switches to Manual mode)
    pub fn set_manual_quality(&mut self, index: usize) {
        if index < self.available_qualities.len() {
            info!("Manual quality selected: index {}", index);
            self.mode = AdaptiveMode::Manual;
            self.current_quality_index = index;
        }
    }

    /// Handle a bandwidth update from chunk download
    fn handle_bandwidth_update(&mut self, bytes: u64, duration: Duration) {
        let bytes_per_second = if duration.as_secs() > 0 {
            bytes / duration.as_secs()
        } else {
            bytes * 1000 / duration.as_millis().max(1) as u64
        };

        let now = Instant::now();

        // Add measurement
        self.speed_measurements.push_back(SpeedMeasurement {
            timestamp: now,
            bytes_per_second,
        });

        // Keep only last 30 seconds
        while let Some(first) = self.speed_measurements.front() {
            if now.duration_since(first.timestamp) > Duration::from_secs(30) {
                self.speed_measurements.pop_front();
            } else {
                break;
            }
        }

        // Update bandwidth metrics
        self.update_bandwidth_metrics();
    }

    /// Run the adaptive quality monitoring loop
    pub async fn run(mut self) {
        info!("AdaptiveQualityManager started");

        loop {
            tokio::select! {
                Some(state) = self.state_rx.recv() => {
                    self.handle_state_change(state).await;
                }
                Some((bytes, duration)) = self.bandwidth_rx.recv() => {
                    self.handle_bandwidth_update(bytes, duration);
                }
                else => {
                    // All channels closed
                    break;
                }
            }
        }

        info!("AdaptiveQualityManager stopped");
    }

    /// Handle player state change
    async fn handle_state_change(&mut self, new_state: PlayerState) {
        match (&self.current_player_state, &new_state) {
            // Buffering started
            (PlayerState::Playing, PlayerState::Loading) => {
                warn!("Buffering started");
                self.buffer_start_time = Some(Instant::now());
            }

            // Buffering ended
            (PlayerState::Loading, PlayerState::Playing) => {
                if let Some(start) = self.buffer_start_time.take() {
                    let duration = start.elapsed();
                    info!("Buffering ended, duration: {:?}", duration);
                    self.record_buffer_event(duration);
                }
            }

            // Playback error
            (_, PlayerState::Error) => {
                warn!("Playback error detected");
                self.playback_metrics.playback_errors += 1;
                self.playback_metrics.health = PlaybackHealth::Failed;
            }

            _ => {}
        }

        self.current_player_state = new_state;
        self.update_playback_health();

        // Make quality decision if in Auto mode
        if self.mode == AdaptiveMode::Auto {
            if let Some(decision) = self.evaluate_quality() {
                debug!("Quality decision: {:?}", decision);
                let _ = self.decision_tx.send(decision);
            }
        }
    }

    /// Record a buffering event
    fn record_buffer_event(&mut self, duration: Duration) {
        let now = Instant::now();

        // Add new buffer event
        self.buffer_history.push_back(BufferEvent {
            timestamp: now,
            duration,
        });

        // Remove events older than 60s
        while let Some(first) = self.buffer_history.front() {
            if now.duration_since(first.timestamp) > Duration::from_secs(60) {
                self.buffer_history.pop_front();
            } else {
                break;
            }
        }

        // Update metrics
        self.playback_metrics.buffer_count = self.buffer_history.len() as u32;

        if !self.buffer_history.is_empty() {
            let total_duration: Duration = self.buffer_history.iter().map(|e| e.duration).sum();

            self.playback_metrics.average_buffer_duration =
                total_duration / self.buffer_history.len() as u32;

            self.playback_metrics.time_since_last_buffer =
                now.duration_since(self.buffer_history.back().unwrap().timestamp);
        }
    }

    /// Update playback health status
    fn update_playback_health(&mut self) {
        self.playback_metrics.health = if self.playback_metrics.playback_errors > 0 {
            PlaybackHealth::Failed
        } else if self.playback_metrics.buffer_count >= 3 {
            PlaybackHealth::Unstable
        } else if self.playback_metrics.buffer_count > 0 {
            PlaybackHealth::Buffering
        } else {
            PlaybackHealth::Healthy
        };
    }

    /// Update bandwidth metrics
    fn update_bandwidth_metrics(&mut self) {
        if self.speed_measurements.is_empty() {
            return;
        }

        // Current speed is most recent measurement
        self.bandwidth_metrics.current_speed_bps = self
            .speed_measurements
            .back()
            .map(|m| m.bytes_per_second)
            .unwrap_or(0);

        // Average speed over window
        let total: u64 = self
            .speed_measurements
            .iter()
            .map(|m| m.bytes_per_second)
            .sum();

        self.bandwidth_metrics.average_speed_bps = total / self.speed_measurements.len() as u64;

        // Estimate available bandwidth (conservative: 80% of average)
        self.bandwidth_metrics.estimated_available_bps =
            (self.bandwidth_metrics.average_speed_bps * 80) / 100;

        // Determine trend
        self.bandwidth_metrics.trend = self.calculate_bandwidth_trend();
    }

    /// Calculate bandwidth trend
    fn calculate_bandwidth_trend(&self) -> BandwidthTrend {
        if self.speed_measurements.len() < 3 {
            return BandwidthTrend::Stable;
        }

        // Compare recent half vs older half
        let mid = self.speed_measurements.len() / 2;
        let older_avg: u64 = self
            .speed_measurements
            .iter()
            .take(mid)
            .map(|m| m.bytes_per_second)
            .sum::<u64>()
            / mid as u64;

        let recent_avg: u64 = self
            .speed_measurements
            .iter()
            .skip(mid)
            .map(|m| m.bytes_per_second)
            .sum::<u64>()
            / (self.speed_measurements.len() - mid) as u64;

        // 20% threshold for trend detection
        let threshold = older_avg / 5;

        if recent_avg > older_avg + threshold {
            BandwidthTrend::Increasing
        } else if recent_avg < older_avg.saturating_sub(threshold) {
            BandwidthTrend::Decreasing
        } else {
            BandwidthTrend::Stable
        }
    }

    /// Evaluate current conditions and decide on quality change
    fn evaluate_quality(&mut self) -> Option<QualityDecision> {
        // Check cooldown period
        if let Some(last_change) = self.last_quality_change {
            if last_change.elapsed() < self.quality_change_cooldown {
                return None; // Too soon to change again
            }
        }

        let current_quality = &self.available_qualities[self.current_quality_index];

        // Emergency: Playback failed completely
        if matches!(self.playback_metrics.health, PlaybackHealth::Failed) {
            return self.emergency_recovery();
        }

        // Critical: Frequent buffering (unstable)
        if matches!(self.playback_metrics.health, PlaybackHealth::Unstable) {
            return self.decrease_quality_progressive();
        }

        // Check if current quality exceeds available bandwidth
        let required_bps = current_quality.bitrate;
        let available_bps = self.bandwidth_metrics.estimated_available_bps;

        if required_bps > available_bps {
            warn!(
                "Insufficient bandwidth: required {} Mbps, available {} Mbps",
                required_bps / 1_000_000,
                available_bps / 1_000_000
            );
            return self.decrease_quality_progressive();
        }

        // Opportunity: Bandwidth increasing and stable playback
        if matches!(
            self.bandwidth_metrics.trend,
            BandwidthTrend::Increasing | BandwidthTrend::Stable
        ) && matches!(self.playback_metrics.health, PlaybackHealth::Healthy)
            && self.playback_metrics.buffer_count == 0
        {
            return self.increase_quality_progressive(available_bps);
        }

        None // Maintain current quality
    }

    /// Emergency recovery: drop to lowest quality
    fn emergency_recovery(&mut self) -> Option<QualityDecision> {
        // Jump down significantly for recovery
        let target_index = if self.current_quality_index >= 2 {
            self.current_quality_index - 2 // Drop 2 levels
        } else {
            self.available_qualities.len() - 1 // Lowest quality
        };

        warn!(
            "Emergency recovery: switching to quality level {}",
            target_index
        );

        self.last_quality_change = Some(Instant::now());
        self.current_quality_index = target_index;

        Some(QualityDecision::Recover(
            self.available_qualities[target_index].clone(),
        ))
    }

    /// Progressively decrease quality by one level
    fn decrease_quality_progressive(&mut self) -> Option<QualityDecision> {
        // Can we go lower?
        if self.current_quality_index >= self.available_qualities.len() - 1 {
            warn!("Already at lowest quality, cannot decrease");
            return None;
        }

        // Drop one level
        self.current_quality_index += 1;
        self.last_quality_change = Some(Instant::now());

        info!(
            "Decreasing quality to level {} ({})",
            self.current_quality_index, self.available_qualities[self.current_quality_index].name
        );

        Some(QualityDecision::Decrease(
            self.available_qualities[self.current_quality_index].clone(),
        ))
    }

    /// Progressively increase quality by one level if bandwidth allows
    fn increase_quality_progressive(&mut self, available_bps: u64) -> Option<QualityDecision> {
        // Can we go higher?
        if self.current_quality_index == 0 {
            return None;
        }

        // Check if next higher quality fits bandwidth (with 20% headroom)
        let next_index = self.current_quality_index - 1;
        let next_quality = &self.available_qualities[next_index];

        let required_with_headroom = (next_quality.bitrate * 120) / 100;

        if available_bps >= required_with_headroom {
            self.current_quality_index = next_index;
            self.last_quality_change = Some(Instant::now());

            info!(
                "Increasing quality to level {} ({})",
                self.current_quality_index, next_quality.name
            );

            Some(QualityDecision::Increase(next_quality.clone()))
        } else {
            None
        }
    }

    /// Get current playback metrics
    pub fn playback_metrics(&self) -> &PlaybackMetrics {
        &self.playback_metrics
    }

    /// Get current bandwidth metrics
    pub fn bandwidth_metrics(&self) -> &BandwidthMetrics {
        &self.bandwidth_metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_quality_options() -> Vec<QualityOption> {
        vec![
            QualityOption {
                name: "1080p".to_string(),
                resolution: Resolution {
                    width: 1920,
                    height: 1080,
                },
                bitrate: 8_000_000,
                url: String::new(),
                requires_transcode: false,
            },
            QualityOption {
                name: "720p".to_string(),
                resolution: Resolution {
                    width: 1280,
                    height: 720,
                },
                bitrate: 4_000_000,
                url: String::new(),
                requires_transcode: true,
            },
            QualityOption {
                name: "480p".to_string(),
                resolution: Resolution {
                    width: 854,
                    height: 480,
                },
                bitrate: 2_000_000,
                url: String::new(),
                requires_transcode: true,
            },
        ]
    }

    #[test]
    fn test_bandwidth_trend_calculation() {
        let (_state_tx, state_rx) = mpsc::unbounded_channel();
        let (_bandwidth_tx, bandwidth_rx) = mpsc::unbounded_channel();
        let (decision_tx, _decision_rx) = mpsc::unbounded_channel();

        let mut manager = AdaptiveQualityManager::new(
            create_test_quality_options(),
            0,
            state_rx,
            bandwidth_rx,
            decision_tx,
        );

        // Add increasing measurements
        for i in 1..=10 {
            manager.handle_bandwidth_update(i * 1_000_000, Duration::from_secs(1));
        }

        assert_eq!(manager.bandwidth_metrics.trend, BandwidthTrend::Increasing);
    }

    #[test]
    fn test_emergency_recovery() {
        let (_state_tx, state_rx) = mpsc::unbounded_channel();
        let (_bandwidth_tx, bandwidth_rx) = mpsc::unbounded_channel();
        let (decision_tx, _decision_rx) = mpsc::unbounded_channel();

        let mut manager = AdaptiveQualityManager::new(
            create_test_quality_options(),
            0,
            state_rx,
            bandwidth_rx,
            decision_tx,
        );

        // Simulate playback failure
        manager.playback_metrics.health = PlaybackHealth::Failed;

        let decision = manager.emergency_recovery();
        assert!(matches!(decision, Some(QualityDecision::Recover(_))));
    }
}
