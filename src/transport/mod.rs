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
//! use grafton_visca::transport::blocking::TcpGat;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let transport = TcpGat::connect("192.168.1.100:5678")?;
//! // transport is ready to use with CameraBlocking
//! # Ok(())
//! # }
//! ```
//!
//! For async transports (with tokio):
//! ```rust,no_run
//! # #[cfg(feature = "tokio")]
//! use grafton_visca::transport::tokio::tcp_gat::TcpGat;
//!
//! # #[cfg(feature = "tokio")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let transport = TcpGat::connect("192.168.1.100:5678").await?;
//! // transport is ready to use with CameraAsync
//! # Ok(())
//! # }
//! ```

// New GAT-based transport trait
pub mod gat_transport;
pub mod any_transport;
pub use gat_transport::{Transport, TransportExt};
pub use any_transport::AnyTransport;

// Blocking transport module (always available)
pub mod blocking;

// Re-export GAT transport implementations
pub use blocking::{TcpGat as BlockingTcp, UdpGat as BlockingUdp};

// GAT-based VISCA protocol
pub mod visca_protocol_gat;
pub use visca_protocol_gat::ViscaProtocol;

// Tokio implementations
#[cfg(all(feature = "async", feature = "tokio"))]
pub mod tokio;

