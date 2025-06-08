//! Unified TCP transport implementation that works for both async and blocking contexts.

use std::io;
use std::time::Duration;

use super::common::{log_frame, parse_frame_type, BufferManager, FrameType};
use crate::{ConnectionStats, ViscaCommand, ViscaError};

#[cfg(feature = "blocking-client")]
use std::net::TcpStream;

#[cfg(feature = "async-client")]
use tokio::net::TcpStream as TokioTcpStream;

/// Configuration for TCP transport.
#[derive(Debug, Clone, Copy)]
pub struct TcpConfig {
    /// Read timeout duration.
    pub read_timeout: Duration,
    /// Write timeout duration.
    pub write_timeout: Duration,
    /// Buffer size for receiving data.
    pub buffer_size: usize,
}

impl Default for TcpConfig {
    fn default() -> Self {
        Self {
            read_timeout: Duration::from_secs(10),
            write_timeout: Duration::from_secs(10),
            buffer_size: 1024,
        }
    }
}

/// Unified TCP transport that can work in both async and blocking contexts.
#[derive(Debug)]
pub struct UnifiedTcpTransport<S> {
    stream: S,
    stats: ConnectionStats,
    buffer: BufferManager,
    config: TcpConfig,
}

impl<S> UnifiedTcpTransport<S> {
    /// Get connection statistics.
    pub const fn stats(&self) -> &ConnectionStats {
        &self.stats
    }

    /// Get the configuration.
    pub const fn config(&self) -> &TcpConfig {
        &self.config
    }
}

// Blocking implementation
#[cfg(feature = "blocking-client")]
impl UnifiedTcpTransport<TcpStream> {
    /// Create a new blocking TCP transport.
    ///
    /// # Errors
    /// Returns `io::Error` if the TCP connection fails.
    pub fn new_blocking(address: &str) -> io::Result<Self> {
        Self::with_config_blocking(address, TcpConfig::default())
    }

    /// Create a new blocking TCP transport with custom configuration.
    ///
    /// # Errors
    /// Returns `io::Error` if the TCP connection fails or timeout configuration fails.
    pub fn with_config_blocking(address: &str, config: TcpConfig) -> io::Result<Self> {
        let stream = TcpStream::connect(address)?;
        stream.set_read_timeout(Some(config.read_timeout))?;
        stream.set_write_timeout(Some(config.write_timeout))?;

        Ok(Self {
            stream,
            stats: ConnectionStats::new(),
            buffer: BufferManager::new(config.buffer_size),
            config,
        })
    }

    /// Send a command synchronously.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command encoding or TCP write fails.
    pub fn send_blocking(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError> {
        use std::io::Write;

        let bytes = command.to_bytes()?;
        log_frame("TCP send", &bytes);

        self.stream.write_all(&bytes).map_err(ViscaError::Io)?;
        self.stream.flush().map_err(ViscaError::Io)?;

        self.stats.record_sent(bytes.len());
        Ok(())
    }

    /// Receive responses synchronously.
    ///
    /// # Errors
    /// Returns `ViscaError` if:
    /// - The connection is closed by the camera (`UnexpectedEof`)
    /// - A read timeout occurs (`CommandTimeout`)
    /// - An incomplete VISCA frame is received (`InvalidData`)
    /// - Any other I/O error occurs during reading
    pub fn receive_blocking(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
        use std::io::Read;

        let mut temp_buffer = vec![0u8; self.config.buffer_size];
        let mut all_frames = Vec::new();

        loop {
            match self.stream.read(&mut temp_buffer) {
                Ok(0) => {
                    self.stats.record_error();
                    return Err(ViscaError::Io(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Connection closed by camera",
                    )));
                }
                Ok(size) => {
                    self.buffer
                        .buffer_mut()
                        .extend_from_slice(&temp_buffer[..size]);
                    let frames = self.buffer.extract_frames();

                    for frame in &frames {
                        log_frame("TCP recv", frame);
                        self.stats.record_received(frame.len());

                        // Check if this is a terminal frame
                        match parse_frame_type(frame) {
                            FrameType::Completion { .. }
                            | FrameType::Error { .. }
                            | FrameType::Inquiry => {
                                all_frames.extend(frames);
                                return Ok(all_frames);
                            }
                            _ => {}
                        }
                    }

                    all_frames.extend(frames);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if all_frames.is_empty() && self.buffer.buffer_mut().is_empty() {
                        self.stats.record_error();
                        return Err(ViscaError::CommandTimeout {
                            duration: self.config.read_timeout,
                            command: "receive_response".to_string(),
                        });
                    }

                    if !self.buffer.buffer_mut().is_empty() {
                        // Incomplete frame
                        self.stats.record_error();
                        return Err(ViscaError::Io(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Incomplete VISCA frame",
                        )));
                    }

                    return Ok(all_frames);
                }
                Err(e) => {
                    self.stats.record_error();
                    return Err(ViscaError::Io(e));
                }
            }
        }
    }
}

// Async implementation
#[cfg(feature = "async-client")]
impl UnifiedTcpTransport<TokioTcpStream> {
    /// Create a new async TCP transport.
    ///
    /// # Errors
    /// Returns `io::Error` if the TCP connection fails.
    pub async fn new_async(address: &str) -> io::Result<Self> {
        Self::with_config_async(address, TcpConfig::default()).await
    }

    /// Create a new async TCP transport with custom configuration.
    ///
    /// # Errors
    /// Returns `io::Error` if the TCP connection fails.
    pub async fn with_config_async(address: &str, config: TcpConfig) -> io::Result<Self> {
        let stream = TokioTcpStream::connect(address).await?;

        Ok(Self {
            stream,
            stats: ConnectionStats::new(),
            buffer: BufferManager::new(config.buffer_size),
            config,
        })
    }

    /// Send a command asynchronously.
    ///
    /// # Errors
    /// Returns `ViscaError` if:
    /// - The command encoding fails
    /// - The TCP write operation fails
    /// - The flush operation fails
    pub async fn send_async(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError> {
        use tokio::io::AsyncWriteExt;

        let bytes = command.to_bytes()?;
        log_frame("TCP send", &bytes);

        self.stream
            .write_all(&bytes)
            .await
            .map_err(ViscaError::Io)?;
        self.stream.flush().await.map_err(ViscaError::Io)?;

        self.stats.record_sent(bytes.len());
        Ok(())
    }

    /// Receive responses asynchronously.
    ///
    /// # Errors
    /// Returns `ViscaError` if:
    /// - The connection is closed by the camera (`UnexpectedEof`)
    /// - A read timeout occurs (`CommandTimeout`)
    /// - An incomplete VISCA frame is received (`InvalidData`)
    /// - Any other I/O error occurs during reading
    pub async fn receive_async(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
        use tokio::io::AsyncReadExt;

        let mut temp_buffer = vec![0u8; self.config.buffer_size];
        let mut all_frames = Vec::new();

        loop {
            match tokio::time::timeout(self.config.read_timeout, self.stream.read(&mut temp_buffer))
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
                    self.buffer
                        .buffer_mut()
                        .extend_from_slice(&temp_buffer[..size]);
                    let frames = self.buffer.extract_frames();

                    for frame in &frames {
                        log_frame("TCP recv", frame);
                        self.stats.record_received(frame.len());

                        // Check if this is a terminal frame
                        match parse_frame_type(frame) {
                            FrameType::Completion { .. }
                            | FrameType::Error { .. }
                            | FrameType::Inquiry => {
                                all_frames.extend(frames);
                                return Ok(all_frames);
                            }
                            _ => {}
                        }
                    }

                    all_frames.extend(frames);
                }
                Ok(Err(e)) => {
                    self.stats.record_error();
                    return Err(ViscaError::Io(e));
                }
                Err(_) => {
                    if all_frames.is_empty() && self.buffer.buffer_mut().is_empty() {
                        self.stats.record_error();
                        return Err(ViscaError::CommandTimeout {
                            duration: self.config.read_timeout,
                            command: "receive_response".to_string(),
                        });
                    }

                    if !self.buffer.buffer_mut().is_empty() {
                        // Incomplete frame
                        self.stats.record_error();
                        return Err(ViscaError::Io(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Incomplete VISCA frame",
                        )));
                    }

                    return Ok(all_frames);
                }
            }
        }
    }
}

// Implement the unified transport trait for blocking
#[cfg(feature = "blocking-client")]
impl super::unified::BlockingTransport for UnifiedTcpTransport<TcpStream> {
    fn send_blocking(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError> {
        self.send_blocking(command)
    }

    fn receive_blocking(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
        self.receive_blocking()
    }
}

// Implement the unified transport trait for async
#[cfg(feature = "async-client")]
impl super::unified::UnifiedTransport for UnifiedTcpTransport<TokioTcpStream> {
    type SendFuture<'a> =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ViscaError>> + Send + 'a>>;
    type ReceiveFuture<'a> = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<Vec<u8>>, ViscaError>> + Send + 'a>,
    >;

    fn send_command<'a>(&'a mut self, command: &'a dyn ViscaCommand) -> Self::SendFuture<'a> {
        Box::pin(self.send_async(command))
    }

    fn receive_response(&mut self) -> Self::ReceiveFuture<'_> {
        Box::pin(self.receive_async())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcp_config_default() {
        let config = TcpConfig::default();
        assert_eq!(config.read_timeout, Duration::from_secs(10));
        assert_eq!(config.write_timeout, Duration::from_secs(10));
        assert_eq!(config.buffer_size, 1024);
    }
}
