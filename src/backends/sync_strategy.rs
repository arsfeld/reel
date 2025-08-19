use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SyncStrategy {
    // Sync intervals
    pub full_sync_interval: Duration,       // Default: 24 hours
    pub incremental_sync_interval: Duration, // Default: 1 hour
    pub on_demand_sync: bool,               // Sync when opening library
    
    // Network conditions
    pub wifi_only: bool,                    // Only sync on WiFi
    pub metered_connection_limit: usize,    // MB limit on metered connections
    
    // Content strategy
    pub auto_download_next_episodes: bool,
    pub keep_watched_items_days: u32,
    pub max_offline_storage_gb: u32,
}

impl Default for SyncStrategy {
    fn default() -> Self {
        Self {
            full_sync_interval: Duration::from_secs(86400),      // 24 hours
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