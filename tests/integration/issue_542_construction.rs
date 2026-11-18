//! Focused coverage for the final owner-backed standard constructors.
//!
//! Standard-construction tests use a recording runtime rather than opening
//! loopback sockets. This keeps endpoint/configuration assertions at the
//! pre-I/O boundary while still starting and shutting down a real owner.
//!
//! The recording runtime hands out wires onto a silent `FakeCamera`. The
//! standard-path scenarios run under each enabled runtime, as the cases
//! `async_standard::standard_paths::{tokio,smol}` and
//! `async_standard::bracketed_ipv6_endpoint_and_ambiguous_ipv6_rejection::{tokio,smol}`;
//! the remaining async scenarios (preflight and options, buffer bounds,
//! runtime-selected transports, serial buffer bounds) are Tokio-only.

#[cfg(any(
    feature = "blocking",
    all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]
use grafton_visca_test_support::fake_camera;

#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
mod async_standard {
    use grafton_visca_test_support::runtime_matrix;
    use std::{
        future::Future,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use grafton_visca::{
        config::CameraConfig,
        profiles::{PtzOpticsG2, SonyFr7},
        runtime::{Executor, Runtime},
        transport::{AddressingMode, BufferConfig, IoBounds, TcpConnect, UdpConnect},
        Error,
    };

    use super::fake_camera::{AsyncWire, FakeCamera};

    #[cfg(feature = "runtime-tokio")]
    use grafton_visca::config::OperationalTuning;

    #[cfg(feature = "runtime-tokio")]
    use grafton_visca::transport::TcpKeepaliveConfig;
    #[cfg(feature = "runtime-tokio")]
    use std::num::NonZeroUsize;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Kind {
        Tcp(TcpConnect),
        Udp(UdpConnect),
    }

    #[derive(Debug, Clone)]
    struct Call {
        kind: Kind,
        address: String,
        config: IoBounds,
    }

    /// The wire the recording connectors hand out: an idle camera that
    /// reports IP addressing and the connector's transport configuration.
    fn probe_wire(config: IoBounds) -> AsyncWire {
        FakeCamera::silent()
            .async_wire()
            .with_config(config)
            .with_addressing(AddressingMode::Ip)
    }

    #[derive(Debug, Clone)]
    struct ProbeRuntime<E> {
        inner: E,
        calls: Arc<Mutex<Vec<Call>>>,
    }

    impl<E> ProbeRuntime<E> {
        fn new(inner: E) -> Self {
            Self {
                inner,
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn calls(&self) -> Arc<Mutex<Vec<Call>>> {
            Arc::clone(&self.calls)
        }
    }

    #[allow(refining_impl_trait_reachable)]
    impl<E: Executor> Executor for ProbeRuntime<E> {
        type Timer = E::Timer;

        fn timer(&self, deadline: std::time::Instant) -> Self::Timer {
            self.inner.timer(deadline)
        }

        fn spawn_detached<F>(&self, fut: F)
        where
            F: Future<Output = ()> + Send + 'static,
        {
            self.inner.spawn_detached(fut);
        }

        fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
            self.inner.sleep(duration)
        }

        fn now(&self) -> std::time::Instant {
            self.inner.now()
        }
    }

    impl<E: Executor> Runtime for ProbeRuntime<E> {
        type TcpTransport = AsyncWire;
        type UdpTransport = AsyncWire;
        #[cfg(feature = "transport-serial-tokio")]
        type SerialTransport = std::convert::Infallible;

        #[allow(clippy::manual_async_fn)]
        fn connect_tcp(
            &self,
            endpoint: grafton_visca::transport::NetworkEndpoint,
            options: TcpConnect,
            io_bounds: IoBounds,
        ) -> impl Future<Output = Result<Self::TcpTransport, Error>> + Send + '_ {
            async move {
                self.calls.lock().expect("calls lock").push(Call {
                    kind: Kind::Tcp(options),
                    address: endpoint.to_string(),
                    config: io_bounds,
                });
                Ok(probe_wire(io_bounds))
            }
        }

        #[allow(clippy::manual_async_fn)]
        fn connect_udp(
            &self,
            endpoint: grafton_visca::transport::NetworkEndpoint,
            options: UdpConnect,
            io_bounds: IoBounds,
        ) -> impl Future<Output = Result<Self::UdpTransport, Error>> + Send + '_ {
            async move {
                self.calls.lock().expect("calls lock").push(Call {
                    kind: Kind::Udp(options),
                    address: endpoint.to_string(),
                    config: io_bounds,
                });
                Ok(probe_wire(io_bounds))
            }
        }

        #[cfg(feature = "transport-serial-tokio")]
        #[allow(clippy::manual_async_fn)]
        fn connect_serial(
            &self,
            _config: grafton_visca::transport::SerialConfig,
            _io_bounds: grafton_visca::transport::IoBounds,
        ) -> impl Future<Output = Result<Self::SerialTransport, Error>> + Send + '_ {
            async { Err(std::io::Error::from(std::io::ErrorKind::Unsupported).into()) }
        }
    }

    fn assert_one_call(calls: &Arc<Mutex<Vec<Call>>>, kind: Kind, address: &str) -> Call {
        let mut calls = calls.lock().expect("calls lock");
        assert_eq!(calls.len(), 1);
        let call = calls.pop().expect("recorded connector call");
        assert_eq!(
            std::mem::discriminant(&call.kind),
            std::mem::discriminant(&kind)
        );
        assert_eq!(call.address, address);
        call
    }

    async fn standard_paths<E: Executor>(executor: E) {
        let runtime = ProbeRuntime::new(executor);
        let calls = runtime.calls();
        let session = grafton_visca::r#async::CameraSession::<PtzOpticsG2>::connect_tcp(
            "camera.local",
            runtime.clone(),
        )
        .await
        .expect("fake TCP owner session");
        assert_eq!(
            assert_one_call(
                &calls,
                Kind::Tcp(TcpConnect::default()),
                "camera.local:5678"
            )
            .config
            .buffer_config(),
            BufferConfig::for_tcp()
        );
        session.shutdown().expect("shutdown");

        let calls = runtime.calls();
        let session = grafton_visca::r#async::CameraSession::connect(
            &CameraConfig::<PtzOpticsG2>::udp("camera.local"),
            runtime.clone(),
        )
        .await
        .expect("fake UDP owner session");
        assert_eq!(
            assert_one_call(
                &calls,
                Kind::Udp(UdpConnect::default()),
                "camera.local:1259"
            )
            .config
            .buffer_config(),
            BufferConfig::for_udp()
        );
        session.shutdown().expect("shutdown");

        let calls = runtime.calls();
        let session = grafton_visca::r#async::CameraSession::connect(
            &CameraConfig::<SonyFr7>::udp("camera.local"),
            runtime.clone(),
        )
        .await
        .expect("fake Sony UDP owner session");
        assert_one_call(
            &calls,
            Kind::Udp(UdpConnect::default()),
            "camera.local:52381",
        );
        session.shutdown().expect("shutdown");
    }

    async fn bracketed_ipv6_endpoint_and_ambiguous_ipv6_rejection<E: Executor>(executor: E) {
        let runtime = ProbeRuntime::new(executor);
        let calls = runtime.calls();
        let session = grafton_visca::r#async::CameraSession::connect(
            &CameraConfig::<PtzOpticsG2>::tcp("[2001:db8::1]:5678"),
            runtime.clone(),
        )
        .await
        .expect("fake bracketed IPv6 owner session");
        assert_one_call(
            &calls,
            Kind::Tcp(TcpConnect::default()),
            "[2001:db8::1]:5678",
        );
        session.shutdown().expect("shutdown");

        let calls = runtime.calls();
        let error = grafton_visca::r#async::CameraSession::connect(
            &CameraConfig::<PtzOpticsG2>::tcp("2001:db8::1"),
            runtime.clone(),
        )
        .await
        .expect_err("ambiguous unbracketed IPv6 must be rejected");
        assert!(matches!(error, Error::InvalidAddress { .. }));
        assert!(calls.lock().expect("calls lock").is_empty());
    }

    runtime_matrix!(
        standard_paths,
        bracketed_ipv6_endpoint_and_ambiguous_ipv6_rejection
    );

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn preflight_and_options_happen_before_connector() {
        let runtime = ProbeRuntime::new(
            grafton_visca::runtime::TokioRuntime::from_current().expect("runtime"),
        );
        let calls = runtime.calls();

        let error = grafton_visca::r#async::CameraSession::connect(
            &grafton_visca::config::CameraConfig::new(
                grafton_visca::profiles::ProfileSpec::from_compile_time::<SonyFr7>()
                    .expect("source profile"),
                grafton_visca::config::TransportOptions::Custom,
            )
            .into_typed::<SonyFr7>()
            .expect("typed config")
            .with_transport(grafton_visca::config::TransportOptions::tcp(
                "does-not-resolve.invalid",
            )),
            runtime.clone(),
        )
        .await
        .expect_err("unsupported Sony TCP must be rejected in preflight");
        assert!(matches!(error, Error::UnsupportedTransport { .. }));
        assert!(calls.lock().expect("calls lock").is_empty());

        let error = grafton_visca::r#async::CameraSession::connect(
            &CameraConfig::<PtzOpticsG2>::tcp("does-not-resolve.invalid")
                .with_tuning(OperationalTuning::new().with_quick_timeout(Duration::from_secs(1))),
            runtime.clone(),
        )
        .await
        .expect_err("a category timeout below the profile floor must fail in preflight");
        assert!(matches!(error, Error::InvalidRequest { .. }));
        assert!(calls.lock().expect("calls lock").is_empty());

        let error = grafton_visca::r#async::CameraSession::<PtzOpticsG2>::connect_udp(
            "invalid:address:format",
            runtime.clone(),
        )
        .await
        .expect_err("invalid endpoint must be rejected before DNS");
        assert!(matches!(error, Error::InvalidAddress { .. }));
        assert!(calls.lock().expect("calls lock").is_empty());

        let keepalive = TcpKeepaliveConfig::new(Duration::from_secs(4));
        let tcp = TcpConnect::default()
            .with_connect_timeout(Duration::from_millis(17))
            .expect("connect timeout")
            .with_nodelay(Some(false))
            .with_keepalive(Some(keepalive))
            .expect("keepalive");
        let tcp_bounds = IoBounds::new(
            Duration::from_millis(19),
            Duration::from_millis(23),
            BufferConfig::default()
                .with_recv_buffer_size(31)
                .with_max_buffer_size(37),
        )
        .expect("TCP bounds");
        let session = grafton_visca::r#async::CameraSession::connect(
            &CameraConfig::<PtzOpticsG2>::tcp_configured("192.0.2.10:5678", tcp, tcp_bounds),
            runtime.clone(),
        )
        .await
        .expect("fake configured owner session");
        let call = assert_one_call(&calls, Kind::Tcp(tcp), "192.0.2.10:5678");
        assert_eq!(call.kind, Kind::Tcp(tcp));
        assert_eq!(call.config, tcp_bounds);
        session.shutdown().expect("shutdown");

        let udp = UdpConnect::default()
            .with_connect_timeout(Duration::from_millis(31))
            .expect("connect timeout")
            .with_ttl(Some(59))
            .expect("TTL");
        let udp_bounds = IoBounds::new(
            Duration::from_millis(37),
            Duration::from_millis(41),
            BufferConfig::default()
                .with_recv_buffer_size(43)
                .with_max_buffer_size(53),
        )
        .expect("UDP bounds");
        let udp_camera_config =
            CameraConfig::<PtzOpticsG2>::udp_configured("192.0.2.11:1259", udp, udp_bounds)
                .with_admission_capacity(NonZeroUsize::new(61).expect("nonzero queue depth"));
        assert_eq!(
            udp_camera_config
                .session_config()
                .expect("session config")
                .admission_capacity(),
            NonZeroUsize::new(61).expect("nonzero queue depth")
        );
        let session =
            grafton_visca::r#async::CameraSession::connect(&udp_camera_config, runtime.clone())
                .await
                .expect("fake configured UDP owner session");
        let call = assert_one_call(&calls, Kind::Udp(udp), "192.0.2.11:1259");
        assert_eq!(call.kind, Kind::Udp(udp));
        assert_eq!(call.config, udp_bounds);

        session.shutdown().expect("shutdown");
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn invalid_buffer_bounds_fail_before_network_connectors() {
        let runtime = ProbeRuntime::new(
            grafton_visca::runtime::TokioRuntime::from_current().expect("runtime"),
        );
        let calls = runtime.calls();

        for (buffer_config, message) in [
            (
                {
                    let mut config = BufferConfig::default();
                    config = config.with_recv_buffer_size(0);
                    config = config.with_max_buffer_size(64);
                    config
                },
                "transport receive buffer must hold the largest VISCA reply (24 bytes)",
            ),
            (
                {
                    let mut config = BufferConfig::default();
                    config = config.with_recv_buffer_size(BufferConfig::MIN_RECV_BUFFER_SIZE - 1);
                    config = config.with_max_buffer_size(64);
                    config
                },
                "transport receive buffer must hold the largest VISCA reply (24 bytes)",
            ),
            (
                {
                    let mut config = BufferConfig::default();
                    config = config.with_recv_buffer_size(64);
                    config = config.with_max_buffer_size(0);
                    config
                },
                "transport maximum buffer must be non-zero",
            ),
            (
                {
                    let mut config = BufferConfig::default();
                    config = config.with_recv_buffer_size(65);
                    config = config.with_max_buffer_size(64);
                    config
                },
                "transport receive buffer cannot exceed maximum buffer",
            ),
        ] {
            let error = IoBounds::default()
                .with_buffer_config(buffer_config)
                .expect_err("invalid buffer bounds are refused at construction");
            assert!(matches!(
                error,
                Error::InvalidRequest { reason: actual } if actual.as_ref() == message
            ));
            assert!(calls.lock().expect("calls lock").is_empty());
        }
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn runtime_selected_network_transport_uses_profile_defaults() {
        let runtime = ProbeRuntime::new(
            grafton_visca::runtime::TokioRuntime::from_current().expect("runtime"),
        );
        let calls = runtime.calls();

        let session = grafton_visca::r#async::CameraSession::<PtzOpticsG2>::connect(
            &grafton_visca::config::CameraConfig::new(
                grafton_visca::profiles::ProfileSpec::from_compile_time::<PtzOpticsG2>()
                    .expect("source profile"),
                grafton_visca::config::TransportOptions::tcp("camera.local"),
            )
            .into_typed::<PtzOpticsG2>()
            .expect("typed config"),
            runtime.clone(),
        )
        .await
        .expect("runtime-selected TCP transport");
        assert_one_call(
            &calls,
            Kind::Tcp(TcpConnect::default()),
            "camera.local:5678",
        );
        session.shutdown().expect("shutdown");

        let session = grafton_visca::r#async::CameraSession::<PtzOpticsG2>::connect(
            &grafton_visca::config::CameraConfig::new(
                grafton_visca::profiles::ProfileSpec::from_compile_time::<PtzOpticsG2>()
                    .expect("source profile"),
                grafton_visca::config::TransportOptions::udp("camera.local"),
            )
            .into_typed::<PtzOpticsG2>()
            .expect("typed config"),
            runtime.clone(),
        )
        .await
        .expect("runtime-selected UDP transport");
        assert_one_call(
            &calls,
            Kind::Udp(UdpConnect::default()),
            "camera.local:1259",
        );
        session.shutdown().expect("shutdown");

        let error = grafton_visca::r#async::CameraSession::<SonyFr7>::connect(
            &grafton_visca::config::CameraConfig::new(
                grafton_visca::profiles::ProfileSpec::from_compile_time::<SonyFr7>()
                    .expect("source profile"),
                grafton_visca::config::TransportOptions::tcp("camera.local"),
            )
            .into_typed::<SonyFr7>()
            .expect("typed config"),
            runtime,
        )
        .await
        .expect_err("runtime-selected transport still validates the profile");
        assert!(matches!(error, Error::UnsupportedTransport { .. }));
        assert!(calls.lock().expect("calls lock").is_empty());
    }
}

#[cfg(feature = "blocking")]
mod blocking_standard {
    use std::time::Duration;

    use grafton_visca::{
        blocking::Session,
        config::{CameraConfig, SessionConfig},
        profiles::PtzOpticsG2,
        transport::{AddressingMode, IoBounds, SendSemantics},
        Error,
    };

    use super::fake_camera::{BlockingWire, FakeCamera};

    /// A camera that answers every command with ACK and completion, behind a
    /// wire that reports serial addressing and stream semantics.
    fn serial_wire(config: IoBounds) -> BlockingWire {
        FakeCamera::acking(1)
            .blocking_wire()
            .with_config(config)
            .with_addressing(AddressingMode::Serial)
            .with_semantics(SendSemantics::Stream)
    }

    #[test]
    fn custom_blocking_session_open_uses_one_owner_and_hint() {
        let transport = serial_wire(IoBounds::default());
        let config = SessionConfig::for_target(
            grafton_visca::types::IndividualCameraId::CAMERA_1,
            grafton_visca::profiles::ProfileSpec::from_compile_time::<PtzOpticsG2>()
                .expect("profile"),
        );
        let session = Session::open(transport, &config).expect("custom blocking owner session");
        session.shutdown().expect("shutdown");
    }

    #[test]
    fn custom_transport_config_rejects_zero_io_timeouts_before_open() {
        for (io_bounds, message) in [
            (
                IoBounds::default().with_read_timeout(Duration::ZERO),
                "transport read timeout must be non-zero",
            ),
            (
                IoBounds::default().with_write_timeout(Duration::ZERO),
                "transport write timeout must be non-zero",
            ),
        ] {
            // HasTransportConfig requires the proof before a custom transport
            // can be passed to Session::open.
            let error = io_bounds.expect_err("a custom transport must prove nonzero I/O timeouts");
            assert!(matches!(
                error,
                Error::InvalidRequest { reason: actual } if actual.as_ref() == message
            ));
        }
    }

    #[test]
    fn standard_blocking_preflight_does_not_open_socket() {
        let error = grafton_visca::blocking::CameraSession::connect(
            &CameraConfig::<PtzOpticsG2>::tcp("invalid:address:format"),
        )
        .expect_err("invalid endpoint must fail before socket I/O");
        assert!(matches!(error, Error::InvalidAddress { .. }));
    }
}

#[cfg(all(feature = "async", feature = "blocking", feature = "runtime-tokio"))]
mod coexistence {
    async fn async_return_type() -> grafton_visca::Result<
        grafton_visca::r#async::CameraSession<grafton_visca::profiles::PtzOpticsG2>,
    > {
        let runtime = grafton_visca::runtime::TokioRuntime::from_current().expect("runtime");
        grafton_visca::r#async::CameraSession::<grafton_visca::profiles::PtzOpticsG2>::connect_udp(
            "127.0.0.1:1259",
            runtime,
        )
        .await
    }

    fn blocking_return_type() -> grafton_visca::Result<
        grafton_visca::blocking::CameraSession<grafton_visca::profiles::PtzOpticsG2>,
    > {
        grafton_visca::blocking::CameraSession::<grafton_visca::profiles::PtzOpticsG2>::connect_udp(
            "127.0.0.1:1259",
        )
    }

    fn config_return_types() {
        let _async_config = grafton_visca::config::CameraConfig::new(
            grafton_visca::profiles::ProfileSpec::from_compile_time::<
                grafton_visca::profiles::PtzOpticsG2,
            >()
            .expect("source profile"),
            grafton_visca::config::TransportOptions::Custom,
        )
        .into_typed::<grafton_visca::profiles::PtzOpticsG2>()
        .expect("typed config");
        let _blocking_config = grafton_visca::config::CameraConfig::new(
            grafton_visca::profiles::ProfileSpec::from_compile_time::<
                grafton_visca::profiles::PtzOpticsG2,
            >()
            .expect("source profile"),
            grafton_visca::config::TransportOptions::Custom,
        )
        .into_typed::<grafton_visca::profiles::PtzOpticsG2>()
        .expect("typed config");
        let _ = _async_config
            .session_config()
            .expect("async session config");
        let _ = _blocking_config
            .session_config()
            .expect("blocking session config");
    }

    #[test]
    fn async_and_blocking_final_names_coexist() {
        // Referencing the functions type-checks the canonical return types while
        // avoiding a real socket connection in this compile/runtime contract.
        let _ = async_return_type;
        let _ = blocking_return_type;
        config_return_types();
    }
}
