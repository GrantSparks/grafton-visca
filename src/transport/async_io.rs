//! Unified async I/O helpers for transport implementations.
//!
//! This module provides shared functionality across different runtime transports
//! to reduce code duplication while maintaining zero-cost abstractions.

use std::future::Future;

use crate::{
    transport::{datagram::exact_fill_outcome, ReceiveOutcome},
    Error,
};

/// Trait abstracting async read operations across different runtimes.
///
/// This trait unifies the async read capabilities needed for VISCA communication
/// across tokio and smol runtimes. Not all methods will be used by
/// all runtimes, which is expected for a unified interface.
pub trait AsyncReadExt {
    /// Read data into a buffer, returning the number of bytes read.
    ///
    /// Returns 0 when the stream is closed.
    /// The returned future borrows both the transport and buffer for the same
    /// operation lifetime, stated explicitly so callers do not need to recover
    /// that relationship through an opaque future type.
    fn read<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> impl Future<Output = Result<usize, Error>> + Send + 'a;
}

/// Trait abstracting async write operations across different runtimes.
///
/// This trait unifies the async write capabilities needed for VISCA communication
/// across tokio and smol runtimes.
pub trait AsyncWriteExt {
    /// Write all data in the buffer.
    ///
    /// This ensures all bytes are written before returning.
    /// The returned future borrows both the transport and buffer for the same
    /// explicitly named operation lifetime.
    fn write_all<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> impl Future<Output = Result<(), Error>> + Send + 'a;

    /// Flush any buffered data to the underlying transport.
    fn flush(&mut self) -> impl Future<Output = Result<(), Error>> + Send + '_;
}

/// Trait abstracting async datagram (UDP) operations across different runtimes.
///
/// This trait unifies the async UDP socket capabilities needed for VISCA communication
/// across tokio and smol runtimes, enabling zero-cost abstractions
/// through monomorphization.
pub trait AsyncDatagram: Send + Sync {
    /// Send data on the socket to the connected remote address.
    ///
    /// Returns the number of bytes written on success.
    fn send(&self, buf: &[u8]) -> impl Future<Output = Result<usize, Error>> + Send;

    /// Receive data from the socket.
    ///
    /// Returns the number of bytes read on success.
    fn recv(&self, buf: &mut [u8]) -> impl Future<Output = Result<usize, Error>> + Send;

    /// Receive one datagram with its truncation status.
    ///
    /// This is the metadata-aware companion to [`AsyncDatagram::recv`]. The
    /// default cannot observe truncation, so it reports a buffer-filling
    /// receive as [`ReceiveOutcome::PossiblyTruncated`] (see
    /// [`crate::transport::datagram`]). Implementations backed by a runtime or
    /// OS API that exposes truncation must override this method so an
    /// exact-size datagram can remain valid.
    fn recv_with_outcome<'a>(
        &'a self,
        buf: &'a mut [u8],
    ) -> impl Future<Output = Result<ReceiveOutcome, Error>> + Send {
        async move {
            let capacity = buf.len();
            let bytes = self.recv(buf).await?;
            Ok(exact_fill_outcome(bytes, capacity))
        }
    }
}

/// Unified helper for writing data with proper flushing.
///
/// This function handles the common pattern of write_all followed by flush
/// that is used across all transport implementations.
pub async fn write_all_flush<W: AsyncWriteExt>(writer: &mut W, data: &[u8]) -> Result<(), Error> {
    writer.write_all(data).await?;
    writer.flush().await?;
    Ok(())
}
