//! Generic async TCP transport implementation with zero-cost abstractions.
//!
//! This module provides a runtime-agnostic TCP transport that works with any
//! stream type implementing the AsyncReadExt and AsyncWriteExt traits.

use crate::{
    transport::{
        async_io::{write_all_flush, AsyncReadExt, AsyncWriteExt},
        builder::TransportConfig,
        AddressingMode, AsyncTransport, HasTransportConfig,
    },
    Error,
};

/// Generic TCP transport for async VISCA communication.
///
/// This transport uses native async functions without boxing and unified helpers
/// to reduce code duplication across runtimes. It works with any stream type
/// implementing the AsyncReadExt and AsyncWriteExt traits.
#[derive(Debug)]
pub struct Tcp<S: AsyncReadExt + AsyncWriteExt> {
    pub(crate) stream: S,
    config: TransportConfig,
}

impl<S: AsyncReadExt + AsyncWriteExt> Tcp<S> {
    /// Create a new TCP transport from a connected stream.
    ///
    /// The stream should already be connected to the remote endpoint.
    pub fn new(stream: S, config: TransportConfig) -> Self {
        Self { stream, config }
    }

    /// Create a new TCP transport with default configuration.
    ///
    /// Uses default buffer configuration for raw IP protocol.
    pub fn new_default(stream: S) -> Self {
        Self::new(stream, TransportConfig::default())
    }

    /// Get the transport configuration.
    pub fn config(&self) -> &TransportConfig {
        &self.config
    }
}

impl<S: AsyncReadExt + AsyncWriteExt + Send> AsyncTransport for Tcp<S> {
    async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        write_all_flush(&mut self.stream, data).await
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        // Read chunk of data directly into the provided buffer
        let n = self.stream.read(dst).await?;

        if n == 0 {
            return Err(Error::ConnectionClosed {
                reason: Some(std::borrow::Cow::Borrowed("peer closed connection")),
            });
        }

        Ok(n)
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(AddressingMode::Ip)
    }
}

impl<S: AsyncReadExt + AsyncWriteExt> HasTransportConfig for Tcp<S> {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }

    fn standard_transport_kind(&self) -> Option<crate::camera::TransportKind> {
        Some(crate::camera::TransportKind::Tcp)
    }
}
