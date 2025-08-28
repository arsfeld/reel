use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SyncStrategy {
    // Sync intervals
    pub full_sync_interval: Duration,        // Default: 24 hours
    pub incremental_sync_interval: Duration, // Default: 1 hour
    pub on_demand_sync: bool,                // Sync when opening library

    // Network conditions
    pub wifi_only: bool,                 // Only sync on WiFi
    pub metered_connection_limit: usize, // MB limit on metered connections

    // Content strategy
    pub auto_download_next_episodes: bool,
    pub keep_watched_items_days: u32,
    pub max_offline_storage_gb: u32,
}

impl Default for SyncStrategy {
    fn default() -> Self {
        Self {
            full_sync_interval: Duration::from_secs(86400), // 24 hours
            incremental_sync_interval: Duration::from_secs(3600), // 1 hour
            on_demand_sync: true,
            wifi_only: false,
            metered_connection_limit: 100, // 100 MB
            auto_download_next_episodes: true,
            keep_watched_items_days: 7,
            max_offline_storage_gb: 10,
        }
    }
}

impl SyncStrategy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_wifi_only(mut self) -> Self {
        self.wifi_only = true;
        self
    }

    pub fn with_intervals(mut self, full: Duration, incremental: Duration) -> Self {
        self.full_sync_interval = full;
        self.incremental_sync_interval = incremental;
        self
    }

    pub fn with_storage_limit(mut self, gb: u32) -> Self {
        self.max_offline_storage_gb = gb;
        self
    }

    pub fn should_sync_now(&self, last_sync: Option<chrono::DateTime<chrono::Utc>>) -> bool {
        match last_sync {
            None => true, // Never synced before
            Some(last) => {
                let elapsed = chrono::Utc::now() - last;
                elapsed.to_std().unwrap_or(Duration::from_secs(0)) > self.incremental_sync_interval
            }
        }
    }

    pub fn should_full_sync(&self, last_full_sync: Option<chrono::DateTime<chrono::Utc>>) -> bool {
        match last_full_sync {
            None => true, // Never done a full sync
            Some(last) => {
                let elapsed = chrono::Utc::now() - last;
                elapsed.to_std().unwrap_or(Duration::from_secs(0)) > self.full_sync_interval
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, TimeDelta, Utc};

    #[test]
    fn test_default_values() {
        let strategy = SyncStrategy::default();

        assert_eq!(strategy.full_sync_interval, Duration::from_secs(86400)); // 24 hours
        assert_eq!(
            strategy.incremental_sync_interval,
            Duration::from_secs(3600)
        ); // 1 hour
        assert!(strategy.on_demand_sync);
        assert!(!strategy.wifi_only);
        assert_eq!(strategy.metered_connection_limit, 100);
        assert!(strategy.auto_download_next_episodes);
        assert_eq!(strategy.keep_watched_items_days, 7);
        assert_eq!(strategy.max_offline_storage_gb, 10);
    }

    #[test]
    fn test_new_same_as_default() {
        let strategy_new = SyncStrategy::new();
        let strategy_default = SyncStrategy::default();

        assert_eq!(
            strategy_new.full_sync_interval,
            strategy_default.full_sync_interval
        );
        assert_eq!(
            strategy_new.incremental_sync_interval,
            strategy_default.incremental_sync_interval
        );
        assert_eq!(strategy_new.on_demand_sync, strategy_default.on_demand_sync);
        assert_eq!(strategy_new.wifi_only, strategy_default.wifi_only);
    }

    #[test]
    fn test_with_wifi_only() {
        let strategy = SyncStrategy::new().with_wifi_only();

        assert!(strategy.wifi_only);
        // Other values should remain default
        assert_eq!(strategy.full_sync_interval, Duration::from_secs(86400));
        assert_eq!(
            strategy.incremental_sync_interval,
            Duration::from_secs(3600)
        );
    }

    #[test]
    fn test_with_intervals() {
        let full_interval = Duration::from_secs(7200); // 2 hours
        let incremental_interval = Duration::from_secs(600); // 10 minutes

        let strategy = SyncStrategy::new().with_intervals(full_interval, incremental_interval);

        assert_eq!(strategy.full_sync_interval, full_interval);
        assert_eq!(strategy.incremental_sync_interval, incremental_interval);
    }

    #[test]
    fn test_with_storage_limit() {
        let strategy = SyncStrategy::new().with_storage_limit(50);

        assert_eq!(strategy.max_offline_storage_gb, 50);
        // Other values should remain default
        assert_eq!(strategy.full_sync_interval, Duration::from_secs(86400));
    }

    #[test]
    fn test_chaining_builders() {
        let strategy = SyncStrategy::new()
            .with_wifi_only()
            .with_storage_limit(25)
            .with_intervals(Duration::from_secs(3600), Duration::from_secs(300));

        assert!(strategy.wifi_only);
        assert_eq!(strategy.max_offline_storage_gb, 25);
        assert_eq!(strategy.full_sync_interval, Duration::from_secs(3600));
        assert_eq!(strategy.incremental_sync_interval, Duration::from_secs(300));
    }

    #[test]
    fn test_should_sync_now_never_synced() {
        let strategy = SyncStrategy::new();

        assert!(strategy.should_sync_now(None));
    }

    #[test]
    fn test_should_sync_now_recent_sync() {
        let strategy =
            SyncStrategy::new().with_intervals(Duration::from_secs(3600), Duration::from_secs(600));

        // Last sync was 5 minutes ago
        let last_sync = Utc::now() - TimeDelta::seconds(300);

        // Should not sync (interval is 10 minutes)
        assert!(!strategy.should_sync_now(Some(last_sync)));
    }

    #[test]
    fn test_should_sync_now_old_sync() {
        let strategy =
            SyncStrategy::new().with_intervals(Duration::from_secs(3600), Duration::from_secs(600));

        // Last sync was 20 minutes ago
        let last_sync = Utc::now() - TimeDelta::seconds(1200);

        // Should sync (interval is 10 minutes)
        assert!(strategy.should_sync_now(Some(last_sync)));
    }

    #[test]
    fn test_should_sync_now_exactly_at_interval() {
        let strategy =
            SyncStrategy::new().with_intervals(Duration::from_secs(3600), Duration::from_secs(600));

        // Last sync was exactly 10 minutes ago (minus a small buffer to account for execution time)
        let last_sync = Utc::now() - TimeDelta::seconds(599);

        // Should not sync (needs to be greater than interval)
        assert!(!strategy.should_sync_now(Some(last_sync)));
    }

    #[test]
    fn test_should_full_sync_never_synced() {
        let strategy = SyncStrategy::new();

        assert!(strategy.should_full_sync(None));
    }

    #[test]
    fn test_should_full_sync_recent() {
        let strategy =
            SyncStrategy::new().with_intervals(Duration::from_secs(3600), Duration::from_secs(600));

        // Last full sync was 30 minutes ago
        let last_sync = Utc::now() - TimeDelta::seconds(1800);

        // Should not full sync (interval is 1 hour)
        assert!(!strategy.should_full_sync(Some(last_sync)));
    }

    #[test]
    fn test_should_full_sync_old() {
        let strategy =
            SyncStrategy::new().with_intervals(Duration::from_secs(3600), Duration::from_secs(600));

        // Last full sync was 2 hours ago
        let last_sync = Utc::now() - TimeDelta::seconds(7200);

        // Should full sync (interval is 1 hour)
        assert!(strategy.should_full_sync(Some(last_sync)));
    }

    #[test]
    fn test_should_full_sync_exactly_at_interval() {
        let strategy =
            SyncStrategy::new().with_intervals(Duration::from_secs(3600), Duration::from_secs(600));

        // Last full sync was exactly 1 hour ago (minus a small buffer to account for execution time)
        let last_sync = Utc::now() - TimeDelta::seconds(3599);

        // Should not full sync (needs to be greater than interval)
        assert!(!strategy.should_full_sync(Some(last_sync)));
    }

    #[test]
    fn test_sync_intervals_edge_case_future_time() {
        let strategy = SyncStrategy::new();

        // Future time (shouldn't happen in practice, but testing edge case)
        let future_sync = Utc::now() + TimeDelta::seconds(3600);

        // Should not sync if last sync is in the future
        assert!(!strategy.should_sync_now(Some(future_sync)));
        assert!(!strategy.should_full_sync(Some(future_sync)));
    }

    #[test]
    fn test_all_fields_custom() {
        let mut strategy = SyncStrategy::new();
        strategy.full_sync_interval = Duration::from_secs(7200);
        strategy.incremental_sync_interval = Duration::from_secs(300);
        strategy.on_demand_sync = false;
        strategy.wifi_only = true;
        strategy.metered_connection_limit = 50;
        strategy.auto_download_next_episodes = false;
        strategy.keep_watched_items_days = 14;
        strategy.max_offline_storage_gb = 20;

        assert_eq!(strategy.full_sync_interval, Duration::from_secs(7200));
        assert_eq!(strategy.incremental_sync_interval, Duration::from_secs(300));
        assert!(!strategy.on_demand_sync);
        assert!(strategy.wifi_only);
        assert_eq!(strategy.metered_connection_limit, 50);
        assert!(!strategy.auto_download_next_episodes);
        assert_eq!(strategy.keep_watched_items_days, 14);
        assert_eq!(strategy.max_offline_storage_gb, 20);
    }
}
