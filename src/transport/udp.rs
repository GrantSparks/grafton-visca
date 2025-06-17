//! UDP transport implementation for VISCA over IP.

// Standard library imports
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use std::{io, time::Duration};

#[cfg(feature = "blocking-client")]
use std::net::UdpSocket;

// Third-party imports
#[cfg(feature = "async-client")]
use tokio::net::UdpSocket as TokioUdpSocket;

// Crate imports
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use crate::{connection::ConnectionStats, error::Error, Command};

#[cfg(feature = "blocking-client")]
use super::BlockingTransport;

#[cfg(feature = "async-client")]
use super::{Transport, TransportFuture};

/// Blocking UDP transport for VISCA communication.
#[cfg(feature = "blocking-client")]
#[derive(Debug)]
pub struct UdpTransport {
    socket: UdpSocket,
    address: String,
    stats: ConnectionStats,
}

#[cfg(feature = "blocking-client")]
impl UdpTransport {
    /// Creates a new UDP transport connected to the specified camera address.
    ///
    /// Sets read and write timeouts of 10 seconds by default.
    ///
    /// # Arguments
    /// * `address` - The camera's IP address and port (e.g., "192.168.1.100:5678")
    ///
    /// # Errors
    /// Returns an error if the socket cannot be created or configured.
    pub fn new(address: &str) -> io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        let timeout = Some(Duration::from_secs(10));
        socket.set_read_timeout(timeout)?;
        socket.set_write_timeout(timeout)?;
        Ok(Self {
            socket,
            address: address.to_string(),
            stats: ConnectionStats::new(),
        })
    }

    /// Creates a new UDP transport with a custom timeout.
    ///
    /// # Errors
    ///
    /// Returns an error if the socket cannot be created or bound.
    pub fn with_timeout(address: &str, timeout: Duration) -> io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_read_timeout(Some(timeout))?;
        socket.set_write_timeout(Some(timeout))?;
        Ok(Self {
            socket,
            address: address.to_string(),
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
impl BlockingTransport for UdpTransport {
    /// Sends a VISCA command over the UDP socket.
    ///
    /// # Errors
    /// Returns `Error` if the command serialization fails or if there's
    /// an I/O error sending the UDP packet.
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

        let _ = self
            .socket
            .send_to(&bytes, &self.address)
            .map_err(Error::Io)?;

        self.stats.record_sent(bytes.len());
        Ok(())
    }

    /// Receives VISCA response packets from the UDP socket.
    ///
    /// # Errors
    /// Returns `Error` if a receive timeout occurs or if there's
    /// an I/O error reading from the UDP socket.
    fn receive_response_blocking(&mut self) -> Result<(crate::types::SocketId, Vec<u8>), Error> {
        let mut buffer = [0u8; 1024];

        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((size, _)) => {
                    let data = buffer[..size].to_vec();
                    log::debug!("Received data: {data:02X?}");

                    self.stats.record_received(data.len());

                    if data.len() >= 3 && data[0] == 0x90 && data[data.len() - 1] == 0xFF {
                        // Extract socket ID from response header
                        let socket_id = if data[0] == 0x90 {
                            crate::types::SocketId::SOCKET_0 // Default to socket 0
                        } else {
                            // Response format is 0x9X where X is socket ID
                            let socket_value = data[0] & 0x0F;
                            crate::types::SocketId::new(socket_value)
                                .unwrap_or(crate::types::SocketId::SOCKET_0)
                        };

                        // Return completion or error responses immediately
                        if data.len() >= 3
                            && (data[1] == 0x50 || data[1] == 0x51 || (data[1] & 0x60) == 0x60)
                        {
                            return Ok((socket_id, data));
                        }

                        // For ACK responses, continue waiting
                        if data[1] == 0x40 || data[1] == 0x41 {
                            continue;
                        }

                        // Return other responses
                        return Ok((socket_id, data));
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    self.stats.record_error();
                    return Err(Error::CommandTimeout {
                        duration: Duration::from_secs(10),
                        command: "receive_response".to_string(),
                    });
                }
                Err(e) => {
                    self.stats.record_error();
                    return Err(Error::Io(e));
                }
            }
        }
    }
}

/// Async UDP transport for VISCA communication.
#[cfg(feature = "async-client")]
#[derive(Debug)]
pub struct AsyncUdpTransport {
    socket: TokioUdpSocket,
    address: String,
    stats: ConnectionStats,
}

#[cfg(feature = "async-client")]
impl AsyncUdpTransport {
    /// Creates a new async UDP transport.
    ///
    /// # Errors
    ///
    /// Returns an error if the socket cannot be created or bound.
    pub async fn new(address: &str) -> io::Result<Self> {
        let socket = TokioUdpSocket::bind("0.0.0.0:0").await?;
        Ok(Self {
            socket,
            address: address.to_string(),
            stats: ConnectionStats::new(),
        })
    }

    /// Returns the connection statistics.
    #[must_use]
    pub const fn stats(&self) -> &ConnectionStats {
        &self.stats
    }
}

#[cfg(feature = "async-client")]
impl Transport for AsyncUdpTransport {
    /// Sends a VISCA command over the async UDP socket.
    ///
    /// # Errors
    /// Returns `Error` if the command serialization fails or if there's
    /// an I/O error sending the UDP packet.
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

            let _ = self
                .socket
                .send_to(&bytes, &self.address)
                .await
                .map_err(Error::Io)?;

            self.stats.record_sent(bytes.len());
            Ok(())
        })
    }

    /// Receives VISCA response packets from the async UDP socket.
    ///
    /// # Errors
    /// Returns `Error` if a receive timeout occurs or if there's
    /// an I/O error reading from the UDP socket.
    fn receive_response(&mut self) -> TransportFuture<'_, (crate::types::SocketId, Vec<u8>)> {
        Box::pin(async move {
            let mut buffer = [0u8; 1024];

            // Keep receiving until we get a completion or error response
            loop {
                match tokio::time::timeout(
                    Duration::from_secs(10),
                    self.socket.recv_from(&mut buffer),
                )
                .await
                {
                    Ok(Ok((size, _))) => {
                        let data = buffer[..size].to_vec();
                        log::debug!("Received data: {data:02X?}");

                        self.stats.record_received(data.len());

                        // Check if this is a complete VISCA response
                        if data.len() >= 3 && data[0] == 0x90 && data[data.len() - 1] == 0xFF {
                            // Extract socket ID from response header
                            let socket_id = if data[0] == 0x90 {
                                crate::types::SocketId::SOCKET_0 // Default to socket 0
                            } else {
                                // Response format is 0x9X where X is socket ID
                                let socket_value = data[0] & 0x0F;
                                crate::types::SocketId::new(socket_value)
                                    .unwrap_or(crate::types::SocketId::SOCKET_0)
                            };

                            // Return completion or error responses immediately
                            if data.len() >= 3
                                && (data[1] == 0x50 || data[1] == 0x51 || (data[1] & 0x60) == 0x60)
                            {
                                return Ok((socket_id, data));
                            }

                            // For ACK responses, continue waiting
                            if data[1] == 0x40 || data[1] == 0x41 {
                                continue;
                            }

                            // Return other responses
                            return Ok((socket_id, data));
                        }
                    }
                    Ok(Err(e)) => {
                        self.stats.record_error();
                        return Err(Error::Io(e));
                    }
                    Err(_) => {
                        self.stats.record_error();
                        return Err(Error::CommandTimeout {
                            duration: Duration::from_secs(10),
                            command: "receive_response".to_string(),
                        });
                    }
                }
            }
        })
    }
}
