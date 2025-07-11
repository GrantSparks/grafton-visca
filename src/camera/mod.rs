//! New camera API with compile-time safety.
//!
//! This module implements the new Camera API where methods only exist
//! for cameras that support the corresponding capabilities.

use std::marker::PhantomData;

use crate::capabilities::ProfileMetadata;
use crate::Error;

// Core camera implementation
pub mod core;

// Facade implementations  
pub mod async_facade;
pub mod blocking_facade;

pub mod methods;
pub mod profiles;
pub mod transport_adapter;

use transport_adapter::TransportAdapter;

/// Camera control interface with compile-time feature detection.
///
/// The Camera type is parameterized by:
/// - `P`: The camera profile implementing capability traits
/// - `T`: The transport type (blocking or async)
///
/// Methods are added to Camera via blanket trait implementations
/// based on which capability traits the profile implements.
#[derive(Debug)]
pub struct Camera<P, T>
where
    P: ProfileMetadata,
{
    profile: PhantomData<P>,
    transport: TransportAdapter<T>,
}

// Core implementation for all cameras
impl<P, T> Camera<P, T>
where
    P: ProfileMetadata,
{
    /// Create a new camera with the given transport.
    pub fn new(transport: T) -> Self {
        Self {
            profile: PhantomData,
            transport: TransportAdapter::new(transport),
        }
    }

    /// Get the camera model name.
    pub fn model_name(&self) -> &'static str {
        P::MODEL_NAME
    }

    /// Get a reference to the transport adapter.
    pub fn transport(&self) -> &TransportAdapter<T> {
        &self.transport
    }

    /// Get a mutable reference to the transport adapter.
    pub fn transport_mut(&mut self) -> &mut TransportAdapter<T> {
        &mut self.transport
    }
}

// Blocking transport implementation
#[cfg(not(feature = "async"))]
impl<P, T> Camera<P, T>
where
    P: ProfileMetadata,
    T: crate::transport::blocking::BlockingTransport,
{
    /// Send a raw VISCA command and wait for response.
    ///
    /// This is a low-level method. Prefer the high-level capability methods.
    pub fn send_raw(&mut self, command: &[u8]) -> Result<Vec<u8>, Error> {
        self.transport.send_blocking(command)
    }

    /// Send a const command (zero allocation).
    pub fn send_const(&mut self, command: &'static [u8]) -> Result<(), Error> {
        self.transport.send_const_blocking(command)?;
        Ok(())
    }

    /// Send a stack-allocated command.
    pub fn send_array<const N: usize>(&mut self, command: [u8; N]) -> Result<(), Error> {
        self.transport.send_array_blocking(command)?;
        Ok(())
    }
}

// Async transport implementation
#[cfg(feature = "async")]
impl<P, T> Camera<P, T>
where
    P: ProfileMetadata,
    T: crate::transport::AsyncTransport,
{
    /// Send a raw VISCA command and wait for response.
    ///
    /// This is a low-level method. Prefer the high-level capability methods.
    pub async fn send_raw(&self, command: &[u8]) -> Result<Vec<u8>, Error> {
        self.transport.send_async(command).await
    }

    /// Send a const command (zero allocation).
    pub async fn send_const(&self, command: &'static [u8]) -> Result<(), Error> {
        self.transport.send_const_async(command).await?;
        Ok(())
    }

    /// Send a stack-allocated command.
    pub async fn send_array<const N: usize>(&self, command: [u8; N]) -> Result<(), Error> {
        self.transport.send_array_async(command).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::{PTZOpticsG2, SonyFR7};

    #[test]
    fn test_camera_creation() {
        // Mock transport
        struct MockTransport;

        let camera: Camera<PTZOpticsG2, MockTransport> = Camera::new(MockTransport);
        assert_eq!(camera.model_name(), "PTZOptics G2");

        let camera: Camera<SonyFR7, MockTransport> = Camera::new(MockTransport);
        assert_eq!(camera.model_name(), "Sony FR7");
    }
}
