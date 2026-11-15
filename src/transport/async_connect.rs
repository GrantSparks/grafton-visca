//! Async driver of the shared IP connect pipeline.
//!
//! [`crate::transport::connect`] owns the pipeline's rules. This module runs
//! them once for every async runtime: a runtime contributes only its I/O
//! primitives through [`AsyncNet`], and every connect step is bounded by the
//! one [`within`] helper instead of a per-runtime timeout wrapper.

use std::{
    future::Future,
    io,
    net::SocketAddr,
    time::{Duration, Instant},
};

use futures_lite::future::race;

use crate::{
    timeout::Deadline,
    transport::{
        address::{bind_address_for, literal_socket_addr, resolved},
        connect::{connect_deadline, socket_setup, udp_target, TcpAttempts},
        socket_options::{
            apply_tcp_socket_options, apply_udp_socket_options, TcpConnectionConfig,
            UdpSocketConfig,
        },
    },
    Error, Result,
};

/// The I/O primitives one async runtime contributes to the connect pipeline.
pub trait AsyncNet {
    /// The runtime's connected TCP stream.
    type TcpStream: Send;
    /// The runtime's UDP socket.
    type UdpSocket: Send + Sync;

    /// Resolve a canonical `host:port` endpoint.
    fn lookup(endpoint: String) -> impl Future<Output = io::Result<Vec<SocketAddr>>> + Send;
    /// Connect a TCP stream to one resolved address.
    fn connect_tcp(address: SocketAddr)
        -> impl Future<Output = io::Result<Self::TcpStream>> + Send;
    /// Bind a UDP socket to a local address.
    fn bind_udp(address: SocketAddr) -> impl Future<Output = io::Result<Self::UdpSocket>> + Send;
    /// Connect a bound UDP socket to one resolved address.
    fn connect_udp(
        socket: &Self::UdpSocket,
        address: SocketAddr,
    ) -> impl Future<Output = io::Result<()>> + Send + '_;
    /// The OS socket behind a TCP stream, for socket options.
    fn tcp_socket(stream: &Self::TcpStream) -> socket2::SockRef<'_>;
    /// The OS socket behind a UDP socket, for socket options.
    fn udp_socket(socket: &Self::UdpSocket) -> socket2::SockRef<'_>;
    /// Sleep on the runtime's timer.
    fn sleep(duration: Duration) -> impl Future<Output = ()> + Send;
}

/// Run one connect step within `remaining`; `None` means the budget ran out.
async fn within<N: AsyncNet, T>(remaining: Duration, step: impl Future<Output = T>) -> Option<T> {
    race(async { Some(step.await) }, async {
        N::sleep(remaining).await;
        None
    })
    .await
}

/// Resolve `endpoint` within the connect budget.
async fn resolve<N: AsyncNet>(endpoint: &str, deadline: Deadline) -> Result<Vec<SocketAddr>> {
    if let Some(address) = literal_socket_addr(endpoint) {
        return Ok(vec![address]);
    }
    let remaining = deadline.remaining_or(Instant::now(), Error::connect_timeout)?;
    match within::<N, _>(remaining, N::lookup(endpoint.to_owned())).await {
        Some(lookup) => resolved(endpoint, lookup),
        None => Err(Error::connect_timeout()),
    }
}

/// Connect a configured TCP stream to a canonical endpoint.
pub(crate) async fn connect_tcp<N: AsyncNet>(
    endpoint: &str,
    options: TcpConnectionConfig,
) -> Result<N::TcpStream> {
    let deadline = connect_deadline(Instant::now(), options.connect_timeout)?;
    let mut attempts = TcpAttempts::new(resolve::<N>(endpoint, deadline).await?, deadline);
    while let Some((address, remaining)) = attempts.next_attempt(Instant::now()) {
        match within::<N, _>(remaining, N::connect_tcp(address)).await {
            Some(Ok(stream)) => {
                socket_setup(
                    endpoint,
                    apply_tcp_socket_options(N::tcp_socket(&stream), options),
                )?;
                return Ok(stream);
            }
            Some(Err(error)) => attempts.failed(Some(error)),
            None => attempts.failed(None),
        }
    }
    Err(attempts.into_error(endpoint, Instant::now()))
}

/// Bind and connect a configured UDP socket to a canonical endpoint.
pub(crate) async fn connect_udp<N: AsyncNet>(
    endpoint: &str,
    options: UdpSocketConfig,
) -> Result<N::UdpSocket> {
    let deadline = connect_deadline(Instant::now(), options.connect_timeout)?;
    let target = udp_target(&resolve::<N>(endpoint, deadline).await?)?;

    let remaining = deadline.remaining_or(Instant::now(), Error::connect_timeout)?;
    let socket = socket_setup(
        endpoint,
        within::<N, _>(remaining, N::bind_udp(bind_address_for(target)))
            .await
            .ok_or_else(Error::connect_timeout)?,
    )?;
    let remaining = deadline.remaining_or(Instant::now(), Error::connect_timeout)?;
    socket_setup(
        endpoint,
        within::<N, _>(remaining, N::connect_udp(&socket, target))
            .await
            .ok_or_else(Error::connect_timeout)?,
    )?;

    socket_setup(
        endpoint,
        apply_udp_socket_options(N::udp_socket(&socket), options),
    )?;
    Ok(socket)
}

#[cfg(all(test, feature = "runtime-tokio"))]
#[allow(clippy::expect_used)]
mod tests {
    use std::{
        net::{TcpListener, TcpStream, UdpSocket},
        sync::OnceLock,
    };

    use super::*;
    use crate::transport::builder::TransportConfig;

    /// An address that never answers (TEST-NET-1).
    const UNROUTABLE: &str = "192.0.2.1:5678";
    static REACHABLE: OnceLock<SocketAddr> = OnceLock::new();

    /// A runtime whose resolver returns an unroutable address first and a
    /// reachable one second.
    struct TwoAddressNet;

    impl AsyncNet for TwoAddressNet {
        type TcpStream = TcpStream;
        type UdpSocket = UdpSocket;

        async fn lookup(_endpoint: String) -> io::Result<Vec<SocketAddr>> {
            Ok(vec![
                UNROUTABLE.parse().expect("address"),
                *REACHABLE.get().expect("listener address"),
            ])
        }

        async fn connect_tcp(address: SocketAddr) -> io::Result<TcpStream> {
            if address == UNROUTABLE.parse().expect("address") {
                std::future::pending().await
            } else {
                TcpStream::connect(address)
            }
        }

        async fn bind_udp(_address: SocketAddr) -> io::Result<UdpSocket> {
            Err(io::ErrorKind::Unsupported.into())
        }

        async fn connect_udp(_socket: &UdpSocket, _address: SocketAddr) -> io::Result<()> {
            Err(io::ErrorKind::Unsupported.into())
        }

        fn tcp_socket(stream: &TcpStream) -> socket2::SockRef<'_> {
            socket2::SockRef::from(stream)
        }

        fn udp_socket(socket: &UdpSocket) -> socket2::SockRef<'_> {
            socket2::SockRef::from(socket)
        }

        async fn sleep(duration: Duration) {
            tokio::time::sleep(duration).await;
        }
    }

    /// Inject failures at the real shared connector boundary. The socket
    /// primitives are runtime-independent, so these errors exercise the same
    /// code used by Tokio and smol.
    struct FailingUdp<const STAGE: u8>;

    impl<const STAGE: u8> AsyncNet for FailingUdp<STAGE> {
        type TcpStream = TcpStream;
        type UdpSocket = UdpSocket;

        async fn lookup(_: String) -> io::Result<Vec<SocketAddr>> {
            unreachable!("numeric test endpoints do not resolve")
        }
        async fn connect_tcp(_: SocketAddr) -> io::Result<TcpStream> {
            unreachable!("UDP test")
        }
        async fn bind_udp(address: SocketAddr) -> io::Result<UdpSocket> {
            if STAGE == 3 {
                return std::future::pending().await;
            }
            if STAGE == 0 {
                Err(io::Error::new(
                    io::ErrorKind::AddrNotAvailable,
                    "bind failed",
                ))
            } else {
                UdpSocket::bind(address)
            }
        }
        async fn connect_udp(socket: &UdpSocket, address: SocketAddr) -> io::Result<()> {
            if STAGE == 1 {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "connect failed",
                ))
            } else {
                socket.connect(address)
            }
        }
        fn tcp_socket(stream: &TcpStream) -> socket2::SockRef<'_> {
            socket2::SockRef::from(stream)
        }
        fn udp_socket(socket: &UdpSocket) -> socket2::SockRef<'_> {
            socket2::SockRef::from(socket)
        }
        async fn sleep(duration: Duration) {
            tokio::time::sleep(duration).await;
        }
    }

    #[tokio::test]
    async fn udp_bind_connect_and_option_failures_use_connection_failed() {
        const ENDPOINT: &str = "127.0.0.1:1259";
        let options = UdpSocketConfig {
            ttl: None,
            connect_timeout: Duration::from_secs(1),
        };
        let mut invalid_options = options;
        // Bypass public preflight only to force an actual socket-option error.
        invalid_options.ttl = Some(300);
        for (result, expected_kind) in [
            (
                connect_udp::<FailingUdp<0>>(ENDPOINT, options).await,
                Some(io::ErrorKind::AddrNotAvailable),
            ),
            (
                connect_udp::<FailingUdp<1>>(ENDPOINT, options).await,
                Some(io::ErrorKind::PermissionDenied),
            ),
            (
                connect_udp::<FailingUdp<2>>(ENDPOINT, invalid_options).await,
                None,
            ),
        ] {
            let Error::ConnectionFailed { addr, source } = result.expect_err("UDP setup fails")
            else {
                panic!("UDP setup must use ConnectionFailed");
            };
            assert_eq!(addr, ENDPOINT);
            if let Some(kind) = expected_kind {
                assert_eq!(source.kind(), kind);
            }
        }
    }

    #[tokio::test]
    async fn udp_setup_budget_expiry_keeps_the_session_timeout_contract() {
        let options = UdpSocketConfig {
            ttl: None,
            connect_timeout: Duration::from_millis(1),
        };
        assert!(matches!(
            connect_udp::<FailingUdp<3>>("127.0.0.1:1259", options).await,
            Err(Error::Timeout { context }) if context == crate::FailureContext::new(
                crate::FailureStage::Session,
                crate::Certainty::NotAccepted,
            )
        ));
    }

    #[tokio::test]
    async fn a_numeric_ipv6_udp_target_is_used_as_given() {
        if UdpSocket::bind("[::1]:0").is_err() {
            return; // This host has no IPv6 loopback support.
        }
        let options = UdpSocketConfig {
            ttl: None,
            connect_timeout: Duration::from_secs(1),
        };
        let socket = connect_udp::<FailingUdp<2>>("[::1]:1259", options)
            .await
            .expect("numeric IPv6 setup");
        assert_eq!(
            socket.peer_addr().expect("peer"),
            "[::1]:1259".parse::<SocketAddr>().expect("IPv6 endpoint")
        );
    }

    /// #798 review: an unroutable first address used to consume the whole
    /// connect budget, so the reachable second address was never tried.
    #[tokio::test]
    async fn an_unroutable_first_address_does_not_starve_the_next() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        REACHABLE
            .set(listener.local_addr().expect("listener address"))
            .expect("set once");
        let mut config = TransportConfig::for_tcp();
        config.connect_timeout = Duration::from_millis(600);

        let stream = connect_tcp::<TwoAddressNet>("camera.test:5678", config.into())
            .await
            .expect("the second resolved address connects within the budget");
        assert_eq!(
            stream.peer_addr().expect("peer"),
            *REACHABLE.get().expect("listener address")
        );
    }
}
