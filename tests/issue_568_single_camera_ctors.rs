//! Single-camera constructors keep the compile-time profile bind (#568).
//!
//! Standard TCP/UDP entry points return a `CameraSession<P>`: the profile is
//! named once, the camera view is handed out without a fallible projection,
//! and the returned value owns its session. Explicit Session construction
//! remains available for multi-target serial and caller-owned transports.
//!
//! The compile-time half of the contract (a mismatch is not expressible) lives
//! in the `tests/api_contract` fixtures driven by `api_stability_test.rs`.

#![cfg(any(
    feature = "blocking",
    all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]

#[macro_use]
#[path = "common/matrix.rs"]
mod matrix;

#[path = "common/fake_camera.rs"]
mod fake_camera;

use fake_camera::{FakeCamera, ZOOM_STOP};

/// A camera that answers every write with an ACK and a completion for the
/// addressed camera, in one read. IP addressing normalizes the device address
/// byte, so the frame written is the same for every registered target (the
/// shared `ZOOM_STOP`); the target itself is observable on the camera session
/// and in response routing.
fn echo_camera() -> FakeCamera {
    FakeCamera::new(|write, answer| {
        let reply = 0x80 | (write.first().copied().unwrap_or(0x81) & 0x0f) << 4;
        answer.reply(vec![reply, 0x41, 0xff, reply, 0x51, 0xff]);
    })
}

#[cfg(feature = "blocking")]
mod blocking_single_camera {
    use std::time::Duration;

    use grafton_visca::{
        blocking::{CameraConfig, CameraSession, Connect},
        profiles::PtzOpticsG2,
        CameraId, Error,
    };

    use super::{echo_camera, FakeCamera, ZOOM_STOP};

    /// Whether the one wire built from `camera` has been dropped.
    fn transport_dropped(camera: &FakeCamera) -> bool {
        camera.wires_dropped() == 1
    }

    #[test]
    fn the_profile_is_named_once_and_the_view_needs_no_projection() {
        let fake = echo_camera();

        // `PtzOpticsG2` appears exactly once, on the configuration. The camera
        // view below is bound to it by construction: no turbofish, no `?`.
        let camera = CameraSession::open(fake.blocking_wire(), &CameraConfig::<PtzOpticsG2>::new())
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

        assert_eq!(fake.writes(), vec![ZOOM_STOP.to_vec()]);

        camera.close().expect("explicit teardown");
        assert!(
            transport_dropped(&fake),
            "closing the owned camera must drop its transport"
        );
    }

    #[test]
    fn the_configured_form_keeps_camera_id_and_timing_policy() {
        let fake = echo_camera();
        let config = CameraConfig::<PtzOpticsG2>::new()
            .try_camera_id(3)
            .expect("camera 3 is addressable");

        let camera = CameraSession::open(fake.blocking_wire(), &config)
            .expect("single-camera owner session");

        assert_eq!(camera.target(), CameraId::new(3).expect("camera 3"));
        camera
            .camera()
            .zoom()
            .stop()
            .expect("zoom stop admission")
            .applied()
            .expect("zoom stop application");
        assert_eq!(fake.writes(), vec![ZOOM_STOP.to_vec()]);

        camera.close().expect("explicit teardown");
    }

    #[test]
    fn shutdown_is_observable_and_drop_alone_tears_the_session_down() {
        let fake = echo_camera();
        let camera = CameraSession::open(fake.blocking_wire(), &CameraConfig::<PtzOpticsG2>::new())
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
        // goes with it. The last handle only signals the owner worker (#780),
        // which drops the transport as it exits.
        drop(camera);
        for _ in 0..100 {
            if transport_dropped(&fake) {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("dropping the owned camera must end the owner and its transport");
    }

    #[test]
    fn the_session_view_still_selects_the_same_owner() {
        let fake = echo_camera();
        let camera = CameraSession::open(fake.blocking_wire(), &CameraConfig::<PtzOpticsG2>::new())
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
        let error = Connect::open_tcp::<PtzOpticsG2>("invalid:address:format")
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
        sync::{Arc, Mutex},
        time::Duration,
    };

    use grafton_visca::{
        camera::{CameraConfig, Connect},
        profiles::PtzOpticsG2,
        runtime::Runtime,
        transport::{AddressingMode, TransportConfig},
        CameraId, CameraSession, Error, Executor,
    };

    use super::{echo_camera, fake_camera::AsyncWire, FakeCamera, ZOOM_STOP};

    /// The echo camera's transport: IP addressing, datagram sends.
    fn echo_wire(camera: &FakeCamera) -> AsyncWire {
        camera.async_wire().with_addressing(AddressingMode::Ip)
    }

    /// Whether the one wire built from `camera` has been dropped.
    fn transport_dropped(camera: &FakeCamera) -> bool {
        camera.wires_dropped() == 1
    }

    // Local fake: a runtime whose connector hands out the echo transport, so
    // the standard `Connect::open_tcp` path can be driven without a socket.
    #[derive(Clone, Debug)]
    struct EchoRuntime<E> {
        inner: E,
        endpoints: Arc<Mutex<Vec<String>>>,
        transport: Arc<Mutex<Option<AsyncWire>>>,
    }

    impl<E> EchoRuntime<E> {
        fn new(inner: E, transport: AsyncWire) -> Self {
            Self {
                inner,
                endpoints: Arc::new(Mutex::new(Vec::new())),
                transport: Arc::new(Mutex::new(Some(transport))),
            }
        }

        fn endpoints(&self) -> Vec<String> {
            self.endpoints.lock().expect("endpoints lock").clone()
        }

        fn take_transport(&self, address: &str) -> Result<AsyncWire, Error> {
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
        type TcpTransport = AsyncWire;
        type UdpTransport = AsyncWire;
        #[cfg(feature = "transport-serial-tokio")]
        type SerialTransport = std::convert::Infallible;

        #[allow(clippy::manual_async_fn)]
        fn connect_tcp<'a>(
            &'a self,
            address: &'a str,
            _config: TransportConfig,
        ) -> impl Future<Output = Result<Self::TcpTransport, Error>> + Send + 'a {
            async move { self.take_transport(address) }
        }

        #[allow(clippy::manual_async_fn)]
        fn connect_udp<'a>(
            &'a self,
            address: &'a str,
            _config: TransportConfig,
        ) -> impl Future<Output = Result<Self::UdpTransport, Error>> + Send + 'a {
            async move { self.take_transport(address) }
        }
    }

    /// The one-line standard constructor names the profile once and returns a
    /// camera that owns its session.
    async fn standard_constructor_owns_its_session<E: Executor>(inner: E) {
        let fake = echo_camera();
        let runtime = EchoRuntime::new(inner, echo_wire(&fake));

        let camera = Connect::open_tcp::<PtzOpticsG2, _>("camera.local", runtime.clone())
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
        assert_eq!(fake.writes(), vec![ZOOM_STOP.to_vec()]);

        camera.close().await.expect("explicit teardown");
    }

    /// The configured form is the same bind with the caller's own transport.
    async fn configured_constructor_binds_the_profile_once<E: Executor>(executor: E) {
        let fake = echo_camera();
        let config = CameraConfig::<PtzOpticsG2>::new()
            .try_camera_id(3)
            .expect("camera 3 is addressable");

        let camera = CameraSession::open(echo_wire(&fake), &config, executor)
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
        assert_eq!(fake.writes(), vec![ZOOM_STOP.to_vec()]);

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
        assert_eq!(fake.writes().len(), 2);
    }

    /// Shutdown is observable through the owned camera, and dropping the value
    /// is a complete teardown.
    async fn shutdown_and_drop_tear_the_session_down<E: Executor>(executor: E) {
        let fake = echo_camera();
        let camera = CameraSession::open(
            echo_wire(&fake),
            &CameraConfig::<PtzOpticsG2>::new(),
            executor.clone(),
        )
        .await
        .expect("single-camera owner session");

        camera.shutdown().expect("owner shutdown");
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
            if transport_dropped(&fake) {
                return;
            }
            executor.sleep(Duration::from_millis(5)).await;
        }
        panic!("dropping the owned camera must end the owner and its transport");
    }

    runtime_matrix!(
        standard_constructor_owns_its_session,
        configured_constructor_binds_the_profile_once,
        shutdown_and_drop_tear_the_session_down,
    );
}
