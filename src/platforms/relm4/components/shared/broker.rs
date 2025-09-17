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
//! BROKER.notify_sync_started(source_id, total_items).await;
//! BROKER.notify_sync_progress(source_id, current, total).await;
//! BROKER.notify_sync_completed(source_id, items_synced).await;
//! ```
//!
//! ### Handling Messages in Components
//! ```ignore
//! // In component's Input enum
//! BrokerMsg(BrokerMessage),
//!
//! // In AsyncComponentParts::update()
//! Input::BrokerMsg(msg) => match msg {
//!     BrokerMessage::Source(SourceMessage::SyncStarted { .. }) => {
//!         // Handle sync started
//!     }
//!     _ => {}
//! }
//! ```

use relm4::Sender;
use relm4::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub enum BrokerMessage {
    Navigation(NavigationMessage),
    Data(DataMessage),
    Playback(PlaybackMessage),
    Source(SourceMessage),
}

#[derive(Debug, Clone)]
pub enum NavigationMessage {
    ToHome,
    ToLibrary {
        source_id: String,
        library_id: String,
    },
    ToMovieDetails {
        media_id: String,
    },
    ToShowDetails {
        media_id: String,
    },
    ToPlayer {
        media_id: String,
    },
    ToSources,
    Back,
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
pub enum PlaybackMessage {
    Play { media_id: String },
    Pause,
    Stop,
    Seek { position: f64 },
    ProgressUpdate { media_id: String, position: f64 },
}

#[derive(Debug, Clone)]
pub enum SourceMessage {
    Connected {
        source_id: String,
    },
    Disconnected {
        source_id: String,
    },
    SyncStarted {
        source_id: String,
        total_items: Option<usize>,
    },
    SyncProgress {
        source_id: String,
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
        tracing::debug!("Broadcasting message: {:?}", message);
        let subs = self.subscribers.read().await;
        let subscriber_count = subs.len();
        let mut send_count = 0;
        for senders in subs.values() {
            for sender in senders {
                if sender.send(message.clone()).is_ok() {
                    send_count += 1;
                }
            }
        }
        tracing::debug!(
            "Broadcast complete. Sent to {}/{} subscribers",
            send_count,
            subscriber_count
        );
    }

    pub async fn send_to(&self, component_id: &str, message: BrokerMessage) {
        tracing::debug!(
            "Sending message to component '{}': {:?}",
            component_id,
            message
        );
        let subs = self.subscribers.read().await;
        if let Some(senders) = subs.get(component_id) {
            for sender in senders {
                if sender.send(message.clone()).is_err() {
                    tracing::warn!("Failed to send message to component '{}'", component_id);
                }
            }
        } else {
            tracing::warn!("Component '{}' not found in subscribers", component_id);
        }
    }

    // Helper method to send sync started notification
    pub async fn notify_sync_started(&self, source_id: String, total_items: Option<usize>) {
        self.broadcast(BrokerMessage::Source(SourceMessage::SyncStarted {
            source_id,
            total_items,
        }))
        .await;
    }

    // Helper method to send sync progress
    pub async fn notify_sync_progress(&self, source_id: String, current: usize, total: usize) {
        self.broadcast(BrokerMessage::Source(SourceMessage::SyncProgress {
            source_id: source_id.clone(),
            current,
            total,
        }))
        .await;

        // Also send as DataMessage for UI components that track data loading
        self.broadcast(BrokerMessage::Data(DataMessage::SyncProgress {
            source_id,
            current,
            total,
        }))
        .await;
    }

    // Helper method to send sync completed notification
    pub async fn notify_sync_completed(&self, source_id: String, items_synced: usize) {
        self.broadcast(BrokerMessage::Source(SourceMessage::SyncCompleted {
            source_id: source_id.clone(),
            items_synced,
        }))
        .await;

        // Also notify data load complete
        self.broadcast(BrokerMessage::Data(DataMessage::LoadComplete {
            source: source_id,
        }))
        .await;
    }

    // Helper method to send sync error notification
    pub async fn notify_sync_error(&self, source_id: String, error: String) {
        self.broadcast(BrokerMessage::Source(SourceMessage::SyncError {
            source_id: source_id.clone(),
            error: error.clone(),
        }))
        .await;

        // Also notify data load error
        self.broadcast(BrokerMessage::Data(DataMessage::LoadError {
            source: source_id,
            error,
        }))
        .await;
    }

    // Helper method to notify data loading started
    pub async fn notify_loading_started(&self, source: String) {
        self.broadcast(BrokerMessage::Data(DataMessage::Loading { source }))
            .await;
    }

    // Helper method to notify media updated
    pub async fn notify_media_updated(&self, media_id: String) {
        self.broadcast(BrokerMessage::Data(DataMessage::MediaUpdated { media_id }))
            .await;
    }

    // Helper method to notify library updated
    pub async fn notify_library_updated(&self, library_id: String) {
        self.broadcast(BrokerMessage::Data(DataMessage::LibraryUpdated {
            library_id,
        }))
        .await;
    }

    // Helper method to notify library sync started
    pub async fn notify_library_sync_started(
        &self,
        source_id: String,
        library_id: String,
        library_name: String,
    ) {
        self.broadcast(BrokerMessage::Source(SourceMessage::LibrarySyncStarted {
            source_id,
            library_id,
            library_name,
        }))
        .await;
    }

    // Helper method to notify library sync completed
    pub async fn notify_library_sync_completed(
        &self,
        source_id: String,
        library_id: String,
        library_name: String,
        items_synced: usize,
    ) {
        self.broadcast(BrokerMessage::Source(SourceMessage::LibrarySyncCompleted {
            source_id,
            library_id,
            library_name,
            items_synced,
        }))
        .await;
    }

    // Get subscriber count for monitoring
    pub async fn subscriber_count(&self) -> usize {
        self.subscribers.read().await.len()
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
