//! Blocking TCP transport implementation using GAT.

use crate::transport::core::{blocking::ready, BlockingTransport, Transport};
use crate::transport::UnifiedTransport;
use crate::Error;
use core::future::Ready;
use std::borrow::Cow;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::Duration;

/// TCP transport for blocking VISCA communication.
#[derive(Debug)]
pub struct Tcp {
    reader: Mutex<BufReader<TcpStream>>,
    writer: Mutex<TcpStream>,
}

impl Tcp {
    /// Connect to a TCP endpoint.
    pub fn connect(address: &str) -> Result<Self, Error> {
        Self::connect_timeout(address, Duration::from_secs(5))
    }

    /// Connect with a custom timeout.
    pub fn connect_timeout(address: &str, timeout: Duration) -> Result<Self, Error> {
        let stream = TcpStream::connect_timeout(
            &address
                .parse::<std::net::SocketAddr>()
                .map_err(|e| Error::InvalidAddress {
                    reason: Cow::Owned(e.to_string()),
                })?,
            timeout,
        )?;

        // Set socket options
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        stream.set_nodelay(true)?;

        // Clone the stream for separate reader and writer
        let reader_stream = stream.try_clone()?;

        Ok(Self {
            reader: Mutex::new(BufReader::new(reader_stream)),
            writer: Mutex::new(stream),
        })
    }
}

impl Transport for Tcp {
    type Error = Error;
    type SendFut<'a> = Ready<Result<(), Self::Error>>;
    type RecvFut<'a> = Ready<Result<bytes::Bytes, Self::Error>>;

    fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFut<'a> {
        ready(send_impl(&self.writer, data))
    }

    fn recv(&self) -> Self::RecvFut<'_> {
        ready(recv_impl(&self.reader))
    }
}

impl BlockingTransport for Tcp {}

#[async_trait::async_trait]
impl UnifiedTransport for Tcp {
    async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
        send_impl(&self.writer, bytes)
    }

    async fn recv(&self) -> Result<bytes::Bytes, Error> {
        recv_impl(&self.reader)
    }

    fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error> {
        send_impl(&self.writer, bytes)
    }

    fn recv_blocking_timeout(&self, _timeout: Duration) -> Result<bytes::Bytes, Error> {
        // The timeout is already configured on the TcpStream itself
        recv_impl(&self.reader)
    }
}

fn send_impl(writer: &Mutex<TcpStream>, data: &[u8]) -> Result<(), Error> {
    let mut writer = writer.lock().map_err(|_| Error::LockPoisoned("writer"))?;

    writer.write_all(data)?;
    writer.flush()?;
    Ok(())
}

fn recv_impl(reader: &Mutex<BufReader<TcpStream>>) -> Result<bytes::Bytes, Error> {
    let mut reader = reader.lock().map_err(|_| Error::LockPoisoned("reader"))?;

    let mut buffer = Vec::with_capacity(64);

    // Use buffered read_until to find VISCA terminator
    let n = reader.read_until(0xFF, &mut buffer)?;

    if n == 0 {
        return Err(Error::ConnectionLost {
            reason: Cow::Borrowed("peer closed connection"),
        });
    }

    Ok(bytes::Bytes::from(buffer))
}
