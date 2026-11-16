//! Blocking UDP transport implementation with IPv6 support.

use std::{
    net::UdpSocket,
    time::{Duration, Instant},
};

use crate::{
    timeout::Deadline,
    transport::{
        address::{bind_address_for, resolve_blocking},
        blocking::{Arm, TimedIo},
        builder::TransportConfig,
        connect::{connect_deadline, preflight, socket_setup, udp_target},
        datagram::{deliverable, recv_datagram},
        socket_options::{apply_udp_socket_options, UdpSocketConfig},
        BlockingTransport, HasTransportConfig, ReceiveOutcome,
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

impl Udp {
    /// Connect with a full configuration.
    ///
    /// The address must include an explicit port. Name resolution and socket
    /// setup share `config.connect_timeout`; see [`crate::transport::connect`]
    /// for the error contract shared with the async connectors.
    pub fn connect_with_config(address: &str, config: TransportConfig) -> Result<Self, Error> {
        let socket = Self::preflight_udp_setup(address, config, |endpoint, options| {
            let deadline = connect_deadline(Instant::now(), options.connect_timeout)?;
            let target = udp_target(&resolve_blocking(endpoint, deadline)?)?;
            deadline.remaining_or(Instant::now(), Error::connect_timeout)?;

            let socket = socket_setup(endpoint, UdpSocket::bind(bind_address_for(target)))?;
            socket_setup(endpoint, socket.connect(target))?;
            socket_setup(
                endpoint,
                apply_udp_socket_options(socket2::SockRef::from(&socket), target, options),
            )?;
            Ok(socket)
        })?;

        Ok(Self { socket, config })
    }

    /// Run endpoint parsing and UDP setup only after configuration preflight.
    fn preflight_udp_setup<T>(
        address: &str,
        config: TransportConfig,
        setup: impl FnOnce(&str, UdpSocketConfig) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let endpoint = preflight(address, &config)?;
        setup(&endpoint, config.into())
    }
}

fn arm_read(socket: &mut UdpSocket, timeout: Duration) -> std::io::Result<()> {
    socket.set_read_timeout(Some(timeout))
}

fn arm_write(socket: &mut UdpSocket, timeout: Duration) -> std::io::Result<()> {
    socket.set_write_timeout(Some(timeout))
}

/// Send one datagram before `deadline`.
///
/// A send interrupted by a signal moved nothing and is retried in place,
/// re-armed with the remaining budget, exactly as
/// [`crate::transport::blocking::write_all_bounded`] retries it.
fn send_datagram<S: ?Sized>(
    socket: &mut S,
    arm: Arm<S>,
    deadline: Deadline,
    mut send: impl FnMut(&mut S) -> std::io::Result<usize>,
) -> Result<(), Error> {
    loop {
        let remaining = deadline.remaining_or(Instant::now(), Error::io_timeout)?;
        arm(socket, remaining)?;
        match send(socket) {
            Ok(_) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) if TimedIo::Bounded.ended_idle(&error) => return Err(Error::io_timeout()),
            Err(error) => return Err(error.into()),
        }
    }
}

/// Receive one nonempty datagram within one budget. Empty datagrams and
/// signal interruptions keep the same deadline and re-arm its remainder.
fn receive_datagram<S: ?Sized>(
    socket: &mut S,
    arm: Arm<S>,
    deadline: Deadline,
    capacity: usize,
    mut receive: impl FnMut(&mut S) -> std::io::Result<ReceiveOutcome>,
) -> Result<ReceiveOutcome, Error> {
    loop {
        let remaining = deadline.remaining_or(Instant::now(), Error::io_timeout)?;
        arm(socket, remaining)?;
        match receive(socket) {
            Ok(outcome) => {
                if let Some(outcome) = deliverable(outcome, capacity)? {
                    return Ok(outcome);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) if TimedIo::Bounded.ended_idle(&error) => return Err(Error::io_timeout()),
            Err(error) => return Err(error.into()),
        }
    }
}

impl HasTransportConfig for Udp {
    fn identity(&self) -> crate::transport::TransportIdentity<'_> {
        crate::transport::TransportIdentity::Udp
    }

    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for Udp {
    fn send_with_timeout(&mut self, data: &[u8], timeout: Duration) -> Result<(), Error> {
        let deadline = Deadline::after(Instant::now(), timeout, "write_timeout")?;
        send_datagram(&mut self.socket, arm_write, deadline, |socket| {
            socket.send(data)
        })
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        let deadline = Deadline::after(Instant::now(), timeout, "read_timeout")?;
        receive_datagram(&mut self.socket, arm_read, deadline, dst.len(), |socket| {
            recv_datagram(socket, dst)
        })
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

    /// #800 review: an interrupted send is retried in place, not reported as
    /// a timeout.
    #[test]
    fn an_interrupted_send_is_retried_in_place() {
        struct FakeSocket {
            arms: usize,
            results: Vec<std::io::Result<usize>>,
        }
        fn arm(socket: &mut FakeSocket, _timeout: Duration) -> std::io::Result<()> {
            socket.arms += 1;
            Ok(())
        }

        let mut socket = FakeSocket {
            arms: 0,
            results: vec![Err(std::io::ErrorKind::Interrupted.into()), Ok(5)],
        };
        let deadline = Deadline::after(Instant::now(), Duration::from_secs(1), "write_timeout")
            .expect("budget");
        send_datagram(&mut socket, arm, deadline, |socket| {
            socket.results.remove(0)
        })
        .expect("the retried send succeeds");
        assert_eq!(socket.arms, 2, "re-armed before the retried syscall");
        assert!(socket.results.is_empty());
    }

    #[test]
    fn an_interrupted_receive_is_retried_in_place() {
        // Local fake: the real UDP receive loop owns the retry and re-arm.
        struct FakeSocket {
            armed: Vec<Duration>,
            results: Vec<std::io::Result<ReceiveOutcome>>,
        }
        fn arm(socket: &mut FakeSocket, timeout: Duration) -> std::io::Result<()> {
            socket.armed.push(timeout);
            Ok(())
        }
        let mut socket = FakeSocket {
            armed: Vec::new(),
            results: vec![
                Err(std::io::ErrorKind::Interrupted.into()),
                Ok(ReceiveOutcome::complete(3)),
            ],
        };
        let budget = Duration::from_secs(1);
        let deadline = Deadline::after(Instant::now(), budget, "read_timeout").expect("deadline");
        let outcome = receive_datagram(&mut socket, arm, deadline, 16, |socket| {
            socket.results.remove(0)
        })
        .expect("the reply after EINTR is delivered");
        assert_eq!(outcome, ReceiveOutcome::complete(3));
        assert_eq!(socket.armed.len(), 2);
        assert!(socket.armed[1] <= socket.armed[0]);
        assert!(socket.armed[0] <= budget);
        assert!(socket.results.is_empty());
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
            Err(Error::InvalidRequest { reason: actual })
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

        let received = transport
            .recv_into_with_timeout(&mut dst, Duration::from_secs(1))
            .expect("valid datagram");

        assert_eq!(received, ReceiveOutcome::complete(5));
        assert_eq!(&dst[..5], b"valid");
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
        let truncated = transport
            .recv_into_with_timeout(&mut dst, Duration::from_secs(1))
            .expect("an oversized datagram is consumed");
        assert_eq!(truncated, ReceiveOutcome::truncated(3));

        // The rejected packet is one atomic datagram; the next exact-fit ACK
        // must remain available and valid.
        sender
            .send(&[0x90, 0x41, 0xff])
            .expect("send exact-fit datagram");
        let received = transport
            .recv_into_with_timeout(&mut dst, Duration::from_secs(1))
            .expect("next exact-fit datagram remains readable");
        assert_eq!(received, ReceiveOutcome::complete(3));
        assert_eq!(dst, [0x90, 0x41, 0xff]);
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

        assert!(matches!(result, Err(Error::Timeout { .. })));
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
                parameter: "read_timeout",
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
        assert!(matches!(result, Err(Error::Timeout { .. })));
    }
    #[test]
    fn ipv6_udp_ttl_sets_the_unicast_hop_limit() {
        let Ok(peer) = UdpSocket::bind("[::1]:0") else {
            return;
        };
        let address = peer.local_addr().expect("IPv6 peer");
        let config = TransportConfig::for_udp().with_ttl(Some(7));
        let transport =
            Udp::connect_with_config(&address.to_string(), config).expect("IPv6 UDP setup");
        assert_eq!(
            socket2::SockRef::from(&transport.socket)
                .unicast_hops_v6()
                .expect("IPv6 hops"),
            7
        );
    }
}
