//! Focused coverage for the final owner-backed standard constructors.
//!
//! Standard-construction tests use a recording runtime rather than opening
//! loopback sockets. This keeps endpoint/configuration assertions at the
//! pre-I/O boundary while still starting and shutting down a real owner.

#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
mod async_standard {
    use std::{
        future::Future,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use grafton_visca::{
        camera::{CameraConfig, Connect},
        profiles::{PtzOpticsG2, SonyFR7},
        runtime::Runtime,
        transport::{
            AddressingMode, AsyncTransport, BufferConfig, HasTransportConfig, SendSemantics,
            TransportConfig,
        },
        Error, Executor,
    };

    #[cfg(feature = "runtime-tokio")]
    use grafton_visca::OperationalTuning;

    #[cfg(feature = "runtime-tokio")]
    use grafton_visca::transport::TcpKeepaliveConfig;
    #[cfg(feature = "runtime-tokio")]
    use std::num::NonZeroUsize;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Kind {
        Tcp,
        Udp,
    }

    #[derive(Debug, Clone)]
    struct Call {
        kind: Kind,
        address: String,
        config: TransportConfig,
    }

    #[derive(Debug)]
    struct ProbeTransport {
        config: TransportConfig,
    }

    impl HasTransportConfig for ProbeTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl AsyncTransport for ProbeTransport {
        #[allow(clippy::manual_async_fn)]
        fn send(&mut self, _bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
            async { Ok(()) }
        }

        #[allow(clippy::manual_async_fn)]
        fn recv_into<'a>(
            &'a mut self,
            _dst: &'a mut [u8],
        ) -> impl Future<Output = Result<usize, Error>> + Send {
            std::future::pending()
        }

        fn addressing_mode_hint(&self) -> Option<AddressingMode> {
            Some(AddressingMode::Ip)
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
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
        type Join<T>
            = E::Join<T>
        where
            T: Send + 'static;
        type Detach = E::Detach;

        fn spawn_with_detach<F>(&self, fut: F) -> (Self::Join<F::Output>, Self::Detach)
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            self.inner.spawn_with_detach(fut)
        }

        fn block_on<F: Future>(&self, fut: F) -> F::Output {
            self.inner.block_on(fut)
        }

        fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
            self.inner.sleep(duration)
        }

        fn timeout<'a, F, T>(
            &'a self,
            duration: Duration,
            fut: F,
        ) -> impl Future<Output = Result<T, Error>> + Send + 'a
        where
            F: Future<Output = T> + Send + 'a,
            T: Send + 'a,
        {
            self.inner.timeout(duration, fut)
        }

        fn now(&self) -> std::time::Instant {
            self.inner.now()
        }
    }

    impl<E: Executor> Runtime for ProbeRuntime<E> {
        type TcpTransport = ProbeTransport;
        type UdpTransport = ProbeTransport;
        #[cfg(feature = "transport-serial-tokio")]
        type SerialTransport = std::convert::Infallible;

        #[allow(clippy::manual_async_fn)]
        fn connect_tcp<'a>(
            &'a self,
            address: &'a str,
            config: TransportConfig,
        ) -> impl Future<Output = Result<Self::TcpTransport, Error>> + Send + 'a {
            async move {
                self.calls.lock().expect("calls lock").push(Call {
                    kind: Kind::Tcp,
                    address: address.to_owned(),
                    config,
                });
                Ok(ProbeTransport { config })
            }
        }

        #[allow(clippy::manual_async_fn)]
        fn connect_udp<'a>(
            &'a self,
            address: &'a str,
            config: TransportConfig,
        ) -> impl Future<Output = Result<Self::UdpTransport, Error>> + Send + 'a {
            async move {
                self.calls.lock().expect("calls lock").push(Call {
                    kind: Kind::Udp,
                    address: address.to_owned(),
                    config,
                });
                Ok(ProbeTransport { config })
            }
        }
    }

    fn assert_one_call(calls: &Arc<Mutex<Vec<Call>>>, kind: Kind, address: &str) -> Call {
        let mut calls = calls.lock().expect("calls lock");
        assert_eq!(calls.len(), 1);
        let call = calls.pop().expect("recorded connector call");
        assert_eq!(call.kind, kind);
        assert_eq!(call.address, address);
        call
    }

    async fn standard_paths<E: Runtime>(runtime: ProbeRuntime<E>) {
        let calls = runtime.calls();
        let session = Connect::open_tcp::<PtzOpticsG2, _>("camera.local", runtime.clone())
            .await
            .expect("fake TCP owner session");
        assert_eq!(
            assert_one_call(&calls, Kind::Tcp, "camera.local:5678")
                .config
                .buffer_config,
            BufferConfig::for_raw_ip()
        );
        session.shutdown().await.expect("shutdown");

        let calls = runtime.calls();
        let session = CameraConfig::<PtzOpticsG2>::udp("camera.local")
            .open_async(runtime.clone())
            .await
            .expect("fake UDP owner session");
        assert_eq!(
            assert_one_call(&calls, Kind::Udp, "camera.local:1259")
                .config
                .buffer_config,
            BufferConfig::for_udp()
        );
        session.shutdown().await.expect("shutdown");

        let calls = runtime.calls();
        let session = CameraConfig::<SonyFR7>::udp("camera.local")
            .open_async(runtime.clone())
            .await
            .expect("fake Sony UDP owner session");
        assert_one_call(&calls, Kind::Udp, "camera.local:52381");
        session.shutdown().await.expect("shutdown");
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_standard_paths_record_defaults_and_grammar() {
        let runtime = ProbeRuntime::new(
            grafton_visca::runtime::TokioRuntime::from_current().expect("runtime"),
        );
        standard_paths(runtime.clone()).await;

        let calls = runtime.calls();
        let session = CameraConfig::<PtzOpticsG2>::tcp("[2001:db8::1]:5678")
            .open_async(runtime.clone())
            .await
            .expect("fake bracketed IPv6 owner session");
        assert_one_call(&calls, Kind::Tcp, "[2001:db8::1]:5678");
        session.shutdown().await.expect("shutdown");

        let calls = runtime.calls();
        let error = CameraConfig::<PtzOpticsG2>::tcp("2001:db8::1")
            .open_async(runtime.clone())
            .await
            .expect_err("ambiguous unbracketed IPv6 must be rejected");
        assert!(matches!(error, Error::InvalidAddress { .. }));
        assert!(calls.lock().expect("calls lock").is_empty());
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_standard_paths_record_defaults_and_start_owner() {
        smol::block_on(async {
            let runtime = ProbeRuntime::new(grafton_visca::runtime::SmolRuntime::new());
            standard_paths(runtime).await;
        });
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn preflight_and_options_happen_before_connector() {
        let runtime = ProbeRuntime::new(
            grafton_visca::runtime::TokioRuntime::from_current().expect("runtime"),
        );
        let calls = runtime.calls();

        let error = CameraConfig::<SonyFR7>::new()
            .transport(grafton_visca::camera::TransportOptions::tcp(
                "does-not-resolve.invalid",
            ))
            .open_async(runtime.clone())
            .await
            .expect_err("unsupported Sony TCP must be rejected in preflight");
        assert!(matches!(error, Error::UnsupportedTransport { .. }));
        assert!(calls.lock().expect("calls lock").is_empty());

        let error = CameraConfig::<PtzOpticsG2>::tcp("does-not-resolve.invalid")
            .with_tuning(OperationalTuning::new().quick_timeout(Duration::from_secs(1)))
            .open_async(runtime.clone())
            .await
            .expect_err("a category timeout below the profile floor must fail in preflight");
        assert!(matches!(error, Error::InvalidRequest(_)));
        assert!(calls.lock().expect("calls lock").is_empty());

        let error = Connect::open_udp::<PtzOpticsG2, _>("invalid:address:format", runtime.clone())
            .await
            .expect_err("invalid endpoint must be rejected before DNS");
        assert!(matches!(error, Error::InvalidAddress { .. }));
        assert!(calls.lock().expect("calls lock").is_empty());

        let transport_config = TransportConfig {
            connect_timeout: Duration::from_millis(17),
            read_timeout: Duration::from_millis(19),
            write_timeout: Duration::from_millis(23),
            buffer_config: BufferConfig {
                recv_buffer_size: 11,
                max_buffer_size: 17,
            },
            tcp_nodelay: Some(false),
            tcp_keepalive: Some(TcpKeepaliveConfig::new(Duration::from_secs(4))),
            ..TransportConfig::default()
        };
        let session = CameraConfig::<PtzOpticsG2>::tcp("192.0.2.10:5678")
            .transport_config(transport_config)
            .open_async(runtime.clone())
            .await
            .expect("fake configured owner session");
        let call = assert_one_call(&calls, Kind::Tcp, "192.0.2.10:5678");
        assert_eq!(call.config.connect_timeout, Duration::from_millis(17));
        assert_eq!(call.config.read_timeout, Duration::from_millis(19));
        assert_eq!(call.config.write_timeout, Duration::from_millis(23));
        assert_eq!(call.config.buffer_config, transport_config.buffer_config);
        assert_eq!(call.config.tcp_nodelay, Some(false));
        assert_eq!(call.config.tcp_keepalive, transport_config.tcp_keepalive);
        session.shutdown().await.expect("shutdown");

        let udp_config = TransportConfig {
            connect_timeout: Duration::from_millis(31),
            read_timeout: Duration::from_millis(37),
            write_timeout: Duration::from_millis(41),
            buffer_config: BufferConfig {
                recv_buffer_size: 43,
                max_buffer_size: 53,
            },
            ttl: Some(59),
            addressing: AddressingMode::Ip,
            ..TransportConfig::default()
        };
        let udp_camera_config = CameraConfig::<PtzOpticsG2>::udp("192.0.2.11:1259")
            .with_admission_capacity(NonZeroUsize::new(61).expect("nonzero queue depth"))
            .transport_config(udp_config);
        assert_eq!(
            udp_camera_config
                .session_config()
                .expect("session config")
                .admission_capacity(),
            NonZeroUsize::new(61).expect("nonzero queue depth")
        );
        let session = udp_camera_config
            .open_async(runtime.clone())
            .await
            .expect("fake configured UDP owner session");
        let call = assert_one_call(&calls, Kind::Udp, "192.0.2.11:1259");
        assert_eq!(call.config.connect_timeout, Duration::from_millis(31));
        assert_eq!(call.config.read_timeout, Duration::from_millis(37));
        assert_eq!(call.config.write_timeout, Duration::from_millis(41));
        assert_eq!(call.config.buffer_config, udp_config.buffer_config);
        assert_eq!(call.config.ttl, Some(59));
        assert_eq!(call.config.addressing, AddressingMode::Ip);
        session.shutdown().await.expect("shutdown");
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
                BufferConfig {
                    recv_buffer_size: 0,
                    max_buffer_size: 64,
                },
                "transport receive buffer must be non-zero",
            ),
            (
                BufferConfig {
                    recv_buffer_size: 64,
                    max_buffer_size: 0,
                },
                "transport maximum buffer must be non-zero",
            ),
            (
                BufferConfig {
                    recv_buffer_size: 65,
                    max_buffer_size: 64,
                },
                "transport receive buffer cannot exceed maximum buffer",
            ),
        ] {
            for config in [
                CameraConfig::<PtzOpticsG2>::tcp("camera.local:5678"),
                CameraConfig::<PtzOpticsG2>::udp("camera.local:1259"),
            ] {
                let error = config
                    .transport_config(TransportConfig {
                        buffer_config,
                        ..TransportConfig::default()
                    })
                    .open_async(runtime.clone())
                    .await
                    .expect_err("invalid buffer bounds must fail in preflight");
                assert!(matches!(
                    error,
                    Error::InvalidRequest(actual) if actual.as_ref() == message
                ));
                assert!(calls.lock().expect("calls lock").is_empty());
            }
        }
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn runtime_selected_network_transport_uses_profile_defaults() {
        let runtime = ProbeRuntime::new(
            grafton_visca::runtime::TokioRuntime::from_current().expect("runtime"),
        );
        let calls = runtime.calls();

        let session = Connect::open::<PtzOpticsG2, _>(
            grafton_visca::camera::TransportOptions::tcp("camera.local"),
            runtime.clone(),
        )
        .await
        .expect("runtime-selected TCP transport");
        assert_one_call(&calls, Kind::Tcp, "camera.local:5678");
        session.shutdown().await.expect("shutdown");

        let session = Connect::open::<PtzOpticsG2, _>(
            grafton_visca::camera::TransportOptions::udp("camera.local"),
            runtime.clone(),
        )
        .await
        .expect("runtime-selected UDP transport");
        assert_one_call(&calls, Kind::Udp, "camera.local:1259");
        session.shutdown().await.expect("shutdown");

        let error = Connect::open::<SonyFR7, _>(
            grafton_visca::camera::TransportOptions::tcp("camera.local"),
            runtime,
        )
        .await
        .expect_err("runtime-selected transport still validates the profile");
        assert!(matches!(error, Error::UnsupportedTransport { .. }));
        assert!(calls.lock().expect("calls lock").is_empty());
    }

    #[cfg(feature = "transport-serial-tokio")]
    #[tokio::test]
    async fn invalid_serial_buffer_bounds_fail_before_device_open() {
        let runtime = grafton_visca::runtime::TokioRuntime::from_current().expect("runtime");
        let error =
            CameraConfig::<PtzOpticsG2>::serial("grafton-visca-test-unopened-serial-device", 9_600)
                .transport_config(TransportConfig {
                    buffer_config: BufferConfig {
                        recv_buffer_size: 65,
                        max_buffer_size: 64,
                    },
                    ..TransportConfig::default()
                })
                .open_serial_async(runtime)
                .await
                .expect_err("invalid buffer bounds must fail before serial-device open");
        assert!(matches!(
            error,
            Error::InvalidRequest(actual)
                if actual.as_ref() == "transport receive buffer cannot exceed maximum buffer"
        ));
    }
}

#[cfg(feature = "blocking")]
mod blocking_standard {
    use std::{collections::VecDeque, time::Duration};

    use grafton_visca::{
        blocking::{CameraConfig, Session},
        command::CommandKind,
        profiles::PtzOpticsG2,
        transport::{
            AddressingMode, BlockingTransport, BufferConfig, HasTransportConfig, SendSemantics,
            TransportConfig,
        },
        CameraId, Error, SessionConfig,
    };

    #[derive(Debug)]
    struct ProbeTransport {
        config: TransportConfig,
        responses: VecDeque<Vec<u8>>,
    }

    impl HasTransportConfig for ProbeTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl BlockingTransport for ProbeTransport {
        fn send_with_timeout(
            &mut self,
            _bytes: &[u8],
            _kind: CommandKind,
            _timeout: Duration,
        ) -> Result<(), Error> {
            self.responses.push_back(vec![0x90, 0x41, 0xff]);
            self.responses.push_back(vec![0x90, 0x51, 0xff]);
            Ok(())
        }

        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            _timeout: Duration,
        ) -> Result<usize, Error> {
            let response = self.responses.pop_front().ok_or(Error::Timeout)?;
            dst[..response.len()].copy_from_slice(&response);
            Ok(response.len())
        }

        fn addressing_mode_hint(&self) -> Option<AddressingMode> {
            Some(AddressingMode::Serial)
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Stream
        }
    }

    #[test]
    fn custom_blocking_session_open_uses_one_owner_and_hint() {
        let transport = ProbeTransport {
            config: TransportConfig {
                addressing: AddressingMode::Serial,
                ..TransportConfig::default()
            },
            responses: VecDeque::new(),
        };
        let config = SessionConfig::for_target(
            CameraId::CAMERA_1,
            grafton_visca::ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("profile"),
        )
        .expect("target config");
        let session = Session::open(transport, config).expect("custom blocking owner session");
        session.shutdown().expect("shutdown");
    }

    #[test]
    fn custom_blocking_session_rejects_zero_io_timeouts_at_construction() {
        for (transport_config, message) in [
            (
                TransportConfig {
                    addressing: AddressingMode::Serial,
                    read_timeout: Duration::ZERO,
                    ..TransportConfig::default()
                },
                "transport read timeout must be non-zero",
            ),
            (
                TransportConfig {
                    addressing: AddressingMode::Serial,
                    write_timeout: Duration::ZERO,
                    ..TransportConfig::default()
                },
                "transport write timeout must be non-zero",
            ),
        ] {
            let transport = ProbeTransport {
                config: transport_config,
                responses: VecDeque::new(),
            };
            let config = SessionConfig::for_target(
                CameraId::CAMERA_1,
                grafton_visca::ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("profile"),
            )
            .expect("target config");
            let error = Session::open(transport, config)
                .expect_err("invalid custom transport config must fail at construction");
            assert!(matches!(
                error,
                Error::InvalidRequest(actual) if actual.as_ref() == message
            ));
        }
    }

    #[test]
    fn standard_blocking_preflight_does_not_open_socket() {
        let error = CameraConfig::<PtzOpticsG2>::tcp("invalid:address:format")
            .open()
            .expect_err("invalid endpoint must fail before socket I/O");
        assert!(matches!(error, Error::InvalidAddress { .. }));
    }

    #[test]
    fn invalid_blocking_buffer_bounds_fail_before_socket_open() {
        let error = CameraConfig::<PtzOpticsG2>::tcp("127.0.0.1:1")
            .transport_config(TransportConfig {
                buffer_config: BufferConfig {
                    recv_buffer_size: 65,
                    max_buffer_size: 64,
                },
                ..TransportConfig::default()
            })
            .open()
            .expect_err("invalid buffer bounds must fail before socket I/O");
        assert!(matches!(
            error,
            Error::InvalidRequest(actual)
                if actual.as_ref() == "transport receive buffer cannot exceed maximum buffer"
        ));
    }

    #[cfg(feature = "transport-serial")]
    #[test]
    fn invalid_blocking_serial_buffer_bounds_fail_before_device_open() {
        let error =
            CameraConfig::<PtzOpticsG2>::serial("grafton-visca-test-unopened-serial-device", 9_600)
                .transport_config(TransportConfig {
                    buffer_config: BufferConfig {
                        recv_buffer_size: 65,
                        max_buffer_size: 64,
                    },
                    ..TransportConfig::default()
                })
                .open_serial()
                .expect_err("invalid buffer bounds must fail before serial-device open");
        assert!(matches!(
            error,
            Error::InvalidRequest(actual)
                if actual.as_ref() == "transport receive buffer cannot exceed maximum buffer"
        ));
    }
}

#[cfg(all(feature = "async", feature = "blocking", feature = "runtime-tokio"))]
mod coexistence {
    #[allow(dead_code)]
    async fn async_return_type(
    ) -> grafton_visca::Result<grafton_visca::CameraSession<grafton_visca::profiles::PtzOpticsG2>>
    {
        let runtime = grafton_visca::runtime::TokioRuntime::from_current().expect("runtime");
        grafton_visca::Connect::open_udp::<grafton_visca::profiles::PtzOpticsG2, _>(
            "127.0.0.1:1259",
            runtime,
        )
        .await
    }

    #[allow(dead_code)]
    fn blocking_return_type() -> grafton_visca::Result<
        grafton_visca::blocking::CameraSession<grafton_visca::profiles::PtzOpticsG2>,
    > {
        grafton_visca::blocking::Connect::open_udp::<grafton_visca::profiles::PtzOpticsG2>(
            "127.0.0.1:1259",
        )
    }

    #[allow(dead_code)]
    fn config_return_types() {
        let _async_config =
            grafton_visca::CameraConfig::<grafton_visca::profiles::PtzOpticsG2>::new();
        let _blocking_config =
            grafton_visca::blocking::CameraConfig::<grafton_visca::profiles::PtzOpticsG2>::new();
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
