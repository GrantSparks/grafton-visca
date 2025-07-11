//! Transport layer for VISCA communication.
//!
//! This module provides both blocking and async transport implementations using
//! a unified `Transport` trait with Generic Associated Types (GATs).
//!
//! ## Architecture
//!
//! The transport layer uses GATs to provide a single trait that works for both
//! blocking and async implementations:
//! - **Transport trait** - Unified interface using GAT futures
//! - **ViscaProtocol** - Handles VISCA protocol logic (ACK/completion responses)
//! - **Implementations** - TCP and UDP for both blocking and async
//!
//! ## Usage
//!
//! For blocking transports:
//! ```rust,no_run
//! use grafton_visca::transport::blocking::Tcp;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let transport = Tcp::connect("192.168.1.100:5678")?;
//! // transport is ready to use with CameraBlocking
//! # Ok(())
//! # }
//! ```
//!
//! For async transports (with tokio):
//! ```rust,no_run
//! # #[cfg(feature = "tokio")]
//! use grafton_visca::transport::tokio::tcp::Tcp;
//!
//! # #[cfg(feature = "tokio")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let transport = Tcp::connect("192.168.1.100:5678").await?;
//! // transport is ready to use with CameraAsync
//! # Ok(())
//! # }
//! ```

// Transport trait and utilities
pub mod any_transport;
pub mod gat_transport;
pub use any_transport::AnyTransport;
pub use gat_transport::{Transport, TransportExt};

// Blocking transport module (always available)
pub mod blocking;

// Re-export transport implementations
pub use blocking::{Tcp as BlockingTcp, Udp as BlockingUdp};

// VISCA protocol
pub mod visca_protocol;
pub use visca_protocol::ViscaProtocol;

// Tokio implementations
#[cfg(all(feature = "async", feature = "tokio"))]
pub mod tokio;
