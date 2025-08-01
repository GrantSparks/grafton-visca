//! Transport layer for VISCA communication.
//!
//! This module provides both blocking and async transport implementations using
//! async-trait for clean, safe abstractions.
//!
//! ## Architecture
//!
//! The transport layer uses async-trait to provide clean transport abstractions:
//! - **Transport trait** - Async interface using async-trait
//! - **BlockingTransport** - Marker trait for blocking implementations
//! - **ViscaProtocol** - Handles VISCA protocol logic (ACK/completion responses)
//! - **Implementations** - TCP and UDP for both blocking and async
//!
//! ## Usage
//!
//! For blocking transports:
//! ```rust,no_run
//! use grafton_visca::transport::TcpTransportBlocking;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let transport = TcpTransportBlocking::connect("192.168.1.100:5678")?;
//! // transport is ready to use with Camera::new_blocking()
//! # Ok(())
//! # }
//! ```
//!
//! For async transports (with tokio):
//! ```rust,no_run
//! # #[cfg(feature = "tokio")]
//! use grafton_visca::transport::TcpTransport;
//!
//! # #[cfg(feature = "tokio")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let transport = TcpTransport::connect("192.168.1.100:5678").await?;
//! // transport is ready to use with Camera::new_async()
//! # Ok(())
//! # }
//! ```

// Transport trait and utilities
pub mod envelope;
pub mod unified;

// New async-trait based transport architecture
pub mod async_trait_transport;
pub mod visca_io;
pub mod blocking_adapter;
#[cfg(feature = "tokio")]
pub mod tokio_adapter;

// Re-export main transport trait and types
pub use async_trait_transport::{Transport, BlockingTransport};
pub use unified::{UnifiedTransport, NewAsyncTransportWrapper, BlockingTransportWrapper};

// Re-export transport implementations
pub use blocking_adapter::{TcpTransportBlocking, UdpTransportBlocking};
#[cfg(feature = "tokio")]
pub use tokio_adapter::{TcpTransport, UdpTransport, TimeoutTransport};

// VISCA protocol and envelope
pub mod visca_protocol;
pub use envelope::TransportEnvelope;
pub use visca_protocol::ViscaProtocol;
