//! Generic async serial transport implementation with zero-cost abstractions.
//!
//! This module provides a runtime-agnostic serial transport that works with any
//! stream type implementing the AsyncReadExt and AsyncWriteExt traits.

use crate::{
    transport::{
        async_io::{AsyncReadExt, AsyncWriteExt},
        builder::{TransportConfig, ValidatedTransportConfig},
        serial::SERIAL_PORT_CLOSED,
        stream_read, AddressedBus, AsyncTransport, HasTransportConfig, ReceiveOutcome,
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
    config: ValidatedTransportConfig,
    addressed_bus: Option<AddressedBus>,
}

impl<S: AsyncReadExt + AsyncWriteExt> Serial<S> {
    /// Create a new serial transport from a connected stream.
    ///
    /// The stream should already be configured and connected to the serial port.
    /// Any initialization (I/F Clear, Address Set) should be performed by the
    /// runtime-specific connector before constructing this transport.
    pub fn new(stream: S, config: ValidatedTransportConfig) -> Self {
        Self {
            stream,
            config,
            addressed_bus: None,
        }
    }

    /// Record the bus the startup's Address Set addressed.
    pub(crate) fn with_addressed_bus(mut self, addressed_bus: Option<AddressedBus>) -> Self {
        self.addressed_bus = addressed_bus;
        self
    }

    /// Get the transport configuration.
    pub fn config(&self) -> &TransportConfig {
        self.config.as_config()
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

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome, Error> {
        stream_read(self.stream.read(dst).await?, SERIAL_PORT_CLOSED).map(ReceiveOutcome::complete)
    }
}

impl<S: AsyncReadExt + AsyncWriteExt> HasTransportConfig for Serial<S> {
    fn identity(&self) -> crate::transport::TransportIdentity<'_> {
        crate::transport::TransportIdentity::Serial {
            bus: self.addressed_bus.as_ref(),
        }
    }

    fn transport_config(&self) -> &ValidatedTransportConfig {
        &self.config
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
        async fn read<'a>(&'a mut self, _buf: &'a mut [u8]) -> Result<usize, Error> {
            Err(Error::ConnectionClosed {
                source: Some(std::sync::Arc::new(Error::from(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "test read is unused",
                )))),
            })
        }
    }

    impl AsyncWriteExt for FlushCountingIo {
        async fn write_all<'a>(&'a mut self, bytes: &'a [u8]) -> Result<(), Error> {
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
        let mut transport = Serial::new(
            FlushCountingIo::default(),
            TransportConfig::for_serial()
                .validated()
                .expect("valid connector test configuration"),
        );
        let command = [0x81, 0x01, 0x04, 0x00, 0xFF];

        transport
            .send(&command)
            .await
            .expect("the fake accepts a serial write");

        assert_eq!(transport.stream.writes, [command]);
        assert_eq!(transport.stream.flush_calls, 0);
    }
}
