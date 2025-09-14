//! Generic async serial transport implementation with zero-cost abstractions.
//!
//! This module provides a runtime-agnostic serial transport that works with any
//! stream type implementing the AsyncReadExt and AsyncWriteExt traits.

use crate::{
    transport::{
        async_io::{write_all_flush, AsyncReadExt, AsyncWriteExt},
        async_transport::HasTransportConfig,
        builder::TransportConfig,
        AsyncTransport,
    },
    Error,
};

/// Generic serial transport for async VISCA communication.
///
/// This transport uses native async functions without boxing and unified helpers
/// to reduce code duplication across runtimes. It works with any stream type
/// implementing the AsyncReadExt and AsyncWriteExt traits.
///
/// The transport does not include any internal timeouts - timeouts are handled
/// by the runtime scheduler which races sends against configured timeouts.
#[derive(Debug)]
pub struct Serial<S: AsyncReadExt + AsyncWriteExt> {
    pub(crate) stream: S,
    config: TransportConfig,
}

impl<S: AsyncReadExt + AsyncWriteExt> Serial<S> {
    /// Create a new serial transport from a connected stream.
    ///
    /// The stream should already be configured and connected to the serial port.
    /// Any initialization (I/F Clear, Address Set) should be performed by the
    /// runtime-specific connector before constructing this transport.
    pub fn new(stream: S, config: TransportConfig) -> Self {
        Self { stream, config }
    }

    /// Create a new serial transport with default configuration.
    ///
    /// Uses default buffer configuration for raw VISCA protocol.
    pub fn new_default(stream: S) -> Self {
        Self::new(stream, TransportConfig::default())
    }

    /// Get the transport configuration.
    pub fn config(&self) -> &TransportConfig {
        &self.config
    }
}

impl<S: AsyncReadExt + AsyncWriteExt + Send> AsyncTransport for Serial<S> {
    async fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
        // No internal timeout - rely on the scheduler's timeout
        write_all_flush(&mut self.stream, bytes).await
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        // Read chunk of data directly into the provided buffer
        let n = self.stream.read(dst).await?;

        if n == 0 {
            return Err(Error::ConnectionClosed {
                reason: Some(std::borrow::Cow::Borrowed("serial port closed")),
            });
        }

        Ok(n)
    }
}

impl<S: AsyncReadExt + AsyncWriteExt> HasTransportConfig for Serial<S> {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}
