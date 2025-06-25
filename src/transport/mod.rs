//! Transport layer for VISCA communication.
//!
//! This module provides both blocking and async transport implementations with a
//! unified `Transport` that handles all VISCA protocol details.
//!
//! ## Architecture
//!
//! The transport layer is designed with clear separation of concerns:
//! - **Transport traits** (`blocking::BlockingTransport`, `AsyncTransport`) - Define I/O interfaces
//! - **ViscaProtocol** - Handles VISCA protocol logic (socket management, response correlation)
//! - **Implementations** - TCP, UDP, and custom transports
//!
//! ## Blocking Transport
//!
//! For applications without async requirements:
//! ```rust,no_run
//! use grafton_visca::transport::blocking::create;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let transport = create::tcp("192.168.1.100:5678")?;
//! // transport is ready to use with Camera
//! # Ok(())
//! # }
//! ```
//!
//! ## Async Transport
//!
//! The async support is runtime-agnostic. The library provides tokio implementations,
//! but you can implement `AsyncTransport` for any runtime:
//!
//! ```rust,no_run
//! # #[cfg(feature = "tokio")]
//! use grafton_visca::transport::create;
//!
//! # #[cfg(feature = "tokio")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let transport = create::tcp("192.168.1.100:5678").await?;
//! // transport is ready to use with async Camera methods
//! # Ok(())
//! # }
//! ```
//!
//! ## Custom Transports
//!
//! Implement the appropriate trait for your transport type:
//! - `blocking::BlockingTransport` for blocking I/O
//! - `AsyncTransport` for async I/O
//!
//! The `ViscaProtocol` wrapper handles all protocol details automatically.

// Blocking transport module (always available)
pub mod blocking;

// Re-export blocking types for easier access
pub use blocking::{BlockingTransport, Tcp as BlockingTcp, Udp as BlockingUdp};

// Unified VISCA transport implementation
mod visca_transport;

// Internal modules
mod internal {
    // Synchronization primitives
    #[cfg(feature = "async")]
    pub(super) mod sync_primitives;
}

// Async transport modules (optional)
#[cfg(feature = "async")]
mod runtime_agnostic;

// Simplified async transport trait
#[cfg(feature = "async")]
mod async_transport;

// Clean tokio implementations
#[cfg(all(feature = "async", feature = "tokio"))]
pub mod tokio;

// Export the new simplified async transport trait
#[cfg(feature = "async")]
pub use async_transport::AsyncTransport;

// Export the unified Transport
pub use visca_transport::ViscaProtocol;

/// Convenience functions for creating async transports.
#[cfg(all(feature = "async", feature = "tokio"))]
pub mod create {
    use super::tokio::{Tcp, Udp};
    use crate::Error;

    /// Create an async TCP transport connected to the given address.
    pub async fn tcp(address: &str) -> Result<Tcp, Error> {
        Tcp::connect(address).await
    }

    /// Create an async TCP transport with custom timeout.
    pub async fn tcp_timeout(address: &str, timeout: std::time::Duration) -> Result<Tcp, Error> {
        Tcp::connect_timeout(address, timeout).await
    }

    /// Create an async UDP transport connected to the given address.
    pub async fn udp(address: &str) -> Result<Udp, Error> {
        Udp::connect(address).await
    }
}
