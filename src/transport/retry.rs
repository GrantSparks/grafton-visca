//! Generic retry executor for transport operations.
//!
//! This module provides a unified retry mechanism that can be used across
//! all transport implementations, both blocking and async.

#[cfg(feature = "mode-async")]
use std::future::Future;

use std::time::{Duration, Instant};

use super::RetryConfig;
#[cfg(feature = "mode-async")]
use crate::executor::Executor;
use crate::{
    timeout::{CommandTimeout, Deadline, TimeoutPolicy},
    Error,
};

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
    pub fn with_defaults() -> Self {
        Self::new(RetryConfig::default())
    }
}

impl RetryableOperation for DefaultRetryStrategy {
    fn should_retry(&self, error: &Error, attempt: u32, start_time: Instant) -> bool {
        if !error.is_retryable() {
            return false;
        }
        self.config.should_retry(attempt, start_time)
    }

    fn retry_delay(&self, error: &Error, attempt: u32) -> Option<Duration> {
        if !error.is_retryable() {
            return None;
        }

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
        attempts += 1;

        match operation() {
            Ok(result) => return Ok(result),
            Err(error) => {
                if !error.is_retryable() {
                    return Err(error);
                }

                let retries_done = attempts - 1;

                if retries_done >= config.max_retries {
                    return Err(error);
                }

                if start_time.elapsed() >= config.max_retry_duration {
                    return Err(error);
                }

                let delay = config.calculate_delay(retries_done, error.suggested_retry_delay());

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
/// * `executor` - The executor to use for timing operations
/// * `config` - The retry configuration to use
/// * `operation` - An async closure that performs the operation and returns a Result
///
/// # Returns
/// The result of the operation, or the last error if all retries are exhausted
#[cfg(feature = "mode-async")]
pub async fn execute_with_retry_async<E, T, F, Fut>(
    executor: &E,
    config: &RetryConfig,
    mut operation: F,
) -> Result<T, Error>
where
    E: Executor,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, Error>>,
{
    let start_time = Instant::now();
    let mut attempts = 0;

    loop {
        attempts += 1;

        match operation().await {
            Ok(result) => return Ok(result),
            Err(error) => {
                if !error.is_retryable() {
                    return Err(error);
                }

                let retries_done = attempts - 1;

                if retries_done >= config.max_retries {
                    return Err(error);
                }

                if start_time.elapsed() >= config.max_retry_duration {
                    return Err(error);
                }

                let delay = config.calculate_delay(retries_done, error.suggested_retry_delay());

                executor.sleep(delay).await;
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
    #[cfg(feature = "mode-async")]
    pub async fn execute_async<E, T, F, Fut>(&self, executor: &E, operation: F) -> Result<T, Error>
    where
        E: Executor,
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, Error>>,
    {
        execute_with_retry_async(executor, &self.config, operation).await
    }

    /// Get the current retry configuration.
    pub fn config(&self) -> &RetryConfig {
        &self.config
    }

    /// Update the retry configuration.
    pub fn set_config(&mut self, config: RetryConfig) {
        self.config = config;
    }

    /// Execute a command with timeout-aware retry logic.
    ///
    /// This method automatically uses the command's timeout class to determine
    /// appropriate timeout and retry behavior.
    pub fn execute_command<C, T, F>(
        &self,
        command: &C,
        timeout_policy: &TimeoutPolicy,
        operation: F,
    ) -> Result<T, Error>
    where
        C: CommandTimeout,
        F: FnMut() -> Result<T, Error>,
    {
        execute_command_with_retry(command, timeout_policy, &self.config, operation)
    }

    /// Execute a command with timeout-aware async retry logic.
    ///
    /// This method automatically uses the command's timeout class to determine
    /// appropriate timeout and retry behavior.
    #[cfg(feature = "mode-async")]
    pub async fn execute_command_async<E, C, T, F, Fut>(
        &self,
        executor: &E,
        command: &C,
        timeout_policy: &TimeoutPolicy,
        operation: F,
    ) -> Result<T, Error>
    where
        E: Executor,
        C: CommandTimeout,
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, Error>>,
    {
        execute_command_with_retry_async(executor, command, timeout_policy, &self.config, operation)
            .await
    }

    /// Execute an operation with deadline-based retry logic.
    ///
    /// This method uses a pre-calculated deadline instead of command classification.
    pub fn execute_with_deadline<T, F>(&self, deadline: Deadline, operation: F) -> Result<T, Error>
    where
        F: FnMut() -> Result<T, Error>,
    {
        execute_with_deadline_retry(deadline, &self.config, operation)
    }

    /// Execute an operation with deadline-based async retry logic.
    ///
    /// This method uses a pre-calculated deadline instead of command classification.
    #[cfg(feature = "mode-async")]
    pub async fn execute_with_deadline_async<E, T, F, Fut>(
        &self,
        executor: &E,
        deadline: Deadline,
        operation: F,
    ) -> Result<T, Error>
    where
        E: Executor,
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, Error>>,
    {
        execute_with_deadline_retry_async(executor, deadline, &self.config, operation).await
    }
}

/// Execute a blocking operation with timeout-aware retry logic.
///
/// This function combines command timeout classification with retry logic,
/// using the command's timeout class to determine appropriate timeout and retry behavior.
///
/// # Arguments
/// * `command` - The command being executed (for timeout classification)
/// * `timeout_policy` - The timeout policy to use for creating deadlines
/// * `retry_config` - The retry configuration to use
/// * `operation` - A closure that performs the operation and returns a Result
///
/// # Returns
/// The result of the operation, or the last error if all retries are exhausted
pub fn execute_command_with_retry<C, T, F>(
    command: &C,
    timeout_policy: &TimeoutPolicy,
    retry_config: &RetryConfig,
    operation: F,
) -> Result<T, Error>
where
    C: CommandTimeout,
    F: FnMut() -> Result<T, Error>,
{
    let timeout_class = command.timeout_class();
    let deadline = timeout_policy.deadline_for(timeout_class);

    execute_with_deadline_retry(deadline, retry_config, operation)
}

/// Execute a blocking operation with retry logic using a pre-calculated deadline.
///
/// This function uses a `Deadline` to determine when to stop retrying,
/// providing more precise timeout control than duration-based retry.
///
/// # Arguments
/// * `deadline` - The deadline for the overall operation
/// * `retry_config` - The retry configuration to use
/// * `operation` - A closure that performs the operation and returns a Result
///
/// # Returns
/// The result of the operation, or the last error if all retries are exhausted
pub fn execute_with_deadline_retry<T, F>(
    deadline: Deadline,
    retry_config: &RetryConfig,
    operation: F,
) -> Result<T, Error>
where
    F: FnMut() -> Result<T, Error>,
{
    let start_time = Instant::now();
    let mut attempts = 0;
    let mut operation = operation;

    loop {
        if deadline.is_expired() {
            return Err(Error::Timeout);
        }

        attempts += 1;

        match operation() {
            Ok(result) => return Ok(result),
            Err(error) => {
                if !error.is_retryable() {
                    return Err(error);
                }

                let retries_done = attempts - 1;

                if retries_done >= retry_config.max_retries {
                    return Err(error);
                }

                if deadline.is_expired() || start_time.elapsed() >= retry_config.max_retry_duration
                {
                    return Err(error);
                }

                let retry_delay =
                    retry_config.calculate_delay(retries_done, error.suggested_retry_delay());
                let remaining_time = deadline.remaining();
                let actual_delay = retry_delay.min(remaining_time);

                if actual_delay.is_zero() {
                    return Err(error);
                }

                std::thread::sleep(actual_delay);
            }
        }
    }
}

/// Execute an async operation with timeout-aware retry logic.
///
/// This function combines command timeout classification with async retry logic,
/// using the command's timeout class to determine appropriate timeout and retry behavior.
///
/// # Arguments
/// * `executor` - The executor to use for timing operations
/// * `command` - The command being executed (for timeout classification)
/// * `timeout_policy` - The timeout policy to use for creating deadlines
/// * `retry_config` - The retry configuration to use
/// * `operation` - An async closure that performs the operation and returns a Result
///
/// # Returns
/// The result of the operation, or the last error if all retries are exhausted
#[cfg(feature = "mode-async")]
pub async fn execute_command_with_retry_async<E, C, T, F, Fut>(
    executor: &E,
    command: &C,
    timeout_policy: &TimeoutPolicy,
    retry_config: &RetryConfig,
    operation: F,
) -> Result<T, Error>
where
    E: Executor,
    C: CommandTimeout,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, Error>>,
{
    let timeout_class = command.timeout_class();
    let deadline = timeout_policy.deadline_for(timeout_class);

    execute_with_deadline_retry_async(executor, deadline, retry_config, operation).await
}

/// Execute an async operation with retry logic using a pre-calculated deadline.
///
/// This function uses a `Deadline` to determine when to stop retrying,
/// providing more precise timeout control than duration-based retry.
///
/// # Arguments
/// * `executor` - The executor to use for timing operations
/// * `deadline` - The deadline for the overall operation
/// * `retry_config` - The retry configuration to use
/// * `operation` - An async closure that performs the operation and returns a Result
///
/// # Returns
/// The result of the operation, or the last error if all retries are exhausted
#[cfg(feature = "mode-async")]
pub async fn execute_with_deadline_retry_async<E, T, F, Fut>(
    executor: &E,
    deadline: Deadline,
    retry_config: &RetryConfig,
    mut operation: F,
) -> Result<T, Error>
where
    E: Executor,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, Error>>,
{
    let start_time = Instant::now();
    let mut attempts = 0;

    loop {
        // Check if we've exceeded the overall deadline
        if deadline.is_expired() {
            return Err(Error::Timeout);
        }

        attempts += 1;

        match operation().await {
            Ok(result) => return Ok(result),
            Err(error) => {
                if !error.is_retryable() {
                    return Err(error);
                }

                let retries_done = attempts - 1;

                if retries_done >= retry_config.max_retries {
                    return Err(error);
                }

                if deadline.is_expired() || start_time.elapsed() >= retry_config.max_retry_duration
                {
                    return Err(error);
                }

                let retry_delay =
                    retry_config.calculate_delay(retries_done, error.suggested_retry_delay());
                let remaining_time = deadline.remaining();
                let actual_delay = retry_delay.min(remaining_time);

                if actual_delay.is_zero() {
                    return Err(error);
                }

                executor.sleep(actual_delay).await;
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use super::*;

    #[test]
    fn test_successful_operation_no_retry() {
        let config = RetryConfig::default();
        let mut calls = 0;

        let result = execute_with_retry(&config, || {
            calls += 1;
            Ok::<_, Error>(42)
        });

        assert_eq!(result.expect("Test operation failed"), 42);
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

        assert_eq!(
            result.expect("Test operation should succeed after retries"),
            100
        );
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

        assert_eq!(result.expect("Async test operation failed"), "success");
        assert_eq!(calls, 2);
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn test_async_retry_with_success() {
        use crate::executor::TokioExecutor;

        let config = RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_secs(1),
            exponential_backoff: false,
        };

        let executor = TokioExecutor::from_current().expect("Failed to get Tokio executor");
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let result = execute_with_retry_async(&executor, &config, || {
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

        assert_eq!(result.expect("Async executor test failed"), 200);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[cfg(feature = "runtime-async-std")]
    #[async_std::test]
    async fn test_async_retry_with_async_std() {
        use crate::executor::AsyncStdExecutor;

        let config = RetryConfig {
            max_retries: 2,
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_secs(1),
            exponential_backoff: false,
        };

        let executor = AsyncStdExecutor::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let result = execute_with_retry_async(&executor, &config, || {
            let counter = counter_clone.clone();
            async move {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 1 {
                    Err(Error::CameraBusy)
                } else {
                    Ok(100)
                }
            }
        })
        .await;

        assert_eq!(result.expect("AsyncStd executor test failed"), 100);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn test_async_retry_with_smol() {
        use crate::executor::SmolExecutor;

        let config = RetryConfig {
            max_retries: 2,
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_secs(1),
            exponential_backoff: false,
        };

        let executor = SmolExecutor::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let result = smol::block_on(execute_with_retry_async(&executor, &config, || {
            let counter = counter_clone.clone();
            async move {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 1 {
                    Err(Error::CameraBusy)
                } else {
                    Ok(50)
                }
            }
        }));

        assert_eq!(result.expect("Smol executor test failed"), 50);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_command_timeout_classification() {
        use crate::command::power::{PowerOn, PowerStandby};
        use crate::timeout::{CommandCategory, CommandTimeout};

        // Test that power commands have the correct timeout class
        let power_on = PowerOn::new();
        assert_eq!(power_on.timeout_class(), CommandCategory::Quick);

        // Verify timeout class matches the expected value
        let power_standby = PowerStandby::new();
        assert_eq!(power_standby.timeout_class(), CommandCategory::Quick);
    }

    #[test]
    fn test_deadline_based_retry() {
        use crate::timeout::Deadline;
        use std::time::Duration;

        let config = RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_secs(1),
            exponential_backoff: false,
        };

        // Create a deadline with a short timeout
        let deadline = Deadline::from_timeout(Duration::from_millis(50));
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let result = execute_with_deadline_retry(deadline, &config, || {
            let count = counter_clone.fetch_add(1, Ordering::SeqCst);
            if count < 5 {
                // Always fail but with a retryable error
                Err(Error::CameraBusy)
            } else {
                Ok(100)
            }
        });

        // Should fail due to deadline expiration
        assert!(matches!(result, Err(Error::Timeout)));

        // Should have made at least one attempt
        assert!(counter.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn test_command_with_timeout_policy() {
        use crate::command::power::PowerOn;
        use crate::timeout::{TimeoutConfig, TimeoutPolicy};
        use std::time::Duration;

        // Test that the timeout policy integration works correctly by testing
        // that power commands (Quick timeout class) get appropriate timeout duration
        let timeout_config = TimeoutConfig {
            ack_timeout: Duration::from_millis(500),
            quick_timeout: Duration::from_secs(2), // Reasonable timeout for test
            movement_timeout: Duration::from_secs(5),
            preset_timeout: Duration::from_secs(10),
            long_timeout: Duration::from_secs(30),
            network_timeout: Duration::from_secs(2),
            default_timeout: Duration::from_secs(5),
        };
        let timeout_policy = TimeoutPolicy::new(timeout_config);

        let power_command = PowerOn::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let config = RetryConfig {
            max_retries: 2,
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_secs(1),
            exponential_backoff: false,
        };

        let result = execute_command_with_retry(&power_command, &timeout_policy, &config, || {
            let count = counter_clone.fetch_add(1, Ordering::SeqCst);
            if count < 2 {
                Err(Error::CameraBusy)
            } else {
                Ok("success")
            }
        });

        // Should succeed after retries (timeout should be long enough)
        assert_eq!(
            result.expect("Command should succeed after retries"),
            "success"
        );
        assert_eq!(counter.load(Ordering::SeqCst), 3); // Initial + 2 retries
    }

    #[test]
    fn test_deadline_expiration_behavior() {
        use crate::timeout::Deadline;
        use std::time::{Duration, Instant};

        let config = RetryConfig {
            max_retries: 10,
            base_retry_delay: Duration::from_millis(1),
            max_retry_duration: Duration::from_secs(1),
            exponential_backoff: false,
        };

        // Test 1: Already expired deadline should return immediately without calling operation
        let past_instant = Instant::now() - Duration::from_secs(1);
        let expired_deadline = Deadline::from_instant(past_instant, Duration::from_millis(50));

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let result: Result<&str, Error> =
            execute_with_deadline_retry(expired_deadline, &config, || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                Err(Error::CameraBusy)
            });

        // Should immediately timeout due to expired deadline without calling operation
        assert!(matches!(result, Err(Error::Timeout)));
        assert_eq!(counter.load(Ordering::SeqCst), 0); // No attempts because deadline already expired

        // Test 2: Short deadline that expires after first attempt
        let short_deadline = Deadline::from_timeout(Duration::from_millis(10));
        counter.store(0, Ordering::SeqCst); // Reset counter

        let result: Result<&str, Error> =
            execute_with_deadline_retry(short_deadline, &config, || {
                let count = counter_clone.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    // First attempt: sleep to consume deadline time
                    std::thread::sleep(Duration::from_millis(15));
                }
                Err(Error::CameraBusy) // Always fail
            });

        // Should fail either due to timeout or max retries, but not succeed
        assert!(matches!(result, Err(Error::Timeout)) || matches!(result, Err(Error::CameraBusy)));
        assert!(counter.load(Ordering::SeqCst) >= 1); // At least one attempt
        assert!(counter.load(Ordering::SeqCst) <= (config.max_retries + 1) as usize);
        // Not more than configured attempts
    }

    #[test]
    fn test_retry_executor_with_command() {
        use crate::command::power::PowerOn;
        use crate::timeout::{TimeoutConfig, TimeoutPolicy};
        use std::time::Duration;

        let executor = RetryExecutor::with_defaults();

        // Use longer timeouts to allow successful completion
        let timeout_config = TimeoutConfig {
            ack_timeout: Duration::from_millis(500),
            quick_timeout: Duration::from_secs(2), // Long enough for the test
            movement_timeout: Duration::from_secs(5),
            preset_timeout: Duration::from_secs(10),
            long_timeout: Duration::from_secs(30),
            network_timeout: Duration::from_secs(2),
            default_timeout: Duration::from_secs(5),
        };
        let timeout_policy = TimeoutPolicy::new(timeout_config);

        let power_command = PowerOn::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let result = executor.execute_command(&power_command, &timeout_policy, || {
            let count = counter_clone.fetch_add(1, Ordering::SeqCst);
            if count < 2 {
                Err(Error::CameraBusy)
            } else {
                Ok("success")
            }
        });

        assert_eq!(
            result.expect("Retry executor command test failed"),
            "success"
        );
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_different_command_categories_have_different_timeouts() {
        use crate::command::{
            power::PowerOn, preset::PresetAction, preset::PresetCommand, preset::PresetNumber,
        };
        use crate::timeout::{CommandCategory, CommandTimeout};

        // Test different command categories
        let power_cmd = PowerOn::new();
        assert_eq!(power_cmd.timeout_class(), CommandCategory::Quick);

        // Create a preset command (should be Preset category)
        if let Ok(preset_num) = PresetNumber::new(1) {
            let preset_cmd = PresetCommand {
                action: PresetAction::Recall,
                preset_number: preset_num,
            };
            assert_eq!(preset_cmd.timeout_class(), CommandCategory::Preset);
        }

        // Verify they're different
        assert_ne!(CommandCategory::Quick, CommandCategory::Preset);
    }
}
