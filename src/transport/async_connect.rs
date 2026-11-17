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

use crate::{
    timeout::Deadline,
    transport::{
        address::{bind_address_for, literal_socket_addr, resolved},
        connect::{connect_deadline, socket_setup, udp_target, AttemptFailure, TcpAttempts},
        socket_options::{apply_udp_socket_options, TcpConnectionConfig, UdpSocketConfig},
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
    /// Configure an unconnected TCP socket, then connect to one resolved address.
    /// Socket-option errors are failures of this address attempt.
    fn connect_tcp(
        address: SocketAddr,
        options: TcpConnectionConfig,
    ) -> impl Future<Output = io::Result<Self::TcpStream>> + Send;
    /// Bind a UDP socket to a local address.
    fn bind_udp(address: SocketAddr) -> impl Future<Output = io::Result<Self::UdpSocket>> + Send;
    /// Connect a bound UDP socket to one resolved address.
    fn connect_udp(
        socket: &Self::UdpSocket,
        address: SocketAddr,
    ) -> impl Future<Output = io::Result<()>> + Send + '_;
    /// The OS socket behind a TCP stream, for socket options.
    #[cfg(test)]
    fn tcp_socket(stream: &Self::TcpStream) -> socket2::SockRef<'_>;
    /// The OS socket behind a UDP socket, for socket options.
    fn udp_socket(socket: &Self::UdpSocket) -> socket2::SockRef<'_>;
    /// Sleep on the runtime's timer.
    fn sleep(duration: Duration) -> impl Future<Output = ()> + Send;
}

/// Run one connect step within `remaining`; `None` means the budget ran out.
async fn within<N: AsyncNet, T>(remaining: Duration, step: impl Future<Output = T>) -> Option<T> {
    crate::timeout::within(step, N::sleep(remaining)).await
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
        match within::<N, _>(remaining, N::connect_tcp(address, options)).await {
            Some(Ok(stream)) => return Ok(stream),
            Some(Err(error)) => attempts.failed(AttemptFailure::from_socket(error)),
            None => attempts.failed(AttemptFailure::ShareExpired),
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
        apply_udp_socket_options(N::udp_socket(&socket), target, options),
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
    static OPTION_ADDRESSES: OnceLock<[SocketAddr; 2]> = OnceLock::new();

    /// A runtime whose resolver returns an unroutable address first and a
    /// reachable one second.
    struct TwoAddressNet<
        const REFUSE: bool = false,
        const ALL_EXPIRED: bool = false,
        N = crate::transport::tokio::connectors::TokioNet,
    >(std::marker::PhantomData<N>);

    impl<const REFUSE: bool, const ALL_EXPIRED: bool, N: AsyncNet + 'static> AsyncNet
        for TwoAddressNet<REFUSE, ALL_EXPIRED, N>
    {
        type TcpStream = TcpStream;
        type UdpSocket = UdpSocket;

        async fn lookup(_endpoint: String) -> io::Result<Vec<SocketAddr>> {
            Ok(vec![
                UNROUTABLE.parse().expect("address"),
                if REFUSE || ALL_EXPIRED {
                    "127.0.0.1:9".parse().expect("address")
                } else {
                    *REACHABLE.get().expect("listener address")
                },
            ])
        }

        async fn connect_tcp(
            address: SocketAddr,
            _options: TcpConnectionConfig,
        ) -> io::Result<TcpStream> {
            if ALL_EXPIRED || address == UNROUTABLE.parse().expect("address") {
                std::future::pending().await
            } else if REFUSE {
                Err(io::ErrorKind::ConnectionRefused.into())
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

        #[cfg(test)]
        fn tcp_socket(stream: &TcpStream) -> socket2::SockRef<'_> {
            socket2::SockRef::from(stream)
        }

        fn udp_socket(socket: &UdpSocket) -> socket2::SockRef<'_> {
            socket2::SockRef::from(socket)
        }

        async fn sleep(duration: Duration) {
            N::sleep(duration).await;
        }
    }

    /// Local fake: fail DNS with a chosen typed cause while using each
    /// runtime's production timeout and connector pipeline.
    struct ResolverFailureNet<N>(std::marker::PhantomData<N>);

    impl<N: AsyncNet + 'static> AsyncNet for ResolverFailureNet<N> {
        type TcpStream = N::TcpStream;
        type UdpSocket = N::UdpSocket;

        async fn lookup(endpoint: String) -> io::Result<Vec<SocketAddr>> {
            if endpoint.starts_with("raw_os.") {
                return Err(io::Error::from_raw_os_error(5));
            }
            let kind = if endpoint.starts_with("missing.") {
                io::ErrorKind::NotFound
            } else {
                io::ErrorKind::Other
            };
            Err(io::Error::new(kind, "injected resolver failure"))
        }
        async fn connect_tcp(
            address: SocketAddr,
            options: TcpConnectionConfig,
        ) -> io::Result<Self::TcpStream> {
            N::connect_tcp(address, options).await
        }
        async fn bind_udp(address: SocketAddr) -> io::Result<Self::UdpSocket> {
            N::bind_udp(address).await
        }
        async fn connect_udp(socket: &Self::UdpSocket, address: SocketAddr) -> io::Result<()> {
            N::connect_udp(socket, address).await
        }
        fn tcp_socket(stream: &Self::TcpStream) -> socket2::SockRef<'_> {
            N::tcp_socket(stream)
        }
        fn udp_socket(socket: &Self::UdpSocket) -> socket2::SockRef<'_> {
            N::udp_socket(socket)
        }
        async fn sleep(duration: Duration) {
            N::sleep(duration).await;
        }
    }

    async fn check_resolver_source<N: AsyncNet + 'static>() {
        for (endpoint, kind, raw_os_error) in [
            ("missing.camera:1259", io::ErrorKind::NotFound, None),
            ("failed.camera:1259", io::ErrorKind::Other, None),
            (
                "raw_os.camera:1259",
                io::Error::from_raw_os_error(5).kind(),
                Some(5),
            ),
        ] {
            for error in [
                connect_tcp::<ResolverFailureNet<N>>(endpoint, TransportConfig::for_tcp().into())
                    .await
                    .err()
                    .expect("failed lookup"),
                connect_udp::<ResolverFailureNet<N>>(endpoint, TransportConfig::for_udp().into())
                    .await
                    .err()
                    .expect("failed lookup"),
            ] {
                assert!(
                    matches!(error, Error::ConnectionFailed { ref addr, .. } if addr == endpoint)
                );
                let source = std::error::Error::source(&error)
                    .expect("source")
                    .downcast_ref::<io::Error>()
                    .expect("direct I/O cause");
                assert_eq!(source.kind(), kind);
                assert_eq!(source.raw_os_error(), raw_os_error);
                if let Error::ConnectionFailed { source: stored, .. } = &error {
                    assert!(std::ptr::eq(source, stored.as_ref()));
                }
                let cloned = error.clone();
                let dynamic: &(dyn std::error::Error + 'static) = &cloned;
                let cloned_source = dynamic
                    .source()
                    .expect("dynamic source")
                    .downcast_ref::<io::Error>()
                    .expect("dynamic direct I/O cause");
                assert!(std::ptr::eq(source, cloned_source));
                assert_eq!(cloned_source.kind(), kind);
                assert_eq!(cloned_source.raw_os_error(), raw_os_error);
            }
        }
    }

    #[tokio::test]
    async fn resolver_io_sources_survive_the_runtime_connector_boundary() {
        check_resolver_source::<crate::transport::tokio::connectors::TokioNet>().await;
        #[cfg(feature = "runtime-smol")]
        smol::block_on(check_resolver_source::<
            crate::transport::smol::connectors::SmolNet,
        >());
    }

    /// Local fake: inject one real socket-option failure before a runtime's
    /// normal connect, while exercising the production address-attempt loop.
    struct OptionFailureNet<N>(std::marker::PhantomData<N>);

    impl<N: AsyncNet + 'static> AsyncNet for OptionFailureNet<N> {
        type TcpStream = N::TcpStream;
        type UdpSocket = N::UdpSocket;

        async fn lookup(_: String) -> io::Result<Vec<SocketAddr>> {
            Ok(OPTION_ADDRESSES
                .get()
                .expect("option-test addresses")
                .to_vec())
        }

        async fn connect_tcp(
            address: SocketAddr,
            mut options: TcpConnectionConfig,
        ) -> io::Result<Self::TcpStream> {
            if address == OPTION_ADDRESSES.get().expect("option-test addresses")[0] {
                options.ttl = Some(300);
                crate::transport::socket_options::configured_tcp_socket(address, options)?;
                unreachable!("invalid TTL cannot start a connect");
            }
            N::connect_tcp(address, options).await
        }

        async fn bind_udp(address: SocketAddr) -> io::Result<Self::UdpSocket> {
            N::bind_udp(address).await
        }

        async fn connect_udp(socket: &Self::UdpSocket, address: SocketAddr) -> io::Result<()> {
            N::connect_udp(socket, address).await
        }

        #[cfg(test)]
        fn tcp_socket(stream: &Self::TcpStream) -> socket2::SockRef<'_> {
            N::tcp_socket(stream)
        }

        fn udp_socket(socket: &Self::UdpSocket) -> socket2::SockRef<'_> {
            N::udp_socket(socket)
        }

        async fn sleep(duration: Duration) {
            N::sleep(duration).await;
        }
    }

    async fn check_option_failure_fallback<N: AsyncNet + 'static>() {
        let config = TransportConfig::for_tcp().with_ttl(Some(2));
        let stream = connect_tcp::<OptionFailureNet<N>>("camera.options.test:1259", config.into())
            .await
            .expect("second address connects after option failure");
        assert_eq!(N::tcp_socket(&stream).ttl_v4().expect("TTL"), 2);
        assert_eq!(
            N::tcp_socket(&stream)
                .peer_addr()
                .expect("peer")
                .as_socket(),
            Some(OPTION_ADDRESSES.get().expect("addresses")[1])
        );
    }

    #[tokio::test]
    async fn socket_option_failure_advances_to_the_next_address_on_each_runtime() {
        let first = TcpListener::bind("127.0.0.1:0").expect("first listener");
        first.set_nonblocking(true).expect("nonblocking listener");
        let second = TcpListener::bind("127.0.0.1:0").expect("second listener");
        OPTION_ADDRESSES
            .set([
                first.local_addr().expect("first"),
                second.local_addr().expect("second"),
            ])
            .expect("set option-test addresses");
        check_option_failure_fallback::<crate::transport::tokio::connectors::TokioNet>().await;
        #[cfg(feature = "runtime-smol")]
        smol::block_on(check_option_failure_fallback::<
            crate::transport::smol::connectors::SmolNet,
        >());
        assert_eq!(
            first
                .accept()
                .expect_err("failed setup never connects")
                .kind(),
            io::ErrorKind::WouldBlock
        );
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
        async fn connect_tcp(_: SocketAddr, _: TcpConnectionConfig) -> io::Result<TcpStream> {
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
        #[cfg(test)]
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
            assert!(matches!(&result, Err(Error::ConnectionFailed { .. })));
            let Error::ConnectionFailed { addr, source } = result.expect_err("UDP setup fails")
            else {
                return;
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

    #[tokio::test]
    async fn an_expired_first_address_share_preserves_the_second_address_refusal() {
        let config = TransportConfig::for_tcp().with_connect_timeout(Duration::from_millis(600));
        let error = connect_tcp::<TwoAddressNet<true>>("camera.test:5678", config.into())
            .await
            .expect_err("second address refuses");
        assert!(
            matches!(error, Error::ConnectionFailed { source, .. } if source.kind() == io::ErrorKind::ConnectionRefused)
        );
    }

    #[tokio::test]
    async fn all_expired_address_shares_are_a_timeout_on_async_runtimes() {
        let config = TransportConfig::for_tcp().with_connect_timeout(Duration::from_millis(30));
        assert!(matches!(
            connect_tcp::<TwoAddressNet<false, true>>("camera.test:5678", config.into()).await,
            Err(Error::Timeout { .. })
        ));
        #[cfg(feature = "runtime-smol")]
        assert!(matches!(
            smol::block_on(connect_tcp::<
                TwoAddressNet<false, true, crate::transport::smol::connectors::SmolNet>,
            >("camera.test:5678", config.into())),
            Err(Error::Timeout { .. })
        ));
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_preserves_the_refusal_after_an_expired_first_address_share() {
        let config = TransportConfig::for_tcp().with_connect_timeout(Duration::from_millis(600));
        let error = smol::block_on(connect_tcp::<
            TwoAddressNet<true, false, crate::transport::smol::connectors::SmolNet>,
        >("camera.test:5678", config.into()))
        .expect_err("second address refuses");
        assert!(
            matches!(error, Error::ConnectionFailed { source, .. } if source.kind() == io::ErrorKind::ConnectionRefused)
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

#[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
#[allow(clippy::expect_used)]
mod ipv6_tests {
    use super::*;

    async fn check_hop_limits<N: AsyncNet>() {
        let Ok(listener) = std::net::TcpListener::bind("[::1]:0") else {
            return;
        };
        let address = listener.local_addr().expect("IPv6 listener");
        let tcp = connect_tcp::<N>(
            &address.to_string(),
            crate::transport::TransportConfig::for_tcp()
                .with_ttl(Some(7))
                .into(),
        )
        .await
        .expect("IPv6 TCP connect");
        assert_eq!(
            N::tcp_socket(&tcp)
                .unicast_hops_v6()
                .expect("TCP IPv6 hops"),
            7
        );
        let udp = connect_udp::<N>(
            &address.to_string(),
            crate::transport::TransportConfig::for_udp()
                .with_ttl(Some(7))
                .into(),
        )
        .await
        .expect("IPv6 UDP setup");
        assert_eq!(
            N::udp_socket(&udp)
                .unicast_hops_v6()
                .expect("UDP IPv6 hops"),
            7
        );
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_tcp_and_udp_apply_the_ipv6_hop_limit() {
        check_hop_limits::<crate::transport::tokio::connectors::TokioNet>().await;
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_tcp_and_udp_apply_the_ipv6_hop_limit() {
        smol::block_on(check_hop_limits::<
            crate::transport::smol::connectors::SmolNet,
        >());
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod deadline_tie_tests {
    use std::{
        net::{TcpListener, TcpStream, UdpSocket},
        sync::{Arc, OnceLock},
    };

    use super::*;

    static CONNECTED: OnceLock<Arc<TcpStream>> = OnceLock::new();
    struct ReadyNet;

    impl AsyncNet for ReadyNet {
        type TcpStream = Arc<TcpStream>;
        type UdpSocket = UdpSocket;

        async fn lookup(_: String) -> io::Result<Vec<SocketAddr>> {
            Ok(vec![CONNECTED
                .get()
                .expect("connected stream")
                .peer_addr()?])
        }

        async fn connect_tcp(_: SocketAddr, _: TcpConnectionConfig) -> io::Result<Self::TcpStream> {
            Ok(Arc::clone(CONNECTED.get().expect("connected stream")))
        }

        async fn bind_udp(address: SocketAddr) -> io::Result<UdpSocket> {
            UdpSocket::bind(address)
        }

        async fn connect_udp(socket: &UdpSocket, address: SocketAddr) -> io::Result<()> {
            socket.connect(address)
        }

        fn tcp_socket(stream: &Self::TcpStream) -> socket2::SockRef<'_> {
            socket2::SockRef::from(stream.as_ref())
        }

        fn udp_socket(socket: &UdpSocket) -> socket2::SockRef<'_> {
            socket2::SockRef::from(socket)
        }

        async fn sleep(_: Duration) {}
    }

    #[test]
    fn completed_connect_step_wins_an_already_ready_timer() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let connected = Arc::new(TcpStream::connect(address).expect("connect"));
        let (_peer, _) = listener.accept().expect("accept");
        CONNECTED
            .set(Arc::clone(&connected))
            .expect("one test stream");
        let options = crate::transport::builder::TransportConfig::for_tcp().into();
        pollster::block_on(async {
            for _ in 0..10_000 {
                let stream =
                    within::<ReadyNet, _>(Duration::ZERO, ReadyNet::connect_tcp(address, options))
                        .await
                        .expect("completed step wins")
                        .expect("connected");
                assert!(Arc::ptr_eq(&stream, &connected));
            }
            let stream = connect_tcp::<ReadyNet>("camera.tie.test:1259", options)
                .await
                .expect("lookup and connection both beat the ready timer");
            assert!(Arc::ptr_eq(&stream, &connected));
        });
    }
}
