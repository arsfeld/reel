use crate::models::{ServerConnection, SourceId};
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct ConnectionState {
    pub url: String,
    pub connection_type: ConnectionType,
    pub last_tested: Instant,
    pub response_time_ms: u64,
    pub failure_count: u32,
    pub next_check: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionType {
    Local,
    Remote,
    Relay,
}

impl ConnectionType {
    pub fn from_connection(conn: &ServerConnection) -> Self {
        if conn.local {
            ConnectionType::Local
        } else if conn.relay {
            ConnectionType::Relay
        } else {
            ConnectionType::Remote
        }
    }

    pub fn check_interval(&self) -> Duration {
        match self {
            ConnectionType::Local => Duration::from_secs(300), // 5 minutes
            ConnectionType::Remote => Duration::from_secs(120), // 2 minutes
            ConnectionType::Relay => Duration::from_secs(30),  // 30 seconds
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            ConnectionType::Local => "local".to_string(),
            ConnectionType::Remote => "remote".to_string(),
            ConnectionType::Relay => "relay".to_string(),
        }
    }
}

impl ConnectionState {
    pub fn new(url: String, connection_type: ConnectionType, response_time_ms: u64) -> Self {
        let now = Instant::now();
        let next_check = now + connection_type.check_interval();

        Self {
            url,
            connection_type,
            last_tested: now,
            response_time_ms,
            failure_count: 0,
            next_check,
        }
    }

    pub fn is_local(&self) -> bool {
        matches!(self.connection_type, ConnectionType::Local)
    }

    pub fn age(&self) -> Duration {
        Instant::now().duration_since(self.last_tested)
    }

    pub fn needs_recheck(&self) -> bool {
        Instant::now() >= self.next_check
    }

    pub fn mark_success(&mut self, response_time_ms: u64) {
        self.last_tested = Instant::now();
        self.response_time_ms = response_time_ms;
        self.failure_count = 0;
        self.next_check = self.last_tested + self.connection_type.check_interval();
    }

    pub fn mark_failure(&mut self) {
        self.failure_count += 1;
        self.last_tested = Instant::now();

        // Exponential backoff on failures, capped at 10 minutes
        let backoff_seconds = std::cmp::min(30 * (2_u64.pow(self.failure_count)), 600);
        self.next_check = self.last_tested + Duration::from_secs(backoff_seconds);
    }
}

pub struct ConnectionCache {
    states: Arc<RwLock<LruCache<SourceId, ConnectionState>>>,
}

impl ConnectionCache {
    pub fn new() -> Self {
        let cache_size = NonZeroUsize::new(100).unwrap();
        Self {
            states: Arc::new(RwLock::new(LruCache::new(cache_size))),
        }
    }

    pub async fn get(&self, source_id: &SourceId) -> Option<ConnectionState> {
        let cache = self.states.read().await;
        cache.peek(source_id).cloned()
    }

    pub async fn insert(&self, source_id: SourceId, state: ConnectionState) {
        let mut cache = self.states.write().await;
        debug!(
            "Caching connection for {}: {} ({:?}, {}ms)",
            source_id, state.url, state.connection_type, state.response_time_ms
        );
        cache.put(source_id, state);
    }

    pub async fn update_success(&self, source_id: &SourceId, response_time_ms: u64) {
        let mut cache = self.states.write().await;
        if let Some(state) = cache.get_mut(source_id) {
            state.mark_success(response_time_ms);
            info!(
                "Connection success for {}: {}ms, next check in {:?}",
                source_id,
                response_time_ms,
                state.next_check.duration_since(Instant::now())
            );
        }
    }

    pub async fn update_failure(&self, source_id: &SourceId) {
        let mut cache = self.states.write().await;
        if let Some(state) = cache.get_mut(source_id) {
            state.mark_failure();
            info!(
                "Connection failure for {} (count: {}), next check in {:?}",
                source_id,
                state.failure_count,
                state.next_check.duration_since(Instant::now())
            );
        }
    }

    pub async fn should_skip_test(&self, source_id: &SourceId) -> bool {
        if let Some(state) = self.get(source_id).await {
            // Skip test if:
            // 1. Connection was tested recently and is still within TTL
            // 2. We're in backoff period due to failures
            !state.needs_recheck()
        } else {
            false
        }
    }

    pub async fn clear(&self) {
        let mut cache = self.states.write().await;
        cache.clear();
    }

    pub async fn remove(&self, source_id: &SourceId) {
        let mut cache = self.states.write().await;
        cache.pop(source_id);
    }
}

impl Default for ConnectionCache {
    fn default() -> Self {
        Self::new()
    }
}
