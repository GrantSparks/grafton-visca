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
use crate::{ConnectionStats, ViscaCommand, ViscaError};

#[cfg(feature = "blocking-client")]
use super::BlockingTransport;

#[cfg(feature = "async-client")]
use super::{Transport, TransportFuture};

/// Blocking UDP transport for VISCA communication.
#[cfg(feature = "blocking-client")]
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
    pub fn stats(&self) -> &ConnectionStats {
        &self.stats
    }
}

#[cfg(feature = "blocking-client")]
impl BlockingTransport for UdpTransport {
    fn send_command_blocking(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError> {
        let bytes = command.to_bytes()?;
        log::debug!("Sending command: {:02X?}", bytes);

        self.socket
            .send_to(&bytes, &self.address)
            .map_err(ViscaError::Io)?;

        self.stats.record_sent(bytes.len());
        Ok(())
    }

    fn receive_response_blocking(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
        let mut buffer = [0u8; 1024];
        let mut responses = Vec::new();

        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((size, _)) => {
                    let data = buffer[..size].to_vec();
                    log::debug!("Received data: {:02X?}", data);

                    self.stats.record_received(data.len());

                    if data.len() >= 3 && data[0] == 0x90 && data[data.len() - 1] == 0xFF {
                        responses.push(data.clone());

                        if data.len() >= 3
                            && (data[1] == 0x50 || data[1] == 0x51 || (data[1] & 0x60) == 0x60)
                        {
                            break;
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if responses.is_empty() {
                        self.stats.record_error();
                        return Err(ViscaError::CommandTimeout {
                            duration: Duration::from_secs(10),
                            command: "receive_response".to_string(),
                        });
                    }
                    break;
                }
                Err(e) => {
                    self.stats.record_error();
                    return Err(ViscaError::Io(e));
                }
            }
        }

        Ok(responses)
    }
}

/// Async UDP transport for VISCA communication.
#[cfg(feature = "async-client")]
pub struct AsyncUdpTransport {
    socket: TokioUdpSocket,
    address: String,
    stats: ConnectionStats,
}

#[cfg(feature = "async-client")]
impl AsyncUdpTransport {
    /// Creates a new async UDP transport.
    pub async fn new(address: &str) -> io::Result<Self> {
        let socket = TokioUdpSocket::bind("0.0.0.0:0").await?;
        Ok(Self {
            socket,
            address: address.to_string(),
            stats: ConnectionStats::new(),
        })
    }

    /// Returns the connection statistics.
    pub fn stats(&self) -> &ConnectionStats {
        &self.stats
    }
}

#[cfg(feature = "async-client")]
impl Transport for AsyncUdpTransport {
    fn send_command<'a>(&'a mut self, command: &'a dyn ViscaCommand) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            let bytes = command.to_bytes()?;
            log::debug!("Sending command: {:02X?}", bytes);

            self.socket
                .send_to(&bytes, &self.address)
                .await
                .map_err(ViscaError::Io)?;

            self.stats.record_sent(bytes.len());
            Ok(())
        })
    }

    fn receive_response(&mut self) -> TransportFuture<'_, Vec<Vec<u8>>> {
        Box::pin(async move {
            let mut buffer = [0u8; 1024];
            let mut responses = Vec::new();

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
                        log::debug!("Received data: {:02X?}", data);

                        self.stats.record_received(data.len());

                        // Check if this is a complete VISCA response
                        if data.len() >= 3 && data[0] == 0x90 && data[data.len() - 1] == 0xFF {
                            responses.push(data.clone());

                            // Check for completion or error
                            if data.len() >= 3
                                && (data[1] == 0x50 || data[1] == 0x51 || (data[1] & 0x60) == 0x60)
                            {
                                break;
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        self.stats.record_error();
                        return Err(ViscaError::Io(e));
                    }
                    Err(_) => {
                        if responses.is_empty() {
                            self.stats.record_error();
                            return Err(ViscaError::CommandTimeout {
                                duration: Duration::from_secs(10),
                                command: "receive_response".to_string(),
                            });
                        }
                        break;
                    }
                }
            }

            Ok(responses)
        })
    }
}

// TODO: Remove once ViscaTransport trait is fully removed
// #[cfg(feature = "blocking-client")]
// impl crate::ViscaTransport for UdpTransport {
//     fn send_command(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError> {
//         self.send_command_blocking(command)
//     }
//
//     fn receive_response(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
//         self.receive_response_blocking()
//     }
//
//     fn send_and_wait(&mut self, command: &dyn ViscaCommand) -> Result<crate::ViscaResponse, ViscaError> {
//         // Use the proper send_command_and_wait implementation from the crate
//         crate::send_command_and_wait(self, command)
//     }
// }
