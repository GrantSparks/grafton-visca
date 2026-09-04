//! Generic async serial transport implementation with zero-cost abstractions.
//!
//! This module provides a runtime-agnostic serial transport that works with any
//! stream type implementing the AsyncReadExt and AsyncWriteExt traits.

use crate::{
    transport::{
        async_io::{AsyncReadExt, AsyncWriteExt},
        builder::TransportConfig,
        AddressingMode, AsyncTransport, HasTransportConfig,
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
        // Do not flush a serial device: its backend may enter `tcdrain`, which
        // can block indefinitely despite the owner-level async timeout. The
        // subsequent VISCA reply is the protocol-level confirmation that the
        // queued bytes progressed.
        self.stream.write_all(bytes).await
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

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(AddressingMode::Serial)
    }
}

impl<S: AsyncReadExt + AsyncWriteExt> HasTransportConfig for Serial<S> {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }

    fn standard_transport_kind(&self) -> Option<crate::camera::TransportKind> {
        Some(crate::camera::TransportKind::Serial)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FlushCountingIo {
        writes: Vec<Vec<u8>>,
        flush_calls: usize,
    }

    impl AsyncReadExt for FlushCountingIo {
        async fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Error> {
            Err(Error::ConnectionClosed {
                reason: Some("test read is unused".into()),
            })
        }
    }

    impl AsyncWriteExt for FlushCountingIo {
        async fn write_all(&mut self, bytes: &[u8]) -> Result<(), Error> {
            self.writes.push(bytes.to_vec());
            Ok(())
        }

        async fn flush(&mut self) -> Result<(), Error> {
            self.flush_calls += 1;
            Ok(())
        }
    }

    #[tokio::test]
    async fn command_submission_never_flushes_a_serial_device() {
        let mut transport = Serial::new(FlushCountingIo::default(), TransportConfig::default());
        let command = [0x81, 0x01, 0x04, 0x00, 0xFF];

        transport
            .send(&command)
            .await
            .expect("the fake accepts a serial write");

        assert_eq!(transport.stream.writes, [command]);
        assert_eq!(transport.stream.flush_calls, 0);
    }
}
