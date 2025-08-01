//! Async-trait based transport implementation.
//!
//! This module provides a simplified transport abstraction using async-trait
//! to eliminate manual Future implementations and unsafe code.

use async_trait::async_trait;
use bytes::Bytes;

use crate::Error;

/// Low-level VISCA byte transport using async-trait.
///
/// This trait provides a unified interface for both blocking and async transports
/// without the complexity of GATs and manual Future implementations.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send raw bytes to the device.
    async fn send(&self, bytes: &[u8]) -> Result<(), Error>;

    /// Receive raw bytes from the device.
    /// Returns exactly one VISCA frame.
    async fn recv(&self) -> Result<Bytes, Error>;
}

/// Marker trait for blocking transports.
///
/// This trait is implemented by transports that provide synchronous operations.
/// It's used to enforce at compile time that `CameraBlocking` can only be used
/// with blocking transports.
pub trait BlockingTransport: Transport {
    /// Send raw bytes to the device (blocking).
    fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error>;

    /// Receive raw bytes from the device (blocking).
    /// Returns exactly one VISCA frame.
    fn recv_blocking(&self) -> Result<Bytes, Error>;
}