pub mod auth_commands;
pub mod media_commands;

// Re-export commonly used commands
pub use media_commands::{GetPlaybackProgressCommand, UpdatePlaybackProgressCommand};

use anyhow::Result;
use async_trait::async_trait;

/// Base trait for all commands
#[async_trait]
pub trait Command<T>: Send + Sync {
    /// Execute the command and return the result
    async fn execute(&self) -> Result<T>;
}
