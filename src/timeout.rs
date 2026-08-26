//! Unified timeout configuration and management for VISCA commands.
//!
//! This module provides configurable timeout support for different categories of VISCA commands,
//! allowing fine-tuned control over command execution timeouts based on the expected duration
//! of each operation type. It also includes socket-level timeout management to prevent
//! duplication across transport implementations.

use std::{
    io,
    net::{TcpStream, UdpSocket},
    sync::MutexGuard,
    time::{Duration, Instant},
};

use crate::Error;

/// Categories of VISCA commands with different timeout requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CommandCategory {
    /// Quick commands like inquiry, power status (1-2 seconds).
    Quick,
    /// Movement commands like pan/tilt/zoom (5-10 seconds).
    Movement,
    /// Preset operations like recall/save (30-60 seconds).
    Preset,
    /// Long operations like preset discovery (2-5 minutes).
    LongRunning,
    /// Network commands like multicast/Ndi settings (1-2 seconds).
    Network,
    /// Custom timeout for specific commands.
    Custom,
}

impl CommandCategory {
    /// Returns the default timeout for this category.
    #[must_use]
    pub const fn default_timeout(&self) -> Duration {
        match self {
            Self::Quick => Duration::from_secs(5), // Increased from 2s for network delays
            Self::Movement => Duration::from_secs(30), // Increased from 10s for full-range movements
            Self::Preset => Duration::from_secs(60), // Reduced from 90s to prevent excessive waits
            Self::LongRunning => Duration::from_secs(300), // Keep at 5 minutes for discovery
            Self::Network => Duration::from_secs(5), // Increased from 2s for network operations
            Self::Custom => Duration::from_secs(60), // Increased from 30s as general fallback
        }
    }
}

/// Configuration for command timeouts.
#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde_with::serde_as)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct TimeoutConfig {
    /// Timeout for ACK responses from the camera (default 500ms)
    #[cfg_attr(
        feature = "serde",
        serde_as(as = "serde_with::DurationMilliSeconds<u64>")
    )]
    #[cfg_attr(
        feature = "schemars",
        schemars(with = "u64", description = "Timeout in milliseconds")
    )]
    pub ack_timeout: Duration,
    /// Timeout for quick commands (inquiry, power status)
    #[cfg_attr(
        feature = "serde",
        serde_as(as = "serde_with::DurationMilliSeconds<u64>")
    )]
    #[cfg_attr(
        feature = "schemars",
        schemars(with = "u64", description = "Timeout in milliseconds")
    )]
    pub quick_timeout: Duration,
    /// Timeout for movement commands (pan/tilt/zoom)
    #[cfg_attr(
        feature = "serde",
        serde_as(as = "serde_with::DurationMilliSeconds<u64>")
    )]
    #[cfg_attr(
        feature = "schemars",
        schemars(with = "u64", description = "Timeout in milliseconds")
    )]
    pub movement_timeout: Duration,
    /// Timeout for preset operations
    #[cfg_attr(
        feature = "serde",
        serde_as(as = "serde_with::DurationMilliSeconds<u64>")
    )]
    #[cfg_attr(
        feature = "schemars",
        schemars(with = "u64", description = "Timeout in milliseconds")
    )]
    pub preset_timeout: Duration,
    /// Timeout for long-running operations
    #[cfg_attr(
        feature = "serde",
        serde_as(as = "serde_with::DurationMilliSeconds<u64>")
    )]
    #[cfg_attr(
        feature = "schemars",
        schemars(with = "u64", description = "Timeout in milliseconds")
    )]
    pub long_timeout: Duration,
    /// Timeout for network commands (multicast, Ndi)
    #[cfg_attr(
        feature = "serde",
        serde_as(as = "serde_with::DurationMilliSeconds<u64>")
    )]
    #[cfg_attr(
        feature = "schemars",
        schemars(with = "u64", description = "Timeout in milliseconds")
    )]
    pub network_timeout: Duration,
    /// Default timeout for uncategorized commands
    #[cfg_attr(
        feature = "serde",
        serde_as(as = "serde_with::DurationMilliSeconds<u64>")
    )]
    #[cfg_attr(
        feature = "schemars",
        schemars(with = "u64", description = "Timeout in milliseconds")
    )]
    pub default_timeout: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            ack_timeout: Duration::from_millis(500), // Default ACK timeout as specified in epic
            quick_timeout: Duration::from_secs(5),   // Increased for network delays
            movement_timeout: Duration::from_secs(30), // Increased for full-range movements
            preset_timeout: Duration::from_secs(60), // Reduced from 90s to prevent excessive waits
            long_timeout: Duration::from_secs(300),  // Keep at 5 minutes for discovery
            network_timeout: Duration::from_secs(5), // Increased for network operations
            default_timeout: Duration::from_secs(60), // Increased as general fallback
        }
    }
}

impl TimeoutConfig {
    /// Creates a new timeout configuration with all timeouts set to the same value.
    #[must_use]
    pub const fn uniform(timeout: Duration) -> Self {
        Self {
            ack_timeout: timeout,
            quick_timeout: timeout,
            movement_timeout: timeout,
            preset_timeout: timeout,
            long_timeout: timeout,
            network_timeout: timeout,
            default_timeout: timeout,
        }
    }

    /// Gets the timeout for a specific command category.
    #[must_use]
    pub const fn get_timeout(&self, category: CommandCategory) -> Duration {
        match category {
            CommandCategory::Quick => self.quick_timeout,
            CommandCategory::Movement => self.movement_timeout,
            CommandCategory::Preset => self.preset_timeout,
            CommandCategory::LongRunning => self.long_timeout,
            CommandCategory::Network => self.network_timeout,
            CommandCategory::Custom => self.default_timeout,
        }
    }

    /// Creates a builder for timeout configuration.
    #[must_use]
    pub fn builder() -> TimeoutConfigBuilder {
        TimeoutConfigBuilder::default()
    }
}

/// Builder for creating custom timeout configurations.
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeoutConfigBuilder {
    config: TimeoutConfig,
}

impl TimeoutConfigBuilder {
    /// Sets the timeout for ACK responses.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn ack_timeout(mut self, timeout: Duration) -> Self {
        self.config.ack_timeout = timeout;
        self
    }

    /// Sets the timeout for quick commands.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn quick_timeout(mut self, timeout: Duration) -> Self {
        self.config.quick_timeout = timeout;
        self
    }

    /// Sets the timeout for movement commands.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn movement_timeout(mut self, timeout: Duration) -> Self {
        self.config.movement_timeout = timeout;
        self
    }

    /// Sets the timeout for preset operations.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn preset_timeout(mut self, timeout: Duration) -> Self {
        self.config.preset_timeout = timeout;
        self
    }

    /// Sets the timeout for long-running operations.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn long_timeout(mut self, timeout: Duration) -> Self {
        self.config.long_timeout = timeout;
        self
    }

    /// Sets the timeout for network commands.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn network_timeout(mut self, timeout: Duration) -> Self {
        self.config.network_timeout = timeout;
        self
    }

    /// Sets the default timeout for uncategorized commands.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn default_timeout(mut self, timeout: Duration) -> Self {
        self.config.default_timeout = timeout;
        self
    }

    /// Builds the timeout configuration.
    #[must_use]
    pub const fn build(self) -> TimeoutConfig {
        self.config
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_command_category_defaults() {
        assert_eq!(
            CommandCategory::Quick.default_timeout(),
            Duration::from_secs(5)
        );
        assert_eq!(
            CommandCategory::Movement.default_timeout(),
            Duration::from_secs(30)
        );
        assert_eq!(
            CommandCategory::Preset.default_timeout(),
            Duration::from_secs(60)
        );
        assert_eq!(
            CommandCategory::LongRunning.default_timeout(),
            Duration::from_secs(300)
        );
        assert_eq!(
            CommandCategory::Network.default_timeout(),
            Duration::from_secs(5)
        );
        assert_eq!(
            CommandCategory::Custom.default_timeout(),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn test_timeout_config_default() {
        let config = TimeoutConfig::default();
        assert_eq!(config.ack_timeout, Duration::from_millis(500));
        assert_eq!(config.quick_timeout, Duration::from_secs(5));
        assert_eq!(config.movement_timeout, Duration::from_secs(30));
        assert_eq!(config.preset_timeout, Duration::from_secs(60));
        assert_eq!(config.long_timeout, Duration::from_secs(300));
        assert_eq!(config.network_timeout, Duration::from_secs(5));
        assert_eq!(config.default_timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_timeout_config_uniform() {
        let timeout = Duration::from_secs(5);
        let config = TimeoutConfig::uniform(timeout);
        assert_eq!(config.ack_timeout, timeout);
        assert_eq!(config.quick_timeout, timeout);
        assert_eq!(config.movement_timeout, timeout);
        assert_eq!(config.preset_timeout, timeout);
        assert_eq!(config.long_timeout, timeout);
        assert_eq!(config.network_timeout, timeout);
        assert_eq!(config.default_timeout, timeout);
    }

    #[test]
    fn test_get_timeout() {
        let config = TimeoutConfig::default();
        assert_eq!(
            config.get_timeout(CommandCategory::Quick),
            Duration::from_secs(5)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::Movement),
            Duration::from_secs(30)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::Preset),
            Duration::from_secs(60)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::LongRunning),
            Duration::from_secs(300)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::Network),
            Duration::from_secs(5)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::Custom),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn test_timeout_config_builder() {
        let config = TimeoutConfig::builder()
            .quick_timeout(Duration::from_secs(1))
            .movement_timeout(Duration::from_secs(5))
            .preset_timeout(Duration::from_secs(30))
            .long_timeout(Duration::from_secs(120))
            .default_timeout(Duration::from_secs(15))
            .build();

        assert_eq!(config.quick_timeout, Duration::from_secs(1));
        assert_eq!(config.movement_timeout, Duration::from_secs(5));
        assert_eq!(config.preset_timeout, Duration::from_secs(30));
        assert_eq!(config.long_timeout, Duration::from_secs(120));
        assert_eq!(config.default_timeout, Duration::from_secs(15));
    }
}

/// A trait for managing timeouts on socket operations.
///
/// This trait provides a consistent interface for saving, setting, and
/// restoring timeout values across different socket types.
pub trait TimeoutManager {
    /// Execute a function with a temporary timeout.
    ///
    /// This method saves the current timeout, sets a new timeout for the
    /// duration of the function execution, then restores the original timeout.
    ///
    /// # Arguments
    ///
    /// * `timeout` - The timeout duration to use
    /// * `f` - The function to execute with the timeout
    ///
    /// # Returns
    ///
    /// The result of the function execution.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Getting or setting timeouts fails
    /// - The provided function returns an error
    fn with_timeout<F, R>(&mut self, timeout: Duration, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut Self) -> Result<R, Error>;

    /// Get the current read timeout.
    fn get_read_timeout(&self) -> io::Result<Option<Duration>>;

    /// Set the read timeout.
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()>;

    /// Get the current write timeout.
    fn get_write_timeout(&self) -> io::Result<Option<Duration>>;

    /// Set the write timeout.
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()>;

    /// Execute a function with a temporary read timeout.
    ///
    /// This is a convenience method that specifically manages read timeouts.
    fn with_read_timeout<F, R>(&mut self, timeout: Duration, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut Self) -> Result<R, Error>,
    {
        let original_timeout = self.get_read_timeout()?;
        self.set_read_timeout(Some(timeout))?;
        let result = f(self);
        self.set_read_timeout(original_timeout)?;

        result
    }

    /// Execute a function with a temporary write timeout.
    ///
    /// This is a convenience method that specifically manages write timeouts.
    fn with_write_timeout<F, R>(&mut self, timeout: Duration, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut Self) -> Result<R, Error>,
    {
        let original_timeout = self.get_write_timeout()?;
        self.set_write_timeout(Some(timeout))?;
        let result = f(self);
        self.set_write_timeout(original_timeout)?;

        result
    }
}

/// Implement TimeoutManager for TcpStream.
impl TimeoutManager for TcpStream {
    fn with_timeout<F, R>(&mut self, timeout: Duration, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut Self) -> Result<R, Error>,
    {
        self.with_read_timeout(timeout, f)
    }

    fn get_read_timeout(&self) -> io::Result<Option<Duration>> {
        self.read_timeout()
    }

    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        TcpStream::set_read_timeout(self, timeout)
    }

    fn get_write_timeout(&self) -> io::Result<Option<Duration>> {
        self.write_timeout()
    }

    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        TcpStream::set_write_timeout(self, timeout)
    }
}

/// Implement TimeoutManager for UdpSocket.
impl TimeoutManager for UdpSocket {
    fn with_timeout<F, R>(&mut self, timeout: Duration, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut Self) -> Result<R, Error>,
    {
        self.with_read_timeout(timeout, f)
    }

    fn get_read_timeout(&self) -> io::Result<Option<Duration>> {
        self.read_timeout()
    }

    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        UdpSocket::set_read_timeout(self, timeout)
    }

    fn get_write_timeout(&self) -> io::Result<Option<Duration>> {
        self.write_timeout()
    }

    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        UdpSocket::set_write_timeout(self, timeout)
    }
}

/// A helper struct to manage timeouts on a socket within a closure.
///
/// This struct ensures that timeouts are properly restored even if the
/// operation fails or panics.
pub struct TimeoutGuard<'a, T: TimeoutManager> {
    socket: &'a mut T,
    original_read_timeout: Option<Duration>,
    original_write_timeout: Option<Duration>,
    restored: bool,
}

impl<T: TimeoutManager> std::fmt::Debug for TimeoutGuard<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimeoutGuard")
            .field("original_read_timeout", &self.original_read_timeout)
            .field("original_write_timeout", &self.original_write_timeout)
            .field("restored", &self.restored)
            .finish()
    }
}

impl<'a, T: TimeoutManager> TimeoutGuard<'a, T> {
    /// Create a new timeout guard with specified timeouts.
    pub fn new(
        socket: &'a mut T,
        read_timeout: Option<Duration>,
        write_timeout: Option<Duration>,
    ) -> Result<Self, Error> {
        let original_read_timeout = socket.get_read_timeout()?;
        let original_write_timeout = socket.get_write_timeout()?;

        if let Some(timeout) = read_timeout {
            socket.set_read_timeout(Some(timeout))?;
        }

        if let Some(timeout) = write_timeout {
            socket.set_write_timeout(Some(timeout))?;
        }

        Ok(Self {
            socket,
            original_read_timeout,
            original_write_timeout,
            restored: false,
        })
    }

    /// Restore the original timeouts.
    pub fn restore(&mut self) -> Result<(), Error> {
        if !self.restored {
            self.socket.set_read_timeout(self.original_read_timeout)?;
            self.socket.set_write_timeout(self.original_write_timeout)?;
            self.restored = true;
        }
        Ok(())
    }
}

impl<T: TimeoutManager> Drop for TimeoutGuard<'_, T> {
    fn drop(&mut self) {
        // Best effort to restore timeouts
        let _ = self.restore();
    }
}

/// Execute a function with a temporary timeout on a MutexGuard-wrapped socket.
///
/// This is a utility function for working with sockets that are protected
/// by a Mutex, which is common in the transport implementations.
pub fn with_timeout_on_guard<T, F, R>(
    guard: &mut MutexGuard<'_, T>,
    timeout: Duration,
    f: F,
) -> Result<R, Error>
where
    T: TimeoutManager,
    F: FnOnce(&mut T) -> Result<R, Error>,
{
    guard.with_read_timeout(timeout, f)
}

/// Deadline for timeout operations.
///
/// Provides a consistent way to calculate and check deadlines across
/// different operation types in the library.
///
/// # Clock-Agnostic Design
///
/// This type supports both wall-clock and executor-driven time sources.
/// In async code, prefer the `*_at` variants that accept an explicit `now`
/// parameter to ensure compatibility with deterministic executors that use
/// virtual time. The parameterless methods are provided for convenience in
/// blocking code where wall-clock time is appropriate.
#[derive(Debug, Clone, Copy)]
pub struct Deadline {
    /// The point in time when the operation should timeout.
    pub deadline: Instant,
    /// The original timeout duration.
    pub timeout: Duration,
}

impl Deadline {
    /// Create a new deadline from a timeout duration using wall-clock time.
    ///
    /// # Note
    ///
    /// This method uses `Instant::now()` internally. For async code that needs
    /// to work with deterministic executors, prefer [`Deadline::from_timeout_at`].
    #[must_use]
    pub fn from_timeout(timeout: Duration) -> Self {
        Self::from_timeout_at(Instant::now(), timeout)
    }

    /// Create a new deadline from a timeout duration at a specific instant.
    ///
    /// This is the clock-agnostic constructor that should be used in async code.
    /// The `now` parameter should come from `Executor::now()` to ensure
    /// compatibility with deterministic executors.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let deadline = Deadline::from_timeout_at(executor.now(), Duration::from_secs(5));
    /// ```
    #[must_use]
    pub fn from_timeout_at(now: Instant, timeout: Duration) -> Self {
        let deadline = now + timeout;
        Self { deadline, timeout }
    }

    /// Create a new deadline from a specific instant.
    #[must_use]
    pub const fn from_instant(deadline: Instant, timeout: Duration) -> Self {
        Self { deadline, timeout }
    }

    /// Check if the deadline has been exceeded using wall-clock time.
    ///
    /// # Note
    ///
    /// This method uses `Instant::now()` internally. For async code that needs
    /// to work with deterministic executors, prefer [`Deadline::is_expired_at`].
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.is_expired_at(Instant::now())
    }

    /// Check if the deadline has been exceeded at a specific instant.
    ///
    /// This is the clock-agnostic version that should be used in async code.
    /// The `now` parameter should come from `Executor::now()` to ensure
    /// compatibility with deterministic executors.
    #[must_use]
    pub fn is_expired_at(&self, now: Instant) -> bool {
        now > self.deadline
    }

    /// Get the remaining time until the deadline using wall-clock time.
    ///
    /// # Note
    ///
    /// This method uses `Instant::now()` internally. For async code that needs
    /// to work with deterministic executors, prefer [`Deadline::remaining_at`].
    #[must_use]
    pub fn remaining(&self) -> Duration {
        self.remaining_at(Instant::now())
    }

    /// Get the remaining time until the deadline at a specific instant.
    ///
    /// This is the clock-agnostic version that should be used in async code.
    /// The `now` parameter should come from `Executor::now()` to ensure
    /// compatibility with deterministic executors.
    #[must_use]
    pub fn remaining_at(&self, now: Instant) -> Duration {
        self.deadline.saturating_duration_since(now)
    }

    /// Get the elapsed time since the deadline was created using wall-clock time.
    ///
    /// # Note
    ///
    /// This method uses `Instant::now()` internally. For async code that needs
    /// to work with deterministic executors, prefer [`Deadline::elapsed_at`].
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.elapsed_at(Instant::now())
    }

    /// Get the elapsed time since the deadline was created at a specific instant.
    ///
    /// This is the clock-agnostic version that should be used in async code.
    /// The `now` parameter should come from `Executor::now()` to ensure
    /// compatibility with deterministic executors.
    #[must_use]
    pub fn elapsed_at(&self, now: Instant) -> Duration {
        self.timeout.saturating_sub(self.remaining_at(now))
    }
}

/// Timeout policy configuration that maps classes to durations.
///
/// This provides a runtime-configurable way to adjust timeout behavior
/// across different command categories.
///
/// # Clock-Agnostic Design
///
/// This type supports both wall-clock and executor-driven time sources.
/// In async code, prefer [`TimeoutPolicy::deadline_for_at`] which accepts
/// an explicit `now` parameter to ensure compatibility with deterministic
/// executors that use virtual time.
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeoutPolicy {
    config: TimeoutConfig,
}

impl TimeoutPolicy {
    /// Create a new timeout policy from a timeout configuration.
    #[must_use]
    pub const fn new(config: TimeoutConfig) -> Self {
        Self { config }
    }

    /// Get the timeout duration for a specific class.
    #[must_use]
    pub const fn get_timeout(&self, class: CommandCategory) -> Duration {
        self.config.get_timeout(class)
    }

    /// Create a deadline for a specific timeout class using wall-clock time.
    ///
    /// # Note
    ///
    /// This method uses `Instant::now()` internally. For async code that needs
    /// to work with deterministic executors, prefer [`TimeoutPolicy::deadline_for_at`].
    #[must_use]
    pub fn deadline_for(&self, class: CommandCategory) -> Deadline {
        Deadline::from_timeout(self.get_timeout(class))
    }

    /// Create a deadline for a specific timeout class at a specific instant.
    ///
    /// This is the clock-agnostic version that should be used in async code.
    /// The `now` parameter should come from `Executor::now()` to ensure
    /// compatibility with deterministic executors.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let deadline = timeout_policy.deadline_for_at(executor.now(), CommandCategory::Quick);
    /// ```
    #[must_use]
    pub fn deadline_for_at(&self, now: Instant, class: CommandCategory) -> Deadline {
        Deadline::from_timeout_at(now, self.get_timeout(class))
    }

    /// Update the timeout configuration.
    pub fn update_config(&mut self, config: TimeoutConfig) {
        self.config = config;
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod timeout_manager_tests {
    use std::{
        io,
        net::{TcpListener, TcpStream, UdpSocket},
        sync::Mutex,
        thread,
    };

    use super::*;

    #[derive(Debug, Default)]
    struct MockTimeoutSocket {
        read_timeout: Option<Duration>,
        write_timeout: Option<Duration>,
    }

    impl MockTimeoutSocket {
        const fn new(read_timeout: Option<Duration>, write_timeout: Option<Duration>) -> Self {
            Self {
                read_timeout,
                write_timeout,
            }
        }
    }

    impl TimeoutManager for MockTimeoutSocket {
        fn with_timeout<F, R>(&mut self, timeout: Duration, f: F) -> Result<R, Error>
        where
            F: FnOnce(&mut Self) -> Result<R, Error>,
        {
            self.with_read_timeout(timeout, f)
        }

        fn get_read_timeout(&self) -> io::Result<Option<Duration>> {
            Ok(self.read_timeout)
        }

        fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
            self.read_timeout = timeout;
            Ok(())
        }

        fn get_write_timeout(&self) -> io::Result<Option<Duration>> {
            Ok(self.write_timeout)
        }

        fn set_write_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
            self.write_timeout = timeout;
            Ok(())
        }
    }

    #[test]
    fn test_with_read_timeout_restores_original_timeout_on_success() {
        let original_timeout = Some(Duration::from_secs(5));
        let requested_timeout = Duration::from_secs(1);
        let mut socket = MockTimeoutSocket::new(original_timeout, None);

        let result = socket.with_read_timeout(requested_timeout, |socket| {
            assert_eq!(socket.read_timeout, Some(requested_timeout));
            Ok(42)
        });

        assert_eq!(result.expect("temporary read timeout should succeed"), 42);
        assert_eq!(socket.read_timeout, original_timeout);
    }

    #[test]
    fn test_with_read_timeout_restores_original_timeout_on_error() {
        let original_timeout = Some(Duration::from_secs(5));
        let requested_timeout = Duration::from_secs(1);
        let mut socket = MockTimeoutSocket::new(original_timeout, None);

        let error = socket
            .with_read_timeout(requested_timeout, |socket| {
                assert_eq!(socket.read_timeout, Some(requested_timeout));
                Err::<(), Error>(Error::from(io::Error::other(
                    "expected timeout test failure",
                )))
            })
            .expect_err("closure failure should be propagated");

        assert!(matches!(error, Error::Io(_)));
        assert_eq!(socket.read_timeout, original_timeout);
    }

    #[test]
    fn test_with_write_timeout_restores_original_timeout() {
        let original_timeout = Some(Duration::from_secs(10));
        let requested_timeout = Duration::from_secs(2);
        let mut socket = MockTimeoutSocket::new(None, original_timeout);

        let result = socket.with_write_timeout(requested_timeout, |socket| {
            assert_eq!(socket.write_timeout, Some(requested_timeout));
            Ok("ok")
        });

        assert_eq!(
            result.expect("temporary write timeout should succeed"),
            "ok"
        );
        assert_eq!(socket.write_timeout, original_timeout);
    }

    #[test]
    fn test_timeout_guard_restores_timeouts() {
        let original_read_timeout = Some(Duration::from_secs(5));
        let original_write_timeout = Some(Duration::from_secs(10));
        let requested_read_timeout = Duration::from_secs(1);
        let requested_write_timeout = Duration::from_secs(2);
        let mut socket = MockTimeoutSocket::new(original_read_timeout, original_write_timeout);

        {
            let mut guard = TimeoutGuard::new(
                &mut socket,
                Some(requested_read_timeout),
                Some(requested_write_timeout),
            )
            .expect("Failed to create timeout guard");

            assert_eq!(guard.socket.read_timeout, Some(requested_read_timeout));
            assert_eq!(guard.socket.write_timeout, Some(requested_write_timeout));

            guard
                .restore()
                .expect("Failed to restore original timeouts");
            assert_eq!(guard.socket.read_timeout, original_read_timeout);
            assert_eq!(guard.socket.write_timeout, original_write_timeout);

            guard
                .restore()
                .expect("timeout restoration should be idempotent");
        }

        assert_eq!(socket.read_timeout, original_read_timeout);
        assert_eq!(socket.write_timeout, original_write_timeout);
    }

    #[test]
    fn test_with_timeout_on_guard_restores_read_timeout() {
        let original_timeout = Some(Duration::from_secs(5));
        let requested_timeout = Duration::from_secs(3);
        let socket = Mutex::new(MockTimeoutSocket::new(original_timeout, None));
        let mut guard = socket.lock().expect("mutex should not be poisoned");

        let result = with_timeout_on_guard(&mut guard, requested_timeout, |socket| {
            assert_eq!(socket.read_timeout, Some(requested_timeout));
            Ok("ok")
        });

        assert_eq!(
            result.expect("temporary timeout through mutex guard should succeed"),
            "ok"
        );
        assert_eq!(guard.read_timeout, original_timeout);
    }

    #[test]
    #[cfg_attr(miri, ignore = "requires real TCP sockets")]
    fn test_tcp_timeout_manager_smoke() {
        // Start a TCP server
        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind test listener");
        let addr = listener
            .local_addr()
            .expect("Failed to get listener address");

        // Spawn a thread to accept connections
        thread::spawn(move || {
            let (_stream, _) = listener.accept().expect("Failed to accept connection");
            // Keep the connection open
            thread::sleep(Duration::from_secs(10));
        });

        // Connect to the server
        let mut stream = TcpStream::connect(addr).expect("Failed to connect to test server");

        // Test setting and getting timeouts
        assert_eq!(
            stream.get_read_timeout().expect("Failed to get timeout"),
            None
        );

        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("Failed to set timeout");
        assert_eq!(
            stream.get_read_timeout().expect("Failed to get timeout"),
            Some(Duration::from_secs(5))
        );

        // Test with_read_timeout
        let result = stream.with_read_timeout(Duration::from_secs(1), |s| {
            // Verify timeout is set
            assert_eq!(
                s.get_read_timeout()
                    .expect("Failed to get timeout in guard"),
                Some(Duration::from_secs(1))
            );
            Ok(42)
        });

        assert_eq!(result.expect("Test operation failed"), 42);
        // Verify timeout is restored
        assert_eq!(
            stream
                .get_read_timeout()
                .expect("Failed to get timeout after guard"),
            Some(Duration::from_secs(5))
        );
    }

    #[test]
    #[cfg_attr(miri, ignore = "requires real UDP sockets")]
    fn test_udp_timeout_manager_smoke() {
        let mut socket = UdpSocket::bind("127.0.0.1:0").expect("Failed to bind UDP socket");

        // Test setting and getting timeouts
        assert_eq!(
            socket
                .get_read_timeout()
                .expect("Failed to get UDP timeout"),
            None
        );

        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .expect("Failed to set UDP timeout");
        assert_eq!(
            socket
                .get_read_timeout()
                .expect("Failed to get UDP timeout"),
            Some(Duration::from_secs(3))
        );

        // Test with_read_timeout
        let result = socket.with_read_timeout(Duration::from_secs(2), |s| {
            // Verify timeout is set
            assert_eq!(
                s.get_read_timeout()
                    .expect("Failed to get UDP timeout in guard"),
                Some(Duration::from_secs(2))
            );
            Ok("success")
        });

        assert_eq!(result.expect("UDP test operation failed"), "success");
        // Verify timeout is restored
        assert_eq!(
            socket
                .get_read_timeout()
                .expect("Failed to get UDP timeout after guard"),
            Some(Duration::from_secs(3))
        );
    }

    #[test]
    fn test_timeout_guard_restores_timeouts_on_drop() {
        let original_read_timeout = Some(Duration::from_secs(5));
        let original_write_timeout = Some(Duration::from_secs(10));
        let mut socket = MockTimeoutSocket::new(original_read_timeout, original_write_timeout);

        {
            let guard = TimeoutGuard::new(
                &mut socket,
                Some(Duration::from_secs(1)),
                Some(Duration::from_secs(2)),
            )
            .expect("Failed to create timeout guard");

            assert_eq!(guard.socket.read_timeout, Some(Duration::from_secs(1)));
            assert_eq!(guard.socket.write_timeout, Some(Duration::from_secs(2)));
        }

        assert_eq!(socket.read_timeout, original_read_timeout);
        assert_eq!(socket.write_timeout, original_write_timeout);
    }

    #[test]
    fn test_deadline() {
        let timeout = Duration::from_millis(100);
        let now = Instant::now();
        let deadline = Deadline::from_timeout_at(now, timeout);

        assert!(!deadline.is_expired_at(now));
        assert_eq!(deadline.timeout, timeout);
        assert_eq!(deadline.remaining_at(now), timeout);
        assert_eq!(deadline.elapsed_at(now), Duration::ZERO);

        let during_timeout = now + Duration::from_millis(10);
        assert!(!deadline.is_expired_at(during_timeout));
        assert_eq!(
            deadline.remaining_at(during_timeout),
            Duration::from_millis(90)
        );
        assert_eq!(
            deadline.elapsed_at(during_timeout),
            Duration::from_millis(10)
        );

        let at_deadline = now + timeout;
        assert!(!deadline.is_expired_at(at_deadline));
        assert_eq!(deadline.remaining_at(at_deadline), Duration::ZERO);

        let after_deadline = at_deadline + Duration::from_nanos(1);
        assert!(deadline.is_expired_at(after_deadline));
        assert_eq!(deadline.remaining_at(after_deadline), Duration::ZERO);
    }

    #[test]
    fn test_timeout_policy() {
        let policy = TimeoutPolicy::default();

        assert_eq!(
            policy.get_timeout(CommandCategory::Quick),
            Duration::from_secs(5)
        );
        assert_eq!(
            policy.get_timeout(CommandCategory::Movement),
            Duration::from_secs(30)
        );

        let deadline = policy.deadline_for(CommandCategory::Quick);
        assert!(!deadline.is_expired());
        assert_eq!(deadline.timeout, Duration::from_secs(5));

        // Test config update
        let mut policy = TimeoutPolicy::default();
        let new_config = TimeoutConfig::uniform(Duration::from_secs(10));
        policy.update_config(new_config);

        assert_eq!(
            policy.get_timeout(CommandCategory::Quick),
            Duration::from_secs(10)
        );
    }
}
