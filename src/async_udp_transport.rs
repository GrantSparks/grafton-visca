//! Async UDP transport implementation for VISCA over IP.

#![allow(deprecated)]

// Standard library imports
use std::net::SocketAddr;

// Third-party imports
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};

// Crate imports
use crate::{
    async_transport::{AsyncViscaTransport, TransportFuture},
    parse_response, ConnectionStats, TimeoutConfig, ViscaCommand, ViscaError,
};

/// Async UDP transport for VISCA over IP communication.
#[cfg(feature = "async-client")]
pub struct AsyncUdpTransport {
    socket: UdpSocket,
    camera_addr: SocketAddr,
    buffer: Vec<u8>,
    timeout_duration: Duration,
    timeout_config: Option<TimeoutConfig>,
    stats: ConnectionStats,
}

#[cfg(feature = "async-client")]
impl AsyncUdpTransport {
    /// Create a new async UDP transport.
    pub async fn new(camera_addr: SocketAddr) -> Result<Self, ViscaError> {
        let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(ViscaError::Io)?;

        Ok(Self {
            socket,
            camera_addr,
            buffer: vec![0; 1024],
            timeout_duration: Duration::from_secs(10),
            timeout_config: None,
            stats: ConnectionStats::new(),
        })
    }

    /// Creates a new async UDP transport with custom timeout configuration.
    ///
    /// # Arguments
    /// * `camera_addr` - The camera's socket address
    /// * `timeout_config` - Timeout configuration for different command types
    pub async fn with_timeout_config(
        camera_addr: SocketAddr,
        timeout_config: TimeoutConfig,
    ) -> Result<Self, ViscaError> {
        let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(ViscaError::Io)?;

        Ok(Self {
            socket,
            camera_addr,
            buffer: vec![0; 1024],
            timeout_duration: timeout_config.default_timeout,
            timeout_config: Some(timeout_config),
            stats: ConnectionStats::new(),
        })
    }

    /// Get connection statistics
    pub fn stats(&self) -> &ConnectionStats {
        &self.stats
    }

    /// Set the timeout duration for receive operations.
    pub fn set_timeout(&mut self, duration: Duration) {
        self.timeout_duration = duration;
    }

    /// Get the timeout configuration
    pub fn timeout_config(&self) -> Option<&TimeoutConfig> {
        self.timeout_config.as_ref()
    }
}

#[cfg(feature = "async-client")]
impl AsyncViscaTransport for AsyncUdpTransport {
    fn send_command<'a>(&'a mut self, command: &'a dyn ViscaCommand) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            // Set timeout based on command category if timeout config is available
            if let Some(ref config) = self.timeout_config {
                self.timeout_duration = config.get_timeout(command.command_category());
            }

            let bytes = command.to_bytes()?;

            log::debug!("Sending command: {:02X?}", bytes);

            match self
                .socket
                .send_to(&bytes, self.camera_addr)
                .await
                .map_err(ViscaError::Io)
            {
                Ok(_) => {
                    self.stats.record_sent(bytes.len());
                    Ok(())
                }
                Err(e) => {
                    self.stats.record_error();
                    Err(e)
                }
            }
        })
    }

    fn receive_response(&mut self) -> TransportFuture<'_, Vec<Vec<u8>>> {
        Box::pin(async move {
            match timeout(
                self.timeout_duration,
                self.socket.recv_from(&mut self.buffer),
            )
            .await
            {
                Ok(Ok((len, _addr))) => {
                    let received_data = &self.buffer[..len];
                    log::debug!("Received data: {:02X?}", received_data);

                    match parse_response(received_data) {
                        Ok(responses) => {
                            self.stats.record_received(len);
                            Ok(responses)
                        }
                        Err(e) => {
                            log::error!("Failed to parse response: {:?}", e);
                            self.stats.record_error();
                            Err(ViscaError::ParseError(format!(
                                "Failed to parse response: {:?}",
                                e
                            )))
                        }
                    }
                }
                Ok(Err(e)) => {
                    log::error!("Socket receive error: {:?}", e);
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
impl Drop for AsyncUdpTransport {
    fn drop(&mut self) {
        // UDP sockets don't require explicit shutdown
        // The OS will clean up when the UdpSocket is dropped
        log::debug!("Dropping AsyncUdpTransport");
    }
}

#[cfg(feature = "async-client")]
impl crate::AsyncConnectionManagement for AsyncUdpTransport {
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
