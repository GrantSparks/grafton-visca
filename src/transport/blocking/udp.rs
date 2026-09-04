//! Blocking UDP transport implementation with IPv6 support.

use std::{
    io,
    net::UdpSocket,
    time::{Duration, Instant},
};

#[cfg(unix)]
use std::io::IoSliceMut;
#[cfg(any(unix, windows))]
use std::io::Read;

use crate::{
    command::CommandKind,
    transport::{
        address::{canonicalize_endpoint, AddressResolver},
        blocking::TimeoutRestoreGuard,
        buffer::BufferConfig,
        builder::{AddressingMode, TransportConfig},
        BlockingTransport, HasTransportConfig, SendSemantics,
    },
    Error,
};

/// UDP transport for blocking VISCA communication.
///
/// This transport supports DNS resolution and both IPv4 and IPv6 addresses.
/// It operates at the datagram level, treating each datagram as a frame.
#[derive(Debug)]
pub struct Udp {
    socket: UdpSocket,
    config: TransportConfig,
}

/// One UDP receive together with whether the caller's buffer held the entire
/// datagram.
///
/// A valid-looking prefix is not a valid VISCA response.  Keep this detail at
/// the transport boundary so neither the public `BlockingTransport` trait nor
/// the owner has to guess whether an exact-fill UDP read was truncated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DatagramReceive {
    Complete(usize),
    Truncated,
}

impl Udp {
    /// Connect to a UDP endpoint.
    ///
    /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
    /// The socket will bind to the appropriate unspecified address based on the
    /// target address family. The address must include an explicit port; explicit
    /// IPv6 ports require brackets.
    pub fn connect(address: &str) -> Result<Self, Error> {
        let config = TransportConfig {
            read_timeout: Duration::from_secs(5),
            write_timeout: Duration::from_secs(5),
            buffer_config: BufferConfig::for_udp(),
            addressing: AddressingMode::Ip, // UDP is always IP mode
            ..Default::default()
        };
        Self::connect_with_config(address, config)
    }

    /// Connect with a full configuration.
    ///
    /// This method provides full control over connection and socket parameters.
    /// The address must include an explicit port.
    pub fn connect_with_config(address: &str, config: TransportConfig) -> Result<Self, Error> {
        let socket = Self::preflight_udp_setup(address, config, |canonical_addr, config| {
            // Use the common address resolver.
            let resolver = AddressResolver::new();
            let target_addr = resolver.resolve_first(canonical_addr)?;

            // Bind to the appropriate unspecified address based on target family.
            let bind_addr = resolver.bind_address_for(&target_addr);

            let socket = UdpSocket::bind(bind_addr)?;
            socket.connect(target_addr)?;

            // Apply socket options from config.
            socket.set_read_timeout(Some(config.read_timeout))?;
            socket.set_write_timeout(Some(config.write_timeout))?;
            if let Some(ttl) = config.ttl {
                socket.set_ttl(ttl)?;
            }

            Ok(socket)
        })?;

        Ok(Self { socket, config })
    }

    /// Run endpoint parsing and UDP setup only after buffer preflight succeeds.
    fn preflight_udp_setup<T>(
        address: &str,
        config: TransportConfig,
        setup: impl FnOnce(&str, TransportConfig) -> Result<T, Error>,
    ) -> Result<T, Error> {
        config.validate()?;
        let canonical_addr = canonicalize_endpoint(address, None)?;
        setup(&canonical_addr, config)
    }
}

/// Receive one UDP datagram, retaining truncation information where the
/// operating system exposes it.
///
/// On Unix, a one-byte sentinel extends `dst`, so a packet that reaches it is
/// known to exceed the caller's buffer.  On Windows, Winsock reports
/// `WSAEMSGSIZE` for the same condition.  Other targets conservatively reject
/// an exact-fill receive because their standard UDP API exposes no portable way
/// to distinguish it from truncation.
fn recv_datagram(socket: &UdpSocket, dst: &mut [u8]) -> io::Result<DatagramReceive> {
    #[cfg(unix)]
    {
        let socket = socket2::SockRef::from(socket);
        let capacity = dst.len();
        let mut sentinel = [0_u8; 1];
        let mut buffers = [IoSliceMut::new(dst), IoSliceMut::new(&mut sentinel)];
        let mut socket = &*socket;
        let received = socket.read_vectored(&mut buffers)?;
        Ok(if received > capacity {
            DatagramReceive::Truncated
        } else {
            DatagramReceive::Complete(received)
        })
    }

    #[cfg(windows)]
    {
        let socket = socket2::SockRef::from(socket);
        let mut socket = &*socket;
        // `Socket2` preserves WSAEMSGSIZE from `recv`, unlike its vectored
        // compatibility adapter.  The datagram has already been consumed,
        // so report it as a discarded oversized packet.
        const WSAEMSGSIZE: i32 = 10_040;
        return match socket.read(dst) {
            Ok(received) => Ok(DatagramReceive::Complete(received)),
            Err(error) if error.raw_os_error() == Some(WSAEMSGSIZE) => {
                Ok(DatagramReceive::Truncated)
            }
            Err(error) => Err(error),
        };
    }

    #[cfg(not(any(unix, windows)))]
    {
        let received = socket.recv(dst)?;
        return Ok(if received == dst.len() {
            DatagramReceive::Truncated
        } else {
            DatagramReceive::Complete(received)
        });
    }
}

fn udp_read_timeout(socket: &UdpSocket) -> io::Result<Option<Duration>> {
    socket.read_timeout()
}

fn set_udp_read_timeout(socket: &mut UdpSocket, timeout: Option<Duration>) -> io::Result<()> {
    socket.set_read_timeout(timeout)
}

fn udp_write_timeout(socket: &UdpSocket) -> io::Result<Option<Duration>> {
    socket.write_timeout()
}

fn set_udp_write_timeout(socket: &mut UdpSocket, timeout: Option<Duration>) -> io::Result<()> {
    socket.set_write_timeout(timeout)
}

fn send_with_timeout<S>(
    socket: &mut S,
    data: &[u8],
    timeout: Duration,
    timeout_for: fn(&S) -> io::Result<Option<Duration>>,
    set_timeout: fn(&mut S, Option<Duration>) -> io::Result<()>,
    send: impl FnOnce(&mut S, &[u8]) -> io::Result<usize>,
) -> Result<(), Error> {
    let original_timeout = timeout_for(socket)?;
    let mut timeout_guard =
        TimeoutRestoreGuard::new(socket, original_timeout, set_timeout, "UDP write");
    timeout_guard.set_timeout(Some(timeout))?;
    send(timeout_guard.socket_mut(), data)
        .map(|_| ())
        .map_err(|error| {
            if matches!(
                error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            ) {
                Error::Timeout
            } else {
                error.into()
            }
        })
}

fn recv_with_timeout<S>(
    socket: &mut S,
    dst: &mut [u8],
    duration: Duration,
    timeout_for: fn(&S) -> io::Result<Option<Duration>>,
    set_timeout: fn(&mut S, Option<Duration>) -> io::Result<()>,
    mut receive: impl FnMut(&mut S, &mut [u8]) -> io::Result<DatagramReceive>,
) -> Result<usize, Error> {
    let original_timeout = timeout_for(socket)?;
    let deadline = Instant::now()
        .checked_add(duration)
        .ok_or_else(|| Error::InvalidParameter {
            parameter: "read timeout",
            value: format!("{duration:?}").into(),
            reason: "duration exceeds the monotonic clock range".into(),
        })?;
    let mut timeout_guard =
        TimeoutRestoreGuard::new(socket, original_timeout, set_timeout, "UDP read");

    // Keep one deadline for the whole operation. Empty datagrams must not give
    // the caller a fresh full timeout on every receive attempt.
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(Error::Timeout);
        }

        if let Err(error) = timeout_guard.set_timeout(Some(remaining)) {
            return Err(error.into());
        }

        match receive(timeout_guard.socket_mut(), dst) {
            Ok(DatagramReceive::Complete(0)) => continue,
            Ok(DatagramReceive::Complete(received)) => return Ok(received),
            Ok(DatagramReceive::Truncated) => {
                return Err(Error::ResponseTooLarge {
                    max_size: dst.len(),
                });
            }
            Err(error)
                if error.kind() == io::ErrorKind::TimedOut
                    || error.kind() == io::ErrorKind::WouldBlock =>
            {
                return Err(Error::Timeout);
            }
            Err(error) => return Err(error.into()),
        }
    }
}

impl HasTransportConfig for Udp {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }

    fn standard_transport_kind(&self) -> Option<crate::camera::TransportKind> {
        Some(crate::camera::TransportKind::Udp)
    }
}

impl BlockingTransport for Udp {
    fn send_with_timeout(
        &mut self,
        data: &[u8],
        _kind: CommandKind,
        timeout: Duration,
    ) -> Result<(), Error> {
        send_with_timeout(
            &mut self.socket,
            data,
            timeout,
            udp_write_timeout,
            set_udp_write_timeout,
            |socket, data| socket.send(data),
        )
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        duration: Duration,
    ) -> Result<usize, Error> {
        recv_with_timeout(
            &mut self.socket,
            dst,
            duration,
            udp_read_timeout,
            set_udp_read_timeout,
            |socket, dst| recv_datagram(socket, dst),
        )
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(AddressingMode::Ip)
    }

    fn send_semantics(&self) -> SendSemantics {
        // UDP sends are atomic at the datagram boundary - a failed send
        // does not affect the state for subsequent sends
        SendSemantics::Datagram
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::{
        net::UdpSocket,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        thread,
        time::{Duration, Instant},
    };

    use super::*;
    use crate::transport::{BlockingTransport, BufferConfig, TransportConfig};

    #[derive(Debug)]
    struct RestoreFailSocket {
        original_timeout: Option<Duration>,
        timeout: Option<Duration>,
        sent: Vec<u8>,
        response: Vec<u8>,
        restore_attempted: bool,
    }

    impl RestoreFailSocket {
        fn new(original_timeout: Option<Duration>, response: impl Into<Vec<u8>>) -> Self {
            Self {
                original_timeout,
                timeout: original_timeout,
                sent: Vec::new(),
                response: response.into(),
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
                    io::ErrorKind::InvalidInput,
                    "injected timeout restore failure",
                ));
            }
            self.timeout = timeout;
            Ok(())
        }

        fn send(&mut self, data: &[u8]) -> io::Result<usize> {
            self.sent.extend_from_slice(data);
            Ok(data.len())
        }

        fn receive(&mut self, dst: &mut [u8]) -> io::Result<DatagramReceive> {
            dst[..self.response.len()].copy_from_slice(&self.response);
            Ok(DatagramReceive::Complete(self.response.len()))
        }
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

    fn assert_invalid_buffer_error<T>(result: Result<T, Error>) {
        assert!(matches!(
            result,
            Err(Error::InvalidRequest(actual))
                if actual.as_ref() == "transport receive buffer cannot exceed maximum buffer"
        ));
    }

    #[test]
    fn completed_send_survives_a_failing_timeout_restore() {
        let original_timeout = Some(Duration::from_secs(1));
        let mut socket = RestoreFailSocket::new(original_timeout, []);

        send_with_timeout(
            &mut socket,
            b"VISCA",
            Duration::from_millis(10),
            RestoreFailSocket::timeout,
            RestoreFailSocket::set_timeout,
            RestoreFailSocket::send,
        )
        .expect("the sent datagram must survive cleanup failure");

        assert_eq!(socket.sent, b"VISCA");
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
            RestoreFailSocket::receive,
        )
        .expect("the copied datagram must survive cleanup failure");

        assert_eq!(received, 5);
        assert_eq!(&dst[..received], b"reply");
        assert!(socket.restore_attempted);
    }

    fn connected_socket_pair() -> (UdpSocket, UdpSocket) {
        let receiver = UdpSocket::bind("127.0.0.1:0").expect("bind receiver");
        let sender = UdpSocket::bind("127.0.0.1:0").expect("bind sender");
        receiver
            .connect(sender.local_addr().expect("sender address"))
            .expect("connect receiver");
        sender
            .connect(receiver.local_addr().expect("receiver address"))
            .expect("connect sender");
        (receiver, sender)
    }

    #[test]
    fn invalid_config_is_rejected_before_udp_socket_setup() {
        // This is the real production setup seam: it includes resolver,
        // bind, connect, and socket options. It must not run for invalid
        // buffers, independent of network/DNS behavior.
        let setup_calls = Arc::new(AtomicUsize::new(0));
        let spy_calls = Arc::clone(&setup_calls);
        let result =
            Udp::preflight_udp_setup("127.0.0.1:9", invalid_buffer_config(), move |_, _| {
                spy_calls.fetch_add(1, Ordering::SeqCst);
                Ok(())
            });

        assert_invalid_buffer_error(result);
        assert_eq!(
            setup_calls.load(Ordering::SeqCst),
            0,
            "invalid buffer configuration must not reach UDP resolver/socket setup"
        );

        // The malformed endpoint makes the buffer error's precedence over
        // address parsing explicit.
        assert_invalid_buffer_error(Udp::preflight_udp_setup(
            "[::1",
            invalid_buffer_config(),
            |_, _| Ok(()),
        ));
    }

    #[test]
    fn recv_discards_empty_datagram_before_valid_datagram() {
        let (receiver, sender) = connected_socket_pair();
        sender.send(&[]).expect("send empty datagram");
        sender.send(b"valid").expect("send valid datagram");

        let mut transport = Udp {
            socket: receiver,
            config: TransportConfig::default(),
        };
        let mut dst = [0; 16];

        let received = transport
            .recv_into_with_timeout(&mut dst, Duration::from_secs(1))
            .expect("valid datagram");

        assert_eq!(received, 5);
        assert_eq!(&dst[..received], b"valid");
    }

    #[test]
    fn recv_rejects_truncated_valid_ack_prefix_and_preserves_next_datagram() {
        let (receiver, sender) = connected_socket_pair();
        // The first three bytes form a valid VISCA ACK.  The trailing byte
        // makes the actual UDP datagram over-size for `dst`, so decoding that
        // copied prefix would incorrectly acknowledge a request.
        sender
            .send(&[0x90, 0x41, 0xff, 0x00])
            .expect("send oversized datagram");

        let mut transport = Udp {
            socket: receiver,
            config: TransportConfig::default(),
        };
        let mut dst = [0; 3];
        let error = transport
            .recv_into_with_timeout(&mut dst, Duration::from_secs(1))
            .expect_err("an oversized datagram must not return its valid prefix");
        assert!(matches!(error, Error::ResponseTooLarge { max_size: 3 }));

        // The rejected packet is one atomic datagram; the next exact-fit ACK
        // must remain available and valid.
        sender
            .send(&[0x90, 0x41, 0xff])
            .expect("send exact-fit datagram");
        let received = transport
            .recv_into_with_timeout(&mut dst, Duration::from_secs(1))
            .expect("next exact-fit datagram remains readable");
        assert_eq!(received, 3);
        assert_eq!(&dst[..received], &[0x90, 0x41, 0xff]);
    }

    #[test]
    fn recv_into_with_timeout_expires_after_empty_datagram() {
        let (receiver, sender) = connected_socket_pair();
        sender.send(&[]).expect("send empty datagram");

        let mut transport = Udp {
            socket: receiver,
            config: TransportConfig::default(),
        };
        let mut dst = [0; 16];
        let timeout = Duration::from_millis(80);
        let started = Instant::now();

        let result = transport.recv_into_with_timeout(&mut dst, timeout);

        assert!(matches!(result, Err(Error::Timeout)));
        assert!(started.elapsed() >= timeout.saturating_sub(Duration::from_millis(20)));
    }

    #[test]
    fn recv_rejects_an_unrepresentable_operation_deadline() {
        let (receiver, _sender) = connected_socket_pair();
        let original_timeout = receiver.read_timeout().expect("read timeout");
        let mut transport = Udp {
            socket: receiver,
            config: TransportConfig::default(),
        };
        let mut dst = [0; 16];

        let error = transport
            .recv_into_with_timeout(&mut dst, Duration::MAX)
            .expect_err("an unrepresentable deadline must fail before reading");

        assert!(matches!(
            error,
            Error::InvalidParameter {
                parameter: "read timeout",
                ..
            }
        ));
        assert_eq!(
            transport.socket.read_timeout().expect("read timeout"),
            original_timeout
        );
    }

    #[test]
    fn recv_into_with_timeout_does_not_restart_after_empty_datagram() {
        let (receiver, sender) = connected_socket_pair();
        let receiver_address = receiver.local_addr().expect("receiver address");
        let sender = thread::spawn(move || {
            thread::sleep(Duration::from_millis(40));
            sender
                .send_to(&[], receiver_address)
                .expect("send empty datagram");
            thread::sleep(Duration::from_millis(60));
            sender
                .send_to(b"too late", receiver_address)
                .expect("send valid datagram");
        });

        let mut transport = Udp {
            socket: receiver,
            config: TransportConfig::default(),
        };
        let mut dst = [0; 16];
        let result = transport.recv_into_with_timeout(&mut dst, Duration::from_millis(70));

        sender.join().expect("sender thread");
        assert!(matches!(result, Err(Error::Timeout)));
    }
}
