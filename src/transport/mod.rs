//! Transport layer for VISCA communication.
//!
//! This module provides both blocking and async transport implementations.
//! The transport layer has been redesigned to use separate traits for blocking
//! and async transports, enabling zero-cost abstractions and better type safety.
//!
//! ## Architecture
//!
//! The transport layer now uses two separate traits:
//! - **AsyncTransport** - Native async functions for zero-cost async transports
//! - **BlockingTransport** - Blocking methods with OS-level timeout support
//! - Implementations: TCP and UDP for both blocking and async
//!
//! ## Send Semantics
//!
//! Transports are classified by their send semantics, which determines how
//! the runtime handles send failures:
//!
//! - **Stream** (TCP, Serial): A partial write can leave the byte stream in an
//!   unknown state. On send failure/timeout, the transport is "poisoned" and
//!   must be dropped to prevent protocol desynchronization.
//!
//! - **Datagram** (UDP): Each send is atomic at the datagram boundary. A failed
//!   send does not affect subsequent sends, so the transport can continue operating.
//!
//! ## Usage
//!
//! For blocking transports (using camera-first API):
//! ```rust,ignore
//! # #[cfg(not(feature = "mode-async"))]
//! use grafton_visca::{
//!     camera::{Connect, profiles::PtzOpticsG2},
//! };
//!
//! # #[cfg(not(feature = "mode-async"))]
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # #[cfg(not(feature = "mode-async"))]
//! let camera = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110:5678")?;
//! # #[cfg(not(feature = "mode-async"))]
//! // Camera is ready to use with accessor pattern
//! # #[cfg(not(feature = "mode-async"))]
//! camera.power().on()?;
//! # #[cfg(not(feature = "mode-async"))]
//! camera.zoom().tele()?;
//! # Ok(())
//! # }
//! ```
//!
//! For async transports (with tokio):
//! ```rust,ignore
//! # #[cfg(feature = "runtime-tokio")]
//! use grafton_visca::{
//!     camera::{Connect, profiles::PtzOpticsG2},
//!     runtime::{Runtime, TokioRuntime},
//! };
//!
//! # #[cfg(feature = "runtime-tokio")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let runtime = TokioRuntime::from_current()?;
//! let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(
//!     "192.168.0.110:5678",
//!     runtime
//! ).await?;
//! // Camera is ready to use with accessor pattern
//! camera.power().on().await?;
//! camera.zoom().tele().await?;
//! # Ok(())
//! # }
//! ```

use std::{
    num::NonZeroU32,
    time::{Duration, Instant},
};

/// Send semantics for transport classification.
///
/// This enum categorizes transports by how their send operations behave,
/// which determines the runtime's failure handling strategy:
///
/// - **Stream transports** (TCP, Serial): Byte-oriented, where a partial write
///   can leave the stream in an unknown state. On send failure or timeout,
///   the transport must be poisoned to prevent protocol desynchronization.
///
/// - **Datagram transports** (UDP): Message-oriented, where each send is atomic.
///   A failed send does not affect subsequent sends, so the transport can
///   continue operating.
///
/// # Usage
///
/// The runtime queries this via `send_semantics()` on send errors to decide
/// whether to poison the transport (Stream) or continue (Datagram).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SendSemantics {
    /// Byte-stream transport (TCP, Serial).
    ///
    /// A send failure/timeout can leave the stream in an unknown state
    /// (partial frame written). The transport must be poisoned on send
    /// failure to prevent protocol desynchronization.
    Stream,

    /// Datagram transport (UDP).
    ///
    /// Each send is atomic at the datagram boundary. A failed send
    /// does not affect subsequent sends, so the transport can continue
    /// operating after a send failure.
    Datagram,
}

pub mod address;
#[cfg(any(
    feature = "runtime-tokio",
    feature = "runtime-async-std",
    feature = "runtime-smol"
))]
pub(crate) mod async_io;
#[cfg(feature = "transport-serial-tokio")]
pub(crate) mod async_serial;
#[cfg(all(feature = "mode-async", feature = "runtime-async-std"))]
pub(crate) mod async_std;
#[cfg(any(
    feature = "runtime-tokio",
    feature = "runtime-async-std",
    feature = "runtime-smol"
))]
pub(crate) mod async_tcp;
#[cfg(feature = "mode-async")]
pub mod async_transport;
#[cfg(any(
    feature = "runtime-tokio",
    feature = "runtime-async-std",
    feature = "runtime-smol"
))]
pub(crate) mod async_udp;
#[cfg(not(feature = "mode-async"))]
pub(crate) mod blocking;
pub mod blocking_transport;
pub mod buffer;
pub mod builder;
pub mod envelope;
pub mod retry;
#[cfg(any(
    feature = "runtime-tokio",
    feature = "runtime-async-std",
    feature = "runtime-smol"
))]
#[macro_use]
pub(crate) mod runtime_common;
#[cfg(any(feature = "transport-serial", feature = "transport-serial-tokio"))]
pub mod serial;
#[cfg(all(not(feature = "mode-async"), feature = "transport-serial"))]
pub(crate) mod serial_blocking;
#[cfg(all(feature = "mode-async", feature = "runtime-smol"))]
pub(crate) mod smol;
#[cfg(any(
    not(feature = "mode-async"),
    feature = "runtime-tokio",
    feature = "runtime-async-std",
    feature = "runtime-smol"
))]
pub(crate) mod socket_options;
#[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
pub(crate) mod tokio;

#[cfg(feature = "mode-async")]
pub use async_transport::AsyncTransport;
#[cfg(not(feature = "mode-async"))]
pub use blocking_transport::BlockingTransportHandle;
pub use blocking_transport::{BlockingTransport, HasTransportConfig};
#[cfg(not(feature = "mode-async"))]
pub use builder::{NetTransportBuilder, Transport, TransportBuilderExt};

/// Backoff strategy for retry delays.
///
/// Determines how the delay between retry attempts is calculated.
/// Different strategies are appropriate for different failure scenarios:
///
/// - **Constant**: Best for transient failures with predictable recovery times.
///   Each retry waits the same `base_retry_delay`.
///
/// - **Exponential**: Best for congestion or rate-limiting scenarios where
///   giving the system more time to recover increases success probability.
///   Delay doubles with each attempt: `base_retry_delay * 2^attempt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum BackoffStrategy {
    /// Constant delay between retries.
    ///
    /// Each retry uses the same `base_retry_delay` regardless of attempt number.
    /// Simple and predictable, suitable for transient failures.
    Constant,

    /// Exponential backoff: delay doubles with each attempt.
    ///
    /// Delay = `base_retry_delay * 2^(attempt - 1)`
    ///
    /// - Attempt 1: `base_retry_delay`
    /// - Attempt 2: `base_retry_delay * 2`
    /// - Attempt 3: `base_retry_delay * 4`
    ///
    /// Reduces pressure on congested systems and improves recovery probability.
    #[default]
    Exponential,
}

/// Type-safe wrapper representing the Nth retry attempt (1-based).
///
/// This newtype enforces that retry attempts start at 1 (first retry),
/// preventing off-by-one errors in backoff calculations. The internal
/// representation uses [`NonZeroU32`] to make "attempt 0" unrepresentable.
///
/// # Semantics
///
/// - `RetryAttempt(1)` = first retry (delay = base)
/// - `RetryAttempt(2)` = second retry (delay = base * 2 for exponential)
/// - `RetryAttempt(3)` = third retry (delay = base * 4 for exponential)
///
/// # Usage
///
/// Convert from a 0-based retry count (`retries_done`) using [`RetryAttempt::from_retries_done`]:
///
/// ```
/// use grafton_visca::transport::RetryAttempt;
///
/// // After 0 retries done, we're about to do attempt #1
/// let attempt = RetryAttempt::from_retries_done(0).unwrap();
/// assert_eq!(attempt.get(), 1);
///
/// // After 2 retries done, we're about to do attempt #3
/// let attempt = RetryAttempt::from_retries_done(2).unwrap();
/// assert_eq!(attempt.get(), 3);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct RetryAttempt(NonZeroU32);

impl RetryAttempt {
    /// The first retry attempt.
    pub const FIRST: Self = Self(
        // SAFETY: 1 is non-zero
        match NonZeroU32::new(1) {
            Some(n) => n,
            None => unreachable!(),
        },
    );

    /// Create a new retry attempt from a 1-based attempt number.
    ///
    /// Returns `None` if `attempt` is 0.
    #[must_use]
    #[inline]
    pub const fn new(attempt: u32) -> Option<Self> {
        match NonZeroU32::new(attempt) {
            Some(n) => Some(Self(n)),
            None => None,
        }
    }

    /// Create a retry attempt from the number of retries already done (0-based).
    ///
    /// After `retries_done` retries, we're about to attempt retry `retries_done + 1`.
    ///
    /// Returns `None` if the resulting attempt number would overflow.
    #[must_use]
    #[inline]
    pub const fn from_retries_done(retries_done: u32) -> Option<Self> {
        match retries_done.checked_add(1) {
            Some(attempt) => Self::new(attempt),
            None => None,
        }
    }

    /// Get the 1-based attempt number.
    #[must_use]
    #[inline]
    pub const fn get(self) -> u32 {
        self.0.get()
    }

    /// Get the number of retries done before this attempt (0-based).
    ///
    /// This is always `attempt - 1`.
    #[must_use]
    #[inline]
    pub const fn retries_done(self) -> u32 {
        self.0.get() - 1
    }

    /// Get the next retry attempt, saturating at `u32::MAX`.
    #[must_use]
    #[inline]
    pub const fn saturating_next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// Check if this attempt exceeds the maximum allowed retries.
    ///
    /// Returns `true` if `self.get() > max_retries`.
    #[must_use]
    #[inline]
    pub const fn exceeds_max(self, max_retries: u32) -> bool {
        self.0.get() > max_retries
    }
}

impl std::fmt::Display for RetryAttempt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "attempt {}", self.0)
    }
}

/// Retry configuration for transport layer operations.
///
/// Provides configurable retry logic for handling transient failures
/// in VISCA communication across all transport types.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde_with::serde_as)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Base delay between retries (will be adjusted based on error type).
    #[cfg_attr(
        feature = "serde",
        serde_as(as = "serde_with::DurationMilliSeconds<u64>")
    )]
    #[cfg_attr(
        feature = "schemars",
        schemars(with = "u64", description = "Base retry delay in milliseconds")
    )]
    pub base_retry_delay: Duration,
    /// Maximum total time to spend retrying.
    #[cfg_attr(
        feature = "serde",
        serde_as(as = "serde_with::DurationMilliSeconds<u64>")
    )]
    #[cfg_attr(
        feature = "schemars",
        schemars(with = "u64", description = "Maximum retry duration in milliseconds")
    )]
    pub max_retry_duration: Duration,
    /// Strategy for calculating delay between retries.
    pub backoff_strategy: BackoffStrategy,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(10),
            backoff_strategy: BackoffStrategy::Exponential,
        }
    }
}

impl RetryConfig {
    /// Calculate the retry delay for a given attempt.
    ///
    /// The delay calculation depends on the configured [`BackoffStrategy`]:
    /// - [`BackoffStrategy::Constant`]: Returns `base_retry_delay` (or suggested delay)
    /// - [`BackoffStrategy::Exponential`]: Returns `base * 2^(attempt-1)`
    ///
    /// # Arguments
    ///
    /// * `attempt` - The 1-based retry attempt number (use [`RetryAttempt::from_retries_done`]
    ///   to convert from 0-based retry counts)
    /// * `error_suggested_delay` - Optional delay suggested by the error (e.g., for rate limiting)
    ///
    /// # Example
    ///
    /// ```
    /// use grafton_visca::transport::{RetryAttempt, RetryConfig, BackoffStrategy};
    /// use std::time::Duration;
    ///
    /// let config = RetryConfig {
    ///     base_retry_delay: Duration::from_millis(100),
    ///     backoff_strategy: BackoffStrategy::Exponential,
    ///     ..Default::default()
    /// };
    ///
    /// // Attempt 1 = base delay
    /// assert_eq!(config.calculate_delay(RetryAttempt::FIRST, None), Duration::from_millis(100));
    ///
    /// // Attempt 2 = base * 2
    /// let attempt2 = RetryAttempt::new(2).unwrap();
    /// assert_eq!(config.calculate_delay(attempt2, None), Duration::from_millis(200));
    /// ```
    pub fn calculate_delay(
        &self,
        attempt: RetryAttempt,
        error_suggested_delay: Option<Duration>,
    ) -> Duration {
        let base = error_suggested_delay.unwrap_or(self.base_retry_delay);

        match self.backoff_strategy {
            BackoffStrategy::Constant => base,
            BackoffStrategy::Exponential => {
                // attempt.get() is 1-based, so exponent is (attempt - 1)
                let exponent = attempt.retries_done();
                let multiplier = 2_u32.saturating_pow(exponent);
                base.saturating_mul(multiplier)
            }
        }
    }

    /// Check if retry should continue based on attempt count and elapsed time.
    ///
    /// Returns `true` if both conditions are met:
    /// - `retries_done < max_retries` (more retries allowed)
    /// - `start_time.elapsed() < max_retry_duration` (within time budget)
    ///
    /// # Arguments
    ///
    /// * `retries_done` - Number of retries already completed (0-based)
    /// * `start_time` - When the operation started
    pub fn should_retry(&self, retries_done: u32, start_time: Instant) -> bool {
        if retries_done >= self.max_retries {
            return false;
        }

        start_time.elapsed() < self.max_retry_duration
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod retry_tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_retry_delay, Duration::from_millis(100));
        assert_eq!(config.max_retry_duration, Duration::from_secs(10));
        assert_eq!(config.backoff_strategy, BackoffStrategy::Exponential);
    }

    #[test]
    fn test_calculate_delay_constant() {
        let config = RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(10),
            backoff_strategy: BackoffStrategy::Constant,
        };

        // With constant backoff, delay should be the same for all attempts
        let attempt1 = RetryAttempt::new(1).unwrap();
        let attempt2 = RetryAttempt::new(2).unwrap();
        let attempt3 = RetryAttempt::new(3).unwrap();
        assert_eq!(
            config.calculate_delay(attempt1, None),
            Duration::from_millis(100)
        );
        assert_eq!(
            config.calculate_delay(attempt2, None),
            Duration::from_millis(100)
        );
        assert_eq!(
            config.calculate_delay(attempt3, None),
            Duration::from_millis(100)
        );
    }

    #[test]
    fn test_calculate_delay_exponential() {
        let config = RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(10),
            backoff_strategy: BackoffStrategy::Exponential,
        };

        // With exponential backoff, delay should double each time
        let attempt1 = RetryAttempt::new(1).unwrap();
        let attempt2 = RetryAttempt::new(2).unwrap();
        let attempt3 = RetryAttempt::new(3).unwrap();
        assert_eq!(
            config.calculate_delay(attempt1, None),
            Duration::from_millis(100)
        );
        assert_eq!(
            config.calculate_delay(attempt2, None),
            Duration::from_millis(200)
        );
        assert_eq!(
            config.calculate_delay(attempt3, None),
            Duration::from_millis(400)
        );
    }

    #[test]
    fn test_calculate_delay_with_error_suggestion() {
        let config = RetryConfig::default();

        // Should use error's suggested delay if provided
        let suggested = Some(Duration::from_millis(500));
        let attempt1 = RetryAttempt::new(1).unwrap();
        let attempt2 = RetryAttempt::new(2).unwrap();
        assert_eq!(
            config.calculate_delay(attempt1, suggested),
            Duration::from_millis(500)
        );
        assert_eq!(
            config.calculate_delay(attempt2, suggested),
            Duration::from_millis(1000)
        );
    }

    #[test]
    fn test_retry_attempt_from_retries_done() {
        // After 0 retries done, we're about to do attempt #1
        let attempt = RetryAttempt::from_retries_done(0).unwrap();
        assert_eq!(attempt.get(), 1);
        assert_eq!(attempt.retries_done(), 0);

        // After 2 retries done, we're about to do attempt #3
        let attempt = RetryAttempt::from_retries_done(2).unwrap();
        assert_eq!(attempt.get(), 3);
        assert_eq!(attempt.retries_done(), 2);
    }

    #[test]
    fn test_retry_attempt_new_rejects_zero() {
        assert!(RetryAttempt::new(0).is_none());
        assert!(RetryAttempt::new(1).is_some());
    }

    #[test]
    fn test_retry_attempt_exceeds_max() {
        let attempt1 = RetryAttempt::new(1).unwrap();
        let attempt3 = RetryAttempt::new(3).unwrap();
        let attempt4 = RetryAttempt::new(4).unwrap();

        // max_retries = 3 means attempts 1, 2, 3 are allowed
        assert!(!attempt1.exceeds_max(3));
        assert!(!attempt3.exceeds_max(3));
        assert!(attempt4.exceeds_max(3));
    }

    #[test]
    fn test_retry_attempt_saturating_next() {
        let attempt1 = RetryAttempt::new(1).unwrap();
        let attempt2 = attempt1.saturating_next();
        assert_eq!(attempt2.get(), 2);

        // Test saturation at u32::MAX
        let max_attempt = RetryAttempt::new(u32::MAX).unwrap();
        let saturated = max_attempt.saturating_next();
        assert_eq!(saturated.get(), u32::MAX);
    }

    #[test]
    fn test_should_retry_max_attempts() {
        let config = RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(10),
            backoff_strategy: BackoffStrategy::Constant,
        };

        let start = Instant::now();

        assert!(config.should_retry(0, start));
        assert!(config.should_retry(1, start));
        assert!(config.should_retry(2, start));
        assert!(!config.should_retry(3, start)); // Exceeded max_retries
    }

    #[test]
    fn test_should_retry_timeout() {
        let config = RetryConfig {
            max_retries: 10,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_millis(50), // Very short for testing
            backoff_strategy: BackoffStrategy::Constant,
        };

        let start = Instant::now();

        // Should allow retry initially
        assert!(config.should_retry(0, start));

        // Simulate time passing
        std::thread::sleep(Duration::from_millis(100));

        // Should not allow retry after max duration exceeded
        assert!(!config.should_retry(1, start));
    }
}
