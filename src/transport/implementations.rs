//! Built-in transport implementations.
//!
//! These implementations demonstrate how minimal transport-specific code can be
//! when all VISCA protocol logic is handled by ViscaTransport.

use std::{io, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UdpSocket},
};

use super::{RawTransport, TransportFuture};
use crate::error::Error;

/// TCP transport implementation.
#[derive(Debug)]
pub struct TcpTransport {
    stream: TcpStream,
    description: String,
}

impl TcpTransport {
    /// Create a new TCP transport.
    pub async fn connect(address: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(address).await?;
        Ok(Self {
            stream,
            description: format!("TCP connection to {}", address),
        })
    }

    /// Create a new TCP transport with timeout.
    pub async fn connect_timeout(address: &str, timeout: Duration) -> io::Result<Self> {
        let stream = tokio::time::timeout(timeout, TcpStream::connect(address))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Connection timeout"))??;
        Ok(Self {
            stream,
            description: format!("TCP connection to {} (timeout: {:?})", address, timeout),
        })
    }
}

impl RawTransport for TcpTransport {
    fn send<'a>(&'a mut self, data: &'a [u8]) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            self.stream.write_all(data).await.map_err(Error::Io)?;
            self.stream.flush().await.map_err(Error::Io)?;
            Ok(())
        })
    }

    fn receive(&mut self) -> TransportFuture<'_, Vec<u8>> {
        Box::pin(async move {
            let mut buffer = [0u8; 1024];
            let mut current_response = Vec::new();

            loop {
                match tokio::time::timeout(Duration::from_secs(10), self.stream.read(&mut buffer))
                    .await
                {
                    Ok(Ok(0)) => {
                        return Err(Error::Io(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "Connection closed by camera",
                        )));
                    }
                    Ok(Ok(size)) => {
                        for &byte in &buffer[..size] {
                            current_response.push(byte);

                            // Check for end of VISCA frame
                            if byte == 0xFF
                                && current_response.len() >= 3
                                && current_response[0] == 0x90
                            {
                                log::debug!("Received response: {current_response:02X?}");
                                return Ok(current_response);
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        return Err(Error::Io(e));
                    }
                    Err(_) => {
                        return Err(Error::CommandTimeout {
                            duration: Duration::from_secs(10),
                            command: "receive_response".to_string(),
                        });
                    }
                }
            }
        })
    }

    fn is_connected(&self) -> bool {
        // For TCP, we'd need to check the stream state
        // This is a simplified implementation
        true
    }

    fn description(&self) -> &str {
        &self.description
    }
}

/// UDP transport implementation.
#[derive(Debug)]
pub struct UdpTransport {
    socket: UdpSocket,
    address: String,
    description: String,
}

impl UdpTransport {
    /// Create a new UDP transport.
    pub async fn connect(address: &str) -> io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        Ok(Self {
            socket,
            address: address.to_string(),
            description: format!("UDP connection to {}", address),
        })
    }
}

impl RawTransport for UdpTransport {
    fn send<'a>(&'a mut self, data: &'a [u8]) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            self.socket
                .send_to(data, &self.address)
                .await
                .map_err(Error::Io)?;
            Ok(())
        })
    }

    fn receive(&mut self) -> TransportFuture<'_, Vec<u8>> {
        Box::pin(async move {
            let mut buffer = [0u8; 1024];

            match tokio::time::timeout(Duration::from_secs(10), self.socket.recv_from(&mut buffer))
                .await
            {
                Ok(Ok((size, _))) => {
                    let data = buffer[..size].to_vec();
                    log::debug!("Received data: {data:02X?}");
                    Ok(data)
                }
                Ok(Err(e)) => Err(Error::Io(e)),
                Err(_) => Err(Error::CommandTimeout {
                    duration: Duration::from_secs(10),
                    command: "receive_response".to_string(),
                }),
            }
        })
    }

    fn is_connected(&self) -> bool {
        // UDP is connectionless, so always "connected"
        true
    }

    fn description(&self) -> &str {
        &self.description
    }
}

/// Serial transport implementation (mock for demonstration).
#[derive(Debug)]
pub struct SerialTransport {
    camera_address: u8,
    description: String,
}

impl SerialTransport {
    /// Create a new serial transport.
    /// In a real implementation, this would take a serial port.
    pub fn new(camera_address: u8) -> Self {
        Self {
            camera_address,
            description: format!("Serial connection to camera {}", camera_address),
        }
    }
}

impl RawTransport for SerialTransport {
    fn send<'a>(&'a mut self, data: &'a [u8]) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            // In a real implementation, this would use a serial port library
            log::debug!(
                "Serial write to camera {}: {:02X?}",
                self.camera_address,
                data
            );
            Ok(())
        })
    }

    fn receive(&mut self) -> TransportFuture<'_, Vec<u8>> {
        Box::pin(async move {
            // Mock response for demonstration
            let response = vec![0x90, 0x40, 0xFF]; // ACK
            log::debug!(
                "Serial read from camera {}: {:02X?}",
                self.camera_address,
                response
            );
            Ok(response)
        })
    }

    fn is_connected(&self) -> bool {
        // In a real implementation, this would check the serial port state
        true
    }

    fn description(&self) -> &str {
        &self.description
    }
}
