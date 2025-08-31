use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use super::AppState;

/// Platform-agnostic frontend trait
/// Each platform (GTK, macOS, etc.) will implement this trait
#[async_trait]
pub trait Frontend: Send + Sync {
    /// Initialize the frontend with the application state
    /// Using &mut self here allows implementations to store handles/state.
    async fn initialize(&mut self, app_state: Arc<AppState>) -> Result<()>;

    /// Run the frontend main loop
    async fn run(&self) -> Result<()>;

    /// Shutdown the frontend gracefully
    async fn shutdown(&self) -> Result<()>;

    /// Get the frontend name/identifier
    fn name(&self) -> &str;

    /// Check if the frontend is running
    fn is_running(&self) -> bool;
}
