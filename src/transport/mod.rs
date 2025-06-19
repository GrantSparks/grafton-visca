//! Simple, clean transport layer for VISCA communication.
//!
//! ## Core Components
//! - `RawTransport` trait: Simple I/O interface
//! - `ViscaTransport<T>`: Handles all VISCA protocol logic
//! - Built-in implementations: TCP, UDP, Serial
//! - `ChannelTransport`: Thread-safe wrapper

#[cfg(feature = "async-client")]
use std::{fmt::Debug, future::Future, pin::Pin};

#[cfg(feature = "async-client")]
use crate::error::Error;

#[cfg(feature = "async-client")]
/// Type alias for async futures.
pub type TransportFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>;

#[cfg(feature = "async-client")]
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

// Submodules
#[cfg(feature = "async-client")]
mod channel;
#[cfg(feature = "async-client")]
mod implementations;
#[cfg(feature = "async-client")]
mod session;

// Re-exports
#[cfg(feature = "async-client")]
pub use channel::{ChannelConfig, ChannelTransport};
#[cfg(feature = "async-client")]
pub use implementations::{SerialTransport, TcpTransport, UdpTransport};
#[cfg(feature = "async-client")]
pub use session::ViscaTransport;

#[cfg(feature = "async-client")]
/// Simple transport creation functions.
pub mod create {
    use super::*;
    use std::time::Duration;

    /// Create TCP transport.
    pub async fn tcp(address: &str) -> std::io::Result<ViscaTransport<TcpTransport>> {
        let raw = TcpTransport::connect(address).await?;
        Ok(ViscaTransport::new(raw))
    }

    /// Create TCP transport with timeout.
    pub async fn tcp_timeout(
        address: &str,
        timeout: Duration,
    ) -> std::io::Result<ViscaTransport<TcpTransport>> {
        let raw = TcpTransport::connect_timeout(address, timeout).await?;
        Ok(ViscaTransport::new(raw))
    }

    /// Create UDP transport.
    pub async fn udp(address: &str) -> std::io::Result<ViscaTransport<UdpTransport>> {
        let raw = UdpTransport::connect(address).await?;
        Ok(ViscaTransport::new(raw))
    }

    /// Create serial transport.
    pub fn serial(camera_address: u8) -> ViscaTransport<SerialTransport> {
        let raw = SerialTransport::new(camera_address);
        ViscaTransport::new(raw)
    }

    /// Wrap in channel for thread safety.
    pub fn channel<T: RawTransport + 'static>(transport: ViscaTransport<T>) -> ChannelTransport {
        ChannelTransport::new(transport, ChannelConfig::default())
    }
}
