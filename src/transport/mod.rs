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
//! - **BlockingTransport** - Synchronous methods with OS-level timeout support
//! - **ViscaProtocol** - Handles VISCA protocol logic (ACK/completion responses)
//! - **Implementations** - TCP and UDP for both blocking and async
//!
//! ## Usage
//!
//! For blocking transports:
//! ```rust,no_run
//! use grafton_visca::transport::blocking::Tcp;
//! use grafton_visca::transport::BlockingTransport;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let transport = Tcp::connect("192.168.0.110:5678")?;
//! // transport is ready to use with Camera<P, T: BlockingTransport>
//! # Ok(())
//! # }
//! ```
//!
//! For async transports (with tokio):
//! ```rust,no_run
//! # #[cfg(feature = "rt-tokio")]
//! use grafton_visca::runtime_adapters::tokio::TcpTransport;
//! # #[cfg(feature = "rt-tokio")]
//! use grafton_visca::transport::AsyncTransport;
//!
//! # #[cfg(feature = "rt-tokio")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let transport = TcpTransport::connect("192.168.0.110:5678").await?;
//! // transport is ready to use with Camera<P, T: AsyncTransport>
//! # Ok(())
//! # }
//! ```

pub mod address;
pub mod async_transport;
pub mod blocking;
pub mod blocking_transport;
pub mod buffer;
pub mod builder;
pub mod envelope;
pub mod frame_parser;
pub mod ip_raw;
pub mod ip_sony;
pub mod retry;
#[cfg(feature = "serial")]
pub mod serial;
pub mod timeout;

// Runtime-specific transport implementations are feature-gated extensions
// They should be accessed through the runtime_adapters module
#[cfg(feature = "rt-tokio")]
pub(crate) mod tokio;

#[cfg(feature = "rt-async-std")]
pub(crate) mod async_std;

#[cfg(feature = "rt-smol")]
pub(crate) mod smol;

use std::time::{Duration, Instant};

#[cfg(feature = "async")]
pub use async_transport::AsyncTransport;
// Gate blocking exports so they only appear without async
#[cfg(not(feature = "async"))]
pub use blocking::{Tcp as BlockingTcp, Udp as BlockingUdp};
pub use blocking_transport::BlockingTransport;

/// Retry configuration for transport layer operations.
///
/// Provides configurable retry logic for handling transient failures
/// in VISCA communication across all transport types.
#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Base delay between retries (will be adjusted based on error type).
    pub base_retry_delay: Duration,
    /// Maximum total time to spend retrying.
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
    use super::*;
    use std::time::{Duration, Instant};

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
