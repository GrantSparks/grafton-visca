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
//! ## Async Transport (Optional)
//! - `RawTransport` trait: Asynchronous I/O interface
//! - `ViscaTransport<T>`: Async protocol handling
//! - `ChannelTransport`: Thread-safe wrapper

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
#[cfg(feature = "async")]
mod channel;
#[cfg(feature = "async")]
mod runtime_agnostic;
#[cfg(feature = "async")]
mod session;

// Async implementations
#[cfg(feature = "async")]
mod implementations;
#[cfg(feature = "async")]
mod tokio_impl;

// Re-exports for async support
#[cfg(feature = "async")]
pub use runtime_agnostic::{CustomTransport, SerialTransport};
#[cfg(feature = "async")]
pub use session::ViscaTransport;

// Re-export channel transport
#[cfg(feature = "async")]
pub use channel::{ChannelConfig, ChannelTransport};

// Re-export tokio implementations
#[cfg(feature = "async")]
pub use tokio_impl::{TokioTcpTransport, TokioUdpTransport};

// Backward compatibility - keep old names
#[cfg(feature = "async")]
pub use implementations::{TcpTransport, UdpTransport};

// Convenience functions for creating transports
#[cfg(feature = "async")]
/// Simple transport creation functions.
pub mod create {
    use super::*;
    use std::time::Duration;

    /// Create TCP transport using tokio.
    pub async fn tcp(address: &str) -> std::io::Result<ViscaTransport<TcpTransport>> {
        let raw = TcpTransport::connect(address).await?;
        Ok(ViscaTransport::new(raw))
    }

    /// Create TCP transport with timeout using tokio.
    pub async fn tcp_timeout(
        address: &str,
        timeout: Duration,
    ) -> std::io::Result<ViscaTransport<TcpTransport>> {
        let raw = TcpTransport::connect_timeout(address, timeout).await?;
        Ok(ViscaTransport::new(raw))
    }

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
    pub fn channel<T: RawTransport + 'static>(transport: ViscaTransport<T>) -> ChannelTransport {
        ChannelTransport::new(transport, ChannelConfig::default())
    }
}
