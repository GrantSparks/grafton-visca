use crate::{
    async_transport::{AsyncViscaTransport, TransportFuture},
    parse_response, ConnectionStats, ViscaCommand, ViscaError,
};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

const MAX_BUFFER_SIZE: usize = 64 * 1024; // 64KB max buffer size

/// Async TCP transport for VISCA over IP communication.
#[cfg(feature = "async")]
pub struct AsyncTcpTransport {
    stream: TcpStream,
    buffer: Vec<u8>,
    read_buffer: Vec<u8>,
    timeout_duration: Duration,
    stats: ConnectionStats,
}

#[cfg(feature = "async")]
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
            stats: ConnectionStats::new(),
        })
    }

    /// Set the timeout duration for receive operations.
    pub fn set_timeout(&mut self, duration: Duration) {
        self.timeout_duration = duration;
    }
}

#[cfg(feature = "async")]
impl AsyncViscaTransport for AsyncTcpTransport {
    fn send_command<'a>(&'a mut self, command: &'a dyn ViscaCommand) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            let bytes = command.to_bytes()?;

            log::debug!("Sending command: {:02X?}", bytes);

            match self.stream.write_all(&bytes).await {
                Ok(_) => match self.stream.flush().await {
                    Ok(_) => {
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
                    Err(ViscaError::Io(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "Connection closed",
                    )))
                }
                Ok(Ok(n)) => {
                    // Check buffer size before appending
                    if self.buffer.len() + n > MAX_BUFFER_SIZE {
                        // Clear buffer if it's getting too large
                        log::error!("TCP buffer exceeded maximum size, clearing buffer");
                        self.buffer.clear();
                        self.stats.record_error();
                        return Err(ViscaError::Io(std::io::Error::other(
                            "Buffer overflow - too much unparseable data",
                        )));
                    }

                    self.buffer.extend_from_slice(&self.read_buffer[..n]);
                    self.stats.record_received(n);

                    // Try to parse complete responses from the buffer
                    match parse_response(&self.buffer) {
                        Ok(responses) => {
                            // Calculate how many bytes were consumed
                            let consumed_bytes: usize = responses.iter().map(|r| r.len()).sum();

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

#[cfg(feature = "async")]
impl Drop for AsyncTcpTransport {
    fn drop(&mut self) {
        // Best effort to shutdown the TCP connection gracefully
        // We can't do async operations in drop, so we just rely on the OS
        // to clean up the socket when the TcpStream is dropped
        log::debug!("Dropping AsyncTcpTransport");
    }
}

#[cfg(feature = "async")]
impl crate::AsyncConnectionManagement for AsyncTcpTransport {
    fn is_healthy(&mut self) -> TransportFuture<'_, Result<bool, ViscaError>> {
        Box::pin(async move {
            use crate::command::InquiryCommand;
            use tokio::time::timeout;

            // Check cached health result first
            if let Some(cached_healthy) = self.stats.get_cached_health() {
                return Ok(Ok(cached_healthy));
            }

            // Send the power inquiry command
            self.send_command(&InquiryCommand::Power).await?;

            // Try to receive response with a short timeout
            match timeout(Duration::from_secs(1), self.receive_response()).await {
                Ok(Ok(responses)) => {
                    let healthy = !responses.is_empty();
                    self.stats.record_health_check(healthy);
                    Ok(Ok(healthy))
                }
                Ok(Err(e)) => Ok(Err(e)),
                Err(_) => {
                    self.stats.record_health_check(false);
                    Ok(Ok(false)) // Timeout means unhealthy but not an error
                }
            }
        })
    }

    fn connection_stats(&self) -> &ConnectionStats {
        &self.stats
    }
}
