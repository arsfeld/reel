use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use super::config::DynamicCacheLimit;

/// Statistics for the cache downloader
#[derive(Debug, Clone)]
pub struct DownloaderStats {
    /// Total number of downloads started
    pub downloads_started: Arc<AtomicU64>,
    /// Total number of downloads completed
    pub downloads_completed: Arc<AtomicU64>,
    /// Total number of downloads failed
    pub downloads_failed: Arc<AtomicU64>,
    /// Total bytes downloaded
    pub total_bytes_downloaded: Arc<AtomicU64>,
    /// Current number of active downloads
    pub active_downloads: Arc<AtomicU64>,
    /// Number of queued downloads
    pub queued_downloads: Arc<AtomicU64>,
    /// Start time for calculating uptime
    pub start_time: Instant,
}

impl DownloaderStats {
    pub fn new() -> Self {
        Self {
            downloads_started: Arc::new(AtomicU64::new(0)),
            downloads_completed: Arc::new(AtomicU64::new(0)),
            downloads_failed: Arc::new(AtomicU64::new(0)),
            total_bytes_downloaded: Arc::new(AtomicU64::new(0)),
            active_downloads: Arc::new(AtomicU64::new(0)),
            queued_downloads: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
        }
    }

    pub fn increment_started(&self) {
        self.downloads_started.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_completed(&self) {
        self.downloads_completed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_failed(&self) {
        self.downloads_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_bytes_downloaded(&self, bytes: u64) {
        self.total_bytes_downloaded
            .fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn set_active_downloads(&self, count: u64) {
        self.active_downloads.store(count, Ordering::Relaxed);
    }

    pub fn set_queued_downloads(&self, count: u64) {
        self.queued_downloads.store(count, Ordering::Relaxed);
    }

    pub fn format_report(
        &self,
        active_download_details: Vec<(String, u64, f64)>,
        cache_limit: Option<&DynamicCacheLimit>,
    ) -> String {
        let uptime_secs = self.start_time.elapsed().as_secs();
        let hours = uptime_secs / 3600;
        let minutes = (uptime_secs % 3600) / 60;
        let seconds = uptime_secs % 60;

        let total_mb =
            self.total_bytes_downloaded.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0);
        let avg_speed_mbps = if uptime_secs > 0 {
            total_mb / uptime_secs as f64
        } else {
            0.0
        };

        let mut report = format!(
            "📊 Downloader Stats [{}h {}m {}s] | Started: {} | Completed: {} | Failed: {} | Total: {:.1} MB | Avg: {:.2} MB/s",
            hours,
            minutes,
            seconds,
            self.downloads_started.load(Ordering::Relaxed),
            self.downloads_completed.load(Ordering::Relaxed),
            self.downloads_failed.load(Ordering::Relaxed),
            total_mb,
            avg_speed_mbps
        );

        let active = self.active_downloads.load(Ordering::Relaxed);
        let queued = self.queued_downloads.load(Ordering::Relaxed);

        if active > 0 || queued > 0 {
            report.push_str(&format!("\n   Active: {} | Queued: {}", active, queued));

            if !active_download_details.is_empty() {
                report.push_str("\n   Downloads:");
                for (name, speed_bps, progress) in active_download_details.iter().take(3) {
                    let speed_kbps = *speed_bps as f64 / 1024.0;
                    report.push_str(&format!(
                        "\n     • {} [{:.0}%] @ {:.1} KB/s",
                        name,
                        progress * 100.0,
                        speed_kbps
                    ));
                }
            }
        }

        // Add cache limit information if available
        if let Some(limit) = cache_limit {
            let effective_mb = limit.effective_limit_bytes / (1024 * 1024);
            let fixed_max_mb = limit.fixed_max_bytes / (1024 * 1024);
            let cleanup_threshold_mb = limit.cleanup_threshold_bytes / (1024 * 1024);

            if limit.is_limited_by_disk {
                report.push_str(&format!(
                    "\n   Cache Limit: {} MB (disk-limited, fixed max: {} MB) | Cleanup at: {} MB",
                    effective_mb, fixed_max_mb, cleanup_threshold_mb
                ));
            } else {
                report.push_str(&format!(
                    "\n   Cache Limit: {} MB (fixed max) | Cleanup at: {} MB",
                    effective_mb, cleanup_threshold_mb
                ));
            }
        }

        report
    }
}

/// Statistics for the cache proxy
#[derive(Debug, Clone)]
pub struct ProxyStats {
    /// Total number of requests served
    pub requests_served: Arc<AtomicU64>,
    /// Total number of cache hits
    pub cache_hits: Arc<AtomicU64>,
    /// Total number of cache misses
    pub cache_misses: Arc<AtomicU64>,
    /// Total bytes served
    pub bytes_served: Arc<AtomicU64>,
    /// Current number of active streams
    pub active_streams: Arc<AtomicU64>,
    /// Number of range requests
    pub range_requests: Arc<AtomicU64>,
    /// Number of full requests
    pub full_requests: Arc<AtomicU64>,
    /// Number of 503 errors returned
    pub service_unavailable_errors: Arc<AtomicU64>,
    /// Number of initial data timeouts
    pub initial_timeouts: Arc<AtomicU64>,
    /// Total time waited for initial data (in milliseconds)
    pub total_initial_wait_ms: Arc<AtomicU64>,
    /// Number of successful initial data waits
    pub successful_initial_waits: Arc<AtomicU64>,
    /// Start time for calculating uptime
    pub start_time: Instant,
}

impl ProxyStats {
    pub fn new() -> Self {
        Self {
            requests_served: Arc::new(AtomicU64::new(0)),
            cache_hits: Arc::new(AtomicU64::new(0)),
            cache_misses: Arc::new(AtomicU64::new(0)),
            bytes_served: Arc::new(AtomicU64::new(0)),
            active_streams: Arc::new(AtomicU64::new(0)),
            range_requests: Arc::new(AtomicU64::new(0)),
            full_requests: Arc::new(AtomicU64::new(0)),
            service_unavailable_errors: Arc::new(AtomicU64::new(0)),
            initial_timeouts: Arc::new(AtomicU64::new(0)),
            total_initial_wait_ms: Arc::new(AtomicU64::new(0)),
            successful_initial_waits: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
        }
    }

    pub fn increment_request(&self) {
        self.requests_served.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_bytes_served(&self, bytes: u64) {
        self.bytes_served.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn set_active_streams(&self, count: u64) {
        self.active_streams.store(count, Ordering::Relaxed);
    }

    pub fn increment_range_request(&self) {
        self.range_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_full_request(&self) {
        self.full_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_service_unavailable(&self) {
        self.service_unavailable_errors
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_initial_timeout(&self) {
        self.initial_timeouts.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_initial_wait(&self, wait_ms: u64) {
        self.total_initial_wait_ms
            .fetch_add(wait_ms, Ordering::Relaxed);
        self.successful_initial_waits
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn format_report(&self) -> String {
        let uptime_secs = self.start_time.elapsed().as_secs();
        let hours = uptime_secs / 3600;
        let minutes = (uptime_secs % 3600) / 60;
        let seconds = uptime_secs % 60;

        let hits = self.cache_hits.load(Ordering::Relaxed);
        let misses = self.cache_misses.load(Ordering::Relaxed);
        let total_requests = hits + misses;
        let hit_rate = if total_requests > 0 {
            (hits as f64 / total_requests as f64) * 100.0
        } else {
            0.0
        };

        let total_gb =
            self.bytes_served.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0 * 1024.0);
        let avg_speed_mbps = if uptime_secs > 0 {
            (total_gb * 1024.0) / uptime_secs as f64
        } else {
            0.0
        };

        let service_unavail = self.service_unavailable_errors.load(Ordering::Relaxed);
        let initial_timeouts = self.initial_timeouts.load(Ordering::Relaxed);
        let successful_waits = self.successful_initial_waits.load(Ordering::Relaxed);
        let avg_initial_wait_ms = if successful_waits > 0 {
            self.total_initial_wait_ms.load(Ordering::Relaxed) as f64 / successful_waits as f64
        } else {
            0.0
        };

        let mut report = format!(
            "🌐 Proxy Stats [{}h {}m {}s] | Requests: {} | Hit Rate: {:.1}% | Active: {} | Served: {:.2} GB | Avg: {:.2} MB/s | Range: {} | Full: {}",
            hours,
            minutes,
            seconds,
            self.requests_served.load(Ordering::Relaxed),
            hit_rate,
            self.active_streams.load(Ordering::Relaxed),
            total_gb,
            avg_speed_mbps,
            self.range_requests.load(Ordering::Relaxed),
            self.full_requests.load(Ordering::Relaxed)
        );

        if service_unavail > 0 || initial_timeouts > 0 {
            report.push_str(&format!(
                "\n   503 Errors: {} | Init Timeouts: {} | Avg Init Wait: {:.0}ms",
                service_unavail, initial_timeouts, avg_initial_wait_ms
            ));
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downloader_stats_format() {
        let stats = DownloaderStats::new();

        // Set some test values
        stats.increment_started();
        stats.increment_started();
        stats.increment_completed();
        stats.increment_failed();
        stats.add_bytes_downloaded(1024 * 1024 * 10); // 10 MB
        stats.set_active_downloads(1);
        stats.set_queued_downloads(2);

        let active_details = vec![
            ("source1:media1".to_string(), 1024 * 100, 0.5),
            ("source2:media2".to_string(), 1024 * 200, 0.75),
        ];

        let report = stats.format_report(active_details, None);

        // Check that report contains expected elements
        assert!(report.contains("Started: 2"));
        assert!(report.contains("Completed: 1"));
        assert!(report.contains("Failed: 1"));
        assert!(report.contains("Total: 10.0 MB"));
        assert!(report.contains("Active: 1"));
        assert!(report.contains("Queued: 2"));
        assert!(report.contains("source1:media1"));
        assert!(report.contains("[50%]"));
    }

    #[test]
    fn test_proxy_stats_format() {
        let stats = ProxyStats::new();

        // Set some test values
        stats.increment_request();
        stats.increment_request();
        stats.increment_request();
        stats.increment_cache_hit();
        stats.increment_cache_hit();
        stats.increment_cache_miss();
        stats.add_bytes_served(1024 * 1024 * 1024 * 2); // 2 GB
        stats.set_active_streams(3);
        stats.increment_range_request();
        stats.increment_full_request();

        let report = stats.format_report();

        // Check that report contains expected elements
        assert!(report.contains("Requests: 3"));
        assert!(report.contains("Hit Rate: 66.7%"));
        assert!(report.contains("Active: 3"));
        assert!(report.contains("Served: 2.00 GB"));
        assert!(report.contains("Range: 1"));
        assert!(report.contains("Full: 1"));
    }

    #[test]
    fn test_stats_atomic_operations() {
        let stats = DownloaderStats::new();

        // Test atomic increments
        for _ in 0..10 {
            stats.increment_started();
        }
        assert_eq!(stats.downloads_started.load(Ordering::Relaxed), 10);

        // Test atomic byte counter
        stats.add_bytes_downloaded(100);
        stats.add_bytes_downloaded(200);
        assert_eq!(stats.total_bytes_downloaded.load(Ordering::Relaxed), 300);

        // Test atomic set operations
        stats.set_active_downloads(5);
        assert_eq!(stats.active_downloads.load(Ordering::Relaxed), 5);
    }
}
