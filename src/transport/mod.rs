//! Transport layer for VISCA communication.
//!
//! This module provides both blocking and async transport implementations.
//! The blocking API is the primary interface, with async support as an optional feature.
//!
//! ## Blocking Transport (Default)
//! - `blocking::Transport` trait: Synchronous I/O interface
//! - `blocking::ViscaTransport<T>`: Handles VISCA protocol logic
//! - Built-in implementations: TCP, UDP
//!
//! ## Async Transport (Optional, Runtime-Agnostic)
//! The async transport layer is designed to work with ANY async runtime (tokio, async-std, smol, etc.)
//! without forcing a specific runtime dependency.
//!
//! ### Core Traits:
//! - `AsyncTransport`: Simple trait for implementing async transports with any runtime
//! - `AsyncTransportAdapter`: Adapter to use AsyncTransport implementations
//! - `RawTransport`: Low-level async I/O interface (used internally)
//! - `ViscaTransport<T>`: Async protocol handling
//!
//! ### Using with Different Runtimes:
//! ```rust,ignore
//! // Example with async-std
//! use async_std::net::TcpStream;
//! use grafton_visca::transport::{AsyncTransport, AsyncTransportAdapter};
//!
//! struct AsyncStdTransport { stream: TcpStream }
//!
//! impl AsyncTransport for AsyncStdTransport {
//!     // ... implement send() and receive() using async-std's APIs
//! }
//!
//! // Then use with AsyncTransportAdapter
//! let adapter = AsyncTransportAdapter::new(transport, "async-std".into());
//! ```
//!
//! See the examples directory for complete implementations with different runtimes.

#[cfg(feature = "async")]
use std::{fmt::Debug, future::Future, pin::Pin};

#[cfg(feature = "async")]
use crate::error::Error;

#[cfg(feature = "async")]
/// Type alias for async futures.
pub type TransportFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>;

#[cfg(feature = "async")]
/// Raw transport trait - implement this to create new transport types.
///
/// Only handles basic I/O. All VISCA protocol logic is in ViscaTransport.
pub trait RawTransport: Send + Sync + Debug {
    /// Send raw bytes.
    fn send<'a>(&'a mut self, data: &'a [u8]) -> TransportFuture<'a, ()>;

    /// Receive raw bytes.
    fn receive(&mut self) -> TransportFuture<'_, Vec<u8>>;

    /// Check if connected.
    fn is_connected(&self) -> bool;

    /// Get description.
    fn description(&self) -> &str;
}

// Blocking transport module (always available)
pub mod blocking;

// Re-export blocking types for easier access
pub use blocking::{
    TcpTransport as BlockingTcpTransport, Transport as BlockingTransport,
    UdpTransport as BlockingUdpTransport, ViscaTransport as BlockingViscaTransport,
};

// For backward compatibility
pub use blocking::Transport;

// Async transport modules (optional)
#[cfg(all(feature = "async", feature = "tokio"))]
mod channel;
#[cfg(feature = "async")]
mod runtime_agnostic;
#[cfg(feature = "async")]
mod session;

// New simplified async transport approach
#[cfg(feature = "async")]
mod async_transport;
#[cfg(all(feature = "async", feature = "tokio"))]
mod tokio_simple;

// Async implementations
#[cfg(all(feature = "async", feature = "tokio"))]
mod implementations;
#[cfg(all(feature = "async", feature = "tokio"))]
mod tokio_impl;

// Re-exports for async support
#[cfg(feature = "async")]
pub use runtime_agnostic::{CustomTransport, SerialTransport};
#[cfg(feature = "async")]
pub use session::ViscaTransport;

// Export the new simplified async transport trait
#[cfg(feature = "async")]
pub use async_transport::{AsyncTransport, AsyncTransportAdapter};

// Export simplified tokio transports when tokio is enabled
#[cfg(all(feature = "async", feature = "tokio"))]
pub use tokio_simple::{SimpleTokioTcpTransport, SimpleTokioUdpTransport};

// Re-export channel transport based on features
#[cfg(all(feature = "async", feature = "tokio"))]
pub use channel::{ChannelConfig, ChannelTransport};

// Re-export tokio implementations when tokio is enabled
#[cfg(all(feature = "async", feature = "tokio"))]
pub use tokio_impl::{TokioTcpTransport, TokioUdpTransport};

// Backward compatibility - keep old names when tokio is enabled
#[cfg(all(feature = "async", feature = "tokio"))]
pub use implementations::{TcpTransport, UdpTransport};

// Convenience functions for creating transports
#[cfg(feature = "async")]
/// Simple transport creation functions.
pub mod create {
    use super::*;
    use std::time::Duration;

    // When tokio feature is enabled, use tokio-specific transports for backwards compatibility
    #[cfg(feature = "tokio")]
    /// Create TCP transport using tokio.
    pub async fn tcp(address: &str) -> std::io::Result<ViscaTransport<TcpTransport>> {
        let raw = TcpTransport::connect(address).await?;
        Ok(ViscaTransport::new(raw))
    }

    #[cfg(feature = "tokio")]
    /// Create TCP transport with timeout using tokio.
    pub async fn tcp_timeout(
        address: &str,
        timeout: Duration,
    ) -> std::io::Result<ViscaTransport<TcpTransport>> {
        let raw = TcpTransport::connect_timeout(address, timeout).await?;
        Ok(ViscaTransport::new(raw))
    }

    #[cfg(feature = "tokio")]
    /// Create UDP transport using tokio.
    pub async fn udp(address: &str) -> std::io::Result<ViscaTransport<UdpTransport>> {
        let raw = UdpTransport::connect(address).await?;
        Ok(ViscaTransport::new(raw))
    }

    /// Create serial transport (runtime-agnostic).
    pub fn serial(camera_address: u8) -> ViscaTransport<SerialTransport> {
        let raw = SerialTransport::new(camera_address);
        ViscaTransport::new(raw)
    }

    /// Wrap in channel for thread safety.
    #[cfg(feature = "tokio")]
    pub fn channel<T: RawTransport + 'static>(transport: ViscaTransport<T>) -> ChannelTransport {
        ChannelTransport::new(transport, ChannelConfig::default())
    }

    /// Simplified async transport creation.
    #[cfg(feature = "tokio")]
    pub mod simple {
        use super::*;
        use crate::transport::{
            AsyncTransportAdapter, SimpleTokioTcpTransport, SimpleTokioUdpTransport,
        };

        /// Create a simple TCP transport without Arc<Mutex<>> overhead.
        pub async fn tcp(
            address: &str,
        ) -> Result<ViscaTransport<AsyncTransportAdapter<SimpleTokioTcpTransport>>, Error> {
            let tcp = SimpleTokioTcpTransport::connect(address).await?;
            let adapter = AsyncTransportAdapter::new(tcp, format!("TCP {}", address));
            Ok(ViscaTransport::new(adapter))
        }

        /// Create a simple UDP transport without Arc<Mutex<>> overhead.
        pub async fn udp(
            local_addr: &str,
            remote_addr: &str,
        ) -> Result<ViscaTransport<AsyncTransportAdapter<SimpleTokioUdpTransport>>, Error> {
            let udp = SimpleTokioUdpTransport::new(local_addr, remote_addr).await?;
            let adapter =
                AsyncTransportAdapter::new(udp, format!("UDP {} -> {}", local_addr, remote_addr));
            Ok(ViscaTransport::new(adapter))
        }
    }
}
