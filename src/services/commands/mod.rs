pub mod auth_commands;
pub mod media_commands;
pub mod sync_commands;

// Re-export commonly used commands
pub use media_commands::{
    GetContinueWatchingCommand, GetLibrariesCommand, GetMediaItemCommand, GetMediaItemsCommand,
    GetPlaybackProgressCommand, GetRecentlyAddedCommand, UpdatePlaybackProgressCommand,
};

use anyhow::Result;
use async_trait::async_trait;

/// Base trait for all commands
#[async_trait]
pub trait Command<T>: Send + Sync {
    /// Execute the command and return the result
    async fn execute(&self) -> Result<T>;
}

/// Result wrapper for command execution
#[derive(Debug)]
pub struct CommandResult<T> {
    pub data: T,
    pub execution_time_ms: u64,
}

impl<T> CommandResult<T> {
    pub fn new(data: T, execution_time_ms: u64) -> Self {
        Self {
            data,
            execution_time_ms,
        }
    }
}

/// Command executor with metrics and error handling
pub struct CommandExecutor;

impl CommandExecutor {
    /// Execute a command with timing and error handling
    pub async fn execute<T>(command: &dyn Command<T>) -> Result<CommandResult<T>> {
        let start = std::time::Instant::now();
        let result = command.execute().await?;
        let execution_time_ms = start.elapsed().as_millis() as u64;

        Ok(CommandResult::new(result, execution_time_ms))
    }
}
