//! Runtime-agnostic transport implementations.
//!
//! These are example implementations that don't depend on any specific async runtime.
//! Users can implement the AsyncTransport trait with their preferred runtime.

use super::AsyncTransport;
use crate::error::Error;
use std::future::Future;
use std::pin::Pin;

/// Example of a custom transport that could work with any runtime.
/// This demonstrates how users can implement their own transports.
#[derive(Debug)]
pub struct CustomTransport {
    #[allow(dead_code)]
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

impl AsyncTransport for CustomTransport {
    type SendFuture<'a> = Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;
    type ReceiveFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;

    fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFuture<'a> {
        let data_copy = data.to_vec();
        Box::pin(async move {
            log::debug!("Custom transport sending: {:02X?}", data_copy);
            // Users would implement actual I/O here using their runtime of choice
            Ok(())
        })
    }

    fn receive(&self) -> Self::ReceiveFuture<'_> {
        Box::pin(async move {
            // Users would implement actual I/O here using their runtime of choice
            // For now, return a mock ACK response
            Ok(vec![0x90, 0x40, 0xFF])
        })
    }
}
