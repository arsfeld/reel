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
    Loading { source: String },
    LoadComplete { source: String },
    LoadError { source: String, error: String },
    MediaUpdated { media_id: String },
    LibraryUpdated { library_id: String },
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
    Connected { source_id: String },
    Disconnected { source_id: String },
    SyncStarted { source_id: String },
    SyncCompleted { source_id: String },
    SyncError { source_id: String, error: String },
}

pub struct MessageBroker {
    subscribers: Arc<RwLock<HashMap<String, Vec<Sender<BrokerMessage>>>>>,
}

impl MessageBroker {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn subscribe(&self, component_id: String, sender: Sender<BrokerMessage>) {
        let mut subs = self.subscribers.write().await;
        subs.entry(component_id)
            .or_insert_with(Vec::new)
            .push(sender);
    }

    pub async fn unsubscribe(&self, component_id: &str) {
        let mut subs = self.subscribers.write().await;
        subs.remove(component_id);
    }

    pub async fn broadcast(&self, message: BrokerMessage) {
        let subs = self.subscribers.read().await;
        for senders in subs.values() {
            for sender in senders {
                let _ = sender.send(message.clone());
            }
        }
    }

    pub async fn send_to(&self, component_id: &str, message: BrokerMessage) {
        let subs = self.subscribers.read().await;
        if let Some(senders) = subs.get(component_id) {
            for sender in senders {
                let _ = sender.send(message.clone());
            }
        }
    }
}

impl Default for MessageBroker {
    fn default() -> Self {
        Self::new()
    }
}

lazy_static::lazy_static! {
    pub static ref BROKER: MessageBroker = MessageBroker::new();
}
