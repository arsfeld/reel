use super::types::{DatabaseEvent, EventPriority, EventType};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tracing::trace;

/// Event subscriber handle
pub struct EventSubscriber {
    receiver: broadcast::Receiver<DatabaseEvent>,
    filter: Option<EventFilter>,
}

impl EventSubscriber {
    /// Create a new subscriber with an optional filter
    pub fn new(receiver: broadcast::Receiver<DatabaseEvent>, filter: Option<EventFilter>) -> Self {
        Self { receiver, filter }
    }

    /// Receive the next event matching the filter
    pub async fn recv(&mut self) -> Result<DatabaseEvent> {
        loop {
            let event = self.receiver.recv().await?;

            // Check if event matches filter
            if let Some(ref filter) = self.filter {
                if filter.matches(&event) {
                    return Ok(event);
                }
            } else {
                return Ok(event);
            }
        }
    }

    /// Try to receive without blocking
    pub fn try_recv(&mut self) -> Result<Option<DatabaseEvent>> {
        loop {
            match self.receiver.try_recv() {
                Ok(event) => {
                    if let Some(ref filter) = self.filter {
                        if filter.matches(&event) {
                            return Ok(Some(event));
                        }
                        // Continue to next event
                    } else {
                        return Ok(Some(event));
                    }
                }
                Err(broadcast::error::TryRecvError::Empty) => return Ok(None),
                Err(e) => return Err(e.into()),
            }
        }
    }
}

/// Event filter for selective subscription
#[derive(Debug, Clone)]
pub struct EventFilter {
    event_types: Option<Vec<EventType>>,
    sources: Option<Vec<String>>,
    min_priority: Option<EventPriority>,
}

impl Default for EventFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl EventFilter {
    pub fn new() -> Self {
        Self {
            event_types: None,
            sources: None,
            min_priority: None,
        }
    }

    pub fn with_types(mut self, types: Vec<EventType>) -> Self {
        self.event_types = Some(types);
        self
    }

    pub fn with_sources(mut self, sources: Vec<String>) -> Self {
        self.sources = Some(sources);
        self
    }

    pub fn with_min_priority(mut self, priority: EventPriority) -> Self {
        self.min_priority = Some(priority);
        self
    }

    pub fn matches(&self, event: &DatabaseEvent) -> bool {
        // Check event type
        if let Some(ref types) = self.event_types
            && !types.contains(&event.event_type)
        {
            return false;
        }

        // Check source
        if let Some(ref sources) = self.sources {
            let event_source = format!("{:?}", event.source);
            if !sources.iter().any(|s| event_source.contains(s)) {
                return false;
            }
        }

        // Check priority
        if let Some(min_priority) = self.min_priority
            && event.priority < min_priority
        {
            return false;
        }

        true
    }
}

/// Main event bus for broadcasting database events
#[derive(Debug)]
pub struct EventBus {
    sender: broadcast::Sender<DatabaseEvent>,
    stats: Arc<RwLock<EventBusStats>>,
    event_history: Arc<RwLock<Vec<DatabaseEvent>>>,
    max_history_size: usize,
}

#[derive(Debug, Default)]
pub struct EventBusStats {
    total_events: u64,
    events_by_type: HashMap<String, u64>,

    subscriber_count: usize,
    dropped_events: u64,
}

impl EventBus {
    /// Create a new event bus with specified buffer capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);

        Self {
            sender,
            stats: Arc::new(RwLock::new(EventBusStats::default())),
            event_history: Arc::new(RwLock::new(Vec::new())),
            max_history_size: 100, // Keep last 100 events for debugging
        }
    }

    /// Publish an event to all subscribers
    pub async fn publish(&self, event: DatabaseEvent) -> Result<()> {
        trace!(
            "Publishing event: {:?} with priority {:?}",
            event.event_type, event.priority
        );

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_events += 1;
            let event_type_str = event.event_type.as_str().to_string();
            *stats.events_by_type.entry(event_type_str).or_insert(0) += 1;
        }

        // Add to history
        {
            let mut history = self.event_history.write().await;
            history.push(event.clone());

            // Trim history if needed
            if history.len() > self.max_history_size {
                let excess = history.len() - self.max_history_size;
                history.drain(0..excess);
            }
        }

        // Send event
        match self.sender.send(event) {
            Ok(_count) => {
                // Successfully sent
                Ok(())
            }
            Err(_) => {
                // No subscribers is normal, don't log
                let mut stats = self.stats.write().await;
                stats.dropped_events += 1;
                Ok(()) // Don't fail if no subscribers
            }
        }
    }

    /// Subscribe to all events
    pub fn subscribe(&self) -> EventSubscriber {
        EventSubscriber::new(self.sender.subscribe(), None)
    }

    /// Subscribe with a filter
    pub fn subscribe_filtered(&self, filter: EventFilter) -> EventSubscriber {
        EventSubscriber::new(self.sender.subscribe(), Some(filter))
    }

    /// Subscribe to specific event types
    pub fn subscribe_to_types(&self, types: Vec<EventType>) -> EventSubscriber {
        let filter = EventFilter::new().with_types(types);
        self.subscribe_filtered(filter)
    }

    /// Get current subscriber count
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Get event bus statistics
    pub async fn get_stats(&self) -> EventBusStats {
        let stats = self.stats.read().await;
        EventBusStats {
            total_events: stats.total_events,
            events_by_type: stats.events_by_type.clone(),
            subscriber_count: self.subscriber_count(),
            dropped_events: stats.dropped_events,
        }
    }

    /// Get event history for debugging
    pub async fn get_history(&self) -> Vec<DatabaseEvent> {
        self.event_history.read().await.clone()
    }

    /// Clear event history
    pub async fn clear_history(&self) {
        self.event_history.write().await.clear();
    }

    /// Emit a media created event
    pub async fn emit_media_created(
        &self,
        id: String,
        media_type: String,
        library_id: String,
        source_id: String,
    ) -> Result<()> {
        let event = DatabaseEvent::new(
            EventType::MediaCreated,
            super::types::EventPayload::Media {
                id,
                media_type,
                library_id,
                source_id,
            },
        );
        self.publish(event).await
    }

    /// Emit a sync started event
    pub async fn emit_sync_started(&self, source_id: String, sync_type: String) -> Result<()> {
        let event = DatabaseEvent::new(
            EventType::SyncStarted,
            super::types::EventPayload::Sync {
                source_id,
                sync_type,
                progress: Some(0.0),
                items_synced: None,
                error: None,
            },
        );
        self.publish(event).await
    }

    /// Emit a sync progress event
    pub async fn emit_sync_progress(
        &self,
        source_id: String,
        sync_type: String,
        progress: f32,
        items_synced: usize,
    ) -> Result<()> {
        let event = DatabaseEvent::new(
            EventType::SyncProgress,
            super::types::EventPayload::Sync {
                source_id,
                sync_type,
                progress: Some(progress),
                items_synced: Some(items_synced),
                error: None,
            },
        );
        self.publish(event).await
    }

    /// Emit a sync completed event
    pub async fn emit_sync_completed(
        &self,
        source_id: String,
        sync_type: String,
        items_synced: usize,
    ) -> Result<()> {
        let event = DatabaseEvent::new(
            EventType::SyncCompleted,
            super::types::EventPayload::Sync {
                source_id,
                sync_type,
                progress: Some(100.0),
                items_synced: Some(items_synced),
                error: None,
            },
        );
        self.publish(event).await
    }

    /// Emit a playback position updated event
    pub async fn emit_playback_position(
        &self,
        media_id: String,
        position: std::time::Duration,
        duration: std::time::Duration,
    ) -> Result<()> {
        let event = DatabaseEvent::new(
            EventType::PlaybackPositionUpdated,
            super::types::EventPayload::Playback {
                media_id,
                position: Some(position),
                duration: Some(duration),
            },
        );
        self.publish(event).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus_publish_subscribe() {
        let bus = EventBus::new(10);
        let mut subscriber = bus.subscribe();

        // Publish an event
        bus.emit_media_created(
            "media1".to_string(),
            "movie".to_string(),
            "lib1".to_string(),
            "source1".to_string(),
        )
        .await
        .unwrap();

        // Receive the event
        let event = subscriber.recv().await.unwrap();
        assert_eq!(event.event_type, EventType::MediaCreated);
    }

    #[tokio::test]
    async fn test_event_filter() {
        let bus = EventBus::new(10);

        // Subscribe only to sync events
        let mut sync_subscriber =
            bus.subscribe_to_types(vec![EventType::SyncStarted, EventType::SyncCompleted]);

        // Publish various events
        bus.emit_media_created(
            "media1".to_string(),
            "movie".to_string(),
            "lib1".to_string(),
            "source1".to_string(),
        )
        .await
        .unwrap();

        bus.emit_sync_started("source1".to_string(), "full".to_string())
            .await
            .unwrap();

        // Should only receive sync event
        let event = sync_subscriber.recv().await.unwrap();
        assert_eq!(event.event_type, EventType::SyncStarted);
    }

    #[tokio::test]
    async fn test_event_history() {
        let bus = EventBus::new(10);

        // Publish some events
        for i in 0..5 {
            bus.emit_media_created(
                format!("media{}", i),
                "movie".to_string(),
                "lib1".to_string(),
                "source1".to_string(),
            )
            .await
            .unwrap();
        }

        // Check history
        let history = bus.get_history().await;
        assert_eq!(history.len(), 5);
    }

    #[tokio::test]
    async fn test_event_stats() {
        let bus = EventBus::new(10);

        // Publish various events
        bus.emit_sync_started("source1".to_string(), "full".to_string())
            .await
            .unwrap();
        bus.emit_sync_completed("source1".to_string(), "full".to_string(), 100)
            .await
            .unwrap();

        // Check stats
        let stats = bus.get_stats().await;
        assert_eq!(stats.total_events, 2);
        assert_eq!(stats.events_by_type.get("sync.started"), Some(&1));
        assert_eq!(stats.events_by_type.get("sync.completed"), Some(&1));
    }
}
