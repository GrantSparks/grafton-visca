//! Tokio's I/O primitives for the shared async transports.

use std::{io, net::SocketAddr, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream, UdpSocket,
    },
};

use crate::{
    transport::{
        async_connect::{self, AsyncNet},
        async_io::{
            AsyncDatagram, AsyncReadExt as AsyncReadExtTrait, AsyncWriteExt as AsyncWriteExtTrait,
        },
        async_tcp::NetStream,
        async_udp::NetDatagram,
        datagram::recv_datagram,
        socket_options::{TcpConnectionConfig, UdpSocketConfig},
        ReceiveOutcome,
    },
    Error,
};

/// Tokio's connect primitives.
#[derive(Debug, Clone, Copy)]
pub struct TokioNet;

impl AsyncNet for TokioNet {
    type TcpStream = TcpStream;
    type UdpSocket = UdpSocket;

    async fn lookup(endpoint: String) -> io::Result<Vec<SocketAddr>> {
        Ok(tokio::net::lookup_host(endpoint).await?.collect())
    }

    async fn connect_tcp(address: SocketAddr) -> io::Result<TcpStream> {
        TcpStream::connect(address).await
    }

    async fn bind_udp(address: SocketAddr) -> io::Result<UdpSocket> {
        UdpSocket::bind(address).await
    }

    async fn connect_udp(socket: &UdpSocket, address: SocketAddr) -> io::Result<()> {
        socket.connect(address).await
    }

    fn tcp_socket(stream: &TcpStream) -> socket2::SockRef<'_> {
        socket2::SockRef::from(stream)
    }

    fn udp_socket(socket: &UdpSocket) -> socket2::SockRef<'_> {
        socket2::SockRef::from(socket)
    }

    async fn sleep(duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

/// Wrapper around tokio's BufReader to implement our AsyncReadExt trait.
#[derive(Debug)]
pub struct TokioBufferedReader<R> {
    inner: BufReader<R>,
}

impl<R: AsyncReadExt + Unpin + Send> AsyncReadExtTrait for TokioBufferedReader<R> {
    async fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> Result<usize, Error> {
        Ok(self.inner.read(buf).await?)
    }
}

/// Wrapper around tokio streams to implement our AsyncWriteExt trait.
#[derive(Debug)]
pub struct TokioWriter<W> {
    inner: W,
}

impl<W: AsyncWriteExt + Unpin + Send> AsyncWriteExtTrait for TokioWriter<W> {
    async fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> Result<(), Error> {
        Ok(self.inner.write_all(buf).await?)
    }

    async fn flush(&mut self) -> Result<(), Error> {
        Ok(self.inner.flush().await?)
    }
}

/// Tokio TCP stream split into owned halves.
#[derive(Debug)]
pub struct TokioTcpStream {
    reader: TokioBufferedReader<OwnedReadHalf>,
    writer: TokioWriter<OwnedWriteHalf>,
}

impl AsyncReadExtTrait for TokioTcpStream {
    async fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> Result<usize, Error> {
        self.reader.read(buf).await
    }
}

impl AsyncWriteExtTrait for TokioTcpStream {
    async fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> Result<(), Error> {
        self.writer.write_all(buf).await
    }

    async fn flush(&mut self) -> Result<(), Error> {
        self.writer.flush().await
    }
}

impl NetStream for TokioTcpStream {
    type Net = TokioNet;
    type Reader = TokioBufferedReader<OwnedReadHalf>;
    type Writer = TokioWriter<OwnedWriteHalf>;

    fn from_connected(stream: TcpStream) -> Self {
        let (read_half, write_half) = stream.into_split();
        Self {
            reader: TokioBufferedReader {
                inner: BufReader::new(read_half),
            },
            writer: TokioWriter { inner: write_half },
        }
    }

    fn split(self) -> (Self::Reader, Self::Writer) {
        (self.reader, self.writer)
    }
}

impl NetDatagram for UdpSocket {
    type Net = TokioNet;
}

/// Connect TCP on an explicitly selected Tokio runtime.
///
/// `TokioRuntime::from_handle` has already selected a runtime, so its DNS,
/// timer and socket work must execute on that handle even if its future is
/// awaited elsewhere.
pub(crate) async fn connect_tcp_on(
    handle: &tokio::runtime::Handle,
    address: String,
    config: TcpConnectionConfig,
) -> Result<TokioTcpStream, Error> {
    handle
        .spawn(async move {
            async_connect::connect_tcp::<TokioNet>(&address, config)
                .await
                .map(TokioTcpStream::from_connected)
        })
        .await
        .map_err(|error| {
            Error::InvalidState(
                format!("selected Tokio runtime stopped while connecting TCP: {error}").into(),
            )
        })?
}

/// Connect UDP on an explicitly selected Tokio runtime.
///
/// See [`connect_tcp_on`] for why runtime-bound callers use this rather than
/// the ambient connector directly.
pub(crate) async fn connect_udp_on(
    handle: &tokio::runtime::Handle,
    address: String,
    config: UdpSocketConfig,
) -> Result<UdpSocket, Error> {
    handle
        .spawn(async move { async_connect::connect_udp::<TokioNet>(&address, config).await })
        .await
        .map_err(|error| {
            Error::InvalidState(
                format!("selected Tokio runtime stopped while connecting UDP: {error}").into(),
            )
        })?
}

/// Implement AsyncDatagram for tokio's UdpSocket
impl AsyncDatagram for UdpSocket {
    async fn send(&self, buf: &[u8]) -> Result<usize, Error> {
        Ok(UdpSocket::send(self, buf).await?)
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<ReceiveOutcome, Error> {
        Ok(self
            .async_io(
                tokio::io::Interest::READABLE | tokio::io::Interest::ERROR,
                || recv_datagram(self, buf),
            )
            .await?)
    }
}
