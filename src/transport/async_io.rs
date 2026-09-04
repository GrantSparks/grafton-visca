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

    /// Receive data from the socket.
    ///
    /// Returns the number of bytes read on success.
    fn recv(&self, buf: &mut [u8]) -> impl Future<Output = Result<usize, Error>> + Send;

    /// Receive one datagram with its truncation status.
    ///
    /// This is the metadata-aware companion to [`AsyncDatagram::recv`]. The
    /// default keeps existing implementations source-compatible, but treats a
    /// buffer-filling receive as [`ReceiveOutcome::PossiblyTruncated`] rather
    /// than guessing that it was an exact fit. Implementations backed by a
    /// runtime or OS API that exposes truncation must override this method so
    /// an exact-size datagram can remain valid.
    fn recv_with_outcome<'a>(
        &'a self,
        buf: &'a mut [u8],
    ) -> impl Future<Output = Result<ReceiveOutcome, Error>> + Send {
        async move {
            let capacity = buf.len();
            let bytes = self.recv(buf).await?;
            if bytes == capacity {
                Ok(ReceiveOutcome::PossiblyTruncated { copied: bytes })
            } else {
                Ok(ReceiveOutcome::Complete { bytes })
            }
        }
    }
}

/// Receive one datagram through an OS socket and retain its truncation flag.
///
/// A one-byte stack sentinel extends the caller's destination. If the receive
/// reaches that sentinel, the datagram was larger than `dst`; no heap scratch
/// or raw buffer conversion is needed. The caller supplies runtime-specific
/// readiness handling; this helper performs exactly one nonblocking receive.
#[cfg(unix)]
pub(crate) fn recv_datagram_with_outcome<S>(
    socket: &S,
    dst: &mut [u8],
) -> std::io::Result<ReceiveOutcome>
where
    S: std::os::fd::AsFd,
{
    let socket = socket2::SockRef::from(socket);
    let capacity = dst.len();
    let mut sentinel = [0_u8; 1];
    let mut buffers = [
        std::io::IoSliceMut::new(dst),
        std::io::IoSliceMut::new(&mut sentinel),
    ];
    let mut socket = &*socket;
    let received = std::io::Read::read_vectored(&mut socket, &mut buffers)?;
    Ok(if received > capacity {
        ReceiveOutcome::Truncated { copied: capacity }
    } else {
        ReceiveOutcome::Complete { bytes: received }
    })
}

/// Windows counterpart to [`recv_datagram_with_outcome`].
#[cfg(windows)]
pub(crate) fn recv_datagram_with_outcome<S>(
    socket: &S,
    dst: &mut [u8],
) -> std::io::Result<ReceiveOutcome>
where
    S: std::os::windows::io::AsSocket,
{
    let socket = socket2::SockRef::from(socket);
    let mut socket = &*socket;
    // Winsock reports an over-size UDP receive as WSAEMSGSIZE. `Socket2`'s
    // plain `Read` implementation preserves that error (unlike its vectored
    // compatibility adapter, which intentionally suppresses it), so no raw
    // buffer conversion or platform-specific socket call is needed here.
    const WSAEMSGSIZE: i32 = 10_040;
    match std::io::Read::read(&mut socket, dst) {
        Ok(bytes) => Ok(ReceiveOutcome::Complete { bytes }),
        Err(error) if error.raw_os_error() == Some(WSAEMSGSIZE) => {
            Ok(ReceiveOutcome::Truncated { copied: dst.len() })
        }
        Err(error) => Err(error),
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
#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
mod tests {
    // The framing is now handled by ProtocolFramer in the runtime loop,
    // and transports only return raw bytes.
}
