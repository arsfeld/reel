//! MessageBroker System
//!
//! This module provides a centralized message passing system for the Relm4 UI.
//!
//! ## Usage Patterns
//!
//! ### Subscribing to Messages
//! ```ignore
//! // In component's AsyncComponentParts::init()
//! let broker_sender = sender.clone();
//! relm4::spawn(async move {
//!     BROKER.subscribe("ComponentName".to_string(), broker_sender).await;
//! });
//! ```
//!
//! ### Broadcasting Messages
//! ```ignore
//! // From any async context
//! BROKER.broadcast(BrokerMessage::Config(ConfigMessage::Updated { config })).await;
//! ```
//!
//! ### Handling Messages in Components
//! ```ignore
//! // In component's Input enum
//! BrokerMsg(BrokerMessage),
//!
//! // In AsyncComponentParts::update()
//! Input::BrokerMsg(msg) => match msg {
//!     BrokerMessage::Config(ConfigMessage::Updated { config }) => {
//!         // Handle config update
//!     }
//!     _ => {}
//! }
//! ```

use relm4::Sender;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub enum BrokerMessage {
    Data(DataMessage),
    Source(SourceMessage),
    Config(ConfigMessage),
    Cache(CacheMessage),
}

#[derive(Debug, Clone)]
pub enum DataMessage {
    Loading {
        source: String,
    },
    LoadComplete {
        source: String,
    },
    LoadError {
        source: String,
        error: String,
    },
    MediaUpdated {
        media_id: String,
    },
    MediaBatchSaved {
        items: Vec<crate::db::entities::MediaItemModel>,
    },
    LibraryUpdated {
        library_id: String,
    },
    SyncProgress {
        source_id: String,
        current: usize,
        total: usize,
    },
}

#[derive(Debug, Clone)]
pub enum SourceMessage {
    SyncStarted {
        source_id: String,
        total_items: Option<usize>,
    },
    SyncProgress {
        source_id: String,
        library_id: Option<String>,
        current: usize,
        total: usize,
    },
    SyncCompleted {
        source_id: String,
        items_synced: usize,
    },
    SyncError {
        source_id: String,
        error: String,
    },
    LibrarySyncStarted {
        source_id: String,
        library_id: String,
        library_name: String,
    },
    LibrarySyncCompleted {
        source_id: String,
        library_id: String,
        library_name: String,
        items_synced: usize,
    },
}

#[derive(Debug, Clone)]
pub enum ConfigMessage {
    Updated { config: Arc<crate::config::Config> },
    PlayerBackendChanged { backend: String },
}

#[derive(Debug, Clone)]
pub enum CacheMessage {
    CleanupStarted,
    CleanupCompleted {
        entries_removed: u64,
        space_freed_mb: i64,
        duration_ms: u128,
        cleanup_type: String,
    },
    CleanupFailed {
        error: String,
    },
}

pub struct MessageBroker {
    subscribers: Arc<RwLock<HashMap<String, Vec<Sender<BrokerMessage>>>>>,
}

impl MessageBroker {
    pub fn new() -> Self {
        tracing::info!("Initializing MessageBroker");
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn subscribe(&self, component_id: String, sender: Sender<BrokerMessage>) {
        tracing::debug!("Component '{}' subscribing to MessageBroker", component_id);
        let mut subs = self.subscribers.write().await;
        subs.entry(component_id.clone())
            .or_insert_with(Vec::new)
            .push(sender);
        tracing::info!(
            "Component '{}' subscribed. Total subscribers: {}",
            component_id,
            subs.len()
        );
    }

    pub async fn unsubscribe(&self, component_id: &str) {
        tracing::debug!(
            "Component '{}' unsubscribing from MessageBroker",
            component_id
        );
        let mut subs = self.subscribers.write().await;
        if subs.remove(component_id).is_some() {
            tracing::info!(
                "Component '{}' unsubscribed. Remaining subscribers: {}",
                component_id,
                subs.len()
            );
        } else {
            tracing::warn!("Component '{}' was not subscribed", component_id);
        }
    }

    pub async fn broadcast(&self, message: BrokerMessage) {
        let subs = self.subscribers.read().await;
        let _subscriber_count = subs.len();
        let mut _send_count = 0;
        for senders in subs.values() {
            for sender in senders {
                if sender.send(message.clone()).is_ok() {
                    _send_count += 1;
                }
            }
        }
    }
}

impl Default for MessageBroker {
    fn default() -> Self {
        Self::new()
    }
}

use once_cell::sync::Lazy;

pub static BROKER: Lazy<MessageBroker> = Lazy::new(|| {
    tracing::info!("Initializing global MessageBroker instance");
    MessageBroker::new()
});
