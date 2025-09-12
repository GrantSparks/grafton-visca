//! Blocking TCP transport implementation with DNS resolution and IPv6 support.

use bytes::Bytes;
use std::{
    io::{BufReader, Read, Write},
    net::TcpStream,
    time::{Duration, Instant},
};

use crate::{
    command::CommandKind,
    protocol::framer::ProtocolFramer,
    transport::{address::AddressResolver, builder::TransportConfig, SyncTransport},
    Error,
};

/// TCP transport for blocking VISCA communication.
///
/// This transport supports DNS resolution and both IPv4 and IPv6 addresses.
pub struct Tcp {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    framer: ProtocolFramer,
    temp_buf: [u8; 256],
}

impl std::fmt::Debug for Tcp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tcp")
            .field("reader", &self.reader)
            .field("writer", &self.writer)
            .field("framer", &self.framer)
            .field("temp_buf", &format_args!("[u8; 256]"))
            .finish()
    }
}

impl Tcp {
    /// Connect to a TCP endpoint.
    ///
    /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
    pub fn connect(address: &str) -> Result<Self, Error> {
        Self::connect_timeout(address, Duration::from_secs(5))
    }

    /// Connect with a custom timeout.
    ///
    /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
    /// It will try each resolved address in order until one succeeds or the
    /// overall timeout is reached.
    pub fn connect_timeout(address: &str, timeout: Duration) -> Result<Self, Error> {
        let config = TransportConfig {
            connect_timeout: timeout,
            read_timeout: Duration::from_secs(5),
            write_timeout: Duration::from_secs(5),
            tcp_nodelay: Some(true),
            ..Default::default()
        };
        Self::connect_with_config(address, config)
    }

    /// Connect with a full configuration.
    ///
    /// This method provides full control over connection and socket parameters.
    pub fn connect_with_config(address: &str, config: TransportConfig) -> Result<Self, Error> {
        let deadline = Instant::now() + config.connect_timeout;

        // Use the common address resolver
        let resolver = AddressResolver::new();
        let addrs = resolver.resolve(address)?;

        let mut last_error = None;

        // Try each address with remaining time
        for addr in addrs {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }

            match TcpStream::connect_timeout(&addr, remaining) {
                Ok(stream) => {
                    // Apply socket options from config
                    stream.set_read_timeout(Some(config.read_timeout))?;
                    stream.set_write_timeout(Some(config.write_timeout))?;
                    if let Some(nodelay) = config.tcp_nodelay {
                        stream.set_nodelay(nodelay)?;
                    } else {
                        // Default to nodelay for low latency
                        stream.set_nodelay(true)?;
                    }
                    if let Some(ttl) = config.ttl {
                        stream.set_ttl(ttl)?;
                    }

                    // Clone the stream for separate reader and writer
                    let reader_stream = stream.try_clone()?;

                    return Ok(Self {
                        reader: BufReader::new(reader_stream),
                        writer: stream,
                        framer: ProtocolFramer::new_with_config(config.buffer_config),
                        temp_buf: [0u8; 256],
                    });
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        // All attempts failed
        Err(last_error.map(Into::into).unwrap_or_else(|| Error::Timeout))
    }

    // Retry configuration is now handled at the runtime/scheduler level
}

impl SyncTransport for Tcp {
    fn send_with_kind(&mut self, data: &[u8], _kind: CommandKind) -> Result<(), Error> {
        // Send directly - retry logic is handled at the runtime/scheduler level
        self.writer.write_all(data)?;
        self.writer.flush()?;
        Ok(())
    }

    fn recv(&mut self) -> Result<Bytes, Error> {
        // First check if we have a buffered frame from a previous read
        if let Some(frame_result) = self.framer.drain_frames().next() {
            return frame_result;
        }

        // Read more data until we get a complete frame
        loop {
            let n = self.reader.read(&mut self.temp_buf).map_err(Error::Io)?;

            if n == 0 {
                // Connection closed - try to extract any terminated frame
                if let Some(result) = self.framer.drain_on_eof() {
                    return result;
                }

                // No valid frame could be extracted
                if self.framer.is_empty() {
                    return Err(Error::ConnectionClosed {
                        reason: Some("peer closed connection".into()),
                    });
                } else {
                    return Err(Error::ConnectionClosed {
                        reason: Some("connection closed with partial frame".into()),
                    });
                }
            }

            // Push data to framer
            self.framer.push_slice(&self.temp_buf[..n])?;

            // Try to extract a complete frame
            if let Some(frame_result) = self.framer.drain_frames().next() {
                return frame_result;
            }
        }
    }

    fn recv_with_timeout(&mut self, duration: Duration) -> Result<Bytes, Error> {
        // First check if we have a buffered frame from a previous read
        if let Some(frame_result) = self.framer.drain_frames().next() {
            return frame_result;
        }

        // Save the current timeout
        let original_timeout = self.reader.get_ref().read_timeout()?;

        // Set the new timeout for this operation
        self.reader.get_mut().set_read_timeout(Some(duration))?;

        // Read more data until we get a complete frame or timeout
        let result = loop {
            match self.reader.read(&mut self.temp_buf) {
                Ok(0) => {
                    // Connection closed - try to extract any terminated frame
                    if let Some(result) = self.framer.drain_on_eof() {
                        break result;
                    }

                    // No valid frame could be extracted
                    if self.framer.is_empty() {
                        break Err(Error::ConnectionClosed {
                            reason: Some("peer closed connection".into()),
                        });
                    } else {
                        break Err(Error::ConnectionClosed {
                            reason: Some("connection closed with partial frame".into()),
                        });
                    }
                }
                Ok(n) => {
                    // Push data to framer
                    if let Err(e) = self.framer.push_slice(&self.temp_buf[..n]) {
                        break Err(e);
                    }

                    // Try to extract a complete frame
                    if let Some(frame_result) = self.framer.drain_frames().next() {
                        break frame_result;
                    }
                    // Continue looping to read more data
                }
                Err(io_err)
                    if io_err.kind() == std::io::ErrorKind::TimedOut
                        || io_err.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    // Timeout occurred - check if we have a buffered frame
                    if let Some(frame_result) = self.framer.drain_frames().next() {
                        break frame_result;
                    }
                    break Err(Error::Timeout);
                }
                Err(io_err) => {
                    break Err(Error::Io(io_err));
                }
            }
        };

        // Restore the original timeout
        self.reader.get_mut().set_read_timeout(original_timeout)?;

        result
    }
}
