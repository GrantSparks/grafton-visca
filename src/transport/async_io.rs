//! Unified async I/O helpers for transport implementations.
//!
//! This module provides shared functionality across different runtime transports
//! to reduce code duplication while maintaining zero-cost abstractions.

use std::future::Future;

use crate::{transport::ReceiveOutcome, Error};

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

    /// Receive one datagram with its truncation status.
    ///
    /// Report [`ReceiveOutcome::complete`] only when the whole datagram
    /// fitted in `buf`, [`ReceiveOutcome::truncated`] when the runtime or OS
    /// reported a discarded tail, and [`ReceiveOutcome::possibly_truncated`]
    /// when `buf` filled and truncation cannot be observed (see
    /// [`crate::transport::datagram`]).
    fn recv<'a>(
        &'a self,
        buf: &'a mut [u8],
    ) -> impl Future<Output = Result<ReceiveOutcome, Error>> + Send;
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
