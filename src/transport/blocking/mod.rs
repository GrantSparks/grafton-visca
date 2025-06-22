//! Blocking transport implementations for VISCA communication.
//!
//! This module provides synchronous I/O for VISCA camera control,
//! designed as the primary API for most use cases.

use std::time::Duration;

use crate::error::Error;

/// Core blocking transport trait.
///
/// Implement this trait to create new transport types for VISCA communication.
/// This trait handles raw I/O operations; protocol logic is handled by ViscaTransport.
pub trait Transport: Send + Sync + std::fmt::Debug {
    /// Send raw bytes to the device.
    fn send(&mut self, data: &[u8]) -> Result<(), Error>;

    /// Receive raw bytes from the device with timeout.
    fn receive(&mut self, timeout: Duration) -> Result<Vec<u8>, Error>;

    /// Check if the transport is connected.
    fn is_connected(&self) -> bool;

    /// Get a description of this transport.
    fn description(&self) -> &str;
}

// Implement Transport for Box<dyn Transport> to allow dynamic dispatch
impl Transport for Box<dyn Transport> {
    fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        (**self).send(data)
    }

    fn receive(&mut self, timeout: Duration) -> Result<Vec<u8>, Error> {
        (**self).receive(timeout)
    }

    fn is_connected(&self) -> bool {
        (**self).is_connected()
    }

    fn description(&self) -> &str {
        (**self).description()
    }
}

// Submodules
mod tcp;
mod udp;

// Re-exports
pub use tcp::TcpTransport;
pub use udp::UdpTransport;

/// Convenience functions for creating transports.
pub mod create {
    use super::*;
    use std::net::ToSocketAddrs;

    /// Create a TCP transport connected to the given address.
    pub fn tcp<A: ToSocketAddrs>(address: A) -> std::io::Result<TcpTransport> {
        TcpTransport::connect(address)
    }

    /// Create a TCP transport with custom timeout.
    pub fn tcp_timeout<A: ToSocketAddrs>(
        address: A,
        timeout: Duration,
    ) -> std::io::Result<TcpTransport> {
        TcpTransport::connect_timeout(address, timeout)
    }

    /// Create a UDP transport connected to the given address.
    pub fn udp<A: ToSocketAddrs>(address: A) -> std::io::Result<UdpTransport> {
        UdpTransport::connect(address)
    }
}
