//! Blocking TCP transport implementation with DNS resolution and IPv6 support.

use crate::transport::BlockingTransport;
use crate::Error;
use bytes::Bytes;
use std::borrow::Cow;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// TCP transport for blocking VISCA communication.
///
/// This transport supports DNS resolution and both IPv4 and IPv6 addresses.
#[derive(Debug)]
pub struct Tcp {
    reader: Mutex<BufReader<TcpStream>>,
    writer: Mutex<TcpStream>,
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

        // Resolve the address (supports DNS and IPv6)
        let addrs: Vec<SocketAddr> = address
            .to_socket_addrs()
            .map_err(|e| Error::InvalidAddress {
                reason: format!("Failed to resolve '{}': {}", address, e).into(),
            })?
            .collect();

        if addrs.is_empty() {
            return Err(Error::InvalidAddress {
                reason: format!("No addresses resolved for '{}'", address).into(),
            });
        }

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

                    return Ok(Self {
                        reader: Mutex::new(BufReader::new(reader_stream)),
                        writer: Mutex::new(stream),
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
}

impl BlockingTransport for Tcp {
    fn send_blocking(&self, data: &[u8]) -> Result<(), Error> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| Error::LockPoisoned("writer"))?;
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }

    fn recv_blocking(&self) -> Result<Bytes, Error> {
        let mut reader = self
            .reader
            .lock()
            .map_err(|_| Error::LockPoisoned("reader"))?;
        let mut buffer = Vec::with_capacity(64);

        // Use buffered read_until to find VISCA terminator
        let n = reader.read_until(0xFF, &mut buffer)?;

        if n == 0 {
            return Err(Error::ConnectionLost {
                reason: Cow::Borrowed("peer closed connection"),
            });
        }

        Ok(Bytes::from(buffer))
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
        let mut buffer = Vec::with_capacity(64);
        let result = reader.read_until(0xFF, &mut buffer);

        // Restore the original timeout
        reader.get_mut().set_read_timeout(original_timeout)?;

        // Handle the result
        match result {
            Ok(0) => Err(Error::ConnectionLost {
                reason: Cow::Borrowed("peer closed connection"),
            }),
            Ok(_) => Ok(Bytes::from(buffer)),
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
