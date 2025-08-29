pub mod event_bus;
pub mod types;

pub use event_bus::{EventBus, EventFilter};
pub use types::{DatabaseEvent, EventPayload, EventType};

/// Event handler trait for processing events
#[async_trait::async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an event
    async fn handle(&self, event: DatabaseEvent) -> anyhow::Result<()>;

    /// Get the event types this handler is interested in
    fn subscribed_events(&self) -> Vec<String>;
}
