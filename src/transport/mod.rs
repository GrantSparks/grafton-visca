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
//! use grafton_visca::transport::tokio::tcp::Tcp;
//! # #[cfg(feature = "rt-tokio")]
//! use grafton_visca::transport::AsyncTransport;
//!
//! # #[cfg(feature = "rt-tokio")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let transport = Tcp::connect("192.168.0.110:5678").await?;
//! // transport is ready to use with Camera<P, T: AsyncTransport>
//! # Ok(())
//! # }
//! ```

// Transport traits
pub mod async_transport;
pub mod blocking_transport;

// GAT-based Transport trait removed - use AsyncTransport or BlockingTransport

#[cfg(feature = "async")]
pub use async_transport::AsyncTransport;
pub use blocking_transport::BlockingTransport;

// Transport utilities
pub mod envelope;
pub mod frame_parser;

// Blocking transport module (always available)
pub mod blocking;

// Re-export transport implementations
pub use blocking::{Tcp as BlockingTcp, Udp as BlockingUdp};

// Transport envelope for VISCA framing
pub use envelope::TransportEnvelope;

// Tokio implementations
#[cfg(feature = "rt-tokio")]
pub mod tokio;
