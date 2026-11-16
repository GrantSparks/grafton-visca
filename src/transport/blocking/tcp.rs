//! Blocking TCP transport implementation with DNS resolution and IPv6 support.

use std::{
    net::{SocketAddr, TcpStream},
    time::{Duration, Instant},
};

use crate::{
    timeout::Deadline,
    transport::{
        address::resolve_blocking,
        blocking::{read_once_bounded, write_all_bounded, TimedIo},
        builder::TransportConfig,
        connect::{connect_deadline, preflight, AttemptFailure, TcpAttempts},
        socket_options::{configured_tcp_socket, TcpConnectionConfig},
        stream_read, BlockingTransport, HasTransportConfig, ReceiveOutcome,
    },
    Error,
};

fn arm_write(stream: &mut TcpStream, timeout: Duration) -> std::io::Result<()> {
    stream.set_write_timeout(Some(timeout))
}

fn arm_read(stream: &mut TcpStream, timeout: Duration) -> std::io::Result<()> {
    stream.set_read_timeout(Some(timeout))
}

/// TCP transport for blocking VISCA communication.
///
/// This transport supports DNS resolution and both IPv4 and IPv6 addresses.
/// It operates at the stream level, reading/writing raw bytes.
/// Each receive reads directly into the caller's slice, without reading ahead
/// into another user-space buffer. Framing is handled by the runtime layer.
pub struct Tcp {
    stream: TcpStream,
    config: TransportConfig,
}

impl std::fmt::Debug for Tcp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tcp").field("stream", &self.stream).finish()
    }
}

impl Tcp {
    /// Connect with a full configuration.
    ///
    /// The address must include an explicit port; explicit IPv6 ports require
    /// brackets. Name resolution and connection setup share
    /// `config.connect_timeout`, and each resolved address is tried in order.
    /// See [`crate::transport::connect`] for the error contract shared with
    /// the async connectors.
    pub fn connect_with_config(address: &str, config: TransportConfig) -> Result<Self, Error> {
        let endpoint = preflight(address, &config)?;
        let options = TcpConnectionConfig::from(config);
        let deadline = connect_deadline(Instant::now(), options.connect_timeout)?;
        let stream = connect_resolved(
            &endpoint,
            options,
            resolve_blocking(&endpoint, deadline)?,
            deadline,
            connect_one,
            Instant::now,
        )?;
        Ok(Self { stream, config })
    }
}

/// Drive the same address plan used by the async connectors after resolution.
fn connect_resolved(
    endpoint: &str,
    options: TcpConnectionConfig,
    addresses: Vec<SocketAddr>,
    deadline: Deadline,
    mut connect: impl FnMut(&SocketAddr, Duration, TcpConnectionConfig) -> std::io::Result<TcpStream>,
    mut now: impl FnMut() -> Instant,
) -> Result<TcpStream, Error> {
    let mut attempts = TcpAttempts::new(addresses, deadline);
    while let Some((address, remaining)) = attempts.next_attempt(now()) {
        match connect(&address, remaining, options) {
            Ok(stream) => return Ok(stream),
            Err(error) => attempts.failed(AttemptFailure::from_socket(error)),
        }
    }
    Err(attempts.into_error(endpoint, now()))
}

/// Configure an unconnected socket before the blocking handshake.
fn connect_one(
    address: &SocketAddr,
    remaining: Duration,
    options: TcpConnectionConfig,
) -> std::io::Result<TcpStream> {
    let started = Instant::now();
    let socket = configured_tcp_socket(*address, options)?;
    let remaining = remaining.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        return Err(std::io::ErrorKind::TimedOut.into());
    }
    socket.connect_timeout(&(*address).into(), remaining)?;
    Ok(socket.into())
}

impl HasTransportConfig for Tcp {
    fn identity(&self) -> crate::transport::TransportIdentity<'_> {
        crate::transport::TransportIdentity::Tcp
    }

    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for Tcp {
    fn send_with_timeout(&mut self, data: &[u8], timeout: Duration) -> Result<(), Error> {
        let deadline = Deadline::after(Instant::now(), timeout, "write_timeout")?;
        write_all_bounded(&mut self.stream, arm_write, data, deadline, timeout)
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        let read = read_once_bounded(
            &mut self.stream,
            arm_read,
            dst,
            timeout,
            TimedIo::StreamSocketRead,
        )?;
        stream_read(read, "peer closed connection").map(ReceiveOutcome::complete)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::{
        io::{self, ErrorKind},
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
                max_buffer_size: 64,
            },
            ..TransportConfig::default()
        }
    }

    #[test]
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
            Err(Error::InvalidRequest { reason: actual })
                if actual.as_ref() == "transport receive buffer cannot exceed maximum buffer"
        ));
        assert!(
            !accepted,
            "invalid configuration must be rejected before TCP connect"
        );
    }
    #[test]
    fn an_unroutable_first_address_does_not_hide_the_next_refusal() {
        let addresses = [
            "192.0.2.1:5678".parse().expect("test-net address"),
            "127.0.0.1:9".parse().expect("local address"),
        ];
        let options = TcpConnectionConfig::from(
            TransportConfig::for_tcp().with_connect_timeout(Duration::from_millis(800)),
        );
        let started = Instant::now();
        let clock = std::cell::Cell::new(started);
        let deadline = connect_deadline(started, options.connect_timeout).expect("budget");
        let mut tried = Vec::new();
        // Inject OS verdicts and time into the production connector loop.
        // Windows can spend a short budget before a closed local port reports
        // refusal, which correctly yields Timeout instead of this case's
        // conclusive ConnectionRefused. The first attempt spends exactly its
        // share here; the second refusal arrives within the total budget.
        let error = connect_resolved(
            "camera.test:5678",
            options,
            addresses.to_vec(),
            deadline,
            |address, budget, _options| {
                tried.push((*address, budget));
                Err(if *address == addresses[0] {
                    clock.set(clock.get() + budget);
                    ErrorKind::TimedOut.into()
                } else {
                    ErrorKind::ConnectionRefused.into()
                })
            },
            || clock.get(),
        )
        .expect_err("both addresses fail");
        assert_eq!(
            tried,
            [
                (addresses[0], Duration::from_millis(400)),
                (addresses[1], Duration::from_millis(400)),
            ]
        );
        assert!(
            matches!(&error, Error::ConnectionFailed { source, .. } if source.kind() == ErrorKind::ConnectionRefused),
            "{error:?}"
        );
    }

    #[test]
    fn only_expired_attempts_keep_the_blocking_timeout_contract() {
        let addresses = [
            "192.0.2.1:5678".parse().expect("first address"),
            "192.0.2.2:5678".parse().expect("second address"),
        ];
        let options = TcpConnectionConfig::from(
            TransportConfig::for_tcp().with_connect_timeout(Duration::from_millis(800)),
        );
        for spend_shares in [false, true] {
            let started = Instant::now();
            let clock = std::cell::Cell::new(started);
            let deadline = connect_deadline(started, options.connect_timeout).expect("budget");
            let mut tried = Vec::new();
            let error = connect_resolved(
                "camera.test:5678",
                options,
                addresses.to_vec(),
                deadline,
                |address, budget, _options| {
                    tried.push(*address);
                    if spend_shares {
                        clock.set(clock.get() + budget);
                    }
                    Err(ErrorKind::TimedOut.into())
                },
                || clock.get(),
            )
            .expect_err("both shares expire");
            assert_eq!(tried, addresses);
            assert!(matches!(error, Error::Timeout { .. }), "{error:?}");
        }
    }

    #[test]
    fn a_socket_option_failure_tries_the_next_address_before_connecting() {
        let first = TcpListener::bind("127.0.0.1:0").expect("first listener");
        first.set_nonblocking(true).expect("nonblocking listener");
        let second = TcpListener::bind("127.0.0.1:0").expect("second listener");
        let addresses = [
            first.local_addr().expect("first"),
            second.local_addr().expect("second"),
        ];
        let options = TcpConnectionConfig::from(TransportConfig::for_tcp().with_ttl(Some(2)));
        let deadline = connect_deadline(Instant::now(), options.connect_timeout).expect("budget");
        let mut tried = Vec::new();
        let stream = connect_resolved(
            "camera.test:1259",
            options,
            addresses.to_vec(),
            deadline,
            |address, remaining, mut options| {
                tried.push(*address);
                if *address == addresses[0] {
                    // Bypass preflight to exercise an actual setsockopt error.
                    options.ttl = Some(300);
                }
                connect_one(address, remaining, options)
            },
            Instant::now,
        )
        .expect("second address connects");
        assert_eq!(tried, addresses);
        assert_eq!(stream.peer_addr().expect("peer"), addresses[1]);
        assert_eq!(stream.ttl().expect("TTL"), 2);
        assert_eq!(
            first.accept().expect_err("no SYN to failed setup").kind(),
            ErrorKind::WouldBlock
        );
    }

    #[test]
    fn ipv6_tcp_ttl_sets_the_unicast_hop_limit() {
        let Ok(listener) = TcpListener::bind("[::1]:0") else {
            return;
        };
        let address = listener.local_addr().expect("IPv6 listener");
        let config = TransportConfig::for_tcp().with_ttl(Some(7));
        let transport =
            Tcp::connect_with_config(&address.to_string(), config).expect("IPv6 TCP connect");
        assert_eq!(
            socket2::SockRef::from(&transport.stream)
                .unicast_hops_v6()
                .expect("IPv6 hops"),
            7
        );
    }
}
