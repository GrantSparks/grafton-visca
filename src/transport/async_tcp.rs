//! Generic async TCP transport, written once for every async runtime.
//!
//! A runtime supplies a stream type implementing [`NetStream`]; connecting
//! and the transport contract are implemented here.

use crate::{
    transport::{
        async_connect::AsyncNet,
        async_io::{AsyncReadExt, AsyncWriteExt},
        builder::IoBounds,
        stream_read, AsyncTransport, HasTransportConfig, ReceiveOutcome,
    },
    Error,
};

/// The end-of-stream reason every TCP transport reports.
const PEER_CLOSED: &str = "peer closed connection";

/// A runtime's TCP stream as used by [`Tcp`].
pub trait NetStream: AsyncReadExt + AsyncWriteExt + Send + Sized {
    /// The runtime providing the connect primitives.
    type Net: AsyncNet;
    /// Wrap a stream connected by [`NetStream::Net`].
    fn from_connected(stream: <Self::Net as AsyncNet>::TcpStream) -> Self;
}

/// Generic TCP transport for async VISCA communication.
#[derive(Debug)]
pub struct Tcp<S: AsyncReadExt + AsyncWriteExt> {
    pub(crate) stream: S,
    config: IoBounds,
}

impl<S: AsyncReadExt + AsyncWriteExt> Tcp<S> {
    /// Create a new TCP transport from a connected stream.
    ///
    /// The stream should already be connected to the remote endpoint.
    pub(crate) fn new(stream: S, config: IoBounds) -> Self {
        Self { stream, config }
    }
}

#[cfg(feature = "runtime-smol")]
impl<S: NetStream> Tcp<S> {
    pub(crate) async fn connect_endpoint(
        endpoint: &crate::transport::NetworkEndpoint,
        options: crate::transport::TcpConnect,
        io_bounds: IoBounds,
    ) -> Result<Self, Error> {
        let stream =
            crate::transport::async_connect::connect_tcp::<S::Net>(endpoint, options).await?;
        Ok(Self::new(S::from_connected(stream), io_bounds))
    }
}

impl<S: AsyncReadExt + AsyncWriteExt + Send> AsyncTransport for Tcp<S> {
    async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        self.stream.write_all(data).await
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome, Error> {
        stream_read(self.stream.read(dst).await?, PEER_CLOSED).map(ReceiveOutcome::complete)
    }
}

impl<S: AsyncReadExt + AsyncWriteExt> HasTransportConfig for Tcp<S> {
    fn identity(&self) -> crate::transport::TransportIdentity<'_> {
        crate::transport::TransportIdentity::Tcp
    }

    fn io_bounds(&self) -> &IoBounds {
        &self.config
    }
}
