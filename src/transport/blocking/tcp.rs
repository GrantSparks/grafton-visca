//! Blocking TCP transport implementation with DNS resolution and IPv6 support.

use bytes::Bytes;
use std::{
    borrow::Cow,
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use crate::{
    command::bytes::VISCA_TERMINATOR,
    transport::{
        address::AddressResolver, buffer::BufferManager, builder::TransportConfig,
        retry::RetryExecutor, RetryConfig, SyncTransport,
    },
    Error,
};

/// TCP transport for blocking VISCA communication.
///
/// This transport supports DNS resolution and both IPv4 and IPv6 addresses.
#[derive(Debug)]
pub struct Tcp {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    retry_executor: RetryExecutor,
    buffer_manager: BufferManager,
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

                    // Create buffer manager with config
                    let buffer_manager = BufferManager::new(config.buffer_config);

                    // Create retry executor with config
                    let retry_executor = RetryExecutor::new(config.retry_config);

                    return Ok(Self {
                        reader: BufReader::new(reader_stream),
                        writer: stream,
                        retry_executor,
                        buffer_manager,
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

    /// Set the retry configuration for this transport.
    pub fn set_retry_config(&mut self, config: RetryConfig) {
        self.retry_executor.set_config(config);
    }

    /// Get the current retry configuration.
    pub fn retry_config(&self) -> &RetryConfig {
        self.retry_executor.config()
    }

    /// Set TCP nodelay option (disable Nagle's algorithm).
    pub fn set_nodelay(&mut self, nodelay: bool) -> Result<(), Error> {
        self.writer.set_nodelay(nodelay)?;
        Ok(())
    }

    /// Set TTL (Time To Live) for packets.
    pub fn set_ttl(&mut self, ttl: u32) -> Result<(), Error> {
        self.writer.set_ttl(ttl)?;
        Ok(())
    }

    /// Set read timeout for receive operations.
    pub fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<(), Error> {
        self.reader.get_ref().set_read_timeout(timeout)?;
        Ok(())
    }

    /// Set write timeout for send operations.
    pub fn set_write_timeout(&mut self, timeout: Option<Duration>) -> Result<(), Error> {
        self.writer.set_write_timeout(timeout)?;
        Ok(())
    }

    /// Split the TCP transport into separate reader and writer halves.
    ///
    /// This allows for concurrent reading and writing without needing mutable
    /// access to the entire transport. Useful for full-duplex communication patterns.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::transport::blocking::tcp::Tcp;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let transport = Tcp::connect("192.168.1.100:5678")?;
    /// let (reader, writer) = transport.split()?;
    ///
    /// // Can now read and write concurrently from different threads
    /// std::thread::spawn(move || {
    ///     // Use writer in one thread
    ///     writer.send(&[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]).unwrap();
    /// });
    ///
    /// // Use reader in another thread
    /// let response = reader.recv()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn split(self) -> Result<(TcpReader, TcpWriter), Error> {
        let writer_stream = self.writer.try_clone().map_err(|e| {
            Error::TransportError(Cow::Owned(format!(
                "Failed to clone TCP stream for split: {}",
                e
            )))
        })?;

        let reader = TcpReader {
            reader: Arc::new(Mutex::new(self.reader)),
            buffer_manager: Arc::new(Mutex::new(self.buffer_manager)),
        };

        let writer = TcpWriter {
            writer: Arc::new(Mutex::new(writer_stream)),
            retry_executor: Arc::new(Mutex::new(self.retry_executor)),
        };

        Ok((reader, writer))
    }
}

impl SyncTransport for Tcp {
    fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        // Clone data for retry closure
        let data_vec = data.to_vec();

        self.retry_executor.execute(|| {
            self.writer.write_all(&data_vec)?;
            self.writer.flush()?;
            Ok(())
        })
    }

    fn recv(&mut self) -> Result<Bytes, Error> {
        // Note: Receiving data is typically not retried as it might lead to
        // duplicate data or protocol confusion. However, we can retry on
        // specific transient errors like temporary network issues.
        let mut buffer = self.buffer_manager.alloc_vec_buffer();

        // Use buffered read_until to find VISCA terminator
        let n = self.reader.read_until(VISCA_TERMINATOR, &mut buffer)?;

        if n == 0 {
            return Err(Error::ConnectionClosed {
                reason: Some(Cow::Borrowed("peer closed connection")),
            });
        }

        Ok(self
            .buffer_manager
            .process_recv_data_borrowed(&mut buffer, n))
    }

    fn recv_with_timeout(&mut self, duration: Duration) -> Result<Bytes, Error> {
        // Save the current timeout
        let original_timeout = self.reader.get_ref().read_timeout()?;

        // Set the new timeout for this operation
        self.reader.get_mut().set_read_timeout(Some(duration))?;

        // Perform the read operation
        let mut buffer = self.buffer_manager.alloc_vec_buffer();
        let result = self.reader.read_until(VISCA_TERMINATOR, &mut buffer);

        // Restore the original timeout
        self.reader.get_mut().set_read_timeout(original_timeout)?;

        // Handle the result
        match result {
            Ok(0) => Err(Error::ConnectionClosed {
                reason: Some(Cow::Borrowed("peer closed connection")),
            }),
            Ok(n) => Ok(self
                .buffer_manager
                .process_recv_data_borrowed(&mut buffer, n)),
            Err(e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                Err(Error::Timeout)
            }
            Err(e) => Err(e.into()),
        }
    }
}

/// Reader half of a split TCP transport.
///
/// This type allows reading from a TCP connection that has been split
/// into separate reader and writer halves.
#[derive(Debug, Clone)]
pub struct TcpReader {
    reader: Arc<Mutex<BufReader<TcpStream>>>,
    buffer_manager: Arc<Mutex<BufferManager>>,
}

impl TcpReader {
    /// Receive data from the TCP connection.
    pub fn recv(&self) -> Result<Bytes, Error> {
        let mut reader = self
            .reader
            .lock()
            .map_err(|_| Error::TransportError(Cow::Borrowed("Reader mutex poisoned")))?;
        let buffer_manager = self
            .buffer_manager
            .lock()
            .map_err(|_| Error::TransportError(Cow::Borrowed("Buffer manager mutex poisoned")))?;
        let mut buffer = buffer_manager.alloc_vec_buffer();

        // Use buffered read_until to find VISCA terminator
        let n = reader.read_until(VISCA_TERMINATOR, &mut buffer)?;

        if n == 0 {
            return Err(Error::ConnectionClosed {
                reason: Some(Cow::Borrowed("peer closed connection")),
            });
        }

        Ok(buffer_manager.process_recv_data_borrowed(&mut buffer, n))
    }

    /// Receive data with a custom timeout.
    pub fn recv_timeout(&self, duration: Duration) -> Result<Bytes, Error> {
        let mut reader = self
            .reader
            .lock()
            .map_err(|_| Error::TransportError(Cow::Borrowed("Reader mutex poisoned")))?;
        let buffer_manager = self
            .buffer_manager
            .lock()
            .map_err(|_| Error::TransportError(Cow::Borrowed("Buffer manager mutex poisoned")))?;

        // Save the current timeout
        let original_timeout = reader.get_ref().read_timeout()?;

        // Set the new timeout for this operation
        reader.get_mut().set_read_timeout(Some(duration))?;

        // Perform the read operation
        let mut buffer = buffer_manager.alloc_vec_buffer();
        let result = reader.read_until(VISCA_TERMINATOR, &mut buffer);

        // Restore the original timeout
        reader.get_mut().set_read_timeout(original_timeout)?;

        // Handle the result
        match result {
            Ok(0) => Err(Error::ConnectionClosed {
                reason: Some(Cow::Borrowed("peer closed connection")),
            }),
            Ok(n) => Ok(buffer_manager.process_recv_data_borrowed(&mut buffer, n)),
            Err(e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                Err(Error::Timeout)
            }
            Err(e) => Err(e.into()),
        }
    }
}

/// Writer half of a split TCP transport.
///
/// This type allows writing to a TCP connection that has been split
/// into separate reader and writer halves.
#[derive(Debug, Clone)]
pub struct TcpWriter {
    writer: Arc<Mutex<TcpStream>>,
    retry_executor: Arc<Mutex<RetryExecutor>>,
}

impl TcpWriter {
    /// Send data over the TCP connection.
    pub fn send(&self, data: &[u8]) -> Result<(), Error> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| Error::TransportError(Cow::Borrowed("Writer mutex poisoned")))?;
        let retry_executor = self
            .retry_executor
            .lock()
            .map_err(|_| Error::TransportError(Cow::Borrowed("Retry executor mutex poisoned")))?;

        // Clone data for retry closure
        let data_vec = data.to_vec();

        retry_executor.execute(|| {
            writer.write_all(&data_vec)?;
            writer.flush()?;
            Ok(())
        })
    }
}
