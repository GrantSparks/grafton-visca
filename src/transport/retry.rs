//! Generic retry executor for transport operations.
//!
//! This module provides a unified retry mechanism that can be used across
//! all transport implementations, both blocking and async.

use crate::Error;
#[cfg(feature = "async")]
use std::future::Future;
use std::time::{Duration, Instant};

use super::RetryConfig;

/// A trait for operations that can provide retry hints.
pub trait RetryableOperation {
    /// Check if the operation should be retried based on the error.
    fn should_retry(&self, error: &Error, attempt: u32, start_time: Instant) -> bool;

    /// Get the suggested delay before retrying based on the error.
    fn retry_delay(&self, error: &Error, attempt: u32) -> Option<Duration>;
}

/// Default implementation of RetryableOperation using RetryConfig.
#[derive(Debug, Clone, Copy)]
pub struct DefaultRetryStrategy {
    config: RetryConfig,
}

impl DefaultRetryStrategy {
    /// Create a new retry strategy with the given configuration.
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }

    /// Create a retry strategy with default configuration.
    pub fn default() -> Self {
        Self::new(RetryConfig::default())
    }
}

impl RetryableOperation for DefaultRetryStrategy {
    fn should_retry(&self, error: &Error, attempt: u32, start_time: Instant) -> bool {
        // Check if error is retryable
        if !error.is_retryable() {
            return false;
        }

        // Check retry limits
        self.config.should_retry(attempt, start_time)
    }

    fn retry_delay(&self, error: &Error, attempt: u32) -> Option<Duration> {
        if !error.is_retryable() {
            return None;
        }

        // Use error's suggested delay if available, otherwise use config
        let suggested_delay = error.suggested_retry_delay();
        Some(self.config.calculate_delay(attempt, suggested_delay))
    }
}

/// Execute a blocking operation with retry logic.
///
/// This function will retry the operation based on the retry configuration
/// and the error type returned by the operation.
///
/// # Arguments
/// * `config` - The retry configuration to use
/// * `operation` - A closure that performs the operation and returns a Result
///
/// # Returns
/// The result of the operation, or the last error if all retries are exhausted
pub fn execute_with_retry<T, F>(config: &RetryConfig, mut operation: F) -> Result<T, Error>
where
    F: FnMut() -> Result<T, Error>,
{
    let start_time = Instant::now();
    let mut attempts = 0;

    loop {
        // Track attempts (initial + retries)
        attempts += 1;

        match operation() {
            Ok(result) => return Ok(result),
            Err(error) => {
                // Check if error is retryable
                if !error.is_retryable() {
                    return Err(error);
                }

                // Calculate how many retries we've done (attempts - 1)
                let retries_done = attempts - 1;

                // Check if we've exceeded retry limits
                if retries_done >= config.max_retries {
                    return Err(error);
                }

                // Check if we've exceeded max duration
                if start_time.elapsed() >= config.max_retry_duration {
                    return Err(error);
                }

                // Calculate delay for this retry attempt
                let delay = config.calculate_delay(retries_done, error.suggested_retry_delay());

                // Wait before retrying
                std::thread::sleep(delay);
            }
        }
    }
}

/// Execute an async operation with retry logic.
///
/// This function will retry the operation based on the retry configuration
/// and the error type returned by the operation.
///
/// # Arguments
/// * `config` - The retry configuration to use
/// * `operation` - An async closure that performs the operation and returns a Result
///
/// # Returns
/// The result of the operation, or the last error if all retries are exhausted
#[cfg(feature = "async")]
pub async fn execute_with_retry_async<T, F, Fut>(
    config: &RetryConfig,
    mut operation: F,
) -> Result<T, Error>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, Error>>,
{
    let start_time = Instant::now();
    let mut attempts = 0;

    loop {
        // Track attempts (initial + retries)
        attempts += 1;

        match operation().await {
            Ok(result) => return Ok(result),
            Err(error) => {
                // Check if error is retryable
                if !error.is_retryable() {
                    return Err(error);
                }

                // Calculate how many retries we've done (attempts - 1)
                let retries_done = attempts - 1;

                // Check if we've exceeded retry limits
                if retries_done >= config.max_retries {
                    return Err(error);
                }

                // Check if we've exceeded max duration
                if start_time.elapsed() >= config.max_retry_duration {
                    return Err(error);
                }

                // Calculate delay for this retry attempt
                let delay = config.calculate_delay(retries_done, error.suggested_retry_delay());

                // Wait before retrying
                #[cfg(feature = "rt-tokio")]
                tokio::time::sleep(delay).await;

                #[cfg(all(feature = "async", not(feature = "rt-tokio")))]
                {
                    // For async without tokio, we need to use a different sleep mechanism
                    // This is a placeholder - actual implementation would depend on the async runtime
                    use std::pin::Pin;
                    use std::task::Poll;

                    struct Sleep {
                        deadline: Instant,
                    }

                    impl Future for Sleep {
                        type Output = ();

                        fn poll(
                            self: Pin<&mut Self>,
                            _cx: &mut std::task::Context<'_>,
                        ) -> Poll<Self::Output> {
                            if Instant::now() >= self.deadline {
                                Poll::Ready(())
                            } else {
                                Poll::Pending
                            }
                        }
                    }

                    Sleep {
                        deadline: Instant::now() + delay,
                    }
                    .await;
                }
            }
        }
    }
}

/// A wrapper that adds retry logic to any transport operation.
///
/// This can be used to wrap existing transport methods with retry capability.
#[derive(Debug, Clone, Copy)]
pub struct RetryExecutor {
    config: RetryConfig,
}

impl RetryExecutor {
    /// Create a new retry executor with the given configuration.
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }

    /// Create a retry executor with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(RetryConfig::default())
    }

    /// Execute a blocking operation with retry logic.
    pub fn execute<T, F>(&self, operation: F) -> Result<T, Error>
    where
        F: FnMut() -> Result<T, Error>,
    {
        execute_with_retry(&self.config, operation)
    }

    /// Execute an async operation with retry logic.
    #[cfg(feature = "async")]
    pub async fn execute_async<T, F, Fut>(&self, operation: F) -> Result<T, Error>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, Error>>,
    {
        execute_with_retry_async(&self.config, operation).await
    }

    /// Get the current retry configuration.
    pub fn config(&self) -> &RetryConfig {
        &self.config
    }

    /// Update the retry configuration.
    pub fn set_config(&mut self, config: RetryConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_successful_operation_no_retry() {
        let config = RetryConfig::default();
        let mut calls = 0;

        let result = execute_with_retry(&config, || {
            calls += 1;
            Ok::<_, Error>(42)
        });

        assert_eq!(result.unwrap(), 42);
        assert_eq!(calls, 1);
    }

    #[test]
    fn test_non_retryable_error_no_retry() {
        let config = RetryConfig::default();
        let mut calls = 0;

        let result = execute_with_retry(&config, || {
            calls += 1;
            Err::<i32, _>(Error::SyntaxError)
        });

        assert!(matches!(result, Err(Error::SyntaxError)));
        assert_eq!(calls, 1);
    }

    #[test]
    fn test_retryable_error_with_eventual_success() {
        let config = RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_secs(1),
            exponential_backoff: false,
        };

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let result = execute_with_retry(&config, || {
            let count = counter_clone.fetch_add(1, Ordering::SeqCst);
            if count < 2 {
                Err(Error::CameraBusy)
            } else {
                Ok(100)
            }
        });

        assert_eq!(result.unwrap(), 100);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_max_retries_exceeded() {
        let config = RetryConfig {
            max_retries: 2,
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_secs(10), // Increased to allow for Error::Timeout's 2-second suggested delay
            exponential_backoff: false,
        };

        let mut calls = 0;

        let result = execute_with_retry(&config, || {
            calls += 1;
            Err::<i32, _>(Error::Timeout)
        });

        assert!(matches!(result, Err(Error::Timeout)));
        assert_eq!(calls, 3); // Initial + 2 retries
    }

    #[test]
    fn test_retry_executor() {
        let executor = RetryExecutor::with_defaults();
        let mut calls = 0;

        let result = executor.execute(|| {
            calls += 1;
            if calls < 2 {
                Err(Error::CommandBufferFull)
            } else {
                Ok("success")
            }
        });

        assert_eq!(result.unwrap(), "success");
        assert_eq!(calls, 2);
    }

    #[cfg(feature = "rt-tokio")]
    #[tokio::test]
    async fn test_async_retry_with_success() {
        let config = RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_secs(1),
            exponential_backoff: false,
        };

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let result = execute_with_retry_async(&config, || {
            let counter = counter_clone.clone();
            async move {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(Error::CameraBusy)
                } else {
                    Ok(200)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 200);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }
}
