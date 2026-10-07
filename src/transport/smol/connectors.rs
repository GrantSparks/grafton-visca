//! smol's I/O primitives for the shared async transports.

use std::{io, net::SocketAddr, time::Duration};

use smol::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UdpSocket},
};

use crate::{
    transport::{
        async_connect::AsyncNet,
        async_io::{
            AsyncDatagram, AsyncReadExt as AsyncReadExtTrait, AsyncWriteExt as AsyncWriteExtTrait,
        },
        async_tcp::NetStream,
        async_udp::NetDatagram,
        datagram::recv_datagram,
        ReceiveOutcome,
    },
    Error,
};

/// smol's connect primitives.
#[derive(Debug, Clone, Copy)]
pub struct SmolNet;

impl AsyncNet for SmolNet {
    type TcpStream = TcpStream;
    type UdpSocket = UdpSocket;

    async fn lookup(endpoint: String) -> io::Result<Vec<SocketAddr>> {
        // smol has no native resolver; this runs the standard library's
        // blocking lookup on smol's blocking pool.
        smol::net::resolve(endpoint).await
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
        async_io::Timer::after(duration).await;
    }
}

/// smol TCP stream; its halves are clones of the same socket.
#[derive(Debug)]
pub struct SmolTcpStream {
    stream: TcpStream,
}

impl AsyncReadExtTrait for SmolTcpStream {
    async fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> Result<usize, Error> {
        Ok(self.stream.read(buf).await?)
    }
}

impl AsyncWriteExtTrait for SmolTcpStream {
    async fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> Result<(), Error> {
        Ok(self.stream.write_all(buf).await?)
    }

    async fn flush(&mut self) -> Result<(), Error> {
        Ok(self.stream.flush().await?)
    }
}

impl NetStream for SmolTcpStream {
    type Net = SmolNet;
    type Reader = Self;
    type Writer = Self;

    fn from_connected(stream: TcpStream) -> Self {
        Self { stream }
    }

    fn split(self) -> (Self, Self) {
        let writer = Self {
            stream: self.stream.clone(),
        };
        (self, writer)
    }
}

impl NetDatagram for UdpSocket {
    type Net = SmolNet;
}

/// Implement AsyncDatagram for smol's UdpSocket
impl AsyncDatagram for UdpSocket {
    async fn send(&self, buf: &[u8]) -> Result<usize, Error> {
        Ok(UdpSocket::send(self, buf).await?)
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<ReceiveOutcome, Error> {
        let socket: std::sync::Arc<async_io::Async<std::net::UdpSocket>> = self.clone().into();
        Ok(socket
            .read_with(|socket| recv_datagram(socket, buf))
            .await?)
    }
}
