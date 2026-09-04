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
        blocking::TimeoutRestoreGuard,
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

fn tcp_write_timeout(stream: &TcpStream) -> io::Result<Option<Duration>> {
    stream.write_timeout()
}

fn set_tcp_write_timeout(stream: &mut TcpStream, timeout: Option<Duration>) -> io::Result<()> {
    stream.set_write_timeout(timeout)
}

fn tcp_read_timeout(reader: &BufReader<TcpStream>) -> io::Result<Option<Duration>> {
    reader.get_ref().read_timeout()
}

fn set_tcp_read_timeout(
    reader: &mut BufReader<TcpStream>,
    timeout: Option<Duration>,
) -> io::Result<()> {
    reader.get_mut().set_read_timeout(timeout)
}

fn send_all_with_timeout<S>(
    socket: &mut S,
    data: &[u8],
    timeout: Duration,
    timeout_for: fn(&S) -> io::Result<Option<Duration>>,
    set_timeout: fn(&mut S, Option<Duration>) -> io::Result<()>,
) -> Result<(), Error>
where
    S: Write,
{
    let original_timeout = timeout_for(socket)?;
    let started = Instant::now();
    let mut written = 0;
    let mut timeout_guard =
        TimeoutRestoreGuard::new(socket, original_timeout, set_timeout, "TCP write");

    loop {
        if written == data.len() {
            return Ok(());
        }

        // `write_all` may grant every partial write a fresh socket timeout.
        // Re-sample one fixed operation budget before each syscall instead.
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(Error::Timeout);
        }
        if let Err(error) = timeout_guard.set_timeout(Some(remaining)) {
            return Err(error.into());
        }

        match timeout_guard.socket_mut().write(&data[written..]) {
            Ok(0) => {
                return Err(
                    io::Error::new(io::ErrorKind::WriteZero, "TCP write made no progress").into(),
                );
            }
            Ok(count) => written += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if configured_write_deadline_expired(&error) => {
                return Err(Error::Timeout);
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn recv_with_timeout<S>(
    socket: &mut S,
    dst: &mut [u8],
    duration: Duration,
    timeout_for: fn(&S) -> io::Result<Option<Duration>>,
    set_timeout: fn(&mut S, Option<Duration>) -> io::Result<()>,
) -> Result<usize, Error>
where
    S: Read,
{
    let original_timeout = timeout_for(socket)?;
    let mut timeout_guard =
        TimeoutRestoreGuard::new(socket, original_timeout, set_timeout, "TCP read");
    timeout_guard.set_timeout(Some(duration))?;

    match timeout_guard.socket_mut().read(dst) {
        Ok(0) => Err(Error::ConnectionClosed {
            reason: Some("peer closed connection".into()),
        }),
        Ok(received) => Ok(received),
        Err(error) if configured_read_deadline_expired(&error) => Err(Error::Timeout),
        Err(error) => Err(error.into()),
    }
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
        send_all_with_timeout(
            &mut self.writer,
            data,
            timeout,
            tcp_write_timeout,
            set_tcp_write_timeout,
        )
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        duration: Duration,
    ) -> Result<usize, Error> {
        recv_with_timeout(
            &mut self.reader,
            dst,
            duration,
            tcp_read_timeout,
            set_tcp_read_timeout,
        )
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(AddressingMode::Ip)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::{
        io::{self, Cursor, ErrorKind, Read, Write},
        net::TcpListener,
        thread,
        time::{Duration, Instant},
    };

    use super::*;
    use crate::transport::BufferConfig;

    #[derive(Debug)]
    struct RestoreFailSocket {
        original_timeout: Option<Duration>,
        timeout: Option<Duration>,
        written: Vec<u8>,
        readable: Cursor<Vec<u8>>,
        restore_attempted: bool,
    }

    impl RestoreFailSocket {
        fn new(original_timeout: Option<Duration>, readable: impl Into<Vec<u8>>) -> Self {
            Self {
                original_timeout,
                timeout: original_timeout,
                written: Vec::new(),
                readable: Cursor::new(readable.into()),
                restore_attempted: false,
            }
        }

        fn timeout(&self) -> io::Result<Option<Duration>> {
            Ok(self.timeout)
        }

        fn set_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
            if timeout == self.original_timeout && self.timeout != self.original_timeout {
                self.restore_attempted = true;
                return Err(io::Error::new(
                    ErrorKind::InvalidInput,
                    "injected timeout restore failure",
                ));
            }
            self.timeout = timeout;
            Ok(())
        }
    }

    impl Write for RestoreFailSocket {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.written.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Read for RestoreFailSocket {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.readable.read(buf)
        }
    }

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

    #[test]
    fn completed_send_survives_a_failing_timeout_restore() {
        let original_timeout = Some(Duration::from_secs(1));
        let mut socket = RestoreFailSocket::new(original_timeout, []);

        send_all_with_timeout(
            &mut socket,
            b"VISCA",
            Duration::from_millis(10),
            RestoreFailSocket::timeout,
            RestoreFailSocket::set_timeout,
        )
        .expect("the fully written stream frame must survive cleanup failure");

        assert_eq!(socket.written, b"VISCA");
        assert!(socket.restore_attempted);
    }

    #[test]
    fn completed_read_survives_a_failing_timeout_restore() {
        let original_timeout = Some(Duration::from_secs(1));
        let mut socket = RestoreFailSocket::new(original_timeout, b"reply");
        let mut dst = [0_u8; 8];

        let received = recv_with_timeout(
            &mut socket,
            &mut dst,
            Duration::from_millis(10),
            RestoreFailSocket::timeout,
            RestoreFailSocket::set_timeout,
        )
        .expect("the copied stream bytes must survive cleanup failure");

        assert_eq!(received, 5);
        assert_eq!(&dst[..received], b"reply");
        assert!(socket.restore_attempted);
    }

    fn invalid_buffer_config() -> TransportConfig {
        TransportConfig {
            buffer_config: BufferConfig {
                recv_buffer_size: 65,
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
