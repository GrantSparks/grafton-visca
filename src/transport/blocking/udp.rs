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
        config.validate_buffer_bounds()?;
        let canonical_addr = canonicalize_endpoint(address, None)?;
        setup(&canonical_addr, config)
    }

    /// Receive one UDP datagram, retaining truncation information where the
    /// operating system exposes it.
    ///
    /// On Unix, a one-byte sentinel extends `dst`, so a packet that reaches it
    /// is known to exceed the caller's buffer.  On Windows, Winsock reports
    /// `WSAEMSGSIZE` for the same condition.  Other targets conservatively
    /// reject an exact-fill receive because their standard UDP API exposes no
    /// portable way to distinguish it from truncation.
    fn recv_datagram(&self, dst: &mut [u8]) -> io::Result<DatagramReceive> {
        #[cfg(unix)]
        {
            let socket = socket2::SockRef::from(&self.socket);
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
            let socket = socket2::SockRef::from(&self.socket);
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
            let received = self.socket.recv(dst)?;
            return Ok(if received == dst.len() {
                DatagramReceive::Truncated
            } else {
                DatagramReceive::Complete(received)
            });
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
    fn send_with_kind(&mut self, data: &[u8], _kind: CommandKind) -> Result<(), Error> {
        // Send directly - retry logic is handled at the runtime/scheduler level for async
        // For blocking mode, the blocking runner will handle retries
        self.socket.send(data)?;
        Ok(())
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        loop {
            match self.recv_datagram(dst) {
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

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        duration: Duration,
    ) -> Result<usize, Error> {
        // Save the current timeout
        let original_timeout = self.socket.read_timeout()?;
        let deadline = Instant::now()
            .checked_add(duration)
            .unwrap_or_else(Instant::now);

        // Keep one deadline for the whole operation. Empty datagrams must not
        // give the caller a fresh full timeout on every receive attempt.
        let result = loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break Err(Error::Timeout);
            }

            if let Err(error) = self.socket.set_read_timeout(Some(remaining)) {
                break Err(error.into());
            }

            match self.recv_datagram(dst) {
                Ok(DatagramReceive::Complete(0)) => continue,
                Ok(DatagramReceive::Complete(received)) => break Ok(received),
                Ok(DatagramReceive::Truncated) => {
                    break Err(Error::ResponseTooLarge {
                        max_size: dst.len(),
                    });
                }
                Err(error)
                    if error.kind() == io::ErrorKind::TimedOut
                        || error.kind() == io::ErrorKind::WouldBlock =>
                {
                    break Err(Error::Timeout);
                }
                Err(error) => break Err(error.into()),
            }
        };

        // Restore the original timeout
        self.socket.set_read_timeout(original_timeout)?;

        result
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

    fn assert_invalid_buffer_error<T>(result: Result<T, Error>) {
        assert!(matches!(
            result,
            Err(Error::InvalidRequest(actual))
                if actual.as_ref() == "transport receive buffer cannot exceed maximum buffer"
        ));
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

        let received = transport.recv_into(&mut dst).expect("valid datagram");

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
            .recv_into(&mut dst)
            .expect_err("an oversized datagram must not return its valid prefix");
        assert!(matches!(error, Error::ResponseTooLarge { max_size: 3 }));

        // The rejected packet is one atomic datagram; the next exact-fit ACK
        // must remain available and valid.
        sender
            .send(&[0x90, 0x41, 0xff])
            .expect("send exact-fit datagram");
        let received = transport
            .recv_into(&mut dst)
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
