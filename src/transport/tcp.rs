//! TCP transport implementation for VISCA over IP.

// Standard library imports
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use std::{io, time::Duration};

#[cfg(feature = "async-client")]
use std::sync::Arc;

// Crate imports
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use crate::{connection::ConnectionStats, error::Error, Command};

#[cfg(feature = "blocking-client")]
use std::{
    io::{Read, Write},
    net::TcpStream,
};

#[cfg(feature = "blocking-client")]
use super::BlockingTransport;

#[cfg(feature = "async-client")]
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream as TokioTcpStream,
};

#[cfg(feature = "async-client")]
use super::{Transport, TransportFuture};

/// Blocking TCP transport for VISCA communication.
#[cfg(feature = "blocking-client")]
#[derive(Debug)]
pub struct TcpTransport {
    stream: TcpStream,
    stats: ConnectionStats,
}

#[cfg(feature = "blocking-client")]
impl TcpTransport {
    /// Creates a new TCP transport connected to the specified camera address.
    ///
    /// Sets read and write timeouts of 10 seconds by default.
    ///
    /// # Arguments
    /// * `address` - The camera's IP address and port (e.g., "192.168.1.100:5678")
    ///
    /// # Errors
    /// Returns an error if the connection cannot be established or configured.
    pub fn new(address: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(address)?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;
        Ok(Self {
            stream,
            stats: ConnectionStats::new(),
        })
    }

    /// Creates a new TCP transport with a custom timeout.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be established or if the address cannot be parsed.
    pub fn with_timeout(address: &str, timeout: Duration) -> io::Result<Self> {
        let socket_addr = address.parse().map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidInput, format!("Invalid address: {e}"))
        })?;
        let stream = TcpStream::connect_timeout(&socket_addr, timeout)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        Ok(Self {
            stream,
            stats: ConnectionStats::new(),
        })
    }

    /// Returns the connection statistics.
    #[must_use]
    pub const fn stats(&self) -> &ConnectionStats {
        &self.stats
    }
}

#[cfg(feature = "blocking-client")]
impl BlockingTransport for TcpTransport {
    /// Sends a VISCA command over the TCP connection.
    ///
    /// # Errors
    /// Returns `Error` if the command serialization fails or if there's
    /// an I/O error writing to the TCP socket.
    fn send_command_blocking(
        &mut self,
        command: &dyn Command,
        socket_id: crate::types::SocketId,
    ) -> Result<(), Error> {
        let mut bytes = command.to_bytes()?;

        // Encode socket ID in the command header
        if !bytes.is_empty() && bytes[0] == 0x81 {
            bytes[0] = 0x80 | socket_id.value();
        }

        log::debug!(
            "Sending command with socket {}: {bytes:02X?}",
            socket_id.value()
        );

        self.stream.write_all(&bytes).map_err(Error::Io)?;

        self.stream.flush().map_err(Error::Io)?;

        self.stats.record_sent(bytes.len());
        Ok(())
    }

    /// Receives VISCA response frames from the TCP connection.
    ///
    /// # Errors
    /// Returns `Error` if the connection is closed unexpectedly,
    /// if a read timeout occurs, or if an incomplete VISCA frame is received.
    fn receive_response_blocking(&mut self) -> Result<(crate::types::SocketId, Vec<u8>), Error> {
        let mut buffer = [0u8; 1024];
        let mut current_response = Vec::new();

        // Keep receiving until we get a completion or error response
        loop {
            match self.stream.read(&mut buffer) {
                Ok(0) => {
                    self.stats.record_error();
                    return Err(Error::Io(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Connection closed by camera",
                    )));
                }
                Ok(size) => {
                    for &byte in &buffer[..size] {
                        current_response.push(byte);

                        // Check for end of VISCA frame
                        if byte == 0xFF
                            && current_response.len() >= 3
                            && current_response[0] == 0x90
                        {
                            log::debug!("Received response: {current_response:02X?}");

                            self.stats.record_received(current_response.len());

                            // Extract socket ID from response header
                            let socket_id = if current_response[0] == 0x90 {
                                crate::types::SocketId::SOCKET_0 // Default to socket 0
                            } else {
                                // Response format is 0x9X where X is socket ID
                                let socket_value = current_response[0] & 0x0F;
                                crate::types::SocketId::new(socket_value)
                                    .unwrap_or(crate::types::SocketId::SOCKET_0)
                            };

                            // Check for completion or error
                            if current_response.len() >= 3
                                && (current_response[1] == 0x50
                                    || current_response[1] == 0x51
                                    || (current_response[1] & 0x60) == 0x60)
                            {
                                return Ok((socket_id, current_response));
                            }

                            // For ACK responses, continue waiting for completion
                            if current_response[1] == 0x40 || current_response[1] == 0x41 {
                                current_response.clear();
                                continue;
                            }

                            // Return other responses immediately
                            return Ok((socket_id, current_response));
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if current_response.is_empty() {
                        self.stats.record_error();
                        return Err(Error::CommandTimeout {
                            duration: Duration::from_secs(10),
                            command: "receive_response".to_string(),
                        });
                    }
                    // Incomplete frame
                    self.stats.record_error();
                    return Err(Error::Io(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Incomplete VISCA frame",
                    )));
                }
                Err(e) => {
                    self.stats.record_error();
                    return Err(Error::Io(e));
                }
            }
        }
    }
}

/// Async TCP transport for VISCA communication.
#[cfg(feature = "async-client")]
#[derive(Debug, Clone)]
pub struct AsyncTcpTransport {
    stream: Arc<tokio::sync::Mutex<TokioTcpStream>>,
    stats: Arc<std::sync::Mutex<ConnectionStats>>,
}

#[cfg(feature = "async-client")]
impl AsyncTcpTransport {
    /// Creates a new async TCP transport.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be established.
    pub async fn new(address: &str) -> io::Result<Self> {
        let stream = TokioTcpStream::connect(address).await?;
        Ok(Self {
            stream: Arc::new(tokio::sync::Mutex::new(stream)),
            stats: Arc::new(std::sync::Mutex::new(ConnectionStats::new())),
        })
    }

    /// Returns a clone of the connection statistics.
    #[must_use]
    pub fn stats(&self) -> ConnectionStats {
        self.stats.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

#[cfg(feature = "async-client")]
impl Transport for AsyncTcpTransport {
    /// Sends a VISCA command over the async TCP connection.
    ///
    /// # Errors
    /// Returns `Error` if the command serialization fails or if there's
    /// an I/O error writing to the TCP socket.
    fn send_command<'a>(
        &'a mut self,
        command: &'a dyn Command,
        socket_id: crate::types::SocketId,
    ) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            let mut bytes = command.to_bytes()?;

            // Encode socket ID in the command header
            if !bytes.is_empty() && bytes[0] == 0x81 {
                bytes[0] = 0x80 | socket_id.value();
            }

            log::debug!(
                "Sending command with socket {}: {bytes:02X?}",
                socket_id.value()
            );

            let mut stream = self.stream.lock().await;
            stream.write_all(&bytes).await.map_err(Error::Io)?;
            stream.flush().await.map_err(Error::Io)?;
            drop(stream);

            if let Ok(stats) = self.stats.lock() {
                stats.record_sent(bytes.len());
            }
            Ok(())
        })
    }

    /// Receives VISCA response frames from the async TCP connection.
    ///
    /// # Errors
    /// Returns `Error` if the connection is closed unexpectedly,
    /// if a read timeout occurs, or if an incomplete VISCA frame is received.
    fn receive_response(&mut self) -> TransportFuture<'_, (crate::types::SocketId, Vec<u8>)> {
        Box::pin(async move {
            let mut buffer = [0u8; 1024];
            let mut current_response = Vec::new();

            // Keep receiving until we get a completion or error response
            loop {
                let mut stream = self.stream.lock().await;
                match tokio::time::timeout(Duration::from_secs(10), stream.read(&mut buffer))
                    .await
                {
                    Ok(Ok(0)) => {
                        drop(stream);
                        if let Ok(stats) = self.stats.lock() {
                            stats.record_error();
                        }
                        return Err(Error::Io(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "Connection closed by camera",
                        )));
                    }
                    Ok(Ok(size)) => {
                        drop(stream);
                        for &byte in &buffer[..size] {
                            current_response.push(byte);

                            // Check for end of VISCA frame
                            if byte == 0xFF
                                && current_response.len() >= 3
                                && current_response[0] == 0x90
                            {
                                log::debug!("Received response: {current_response:02X?}");

                                if let Ok(stats) = self.stats.lock() {
                                    stats.record_received(current_response.len());
                                }

                                // Extract socket ID from response header
                                let socket_id = if current_response[0] == 0x90 {
                                    crate::types::SocketId::SOCKET_0 // Default to socket 0
                                } else {
                                    // Response format is 0x9X where X is socket ID
                                    let socket_value = current_response[0] & 0x0F;
                                    crate::types::SocketId::new(socket_value)
                                        .unwrap_or(crate::types::SocketId::SOCKET_0)
                                };

                                // Check for completion or error
                                if current_response.len() >= 3
                                    && (current_response[1] == 0x50
                                        || current_response[1] == 0x51
                                        || (current_response[1] & 0x60) == 0x60)
                                {
                                    return Ok((socket_id, current_response));
                                }

                                // For ACK responses, continue waiting for completion
                                if current_response[1] == 0x40 || current_response[1] == 0x41 {
                                    current_response.clear();
                                    continue;
                                }

                                // Return other responses immediately
                                return Ok((socket_id, current_response));
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        drop(stream);
                        if let Ok(stats) = self.stats.lock() {
                            stats.record_error();
                        }
                        return Err(Error::Io(e));
                    }
                    Err(_) => {
                        drop(stream);
                        if current_response.is_empty() {
                            if let Ok(stats) = self.stats.lock() {
                                stats.record_error();
                            }
                            return Err(Error::CommandTimeout {
                                duration: Duration::from_secs(10),
                                command: "receive_response".to_string(),
                            });
                        }
                        // Incomplete frame
                        if let Ok(stats) = self.stats.lock() {
                            stats.record_error();
                        }
                        return Err(Error::Io(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Incomplete VISCA frame",
                        )));
                    }
                }
            }
        })
    }
}
