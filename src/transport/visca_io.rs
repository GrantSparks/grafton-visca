//! Generic VISCA framing module for async I/O.
//!
//! This module provides unified framing logic that works with any AsyncRead + AsyncWrite stream,
//! eliminating duplication between different transport implementations.

use std::io;

use bytes::{Bytes, BytesMut};
use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{transport::async_trait_transport::Transport, Error};

/// VISCA I/O wrapper for generic async streams.
///
/// This struct handles VISCA protocol framing for any stream that implements
/// AsyncRead + AsyncWrite, providing efficient read_until operations and
/// proper frame boundary handling.
#[derive(Debug)]
pub struct ViscaIo<S> {
    inner: S,
}

impl<S> ViscaIo<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    /// Create a new ViscaIo wrapper around a stream.
    pub fn new(stream: S) -> Self {
        Self { inner: stream }
    }

    /// Write a VISCA frame to the stream.
    pub async fn write_frame(&mut self, buf: &[u8]) -> Result<(), Error> {
        self.inner
            .write_all(buf)
            .await
            .map_err(|e| Error::TransportError(e.to_string().into()))?;
        self.inner
            .flush()
            .await
            .map_err(|e| Error::TransportError(e.to_string().into()))?;
        Ok(())
    }

    /// Read a VISCA frame from the stream.
    ///
    /// This method efficiently reads until it finds the VISCA terminator (0xFF),
    /// avoiding the inefficient 1-byte-at-a-time approach.
    pub async fn read_frame(&mut self) -> Result<Bytes, Error> {
        let mut buffer = BytesMut::with_capacity(256);
        let mut temp_buf = vec![0u8; 128];

        loop {
            // Read a chunk of data
            let n = self
                .inner
                .read(&mut temp_buf)
                .await
                .map_err(|e| Error::TransportError(e.to_string().into()))?;

            if n == 0 {
                return Err(Error::TransportError("Connection closed".into()));
            }

            // Scan for terminator
            for i in 0..n {
                buffer.extend_from_slice(&[temp_buf[i]]);
                if temp_buf[i] == 0xFF {
                    // Found terminator
                    return Ok(buffer.freeze());
                }
            }

            // Check for buffer overflow
            if buffer.len() > 1024 {
                return Err(Error::TransportError("Response too large".into()));
            }
        }
    }

    /// Get a mutable reference to the inner stream.
    pub fn inner_mut(&mut self) -> &mut S {
        &mut self.inner
    }

    /// Consume this wrapper and return the inner stream.
    pub fn into_inner(self) -> S {
        self.inner
    }
}

/// Adapter to implement Transport trait for ViscaIo.
#[async_trait::async_trait]
impl<S> Transport for ViscaIo<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    async fn send(&self, _bytes: &[u8]) -> Result<(), Error> {
        // Note: This requires interior mutability or a different design
        // For now, this is a placeholder - actual implementation would need
        // to handle mutability properly (e.g., using Arc<Mutex<S>>)
        todo!("Implement with proper interior mutability")
    }

    async fn recv(&self) -> Result<Bytes, Error> {
        // Same issue with mutability
        todo!("Implement with proper interior mutability")
    }
}

/// Blocking I/O wrapper for std streams.
///
/// This provides the same framing logic for blocking I/O using std::io traits.
#[derive(Debug)]
pub struct ViscaIoBlocking<S> {
    inner: S,
}

impl<S> ViscaIoBlocking<S>
where
    S: io::Read + io::Write,
{
    /// Create a new blocking ViscaIo wrapper.
    pub fn new(stream: S) -> Self {
        Self { inner: stream }
    }

    /// Write a VISCA frame (blocking).
    pub fn write_frame(&mut self, buf: &[u8]) -> Result<(), Error> {
        self.inner
            .write_all(buf)
            .map_err(|e| Error::TransportError(e.to_string().into()))?;
        self.inner
            .flush()
            .map_err(|e| Error::TransportError(e.to_string().into()))?;
        Ok(())
    }

    /// Read a VISCA frame (blocking).
    pub fn read_frame(&mut self) -> Result<Bytes, Error> {
        let mut buffer = Vec::with_capacity(256);
        let mut byte = [0u8; 1];

        loop {
            match self.inner.read_exact(&mut byte) {
                Ok(()) => {
                    buffer.push(byte[0]);
                    if byte[0] == 0xFF {
                        // Found terminator
                        return Ok(Bytes::from(buffer));
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    return Err(Error::TransportError("Connection closed".into()))
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    return Err(Error::Timeout)
                }
                Err(e) => return Err(Error::TransportError(e.to_string().into())),
            }

            // Check for buffer overflow
            if buffer.len() > 1024 {
                return Err(Error::TransportError("Response too large".into()));
            }
        }
    }
}