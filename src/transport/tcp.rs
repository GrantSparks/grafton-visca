//! TCP transport implementation for VISCA over IP.

// Standard library imports
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use std::{io, time::Duration};

// Crate imports
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use crate::{ConnectionStats, ViscaCommand, ViscaError};

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
        let stream = TcpStream::connect_timeout(&address.parse().unwrap(), timeout)?;
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
    fn send_command_blocking(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError> {
        let bytes = command.to_bytes()?;
        log::debug!("Sending command: {bytes:02X?}");

        self.stream.write_all(&bytes).map_err(ViscaError::Io)?;

        self.stream.flush().map_err(ViscaError::Io)?;

        self.stats.record_sent(bytes.len());
        Ok(())
    }

    fn receive_response_blocking(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
        let mut buffer = [0u8; 1024];
        let mut responses = Vec::new();
        let mut current_response = Vec::new();

        // Keep receiving until we get a completion or error response
        loop {
            match self.stream.read(&mut buffer) {
                Ok(0) => {
                    self.stats.record_error();
                    return Err(ViscaError::Io(io::Error::new(
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
                            responses.push(current_response.clone());

                            // Check for completion or error
                            if current_response.len() >= 3
                                && (current_response[1] == 0x50
                                    || current_response[1] == 0x51
                                    || (current_response[1] & 0x60) == 0x60)
                            {
                                return Ok(responses);
                            }

                            current_response.clear();
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if responses.is_empty() && current_response.is_empty() {
                        self.stats.record_error();
                        return Err(ViscaError::CommandTimeout {
                            duration: Duration::from_secs(10),
                            command: "receive_response".to_string(),
                        });
                    }
                    if !current_response.is_empty() {
                        // Incomplete frame
                        self.stats.record_error();
                        return Err(ViscaError::Io(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Incomplete VISCA frame",
                        )));
                    }
                    return Ok(responses);
                }
                Err(e) => {
                    self.stats.record_error();
                    return Err(ViscaError::Io(e));
                }
            }
        }
    }
}

// TODO: Remove once ViscaTransport trait is fully removed
// #[cfg(feature = "blocking-client")]
// impl crate::ViscaTransport for TcpTransport {
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

/// Async TCP transport for VISCA communication.
#[cfg(feature = "async-client")]
pub struct AsyncTcpTransport {
    stream: TokioTcpStream,
    stats: ConnectionStats,
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

#[cfg(feature = "async-client")]
impl Transport for AsyncTcpTransport {
    fn send_command<'a>(&'a mut self, command: &'a dyn ViscaCommand) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            let bytes = command.to_bytes()?;
            log::debug!("Sending command: {bytes:02X?}");

            self.stream
                .write_all(&bytes)
                .await
                .map_err(ViscaError::Io)?;

            self.stream.flush().await.map_err(ViscaError::Io)?;

            self.stats.record_sent(bytes.len());
            Ok(())
        })
    }

    fn receive_response(&mut self) -> TransportFuture<'_, Vec<Vec<u8>>> {
        Box::pin(async move {
            let mut buffer = [0u8; 1024];
            let mut responses = Vec::new();
            let mut current_response = Vec::new();

            // Keep receiving until we get a completion or error response
            loop {
                match tokio::time::timeout(Duration::from_secs(10), self.stream.read(&mut buffer))
                    .await
                {
                    Ok(Ok(0)) => {
                        self.stats.record_error();
                        return Err(ViscaError::Io(io::Error::new(
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

                                self.stats.record_received(current_response.len());
                                responses.push(current_response.clone());

                                // Check for completion or error
                                if current_response.len() >= 3
                                    && (current_response[1] == 0x50
                                        || current_response[1] == 0x51
                                        || (current_response[1] & 0x60) == 0x60)
                                {
                                    return Ok(responses);
                                }

                                current_response.clear();
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        self.stats.record_error();
                        return Err(ViscaError::Io(e));
                    }
                    Err(_) => {
                        if responses.is_empty() && current_response.is_empty() {
                            self.stats.record_error();
                            return Err(ViscaError::CommandTimeout {
                                duration: Duration::from_secs(10),
                                command: "receive_response".to_string(),
                            });
                        }
                        if !current_response.is_empty() {
                            // Incomplete frame
                            self.stats.record_error();
                            return Err(ViscaError::Io(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "Incomplete VISCA frame",
                            )));
                        }
                        return Ok(responses);
                    }
                }
            }
        })
    }
}
