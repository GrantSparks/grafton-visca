use crate::{
    async_transport::{AsyncViscaTransport, TransportFuture},
    parse_response, ConnectionStats, ViscaCommand, ViscaError,
};
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};

/// Async UDP transport for VISCA over IP communication.
#[cfg(feature = "async")]
pub struct AsyncUdpTransport {
    socket: UdpSocket,
    camera_addr: SocketAddr,
    buffer: Vec<u8>,
    timeout_duration: Duration,
    stats: ConnectionStats,
}

#[cfg(feature = "async")]
impl AsyncUdpTransport {
    /// Create a new async UDP transport.
    pub async fn new(camera_addr: SocketAddr) -> Result<Self, ViscaError> {
        let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(ViscaError::Io)?;

        Ok(Self {
            socket,
            camera_addr,
            buffer: vec![0; 1024],
            timeout_duration: Duration::from_secs(10),
            stats: ConnectionStats::new(),
        })
    }

    /// Set the timeout duration for receive operations.
    pub fn set_timeout(&mut self, duration: Duration) {
        self.timeout_duration = duration;
    }
}

#[cfg(feature = "async")]
impl AsyncViscaTransport for AsyncUdpTransport {
    fn send_command<'a>(&'a mut self, command: &'a dyn ViscaCommand) -> TransportFuture<'a, ()> {
        Box::pin(async move {
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

#[cfg(feature = "async")]
impl Drop for AsyncUdpTransport {
    fn drop(&mut self) {
        // UDP sockets don't require explicit shutdown
        // The OS will clean up when the UdpSocket is dropped
        log::debug!("Dropping AsyncUdpTransport");
    }
}

#[cfg(feature = "async")]
impl crate::AsyncConnectionManagement for AsyncUdpTransport {
    fn is_healthy(&mut self) -> TransportFuture<'_, bool> {
        Box::pin(async move {
            use crate::command::InquiryCommand;

            // Temporarily reduce timeout for health check
            let original_timeout = self.timeout_duration;
            self.timeout_duration = Duration::from_secs(1);

            let result = self.send_command(&InquiryCommand::Power).await.is_ok()
                && match self.receive_response().await {
                    Ok(responses) => !responses.is_empty(),
                    Err(_) => false,
                };

            // Restore original timeout
            self.timeout_duration = original_timeout;

            Ok(result)
        })
    }

    fn connection_stats(&self) -> &ConnectionStats {
        &self.stats
    }

    fn connection_stats_mut(&mut self) -> &mut ConnectionStats {
        &mut self.stats
    }
}
