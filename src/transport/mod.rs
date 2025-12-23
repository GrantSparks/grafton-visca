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

use std::time::{Duration, Instant};

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
#[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
pub(crate) mod tokio;

#[cfg(feature = "mode-async")]
pub use async_transport::AsyncTransport;
#[cfg(not(feature = "mode-async"))]
pub use blocking_transport::BlockingTransportHandle;
pub use blocking_transport::{BlockingTransport, HasTransportConfig};
#[cfg(not(feature = "mode-async"))]
pub use builder::{NetTransportBuilder, Transport, TransportBuilderExt};

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
    /// Whether to use exponential backoff.
    pub exponential_backoff: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(10),
            exponential_backoff: true,
        }
    }
}

impl RetryConfig {
    /// Calculate the retry delay for a given attempt.
    ///
    /// If exponential backoff is enabled, the delay doubles with each attempt.
    /// The delay is capped by the maximum retry duration.
    pub fn calculate_delay(
        &self,
        attempt: u32,
        error_suggested_delay: Option<Duration>,
    ) -> Duration {
        let base = error_suggested_delay.unwrap_or(self.base_retry_delay);

        if self.exponential_backoff {
            let multiplier = 2_u32.saturating_pow(attempt.saturating_sub(1));
            base.saturating_mul(multiplier)
        } else {
            base
        }
    }

    /// Check if retry should continue based on elapsed time.
    pub fn should_retry(&self, attempt: u32, start_time: Instant) -> bool {
        if attempt >= self.max_retries {
            return false;
        }

        start_time.elapsed() < self.max_retry_duration
    }
}

#[cfg(test)]
mod retry_tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_retry_delay, Duration::from_millis(100));
        assert_eq!(config.max_retry_duration, Duration::from_secs(10));
        assert!(config.exponential_backoff);
    }

    #[test]
    fn test_calculate_delay_without_backoff() {
        let config = RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(10),
            exponential_backoff: false,
        };

        // Without backoff, delay should be constant
        assert_eq!(config.calculate_delay(1, None), Duration::from_millis(100));
        assert_eq!(config.calculate_delay(2, None), Duration::from_millis(100));
        assert_eq!(config.calculate_delay(3, None), Duration::from_millis(100));
    }

    #[test]
    fn test_calculate_delay_with_exponential_backoff() {
        let config = RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(10),
            exponential_backoff: true,
        };

        // With exponential backoff, delay should double each time
        assert_eq!(config.calculate_delay(1, None), Duration::from_millis(100));
        assert_eq!(config.calculate_delay(2, None), Duration::from_millis(200));
        assert_eq!(config.calculate_delay(3, None), Duration::from_millis(400));
    }

    #[test]
    fn test_calculate_delay_with_error_suggestion() {
        let config = RetryConfig::default();

        // Should use error's suggested delay if provided
        let suggested = Some(Duration::from_millis(500));
        assert_eq!(
            config.calculate_delay(1, suggested),
            Duration::from_millis(500)
        );
        assert_eq!(
            config.calculate_delay(2, suggested),
            Duration::from_millis(1000)
        );
    }

    #[test]
    fn test_should_retry_max_attempts() {
        let config = RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(10),
            exponential_backoff: false,
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
            exponential_backoff: false,
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
