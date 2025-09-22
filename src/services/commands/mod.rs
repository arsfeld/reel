pub mod auth_commands;
pub mod media_commands;
pub mod sync_commands;

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

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    struct SuccessfulCommand {
        value: String,
    }

    #[async_trait]
    impl Command<String> for SuccessfulCommand {
        async fn execute(&self) -> Result<String> {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            Ok(self.value.clone())
        }
    }

    struct FailingCommand {
        error_message: String,
    }

    #[async_trait]
    impl Command<String> for FailingCommand {
        async fn execute(&self) -> Result<String> {
            Err(anyhow!(self.error_message.clone()))
        }
    }

    #[tokio::test]
    async fn test_command_executor_success() {
        let command = SuccessfulCommand {
            value: "test_result".to_string(),
        };

        let result = CommandExecutor::execute(&command).await;
        assert!(result.is_ok());

        let command_result = result.unwrap();
        assert_eq!(command_result.data, "test_result");
        assert!(command_result.execution_time_ms >= 10);
    }

    #[tokio::test]
    async fn test_command_executor_failure() {
        let command = FailingCommand {
            error_message: "Command failed".to_string(),
        };

        let result = CommandExecutor::execute(&command).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Command failed");
    }

    #[tokio::test]
    async fn test_command_result_creation() {
        let result = CommandResult::new("test_data".to_string(), 42);
        assert_eq!(result.data, "test_data");
        assert_eq!(result.execution_time_ms, 42);
    }
}
