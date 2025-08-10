//! Blocking UDP transport implementation using GAT.

use crate::transport::core::{blocking::ready, BlockingTransport, Transport};
use crate::transport::UnifiedTransport;
use crate::Error;
use core::future::Ready;
use std::net::UdpSocket;
use std::sync::Mutex;
use std::time::Duration;

/// UDP transport for blocking VISCA communication.
#[derive(Debug)]
pub struct Udp {
    socket: Mutex<UdpSocket>,
}

impl Udp {
    /// Connect to a UDP endpoint.
    pub fn connect(address: &str) -> Result<Self, Error> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.connect(address)?;

        // Set timeouts
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        socket.set_write_timeout(Some(Duration::from_secs(5)))?;

        Ok(Self {
            socket: Mutex::new(socket),
        })
    }
}

impl Transport for Udp {
    type Error = Error;
    type SendFut<'a> = Ready<Result<(), Self::Error>>;
    type RecvFut<'a> = Ready<Result<bytes::Bytes, Self::Error>>;

    fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFut<'a> {
        ready(send_impl(&self.socket, data))
    }

    fn recv(&self) -> Self::RecvFut<'_> {
        ready(recv_impl(&self.socket))
    }
}

impl BlockingTransport for Udp {}

#[async_trait::async_trait]
impl UnifiedTransport for Udp {
    async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
        send_impl(&self.socket, bytes)
    }

    async fn recv(&self) -> Result<bytes::Bytes, Error> {
        recv_impl(&self.socket)
    }

    fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error> {
        send_impl(&self.socket, bytes)
    }

    fn recv_blocking_timeout(&self, _timeout: Duration) -> Result<bytes::Bytes, Error> {
        // The timeout is already configured on the UdpSocket itself
        recv_impl(&self.socket)
    }
}

fn send_impl(socket: &Mutex<UdpSocket>, data: &[u8]) -> Result<(), Error> {
    let socket = socket
        .lock()
        .map_err(|_| Error::LockPoisoned("socket"))?;

    socket.send(data)?;
    Ok(())
}

fn recv_impl(socket: &Mutex<UdpSocket>) -> Result<bytes::Bytes, Error> {
    let socket = socket
        .lock()
        .map_err(|_| Error::LockPoisoned("socket"))?;

    let mut buffer = vec![0u8; 1024];

    match socket.recv(&mut buffer) {
        Ok(n) => {
            buffer.truncate(n);
            Ok(bytes::Bytes::from(buffer))
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Err(Error::Timeout),
        Err(e) => Err(e.into()),
    }
}
