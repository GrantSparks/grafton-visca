//! Blocking TCP transport implementation with DNS resolution and IPv6 support.

use std::{
    io::{BufReader, Read, Write},
    net::TcpStream,
    time::{Duration, Instant},
};

use crate::{
    command::CommandKind,
    timeout::Deadline,
    transport::{
        address::{canonicalize_endpoint, AddressResolver},
        builder::{AddressingMode, TransportConfig},
        socket_options::apply_tcp_socket_options,
        BlockingTransport, HasTransportConfig,
    },
    Error,
};

/// TCP transport for blocking VISCA communication.
///
/// This transport supports DNS resolution and both IPv4 and IPv6 addresses.
/// It operates at the stream level, reading/writing raw bytes.
/// Framing is handled by the runtime layer.
pub struct Tcp {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    config: TransportConfig,
}

impl std::fmt::Debug for Tcp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tcp")
            .field("reader", &self.reader)
            .field("writer", &self.writer)
            .finish()
    }
}

impl Tcp {
    /// Connect to a TCP endpoint.
    ///
    /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
    /// The address must include an explicit port; explicit IPv6 ports require
    /// brackets.
    pub fn connect(address: &str) -> Result<Self, Error> {
        Self::connect_timeout(address, Duration::from_secs(5))
    }

    /// Connect with a custom timeout.
    ///
    /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
    /// It will try each resolved address in order until one succeeds or the
    /// overall timeout is reached. The address must include an explicit port.
    pub fn connect_timeout(address: &str, timeout: Duration) -> Result<Self, Error> {
        let config = TransportConfig {
            connect_timeout: timeout,
            read_timeout: Duration::from_secs(5),
            write_timeout: Duration::from_secs(5),
            addressing: AddressingMode::Ip, // TCP is always IP mode
            tcp_nodelay: Some(true),
            ..Default::default()
        };
        Self::connect_with_config(address, config)
    }

    /// Connect with a full configuration.
    ///
    /// This method provides full control over connection and socket parameters.
    /// The address must include an explicit port.
    pub fn connect_with_config(address: &str, config: TransportConfig) -> Result<Self, Error> {
        config.validate_buffer_bounds()?;
        let canonical_addr = canonicalize_endpoint(address, None)?;
        let deadline = Deadline::from_timeout(config.connect_timeout)?;

        // Use the common address resolver
        let resolver = AddressResolver::new();
        let addrs = resolver.resolve(&canonical_addr)?;

        let mut last_error = None;

        // Try each address with remaining time
        for addr in addrs {
            let remaining = deadline.remaining_at(Instant::now());
            if remaining.is_zero() {
                break;
            }

            match TcpStream::connect_timeout(&addr, remaining) {
                Ok(stream) => {
                    // Apply socket options from config
                    stream.set_read_timeout(Some(config.read_timeout))?;
                    stream.set_write_timeout(Some(config.write_timeout))?;
                    apply_tcp_socket_options(&stream, config.into())?;

                    // Clone the stream for separate reader and writer
                    let reader_stream = stream.try_clone()?;

                    return Ok(Self {
                        reader: BufReader::new(reader_stream),
                        writer: stream,
                        config,
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

impl HasTransportConfig for Tcp {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }

    fn standard_transport_kind(&self) -> Option<crate::camera::TransportKind> {
        Some(crate::camera::TransportKind::Tcp)
    }
}

impl BlockingTransport for Tcp {
    fn send_with_kind(&mut self, data: &[u8], _kind: CommandKind) -> Result<(), Error> {
        // Send directly - retry logic is handled at the runtime/scheduler level
        self.writer.write_all(data)?;
        self.writer.flush()?;
        Ok(())
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        // Read directly into the provided buffer
        match self.reader.read(dst) {
            Ok(0) => {
                // Connection closed
                Err(Error::ConnectionClosed {
                    reason: Some("peer closed connection".into()),
                })
            }
            Ok(n) => Ok(n),
            Err(e) => Err(e.into()),
        }
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        duration: Duration,
    ) -> Result<usize, Error> {
        // Save the current timeout
        let original_timeout = self.reader.get_ref().read_timeout()?;

        // Set the new timeout for this operation
        self.reader.get_mut().set_read_timeout(Some(duration))?;

        // Read into the provided buffer
        let result = match self.reader.read(dst) {
            Ok(0) => {
                // Connection closed
                Err(Error::ConnectionClosed {
                    reason: Some("peer closed connection".into()),
                })
            }
            Ok(n) => Ok(n),
            Err(io_err)
                if io_err.kind() == std::io::ErrorKind::TimedOut
                    || io_err.kind() == std::io::ErrorKind::WouldBlock =>
            {
                Err(Error::Timeout)
            }
            Err(io_err) => Err(io_err.into()),
        };

        // Restore the original timeout
        self.reader.get_mut().set_read_timeout(original_timeout)?;

        result
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(AddressingMode::Ip)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::{
        io::ErrorKind,
        net::TcpListener,
        thread,
        time::{Duration, Instant},
    };

    use super::*;
    use crate::transport::BufferConfig;

    fn invalid_buffer_config() -> TransportConfig {
        TransportConfig {
            buffer_config: BufferConfig {
                recv_buffer_size: 65,
                send_buffer_size: 64,
                max_buffer_size: 64,
            },
            ..TransportConfig::default()
        }
    }

    #[test]
    #[cfg_attr(miri, ignore = "requires a real TCP listener")]
    fn invalid_config_returns_before_a_listener_can_accept() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        listener
            .set_nonblocking(true)
            .expect("make listener nonblocking");
        let address = listener.local_addr().expect("listener address");

        let accept_thread = thread::spawn(move || -> Result<bool, std::io::Error> {
            let deadline = Instant::now() + Duration::from_millis(100);
            loop {
                match listener.accept() {
                    Ok(_) => return Ok(true),
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return Ok(false);
                        }
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => return Err(error),
                }
            }
        });

        let result = Tcp::connect_with_config(&address.to_string(), invalid_buffer_config());
        let accepted = accept_thread
            .join()
            .expect("join accept observer")
            .expect("listener failed while observing connector");

        assert!(matches!(
            result,
            Err(Error::InvalidRequest(actual))
                if actual.as_ref() == "transport receive buffer cannot exceed maximum buffer"
        ));
        assert!(
            !accepted,
            "invalid configuration must be rejected before TCP connect"
        );
    }
}
