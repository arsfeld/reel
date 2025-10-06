use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, warn};

use super::errors::PlexApiError;

/// Configuration for retry behavior with exponential backoff
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts (not including the initial attempt)
    pub max_attempts: u32,
    /// Base delay for exponential backoff (first retry uses this delay)
    pub base_delay_ms: u64,
    /// Maximum delay between retries (caps exponential growth)
    pub max_delay_ms: u64,
    /// Total timeout for all retries combined
    pub total_timeout: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,                        // Total of 4 attempts (1 initial + 3 retries)
            base_delay_ms: 100,                     // Start with 100ms delay
            max_delay_ms: 10_000,                   // Cap at 10 seconds
            total_timeout: Duration::from_secs(30), // Total timeout of 30 seconds
        }
    }
}

impl RetryPolicy {
    /// Create a new retry policy with custom settings
    pub fn new(max_attempts: u32, base_delay_ms: u64, max_delay_ms: u64) -> Self {
        Self {
            max_attempts,
            base_delay_ms,
            max_delay_ms,
            total_timeout: Duration::from_secs(30),
        }
    }

    /// Calculate the delay for a given attempt number using exponential backoff
    ///
    /// Formula: min(base_delay * 2^attempt, max_delay)
    fn calculate_delay(&self, attempt: u32) -> Duration {
        let delay_ms = self
            .base_delay_ms
            .saturating_mul(2_u64.saturating_pow(attempt))
            .min(self.max_delay_ms);
        Duration::from_millis(delay_ms)
    }

    /// Execute an async operation with retry logic and exponential backoff
    ///
    /// # Arguments
    /// * `operation_name` - Name of the operation for logging
    /// * `f` - Async closure that performs the operation
    ///
    /// # Returns
    /// Result of the operation or the last error encountered
    pub async fn execute<F, Fut, T>(
        &self,
        operation_name: &str,
        mut f: F,
    ) -> Result<T, PlexApiError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, PlexApiError>>,
    {
        let start_time = std::time::Instant::now();
        let mut last_error = None;

        for attempt in 0..=self.max_attempts {
            // Check if we've exceeded total timeout
            if start_time.elapsed() >= self.total_timeout {
                warn!(
                    "{}: Exceeded total timeout of {:?} after {} attempts",
                    operation_name, self.total_timeout, attempt
                );
                break;
            }

            // Log attempt
            if attempt == 0 {
                debug!("{}: Attempting request", operation_name);
            } else {
                debug!(
                    "{}: Retry attempt {} of {}",
                    operation_name, attempt, self.max_attempts
                );
            }

            // Execute the operation
            match f().await {
                Ok(result) => {
                    if attempt > 0 {
                        debug!("{}: Succeeded after {} retries", operation_name, attempt);
                    }
                    return Ok(result);
                }
                Err(err) => {
                    // Check if this error should be retried
                    if !err.is_transient() {
                        warn!(
                            "{}: Non-transient error, not retrying: {}",
                            operation_name, err
                        );
                        return Err(err);
                    }

                    // Store the error for potential return
                    debug!("{}: Transient error: {}", operation_name, err);
                    last_error = Some(err.clone());

                    // Don't sleep after the last attempt
                    if attempt < self.max_attempts {
                        // Calculate delay with special handling for rate limits
                        let delay = if let Some(retry_after) = err.retry_after() {
                            // Use server-specified retry-after if available
                            let server_delay = Duration::from_secs(retry_after);
                            debug!(
                                "{}: Rate limited, waiting {}s as requested by server",
                                operation_name, retry_after
                            );
                            server_delay.min(Duration::from_millis(self.max_delay_ms))
                        } else {
                            // Use exponential backoff
                            self.calculate_delay(attempt)
                        };

                        debug!(
                            "{}: Waiting {:?} before retry {} of {}",
                            operation_name,
                            delay,
                            attempt + 1,
                            self.max_attempts
                        );
                        sleep(delay).await;
                    }
                }
            }
        }

        // All attempts failed, return the last error
        if let Some(err) = last_error {
            warn!(
                "{}: All {} attempts failed, last error: {}",
                operation_name,
                self.max_attempts + 1,
                err
            );
            Err(err)
        } else {
            // This shouldn't happen, but handle it gracefully
            Err(PlexApiError::Other(format!(
                "{}: All attempts failed with no error captured",
                operation_name
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[test]
    fn test_calculate_delay() {
        let policy = RetryPolicy::default();

        // First retry: base_delay * 2^0 = 100ms
        assert_eq!(policy.calculate_delay(0), Duration::from_millis(100));

        // Second retry: base_delay * 2^1 = 200ms
        assert_eq!(policy.calculate_delay(1), Duration::from_millis(200));

        // Third retry: base_delay * 2^2 = 400ms
        assert_eq!(policy.calculate_delay(2), Duration::from_millis(400));

        // Fourth retry: base_delay * 2^3 = 800ms
        assert_eq!(policy.calculate_delay(3), Duration::from_millis(800));
    }

    #[test]
    fn test_max_delay_cap() {
        let policy = RetryPolicy {
            base_delay_ms: 1000,
            max_delay_ms: 5000,
            ..Default::default()
        };

        // Should cap at max_delay_ms
        assert_eq!(policy.calculate_delay(10), Duration::from_millis(5000));
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        let policy = RetryPolicy::default();
        let attempt_count = Arc::new(Mutex::new(0));

        let result = policy
            .execute("test_operation", || {
                let count = Arc::clone(&attempt_count);
                async move {
                    let mut count_guard = count.lock().await;
                    *count_guard += 1;
                    let current_attempt = *count_guard;
                    drop(count_guard);

                    if current_attempt < 3 {
                        Err(PlexApiError::Network("Connection refused".to_string()))
                    } else {
                        Ok("success")
                    }
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(*attempt_count.lock().await, 3);
    }

    #[tokio::test]
    async fn test_no_retry_on_permanent_error() {
        let policy = RetryPolicy::default();
        let attempt_count = Arc::new(Mutex::new(0));

        let result: Result<&str, PlexApiError> = policy
            .execute("test_operation", || {
                let count = Arc::clone(&attempt_count);
                async move {
                    let mut count_guard = count.lock().await;
                    *count_guard += 1;
                    drop(count_guard);

                    Err(PlexApiError::Authentication {
                        status: 401,
                        message: "Unauthorized".to_string(),
                    })
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(*attempt_count.lock().await, 1); // Should not retry
    }

    #[tokio::test]
    async fn test_max_attempts_respected() {
        let policy = RetryPolicy {
            max_attempts: 2,
            base_delay_ms: 1, // Use very short delays for testing
            ..Default::default()
        };
        let attempt_count = Arc::new(Mutex::new(0));

        let result: Result<&str, PlexApiError> = policy
            .execute("test_operation", || {
                let count = Arc::clone(&attempt_count);
                async move {
                    let mut count_guard = count.lock().await;
                    *count_guard += 1;
                    drop(count_guard);

                    Err(PlexApiError::Network("Connection refused".to_string()))
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(*attempt_count.lock().await, 3); // 1 initial + 2 retries
    }
}
