//! Blocking UDP transport implementation with IPv6 support.

use std::{
    net::UdpSocket,
    time::{Duration, Instant},
};

use crate::{
    timeout::Deadline,
    transport::{
        address::{bind_address_for, resolve_blocking},
        blocking::{bounded_io, TimedIo},
        connect::{connect_deadline, socket_setup, udp_target},
        datagram::{deliverable, recv_datagram},
        socket_options::apply_udp_socket_options,
        BlockingTransport, HasTransportConfig, IoBounds, NetworkEndpoint, ReceiveOutcome,
        UdpConnect,
    },
    Error,
};

/// UDP transport for blocking VISCA communication.
///
/// This transport supports DNS resolution and both IPv4 and IPv6 addresses.
/// The owner adapter decodes every frame a datagram carries.
#[derive(Debug)]
pub struct Udp {
    socket: UdpSocket,
    config: IoBounds,
}

impl Udp {
    #[cfg(test)]
    pub(crate) fn connect_with_config(
        address: &str,
        options: UdpConnect,
        io_bounds: IoBounds,
    ) -> Result<Self, Error> {
        let endpoint = NetworkEndpoint::parse(address, None)?;
        Self::connect_endpoint(&endpoint, options, io_bounds)
    }

    pub(crate) fn connect_endpoint(
        endpoint: &NetworkEndpoint,
        options: UdpConnect,
        io_bounds: IoBounds,
    ) -> Result<Self, Error> {
        let deadline = connect_deadline(Instant::now(), options.connect_timeout())?;
        let target = udp_target(&resolve_blocking(endpoint, deadline)?)?;
        deadline.remaining_or(Instant::now(), Error::connect_timeout)?;

        let socket = socket_setup(endpoint, UdpSocket::bind(bind_address_for(target)))?;
        socket_setup(endpoint, socket.connect(target))?;
        socket_setup(
            endpoint,
            apply_udp_socket_options(socket2::SockRef::from(&socket), target, options),
        )?;
        Ok(Self {
            socket,
            config: io_bounds,
        })
    }
}

fn arm_read(socket: &mut UdpSocket, timeout: Duration) -> std::io::Result<()> {
    socket.set_read_timeout(Some(timeout))
}

fn arm_write(socket: &mut UdpSocket, timeout: Duration) -> std::io::Result<()> {
    socket.set_write_timeout(Some(timeout))
}

impl HasTransportConfig for Udp {
    fn identity(&self) -> crate::transport::TransportIdentity<'_> {
        crate::transport::TransportIdentity::Udp
    }

    fn io_bounds(&self) -> &IoBounds {
        &self.config
    }
}

impl BlockingTransport for Udp {
    fn send_with_timeout(&mut self, data: &[u8], timeout: Duration) -> Result<(), Error> {
        let deadline = Deadline::after(Instant::now(), timeout, "write_timeout")?;
        bounded_io(
            &mut self.socket,
            arm_write,
            deadline,
            TimedIo::Datagram,
            Instant::now,
            |socket| Ok(Some(socket.send(data).map(|_| ())?)),
        )
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        let deadline = Deadline::after(Instant::now(), timeout, "read_timeout")?;
        bounded_io(
            &mut self.socket,
            arm_read,
            deadline,
            TimedIo::Datagram,
            Instant::now,
            |socket| Ok(deliverable(recv_datagram(socket, dst)?, dst.len())?),
        )
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::{
        net::UdpSocket,
        thread,
        time::{Duration, Instant},
    };

    use super::*;
    use crate::transport::BlockingTransport;

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
        bounded_io(
            &mut socket,
            arm,
            deadline,
            TimedIo::Datagram,
            Instant::now,
            |socket| Ok(Some(socket.results.remove(0)?)),
        )
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
        let outcome = bounded_io(
            &mut socket,
            arm,
            deadline,
            TimedIo::Datagram,
            Instant::now,
            |socket| Ok(deliverable(socket.results.remove(0)?, 16)?),
        )
        .expect("the reply after EINTR is delivered");
        assert_eq!(outcome, ReceiveOutcome::complete(3));
        assert_eq!(socket.armed.len(), 2);
        assert!(socket.armed[1] <= socket.armed[0]);
        assert!(socket.armed[0] <= budget);
        assert!(socket.results.is_empty());
    }

    #[test]
    fn early_idle_receive_rearms_until_data_or_the_precise_deadline() {
        struct FakeSocket {
            armed: Vec<Duration>,
            reads: usize,
        }
        fn arm(socket: &mut FakeSocket, timeout: Duration) -> std::io::Result<()> {
            socket.armed.push(timeout);
            Ok(())
        }
        for idle_kind in [std::io::ErrorKind::WouldBlock, std::io::ErrorKind::TimedOut] {
            for data_on_retry in [false, true] {
                let started = Instant::now();
                let budget = Duration::from_millis(10);
                let clock = std::cell::Cell::new(started);
                let mut socket = FakeSocket {
                    armed: Vec::new(),
                    reads: 0,
                };
                let deadline = Deadline::after(started, budget, "read_timeout").expect("deadline");
                let result = bounded_io(
                    &mut socket,
                    arm,
                    deadline,
                    TimedIo::Datagram,
                    || clock.get(),
                    |socket| {
                        socket.reads += 1;
                        if socket.reads == 2 && data_on_retry {
                            return Ok(deliverable(ReceiveOutcome::complete(3), 16)?);
                        }
                        let spent = if socket.reads == 1 {
                            Duration::from_millis(4)
                        } else {
                            Duration::from_millis(6)
                        };
                        clock.set(clock.get() + spent);
                        Err(std::io::Error::from(idle_kind).into())
                    },
                );
                assert_eq!(socket.armed, [budget, Duration::from_millis(6)]);
                assert_eq!(socket.reads, 2);
                if data_on_retry {
                    assert_eq!(
                        result.expect("datagram within original budget"),
                        ReceiveOutcome::complete(3)
                    );
                } else {
                    assert!(matches!(result, Err(Error::Timeout { .. })));
                    assert_eq!(clock.get().duration_since(started), budget);
                }
            }
        }
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
    fn recv_discards_empty_datagram_before_valid_datagram() {
        let (receiver, sender) = connected_socket_pair();
        sender.send(&[]).expect("send empty datagram");
        sender.send(b"valid").expect("send valid datagram");

        let mut transport = Udp {
            socket: receiver,
            config: Default::default(),
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
            config: Default::default(),
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
            config: Default::default(),
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
            config: Default::default(),
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
            config: Default::default(),
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
        let config = UdpConnect::default()
            .with_ttl(Some(7))
            .expect("valid connector test configuration");
        let transport =
            Udp::connect_with_config(&address.to_string(), config, IoBounds::UDP_DEFAULT)
                .expect("IPv6 UDP setup");
        assert_eq!(
            socket2::SockRef::from(&transport.socket)
                .unicast_hops_v6()
                .expect("IPv6 hops"),
            7
        );
    }
}
