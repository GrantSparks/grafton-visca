//! Blocking TCP transport implementation with DNS resolution and IPv6 support.

use std::{
    io::{self, BufReader, Read, Write},
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

/// Whether this error is the timeout installed for one bounded read.
///
/// Unix reports an expired `SO_RCVTIMEO` as `EAGAIN` / `WouldBlock`, while
/// `ETIMEDOUT` can instead be the connection-level result of exhausted TCP
/// keepalives. Preserve that distinction so the owner can end a dead session
/// promptly (#719). Other platforms may use `TimedOut` for the configured read
/// deadline itself, so retain the portable historical mapping there.
fn configured_read_deadline_expired(error: &io::Error) -> bool {
    match error.kind() {
        io::ErrorKind::WouldBlock => true,
        io::ErrorKind::TimedOut => cfg!(not(unix)),
        _ => false,
    }
}

/// Whether one write failed because the scoped per-call timeout elapsed.
fn configured_write_deadline_expired(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    )
}

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
        config.validate()?;
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
    fn send_with_timeout(
        &mut self,
        data: &[u8],
        _kind: CommandKind,
        timeout: Duration,
    ) -> Result<(), Error> {
        let original_timeout = self.writer.write_timeout()?;
        let started = Instant::now();
        let mut written = 0;
        let result = loop {
            if written == data.len() {
                break Ok(());
            }

            // `write_all` may grant every partial write a fresh socket timeout.
            // Re-sample one fixed operation budget before each syscall instead.
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                break Err(Error::Timeout);
            }
            if let Err(error) = self.writer.set_write_timeout(Some(remaining)) {
                break Err(error.into());
            }

            match self.writer.write(&data[written..]) {
                Ok(0) => {
                    break Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "TCP write made no progress",
                    )
                    .into());
                }
                Ok(count) => written += count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if configured_write_deadline_expired(&error) => {
                    break Err(Error::Timeout);
                }
                Err(error) => break Err(error.into()),
            }
        };
        self.writer.set_write_timeout(original_timeout)?;
        result
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
            Err(io_err) if configured_read_deadline_expired(&io_err) => Err(Error::Timeout),
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

    #[test]
    fn configured_read_deadline_keeps_unix_connection_timeout_distinct() {
        assert!(configured_read_deadline_expired(&io::Error::from(
            ErrorKind::WouldBlock
        )));
        assert_eq!(
            configured_read_deadline_expired(&io::Error::from(ErrorKind::TimedOut)),
            cfg!(not(unix)),
            "Unix ETIMEDOUT must reach the owner as keepalive/session failure"
        );
    }

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

        let accept_thread = thread::spawn(move || -> Result<bool, io::Error> {
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
