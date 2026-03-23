//! Generic retry executor for transport operations.
//!
//! This module provides a unified retry mechanism that can be used across
//! all transport implementations, both blocking and async.

#[cfg(feature = "mode-async")]
use std::future::Future;
use std::time::{Duration, Instant};

#[cfg(test)]
use super::BackoffStrategy;
use super::{RetryAttempt, RetryConfig};
#[cfg(feature = "mode-async")]
use crate::executor::Executor;
use crate::{
    timeout::{CommandTimeout, Deadline, TimeoutPolicy},
    Error,
};

/// A trait for operations that can provide retry hints.
pub trait RetryableOperation {
    /// Check if the operation should be retried based on the error.
    ///
    /// # Arguments
    ///
    /// * `error` - The error from the failed operation
    /// * `retries_done` - Number of retries already completed (0-based)
    /// * `start_time` - When the operation started
    fn should_retry(&self, error: &Error, retries_done: u32, start_time: Instant) -> bool;

    /// Get the suggested delay before the next retry attempt.
    ///
    /// # Arguments
    ///
    /// * `error` - The error from the failed operation
    /// * `next_attempt` - The 1-based attempt number for the upcoming retry
    fn retry_delay(&self, error: &Error, next_attempt: RetryAttempt) -> Option<Duration>;
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
    fn should_retry(&self, error: &Error, retries_done: u32, start_time: Instant) -> bool {
        if !error.is_retryable() {
            return false;
        }
        self.config.should_retry(retries_done, start_time)
    }

    fn retry_delay(&self, error: &Error, next_attempt: RetryAttempt) -> Option<Duration> {
        if !error.is_retryable() {
            return None;
        }

        let suggested_delay = error.suggested_retry_delay();
        Some(self.config.calculate_delay(next_attempt, suggested_delay))
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
    let mut retries_done: u32 = 0;

    loop {
        match operation() {
            Ok(result) => return Ok(result),
            Err(error) => {
                if !error.is_retryable() {
                    return Err(error);
                }

                if retries_done >= config.max_retries {
                    return Err(error);
                }

                if start_time.elapsed() >= config.max_retry_duration {
                    return Err(error);
                }

                // Calculate delay for the next attempt (retries_done + 1)
                // SAFETY: retries_done < max_retries, so retries_done + 1 >= 1
                let next_attempt =
                    RetryAttempt::from_retries_done(retries_done).unwrap_or(RetryAttempt::FIRST);
                let delay = config.calculate_delay(next_attempt, error.suggested_retry_delay());

                std::thread::sleep(delay);
                retries_done += 1;
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
///
/// # Clock-Agnostic Design
///
/// This function uses `executor.now()` for all time measurements, ensuring
/// compatibility with deterministic executors that use virtual time.
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
    let start_time = executor.now();
    let mut retries_done: u32 = 0;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(error) => {
                if !error.is_retryable() {
                    return Err(error);
                }

                if retries_done >= config.max_retries {
                    return Err(error);
                }

                let now = executor.now();
                let elapsed = now.saturating_duration_since(start_time);
                if elapsed >= config.max_retry_duration {
                    return Err(error);
                }

                // Calculate delay for the next attempt (retries_done + 1)
                let next_attempt =
                    RetryAttempt::from_retries_done(retries_done).unwrap_or(RetryAttempt::FIRST);
                let delay = config.calculate_delay(next_attempt, error.suggested_retry_delay());

                executor.sleep(delay).await;
                retries_done += 1;
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
    let mut retries_done: u32 = 0;
    let mut operation = operation;

    loop {
        if deadline.is_expired() {
            return Err(Error::Timeout);
        }

        match operation() {
            Ok(result) => return Ok(result),
            Err(error) => {
                if !error.is_retryable() {
                    return Err(error);
                }

                if retries_done >= retry_config.max_retries {
                    return Err(error);
                }

                if deadline.is_expired() || start_time.elapsed() >= retry_config.max_retry_duration
                {
                    return Err(error);
                }

                // Calculate delay for the next attempt (retries_done + 1)
                let next_attempt =
                    RetryAttempt::from_retries_done(retries_done).unwrap_or(RetryAttempt::FIRST);
                let retry_delay =
                    retry_config.calculate_delay(next_attempt, error.suggested_retry_delay());
                let remaining_time = deadline.remaining();
                let actual_delay = retry_delay.min(remaining_time);

                if actual_delay.is_zero() {
                    return Err(error);
                }

                std::thread::sleep(actual_delay);
                retries_done += 1;
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
///
/// # Clock-Agnostic Design
///
/// This function uses `executor.now()` for creating deadlines and all time
/// measurements, ensuring compatibility with deterministic executors that use
/// virtual time.
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
    let now = executor.now();
    let deadline = timeout_policy.deadline_for_at(now, timeout_class);

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
///
/// # Clock-Agnostic Design
///
/// This function uses `executor.now()` for all time measurements and uses
/// `deadline.is_expired_at()` / `deadline.remaining_at()` to check deadline
/// status, ensuring compatibility with deterministic executors that use
/// virtual time.
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
    let start_time = executor.now();
    let mut retries_done: u32 = 0;

    loop {
        // Check if we've exceeded the overall deadline
        let now = executor.now();
        if deadline.is_expired_at(now) {
            return Err(Error::Timeout);
        }

        match operation().await {
            Ok(result) => return Ok(result),
            Err(error) => {
                if !error.is_retryable() {
                    return Err(error);
                }

                if retries_done >= retry_config.max_retries {
                    return Err(error);
                }

                let now = executor.now();
                let elapsed = now.saturating_duration_since(start_time);
                if deadline.is_expired_at(now) || elapsed >= retry_config.max_retry_duration {
                    return Err(error);
                }

                // Calculate delay for the next attempt (retries_done + 1)
                let next_attempt =
                    RetryAttempt::from_retries_done(retries_done).unwrap_or(RetryAttempt::FIRST);
                let retry_delay =
                    retry_config.calculate_delay(next_attempt, error.suggested_retry_delay());
                let remaining_time = deadline.remaining_at(now);
                let actual_delay = retry_delay.min(remaining_time);

                if actual_delay.is_zero() {
                    return Err(error);
                }

                executor.sleep(actual_delay).await;
                retries_done += 1;
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
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
            backoff_strategy: BackoffStrategy::Constant,
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
            backoff_strategy: BackoffStrategy::Constant,
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
            backoff_strategy: BackoffStrategy::Constant,
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

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn test_async_retry_with_smol() {
        use crate::executor::SmolExecutor;

        let config = RetryConfig {
            max_retries: 2,
            base_retry_delay: Duration::from_millis(10),
            max_retry_duration: Duration::from_secs(1),
            backoff_strategy: BackoffStrategy::Constant,
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
            backoff_strategy: BackoffStrategy::Constant,
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
            backoff_strategy: BackoffStrategy::Constant,
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
            backoff_strategy: BackoffStrategy::Constant,
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

    /// Test that exponential backoff produces correct delays for retry helpers.
    ///
    /// This is a regression test for issue #488 which identified off-by-one
    /// errors in retry attempt indexing. With exponential backoff:
    /// - First retry (attempt 1): base delay
    /// - Second retry (attempt 2): base * 2
    /// - Third retry (attempt 3): base * 4
    #[test]
    fn test_exponential_backoff_retry_delays_are_correct() {
        use super::RetryAttempt;

        let config = RetryConfig {
            max_retries: 5,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(60),
            backoff_strategy: BackoffStrategy::Exponential,
        };

        // Verify the exponential progression using RetryAttempt
        let expected_delays = [
            (1, 100),  // attempt 1: 100 * 2^0 = 100ms
            (2, 200),  // attempt 2: 100 * 2^1 = 200ms
            (3, 400),  // attempt 3: 100 * 2^2 = 400ms
            (4, 800),  // attempt 4: 100 * 2^3 = 800ms
            (5, 1600), // attempt 5: 100 * 2^4 = 1600ms
        ];

        for (attempt_num, expected_ms) in expected_delays {
            let attempt = RetryAttempt::new(attempt_num).expect("valid attempt");
            let delay = config.calculate_delay(attempt, None);
            assert_eq!(
                delay,
                Duration::from_millis(expected_ms),
                "Attempt {} should have delay {}ms, got {}ms",
                attempt_num,
                expected_ms,
                delay.as_millis()
            );
        }
    }

    /// Test that RetryAttempt::from_retries_done correctly converts 0-based counts
    /// to 1-based attempt numbers.
    #[test]
    fn test_retries_done_to_attempt_conversion() {
        use super::RetryAttempt;

        // retries_done = 0 means we're about to do attempt 1
        let attempt = RetryAttempt::from_retries_done(0).expect("valid conversion");
        assert_eq!(attempt.get(), 1);

        // retries_done = 1 means we're about to do attempt 2
        let attempt = RetryAttempt::from_retries_done(1).expect("valid conversion");
        assert_eq!(attempt.get(), 2);

        // retries_done = 2 means we're about to do attempt 3
        let attempt = RetryAttempt::from_retries_done(2).expect("valid conversion");
        assert_eq!(attempt.get(), 3);
    }

    /// Test that max_retries=0 yields zero retries.
    #[test]
    fn test_max_retries_zero_yields_no_retries() {
        let config = RetryConfig {
            max_retries: 0,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(10),
            backoff_strategy: BackoffStrategy::Exponential,
        };

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        // Operation always fails with retryable error
        let result = execute_with_retry(&config, || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            Err::<(), _>(Error::CameraBusy)
        });

        // Should fail after exactly 1 attempt (the initial attempt, no retries)
        assert!(matches!(result, Err(Error::CameraBusy)));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    /// Test that max_retries=1 yields exactly one retry.
    #[test]
    fn test_max_retries_one_yields_one_retry() {
        let config = RetryConfig {
            max_retries: 1,
            base_retry_delay: Duration::from_millis(1), // Fast for testing
            max_retry_duration: Duration::from_secs(10),
            backoff_strategy: BackoffStrategy::Exponential,
        };

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        // Operation always fails with retryable error
        let result = execute_with_retry(&config, || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            Err::<(), _>(Error::CameraBusy)
        });

        // Should fail after 2 attempts (initial + 1 retry)
        assert!(matches!(result, Err(Error::CameraBusy)));
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    /// Regression test: second retry delay must NOT equal first retry delay
    /// when using exponential backoff. This was the exact failure mode in issue #488.
    #[test]
    fn test_exponential_second_retry_differs_from_first() {
        use super::RetryAttempt;

        let config = RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(10),
            backoff_strategy: BackoffStrategy::Exponential,
        };

        let first_attempt = RetryAttempt::new(1).unwrap();
        let second_attempt = RetryAttempt::new(2).unwrap();

        let first_delay = config.calculate_delay(first_attempt, None);
        let second_delay = config.calculate_delay(second_attempt, None);

        // This was the bug: both were returning the same delay
        assert_ne!(
            first_delay, second_delay,
            "Exponential backoff must have different delays for first and second retry"
        );
        assert_eq!(
            second_delay,
            first_delay * 2,
            "Second retry delay should be double the first"
        );
    }
}
