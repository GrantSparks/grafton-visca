//! Generic async TCP transport implementation with zero-cost abstractions.
//!
//! This module provides a runtime-agnostic TCP transport that works with any
//! stream type implementing the AsyncReadExt and AsyncWriteExt traits.

use bytes::Bytes;

use crate::{
    transport::{
        async_io::{write_all_flush, AsyncReadExt, AsyncWriteExt},
        buffer::{BufferConfig, BufferManager},
        builder::TransportConfig,
        AsyncTransport,
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
    pub(crate) buffer_manager: BufferManager,
}

impl<S: AsyncReadExt + AsyncWriteExt> Tcp<S> {
    /// Create a new TCP transport from a connected stream.
    ///
    /// The stream should already be connected to the remote endpoint.
    pub fn new(stream: S, config: TransportConfig) -> Self {
        Self {
            stream,
            buffer_manager: BufferManager::new(config.buffer_config),
        }
    }

    /// Create a new TCP transport with default configuration.
    ///
    /// Uses default buffer configuration for raw IP protocol.
    pub fn new_default(stream: S) -> Self {
        Self::new(
            stream,
            TransportConfig {
                buffer_config: BufferConfig::for_raw_ip(),
                ..Default::default()
            },
        )
    }
}

impl<S: AsyncReadExt + AsyncWriteExt + Send> AsyncTransport for Tcp<S> {
    async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        write_all_flush(&mut self.stream, data).await
    }

    async fn recv(&mut self) -> Result<Bytes, Error> {
        // Read chunk of data into buffer and return it
        let mut buffer = self.buffer_manager.alloc_vec_buffer();
        let n = self.stream.read(&mut buffer).await?;

        if n == 0 {
            return Err(Error::ConnectionClosed {
                reason: Some(std::borrow::Cow::Borrowed("peer closed connection")),
            });
        }

        Ok(self.buffer_manager.process_recv_data(buffer, n))
    }
}
