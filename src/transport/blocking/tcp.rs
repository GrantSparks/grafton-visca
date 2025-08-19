//! Blocking TCP transport implementation with DNS resolution and IPv6 support.

use bytes::Bytes;

use std::borrow::Cow;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::transport::address::AddressResolver;
use crate::transport::buffer::BufferManager;
use crate::transport::retry::RetryExecutor;
use crate::transport::{BlockingTransport, RetryConfig};
use crate::Error;

/// TCP transport for blocking VISCA communication.
///
/// This transport supports DNS resolution and both IPv4 and IPv6 addresses.
#[derive(Debug)]
pub struct Tcp {
    reader: Mutex<BufReader<TcpStream>>,
    writer: Mutex<TcpStream>,
    retry_executor: RetryExecutor,
    buffer_manager: BufferManager,
    // Store the stream for socket configuration
    stream: Option<TcpStream>,
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
        let deadline = Instant::now() + timeout;

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
                    // Set socket options
                    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
                    stream.set_nodelay(true)?;

                    // Clone the stream for separate reader and writer
                    let reader_stream = stream.try_clone()?;
                    let config_stream = stream.try_clone()?;

                    return Ok(Self {
                        reader: Mutex::new(BufReader::new(reader_stream)),
                        writer: Mutex::new(stream),
                        retry_executor: RetryExecutor::with_defaults(),
                        buffer_manager: BufferManager::with_defaults(),
                        stream: Some(config_stream),
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
        if let Some(ref stream) = self.stream {
            stream.set_nodelay(nodelay)?;
        }
        Ok(())
    }

    /// Set TTL (Time To Live) for packets.
    pub fn set_ttl(&mut self, ttl: u32) -> Result<(), Error> {
        if let Some(ref stream) = self.stream {
            stream.set_ttl(ttl)?;
        }
        Ok(())
    }
}

impl BlockingTransport for Tcp {
    fn send_blocking(&self, data: &[u8]) -> Result<(), Error> {
        // Clone data for retry closure
        let data_vec = data.to_vec();

        self.retry_executor.execute(|| {
            let mut writer = self
                .writer
                .lock()
                .map_err(|_| Error::LockPoisoned("writer"))?;
            writer.write_all(&data_vec)?;
            writer.flush()?;
            Ok(())
        })
    }

    fn recv_blocking(&self) -> Result<Bytes, Error> {
        // Note: Receiving data is typically not retried as it might lead to
        // duplicate data or protocol confusion. However, we can retry on
        // specific transient errors like temporary network issues.
        let mut reader = self
            .reader
            .lock()
            .map_err(|_| Error::LockPoisoned("reader"))?;
        let mut buffer = self.buffer_manager.alloc_vec_buffer();

        // Use buffered read_until to find VISCA terminator
        let n = reader.read_until(0xFF, &mut buffer)?;

        if n == 0 {
            return Err(Error::ConnectionLost {
                reason: Cow::Borrowed("peer closed connection"),
            });
        }

        Ok(self.buffer_manager.process_recv_data(&mut buffer, n))
    }

    fn recv_blocking_with_timeout(&self, duration: Duration) -> Result<Bytes, Error> {
        // Get the reader
        let mut reader = self
            .reader
            .lock()
            .map_err(|_| Error::LockPoisoned("reader"))?;

        // Save the current timeout
        let original_timeout = reader.get_ref().read_timeout()?;

        // Set the new timeout for this operation
        reader.get_mut().set_read_timeout(Some(duration))?;

        // Perform the read operation
        let mut buffer = self.buffer_manager.alloc_vec_buffer();
        let result = reader.read_until(0xFF, &mut buffer);

        // Restore the original timeout
        reader.get_mut().set_read_timeout(original_timeout)?;

        // Handle the result
        match result {
            Ok(0) => Err(Error::ConnectionLost {
                reason: Cow::Borrowed("peer closed connection"),
            }),
            Ok(n) => Ok(self.buffer_manager.process_recv_data(&mut buffer, n)),
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
