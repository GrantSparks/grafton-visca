//! Generic async TCP transport, written once for every async runtime.
//!
//! A runtime supplies a stream type implementing [`NetStream`]; connecting,
//! splitting and the transport contract are implemented here.

use std::time::Duration;

use crate::{
    transport::{
        async_connect::{self, AsyncNet},
        async_io::{write_all_flush, AsyncReadExt, AsyncWriteExt},
        builder::TransportConfig,
        connect::preflight,
        stream_read, AddressingMode, AsyncTransport, HasTransportConfig, ReceiveOutcome,
    },
    Error,
};

/// The end-of-stream reason every TCP transport reports.
const PEER_CLOSED: &str = "peer closed connection";

/// A runtime's TCP stream as used by [`Tcp`].
pub trait NetStream: AsyncReadExt + AsyncWriteExt + Send + Sized {
    /// The runtime providing the connect primitives.
    type Net: AsyncNet;
    /// Read half produced by [`NetStream::split`].
    type Reader: AsyncReadExt + std::fmt::Debug;
    /// Write half produced by [`NetStream::split`].
    type Writer: AsyncWriteExt + std::fmt::Debug;

    /// Wrap a stream connected by [`NetStream::Net`].
    fn from_connected(stream: <Self::Net as AsyncNet>::TcpStream) -> Self;
    /// Split into independently usable read and write halves.
    fn split(self) -> (Self::Reader, Self::Writer);
}

/// Generic TCP transport for async VISCA communication.
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

    /// Get the transport configuration.
    pub fn config(&self) -> &TransportConfig {
        &self.config
    }
}

impl<S: NetStream> Tcp<S> {
    /// Connect to a TCP endpoint with [`TransportConfig::for_tcp`].
    ///
    /// The address must include an explicit port; IPv6 literals must be
    /// bracketed. Host names are resolved.
    pub async fn connect(address: &str) -> Result<Self, Error> {
        Self::connect_with_config(address, TransportConfig::for_tcp()).await
    }

    /// Connect with [`TransportConfig::for_tcp`] and a custom connect timeout.
    pub async fn connect_timeout(address: &str, timeout: Duration) -> Result<Self, Error> {
        let mut config = TransportConfig::for_tcp();
        config.connect_timeout = timeout;
        Self::connect_with_config(address, config).await
    }

    /// Connect with a full configuration.
    ///
    /// Name resolution and connection setup share `config.connect_timeout`,
    /// and each resolved address is tried in order. See
    /// [`crate::transport::connect`] for the error contract shared with the
    /// blocking connector.
    pub async fn connect_with_config(
        address: &str,
        config: TransportConfig,
    ) -> Result<Self, Error> {
        let endpoint = preflight(address, &config)?;
        let stream = async_connect::connect_tcp::<S::Net>(&endpoint, config.into()).await?;
        Ok(Self::new(S::from_connected(stream), config))
    }

    /// Split the TCP transport into separate reader and writer halves.
    ///
    /// This allows concurrent reading and writing without mutable access to
    /// the entire transport.
    pub fn split(self) -> (TcpReader<S::Reader>, TcpWriter<S::Writer>) {
        let (reader, writer) = self.stream.split();
        (TcpReader { reader }, TcpWriter { writer })
    }
}

/// Reader half of a split TCP transport.
#[derive(Debug)]
pub struct TcpReader<R> {
    reader: R,
}

impl<R: AsyncReadExt> TcpReader<R> {
    /// Receive data into `dst`. A byte stream always reports a complete
    /// outcome; a closed connection is reported as [`Error::ConnectionClosed`].
    pub async fn recv_into(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome, Error> {
        stream_read(self.reader.read(dst).await?, PEER_CLOSED).map(ReceiveOutcome::complete)
    }
}

/// Writer half of a split TCP transport.
#[derive(Debug)]
pub struct TcpWriter<W> {
    writer: W,
}

impl<W: AsyncWriteExt> TcpWriter<W> {
    /// Send data over the TCP connection.
    pub async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        write_all_flush(&mut self.writer, data).await
    }
}

impl<S: AsyncReadExt + AsyncWriteExt + Send> AsyncTransport for Tcp<S> {
    async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        write_all_flush(&mut self.stream, data).await
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome, Error> {
        stream_read(self.stream.read(dst).await?, PEER_CLOSED).map(ReceiveOutcome::complete)
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
