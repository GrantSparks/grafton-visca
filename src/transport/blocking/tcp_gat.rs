//! Blocking TCP transport implementation using GAT.

use crate::transport::gat_transport::{Transport, blocking::ready};
use crate::Error;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::Duration;
use core::future::Ready;

/// TCP transport for blocking VISCA communication.
#[derive(Debug)]
pub struct TcpGat {
    stream: Mutex<TcpStream>,
    _address: String,
}

impl TcpGat {
    /// Connect to a TCP endpoint.
    pub fn connect(address: &str) -> Result<Self, Error> {
        Self::connect_timeout(address, Duration::from_secs(5))
    }

    /// Connect with a custom timeout.
    pub fn connect_timeout(address: &str, timeout: Duration) -> Result<Self, Error> {
        let stream = TcpStream::connect_timeout(
            &address.parse().map_err(|e| Error::TransportError(format!("Invalid address: {}", e)))?,
            timeout
        )?;

        // Set socket options
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        stream.set_nodelay(true)?;

        Ok(Self {
            stream: Mutex::new(stream),
            _address: address.to_string(),
        })
    }
}

impl Transport for TcpGat {
    type Error = Error;
    type SendFut<'a> = Ready<Result<(), Self::Error>>;
    type RecvFut<'a> = Ready<Result<bytes::Bytes, Self::Error>>;

    fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFut<'a> {
        ready(send_impl(&self.stream, data))
    }

    fn recv<'a>(&'a self) -> Self::RecvFut<'a> {
        ready(recv_impl(&self.stream))
    }
}

fn send_impl(stream: &Mutex<TcpStream>, data: &[u8]) -> Result<(), Error> {
    let mut stream = stream.lock()
        .map_err(|e| Error::TransportError(format!("Failed to lock stream: {}", e)))?;
    
    stream.write_all(data)?;
    stream.flush()?;
    Ok(())
}

fn recv_impl(stream: &Mutex<TcpStream>) -> Result<bytes::Bytes, Error> {
    let mut stream = stream.lock()
        .map_err(|e| Error::TransportError(format!("Failed to lock stream: {}", e)))?;
    
    let mut buffer = vec![0u8; 1024];
    let mut total_read = 0;
    
    // Read until we find a VISCA terminator (0xFF)
    loop {
        if total_read >= buffer.len() {
            return Err(Error::TransportError("Response too large".to_string()));
        }
        
        match stream.read(&mut buffer[total_read..total_read + 1]) {
            Ok(0) => return Err(Error::TransportError("Connection closed".to_string())),
            Ok(1) => {
                total_read += 1;
                if buffer[total_read - 1] == 0xFF {
                    // Found terminator
                    buffer.truncate(total_read);
                    return Ok(bytes::Bytes::from(buffer));
                }
            }
            Ok(_) => unreachable!(),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                return Err(Error::Timeout);
            }
            Err(e) => return Err(e.into()),
        }
    }
}