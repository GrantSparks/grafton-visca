//! Runtime-agnostic transport implementations.
//!
//! These are example implementations that don't depend on any specific async runtime.
//! Users can implement the RawTransport trait with their preferred runtime.

use super::{RawTransport, TransportFuture};

/// Serial transport implementation (runtime-agnostic).
/// This is a mock implementation for demonstration purposes.
#[derive(Debug)]
pub struct SerialTransport {
    camera_address: u8,
    description: String,
}

impl SerialTransport {
    /// Create a new serial transport.
    /// In a real implementation, this would take a serial port configuration.
    pub fn new(camera_address: u8) -> Self {
        Self {
            camera_address,
            description: format!("Serial connection to camera {}", camera_address),
        }
    }
}

impl RawTransport for SerialTransport {
    fn send<'a>(&'a mut self, data: &'a [u8]) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            // In a real implementation, this would use a serial port library
            log::debug!(
                "Serial write to camera {}: {:02X?}",
                self.camera_address,
                data
            );
            Ok(())
        })
    }

    fn receive(&mut self) -> TransportFuture<'_, Vec<u8>> {
        Box::pin(async move {
            // Mock response for demonstration
            let response = vec![0x90, 0x40, 0xFF]; // ACK
            log::debug!(
                "Serial read from camera {}: {:02X?}",
                self.camera_address,
                response
            );
            Ok(response)
        })
    }

    fn is_connected(&self) -> bool {
        // In a real implementation, this would check the serial port state
        true
    }

    fn description(&self) -> &str {
        &self.description
    }
}

/// Example of a custom transport that could work with any runtime.
/// This demonstrates how users can implement their own transports.
#[derive(Debug)]
pub struct CustomTransport {
    description: String,
    // In a real implementation, this might contain:
    // - A channel for sending/receiving data
    // - A handle to a background task
    // - Runtime-specific types wrapped in a trait object
}

impl CustomTransport {
    /// Create a new custom transport with the given description.
    pub fn new(description: String) -> Self {
        Self { description }
    }
}

impl RawTransport for CustomTransport {
    fn send<'a>(&'a mut self, data: &'a [u8]) -> TransportFuture<'a, ()> {
        let data_copy = data.to_vec();
        Box::pin(async move {
            log::debug!("Custom transport sending: {:02X?}", data_copy);
            // Users would implement actual I/O here using their runtime of choice
            Ok(())
        })
    }

    fn receive(&mut self) -> TransportFuture<'_, Vec<u8>> {
        Box::pin(async move {
            // Users would implement actual I/O here using their runtime of choice
            // For now, return a mock ACK response
            Ok(vec![0x90, 0x40, 0xFF])
        })
    }

    fn is_connected(&self) -> bool {
        true
    }

    fn description(&self) -> &str {
        &self.description
    }
}
