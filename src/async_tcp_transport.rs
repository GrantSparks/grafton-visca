// Standard library imports
use std::net::SocketAddr;

// Third-party crate imports
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

// Workspace / local-crate imports
use crate::{
    async_transport::{AsyncViscaTransport, TransportFuture},
    parse_response, ConnectionStats, TimeoutConfig, ViscaCommand, ViscaError,
};

const MAX_BUFFER_SIZE: usize = 64 * 1024; // 64KB max buffer size

/// Async TCP transport for VISCA over IP communication.
#[cfg(feature = "async-client")]
pub struct AsyncTcpTransport {
    stream: TcpStream,
    buffer: Vec<u8>,
    read_buffer: Vec<u8>,
    timeout_duration: Duration,
    timeout_config: Option<TimeoutConfig>,
    stats: ConnectionStats,
}

#[cfg(feature = "async-client")]
impl AsyncTcpTransport {
    /// Create a new async TCP transport.
    pub async fn new(camera_addr: SocketAddr) -> Result<Self, ViscaError> {
        let stream = TcpStream::connect(camera_addr)
            .await
            .map_err(ViscaError::Io)?;

        Ok(Self {
            stream,
            buffer: Vec::with_capacity(1024),
            read_buffer: vec![0; 1024],
            timeout_duration: Duration::from_secs(30),
            timeout_config: None,
            stats: ConnectionStats::new(),
        })
    }

    /// Creates a new async TCP transport with custom timeout configuration.
    ///
    /// # Arguments
    /// * `camera_addr` - The camera's socket address
    /// * `timeout_config` - Timeout configuration for different command types
    pub async fn with_timeout_config(
        camera_addr: SocketAddr,
        timeout_config: TimeoutConfig,
    ) -> Result<Self, ViscaError> {
        let stream = TcpStream::connect(camera_addr)
            .await
            .map_err(ViscaError::Io)?;

        Ok(Self {
            stream,
            buffer: Vec::with_capacity(1024),
            read_buffer: vec![0; 1024],
            timeout_duration: timeout_config.default_timeout,
            timeout_config: Some(timeout_config),
            stats: ConnectionStats::new(),
        })
    }

    /// Get connection statistics
    #[must_use]
    pub const fn stats(&self) -> &ConnectionStats {
        &self.stats
    }

    /// Set the timeout duration for receive operations.
    pub fn set_timeout(&mut self, duration: Duration) {
        self.timeout_duration = duration;
    }

    /// Get the timeout configuration
    #[must_use]
    pub fn timeout_config(&self) -> Option<&TimeoutConfig> {
        self.timeout_config.as_ref()
    }
}

#[cfg(feature = "async-client")]
impl AsyncViscaTransport for AsyncTcpTransport {
    fn send_command<'a>(&'a mut self, command: &'a dyn ViscaCommand) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            // Set timeout based on command category if timeout config is available
            if let Some(ref config) = self.timeout_config {
                self.timeout_duration = config.get_timeout(command.command_category());
            }

            let bytes = command.to_bytes()?;

            log::debug!("Sending command: {:02X?}", bytes);

            match self.stream.write_all(&bytes).await {
                Ok(()) => match self.stream.flush().await {
                    Ok(()) => {
                        self.stats.record_sent(bytes.len());
                        Ok(())
                    }
                    Err(e) => {
                        self.stats.record_error();
                        Err(ViscaError::Io(e))
                    }
                },
                Err(e) => {
                    self.stats.record_error();
                    Err(ViscaError::Io(e))
                }
            }
        })
    }

    fn receive_response(&mut self) -> TransportFuture<'_, Vec<Vec<u8>>> {
        Box::pin(async move {
            match timeout(
                self.timeout_duration,
                self.stream.read(&mut self.read_buffer),
            )
            .await
            {
                Ok(Ok(0)) => {
                    // Connection closed
                    Err(ViscaError::Io(std::io::Error::other("Connection closed")))
                }
                Ok(Ok(n)) => {
                    // Check buffer size before appending
                    if self.buffer.len() + n > MAX_BUFFER_SIZE {
                        // Try to parse what we have before clearing
                        if let Ok(responses) = parse_response(&self.buffer) {
                            if !responses.is_empty() {
                                // We have some valid responses, return them
                                let consumed_bytes: usize = responses.iter().map(Vec::len).sum();
                                self.buffer.drain(..consumed_bytes);
                                // Add new data if there's room now
                                if self.buffer.len() + n <= MAX_BUFFER_SIZE {
                                    self.buffer.extend_from_slice(&self.read_buffer[..n]);
                                    self.stats.record_received(n);
                                }
                                return Ok(responses);
                            }
                        }

                        // No valid responses found, clear oldest data to make room
                        log::warn!(
                            "TCP buffer near capacity, removing oldest {} bytes",
                            self.buffer.len() / 2
                        );
                        self.buffer.drain(..self.buffer.len() / 2);
                        self.stats.record_error();
                    }

                    self.buffer.extend_from_slice(&self.read_buffer[..n]);
                    self.stats.record_received(n);

                    // Try to parse complete responses from the buffer
                    match parse_response(&self.buffer) {
                        Ok(responses) => {
                            // Calculate how many bytes were consumed
                            let consumed_bytes: usize = responses.iter().map(Vec::len).sum();

                            // Remove consumed bytes from buffer
                            self.buffer.drain(..consumed_bytes);

                            log::debug!(
                                "Parsed {} responses, {} bytes remain in buffer",
                                responses.len(),
                                self.buffer.len()
                            );

                            Ok(responses)
                        }
                        Err(e) => {
                            log::debug!(
                                "Incomplete response in buffer, waiting for more data: {:?}",
                                e
                            );
                            // Return empty vec to indicate no complete responses yet
                            Ok(vec![])
                        }
                    }
                }
                Ok(Err(e)) => {
                    log::error!("Socket read error: {:?}", e);
                    self.stats.record_error();
                    Err(ViscaError::Io(e))
                }
                Err(_) => {
                    log::debug!("Receive timeout");
                    self.stats.record_error();
                    Err(ViscaError::Timeout)
                }
            }
        })
    }
}

#[cfg(feature = "async-client")]
impl Drop for AsyncTcpTransport {
    fn drop(&mut self) {
        // Best effort to shutdown the TCP connection gracefully
        // We can't do async operations in drop, so we just rely on the OS
        // to clean up the socket when the TcpStream is dropped
        log::debug!("Dropping AsyncTcpTransport");
    }
}

#[cfg(feature = "async-client")]
impl crate::AsyncConnectionManagement for AsyncTcpTransport {
    fn is_healthy(&mut self) -> TransportFuture<'_, bool> {
        Box::pin(async move {
            use crate::command::InquiryCommand;
            use tokio::time::timeout;

            // Check cached health result first
            if let Some(cached_healthy) = self.stats.get_cached_health() {
                return Ok(cached_healthy);
            }

            // Save original timeout
            let original_timeout = self.timeout_duration;
            self.timeout_duration = Duration::from_secs(1);

            // Send the power inquiry command
            let send_result = self.send_command(&InquiryCommand::Power).await;

            // Always restore timeout
            self.timeout_duration = original_timeout;

            if send_result.is_err() {
                self.stats.record_health_check(false);
                return Ok(false);
            }

            // Try to receive response with a short timeout
            match timeout(Duration::from_secs(1), self.receive_response()).await {
                Ok(Ok(responses)) => {
                    // Validate that we got a power inquiry response
                    let healthy = responses.iter().any(|response| {
                        // Power inquiry response format: 0x90 0x50 0x0{2,3} 0xFF
                        response.len() == 4
                            && response[0] == 0x90
                            && response[1] == 0x50
                            && (response[2] == 0x02 || response[2] == 0x03)
                            && response[3] == 0xFF
                    });
                    self.stats.record_health_check(healthy);
                    Ok(healthy)
                }
                Ok(Err(_)) | Err(_) => {
                    self.stats.record_health_check(false);
                    Ok(false) // Any error means unhealthy
                }
            }
        })
    }

    fn connection_stats(&self) -> &ConnectionStats {
        &self.stats
    }
}
