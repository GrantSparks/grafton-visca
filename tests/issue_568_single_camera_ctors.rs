//! Single-camera constructors keep the compile-time profile bind (#568).
//!
//! Every other entry point returns a `Session`, so a single-camera program has
//! to name its profile a second time through `session.camera::<P>()?` — and
//! that second naming is a runtime check. These suites pin the replacement:
//! the profile is named once, the camera view is handed out without a fallible
//! projection, and the returned value owns its session.
//!
//! The compile-time half of the contract (a mismatch is not expressible) lives
//! in the `tests/api_contract` fixtures driven by `api_stability_test.rs`.

#[cfg(feature = "blocking")]
mod blocking_single_camera {
    use std::{
        collections::VecDeque,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
        time::Duration,
    };

    use grafton_visca::{
        blocking::{CameraConfig, CameraSession, Connect},
        command::CommandKind,
        profiles::PtzOpticsG2,
        transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
        CameraId, Error,
    };

    /// IP addressing normalizes the device address byte, so the frame is the
    /// same for every registered target; the target itself is observable on
    /// the camera session and in response routing.
    const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];

    /// Answers every write with an ACK and a completion for the addressed
    /// camera, and reports when it is finally dropped.
    #[derive(Debug)]
    struct EchoTransport {
        config: TransportConfig,
        responses: Mutex<VecDeque<Vec<u8>>>,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
        dropped: Arc<AtomicBool>,
    }

    #[derive(Clone, Debug)]
    struct EchoProbe {
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
        dropped: Arc<AtomicBool>,
    }

    impl EchoProbe {
        fn writes(&self) -> Vec<Vec<u8>> {
            self.writes.lock().expect("writes lock").clone()
        }

        fn transport_dropped(&self) -> bool {
            self.dropped.load(Ordering::SeqCst)
        }
    }

    impl EchoTransport {
        fn new() -> (Self, EchoProbe) {
            let writes = Arc::new(Mutex::new(Vec::new()));
            let dropped = Arc::new(AtomicBool::new(false));
            (
                Self {
                    config: TransportConfig::default(),
                    responses: Mutex::new(VecDeque::new()),
                    writes: Arc::clone(&writes),
                    dropped: Arc::clone(&dropped),
                },
                EchoProbe { writes, dropped },
            )
        }
    }

    impl Drop for EchoTransport {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    impl HasTransportConfig for EchoTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl BlockingTransport for EchoTransport {
        fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
            self.writes
                .lock()
                .expect("writes lock")
                .push(bytes.to_vec());
            let reply = 0x80 | (bytes.first().copied().unwrap_or(0x81) & 0x0f) << 4;
            self.responses
                .lock()
                .expect("responses lock")
                .push_back(vec![reply, 0x41, 0xff, reply, 0x51, 0xff]);
            Ok(())
        }

        fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
            self.recv_into_with_timeout(dst, Duration::from_secs(1))
        }

        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            _timeout: Duration,
        ) -> Result<usize, Error> {
            let bytes = self
                .responses
                .lock()
                .expect("responses lock")
                .pop_front()
                .ok_or(Error::Timeout)?;
            dst[..bytes.len()].copy_from_slice(&bytes);
            Ok(bytes.len())
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    #[test]
    fn the_profile_is_named_once_and_the_view_needs_no_projection() {
        let (transport, probe) = EchoTransport::new();

        // `PtzOpticsG2` appears exactly once, on the configuration. The camera
        // view below is bound to it by construction: no turbofish, no `?`.
        let camera = CameraSession::open(transport, &CameraConfig::<PtzOpticsG2>::new())
            .expect("single-camera owner session");

        assert_eq!(camera.target(), CameraId::CAMERA_1);
        assert_eq!(camera.camera().target(), CameraId::CAMERA_1);

        camera
            .camera()
            .zoom()
            .stop()
            .expect("zoom stop admission")
            .applied()
            .expect("zoom stop application");

        assert_eq!(probe.writes(), vec![ZOOM_STOP.to_vec()]);

        camera.close().expect("explicit teardown");
        assert!(
            probe.transport_dropped(),
            "closing the owned camera must drop its transport"
        );
    }

    #[test]
    fn the_configured_form_keeps_camera_id_and_timing_policy() {
        let (transport, probe) = EchoTransport::new();
        let config = CameraConfig::<PtzOpticsG2>::new()
            .try_camera_id(3)
            .expect("camera 3 is addressable");

        let camera = CameraSession::open(transport, &config).expect("single-camera owner session");

        assert_eq!(camera.target(), CameraId::new(3).expect("camera 3"));
        camera
            .camera()
            .zoom()
            .stop()
            .expect("zoom stop admission")
            .applied()
            .expect("zoom stop application");
        assert_eq!(probe.writes(), vec![ZOOM_STOP.to_vec()]);

        camera.close().expect("explicit teardown");
    }

    #[test]
    fn shutdown_is_observable_and_drop_alone_tears_the_session_down() {
        let (transport, probe) = EchoTransport::new();
        let camera = CameraSession::open(transport, &CameraConfig::<PtzOpticsG2>::new())
            .expect("single-camera owner session");

        camera.shutdown().expect("owner shutdown");
        let rejected = camera
            .camera()
            .zoom()
            .stop()
            .expect_err("a shut-down owner must not admit new work");
        assert!(
            matches!(rejected, Error::RuntimeShutdown),
            "unexpected post-shutdown error: {rejected:?}"
        );
        assert!(!rejected.requires_new_session());

        // Dropping the value alone is a complete teardown: the session it owns
        // goes with it.
        drop(camera);
        assert!(probe.transport_dropped());
    }

    #[test]
    fn the_session_view_still_selects_the_same_owner() {
        let (transport, _probe) = EchoTransport::new();
        let camera = CameraSession::open(transport, &CameraConfig::<PtzOpticsG2>::new())
            .expect("single-camera owner session");

        // The multi-camera path is unchanged and reaches the same owner.
        let session_view = camera
            .session()
            .camera::<PtzOpticsG2>()
            .expect("session projection");
        assert_eq!(session_view.target(), camera.camera().target());
        assert_eq!(session_view.profile(), camera.camera().profile());

        camera.close().expect("explicit teardown");
    }

    #[test]
    fn standard_construction_preflight_runs_before_any_socket() {
        let error = Connect::open_tcp_camera::<PtzOpticsG2>("invalid:address:format")
            .expect_err("a malformed endpoint must be rejected");
        assert!(
            matches!(error, Error::InvalidAddress { .. }),
            "unexpected preflight error: {error:?}"
        );
    }
}

#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
mod async_single_camera {
    use std::{
        future::Future,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
        time::Duration,
    };

    use grafton_visca::{
        camera::{CameraConfig, Connect},
        profiles::PtzOpticsG2,
        runtime::Runtime,
        transport::{
            AddressingMode, AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig,
        },
        CameraId, CameraSession, Error, Executor,
    };

    /// IP addressing normalizes the device address byte, so the frame is the
    /// same for every registered target; the target itself is observable on
    /// the camera session and in response routing.
    const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];

    /// Answers every write with an ACK and a completion for the addressed
    /// camera, and reports when it is finally dropped.
    #[derive(Debug)]
    struct EchoTransport {
        config: TransportConfig,
        response_tx: flume::Sender<Vec<u8>>,
        responses: flume::Receiver<Vec<u8>>,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
        dropped: Arc<AtomicBool>,
    }

    #[derive(Clone, Debug)]
    struct EchoProbe {
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
        dropped: Arc<AtomicBool>,
    }

    impl EchoProbe {
        fn writes(&self) -> Vec<Vec<u8>> {
            self.writes.lock().expect("writes lock").clone()
        }

        fn transport_dropped(&self) -> bool {
            self.dropped.load(Ordering::SeqCst)
        }
    }

    impl EchoTransport {
        fn new() -> (Self, EchoProbe) {
            let (response_tx, responses) = flume::unbounded();
            let writes = Arc::new(Mutex::new(Vec::new()));
            let dropped = Arc::new(AtomicBool::new(false));
            (
                Self {
                    config: TransportConfig::default(),
                    response_tx,
                    responses,
                    writes: Arc::clone(&writes),
                    dropped: Arc::clone(&dropped),
                },
                EchoProbe { writes, dropped },
            )
        }
    }

    impl Drop for EchoTransport {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    impl HasTransportConfig for EchoTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl AsyncTransport for EchoTransport {
        fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
            self.writes
                .lock()
                .expect("writes lock")
                .push(bytes.to_vec());
            let reply = 0x80 | (bytes.first().copied().unwrap_or(0x81) & 0x0f) << 4;
            let response_tx = self.response_tx.clone();
            async move {
                response_tx
                    .send_async(vec![reply, 0x41, 0xff, reply, 0x51, 0xff])
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })
            }
        }

        #[allow(clippy::manual_async_fn)]
        fn recv_into<'a>(
            &'a mut self,
            dst: &'a mut [u8],
        ) -> impl Future<Output = Result<usize, Error>> + Send {
            async move {
                let bytes = self
                    .responses
                    .recv_async()
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
                let length = bytes.len();
                dst[..length].copy_from_slice(&bytes);
                Ok(length)
            }
        }

        fn addressing_mode_hint(&self) -> Option<AddressingMode> {
            Some(AddressingMode::Ip)
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    /// A runtime whose connector hands out the echo transport, so the standard
    /// `Connect::open_tcp_camera` path can be driven without a socket.
    #[derive(Clone, Debug)]
    struct EchoRuntime<E> {
        inner: E,
        endpoints: Arc<Mutex<Vec<String>>>,
        transport: Arc<Mutex<Option<EchoTransport>>>,
    }

    impl<E> EchoRuntime<E> {
        fn new(inner: E, transport: EchoTransport) -> Self {
            Self {
                inner,
                endpoints: Arc::new(Mutex::new(Vec::new())),
                transport: Arc::new(Mutex::new(Some(transport))),
            }
        }

        fn endpoints(&self) -> Vec<String> {
            self.endpoints.lock().expect("endpoints lock").clone()
        }

        fn take_transport(&self, address: &str) -> Result<EchoTransport, Error> {
            self.endpoints
                .lock()
                .expect("endpoints lock")
                .push(address.to_owned());
            self.transport
                .lock()
                .expect("transport lock")
                .take()
                .ok_or_else(|| Error::InvalidState("echo transport already taken".into()))
        }
    }

    #[allow(refining_impl_trait_reachable)]
    impl<E: Executor> Executor for EchoRuntime<E> {
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

    impl<E: Executor> Runtime for EchoRuntime<E> {
        type TcpTransport = EchoTransport;
        type UdpTransport = EchoTransport;
        #[cfg(feature = "transport-serial-tokio")]
        type SerialTransport = std::convert::Infallible;

        async fn connect_tcp(
            &self,
            address: &str,
            _config: TransportConfig,
        ) -> Result<Self::TcpTransport, Error> {
            self.take_transport(address)
        }

        async fn connect_udp(
            &self,
            address: &str,
            _config: TransportConfig,
        ) -> Result<Self::UdpTransport, Error> {
            self.take_transport(address)
        }
    }

    /// The one-line standard constructor names the profile once and returns a
    /// camera that owns its session.
    async fn standard_constructor_owns_its_session<E: Executor>(inner: E) {
        let (transport, probe) = EchoTransport::new();
        let runtime = EchoRuntime::new(inner, transport);

        let camera = Connect::open_tcp_camera::<PtzOpticsG2, _>("camera.local", runtime.clone())
            .await
            .expect("single-camera owner session");

        assert_eq!(runtime.endpoints(), vec!["camera.local:5678".to_owned()]);
        assert_eq!(camera.target(), CameraId::CAMERA_1);

        camera
            .camera()
            .zoom()
            .stop()
            .await
            .expect("zoom stop admission")
            .applied()
            .await
            .expect("zoom stop application");
        assert_eq!(probe.writes(), vec![ZOOM_STOP.to_vec()]);

        camera.close().await.expect("explicit teardown");
    }

    /// The configured form is the same bind with the caller's own transport.
    async fn configured_constructor_binds_the_profile_once<E: Executor>(executor: E) {
        let (transport, probe) = EchoTransport::new();
        let config = CameraConfig::<PtzOpticsG2>::new()
            .try_camera_id(3)
            .expect("camera 3 is addressable");

        let camera = CameraSession::open(transport, &config, executor)
            .await
            .expect("single-camera owner session");

        assert_eq!(camera.target(), CameraId::new(3).expect("camera 3"));
        camera
            .camera()
            .zoom()
            .stop()
            .await
            .expect("zoom stop admission")
            .applied()
            .await
            .expect("zoom stop application");
        assert_eq!(probe.writes(), vec![ZOOM_STOP.to_vec()]);

        // The owned camera survives the wrapper: it keeps the owner alive.
        let owned = camera.into_camera();
        owned
            .zoom()
            .stop()
            .await
            .expect("zoom stop admission")
            .applied()
            .await
            .expect("zoom stop application");
        assert_eq!(probe.writes().len(), 2);
    }

    /// Shutdown is observable through the owned camera, and dropping the value
    /// is a complete teardown.
    async fn shutdown_and_drop_tear_the_session_down<E: Executor>(executor: E) {
        let (transport, probe) = EchoTransport::new();
        let camera = CameraSession::open(
            transport,
            &CameraConfig::<PtzOpticsG2>::new(),
            executor.clone(),
        )
        .await
        .expect("single-camera owner session");

        camera.shutdown().await.expect("owner shutdown");
        let rejected = camera
            .camera()
            .zoom()
            .stop()
            .await
            .expect_err("a shut-down owner must not admit new work");
        assert!(
            matches!(rejected, Error::RuntimeShutdown),
            "unexpected post-shutdown error: {rejected:?}"
        );
        assert!(!rejected.requires_new_session());

        drop(camera);
        for _ in 0..100 {
            if probe.transport_dropped() {
                return;
            }
            executor.sleep(Duration::from_millis(5)).await;
        }
        panic!("dropping the owned camera must end the owner and its transport");
    }

    async fn run_matrix<E: Executor>(executor: E) {
        standard_constructor_owns_its_session(executor.clone()).await;
        configured_constructor_binds_the_profile_once(executor.clone()).await;
        shutdown_and_drop_tear_the_session_down(executor).await;
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_single_camera_constructors() {
        run_matrix(grafton_visca::TokioRuntime::from_current().expect("Tokio runtime")).await;
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_single_camera_constructors() {
        smol::block_on(run_matrix(grafton_visca::SmolRuntime::new()));
    }
}
