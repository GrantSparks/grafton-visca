//! Tokio-specific transport implementations.
//!
//! These implementations are only available when the `tokio` feature is enabled.

use std::{io, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UdpSocket},
};

use super::{RawTransport, TransportFuture};
use crate::error::Error;

/// TCP transport implementation using tokio.
#[derive(Debug)]
pub struct TokioTcpTransport {
    stream: TcpStream,
    description: String,
}

impl TokioTcpTransport {
    /// Create a new TCP transport.
    pub async fn connect(address: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(address).await?;
        Ok(Self {
            stream,
            description: format!("Tokio TCP connection to {}", address),
        })
    }

    /// Create a new TCP transport with timeout.
    pub async fn connect_timeout(address: &str, timeout: Duration) -> io::Result<Self> {
        let stream = tokio::time::timeout(timeout, TcpStream::connect(address))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Connection timeout"))??;
        Ok(Self {
            stream,
            description: format!(
                "Tokio TCP connection to {} (timeout: {:?})",
                address, timeout
            ),
        })
    }
}

impl RawTransport for TokioTcpTransport {
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
        // We can't easily check if a TcpStream is connected without trying to use it
        true
    }

    fn description(&self) -> &str {
        &self.description
    }
}

/// UDP transport implementation using tokio.
#[derive(Debug)]
pub struct TokioUdpTransport {
    socket: UdpSocket,
    description: String,
}

impl TokioUdpTransport {
    /// Create a new UDP transport.
    pub async fn connect(address: &str) -> io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(address).await?;
        Ok(Self {
            socket,
            description: format!("Tokio UDP connection to {}", address),
        })
    }
}

impl RawTransport for TokioUdpTransport {
    fn send<'a>(&'a mut self, data: &'a [u8]) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            self.socket.send(data).await.map_err(Error::Io)?;
            Ok(())
        })
    }

    fn receive(&mut self) -> TransportFuture<'_, Vec<u8>> {
        Box::pin(async move {
            let mut buffer = [0u8; 1024];
            let mut current_response = Vec::new();

            loop {
                match tokio::time::timeout(Duration::from_secs(10), self.socket.recv(&mut buffer))
                    .await
                {
                    Ok(Ok(size)) => {
                        for &byte in &buffer[..size] {
                            current_response.push(byte);

                            // Check for end of VISCA frame
                            if byte == 0xFF
                                && current_response.len() >= 3
                                && current_response[0] == 0x90
                            {
                                log::debug!("Received UDP response: {current_response:02X?}");
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
        // UDP is connectionless
        true
    }

    fn description(&self) -> &str {
        &self.description
    }
}

