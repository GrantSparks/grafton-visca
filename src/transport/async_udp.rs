//! Generic async UDP transport implementation with zero-cost abstractions.
//!
//! This module provides a runtime-agnostic UDP transport that works with any
//! socket type implementing the AsyncDatagram trait.

use bytes::Bytes;

use crate::{
    transport::{
        async_io::AsyncDatagram, buffer::BufferManager, builder::TransportConfig, AsyncTransport,
    },
    Error,
};

/// Generic UDP transport for async VISCA communication.
///
/// This transport uses native async functions without boxing and supports
/// both IPv4 and IPv6 addresses. It works with any socket type implementing
/// the AsyncDatagram trait (tokio, async-std, smol, etc.).
#[derive(Debug)]
pub struct Udp<S: AsyncDatagram> {
    socket: S,
    buffer_manager: BufferManager,
}

impl<S: AsyncDatagram> Udp<S> {
    /// Create a new UDP transport from a connected socket.
    ///
    /// The socket should already be connected to the remote endpoint.
    pub fn new(socket: S, config: TransportConfig) -> Self {
        Self {
            socket,
            buffer_manager: BufferManager::new(config.buffer_config),
        }
    }
}

impl<S: AsyncDatagram> AsyncTransport for Udp<S> {
    async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        // Send directly - retry logic is handled at the runtime/scheduler level
        self.socket.send(data).await?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<Bytes, Error> {
        // Receive data from the socket
        let mut buffer = self.buffer_manager.alloc_vec_buffer();
        let n = self.socket.recv(&mut buffer).await?;
        Ok(self.buffer_manager.process_recv_data(buffer, n))
    }
}
