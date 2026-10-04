//! A typed STOP is admitted while ordinary admission is saturated (D26, #778).
//!
//! `SessionConfig::admission_capacity` bounds ordinary requests. Each
//! registered camera also reserves one admission slot per typed STOP its
//! profile supports, which only an urgent typed STOP may use. These tests
//! saturate a capacity-1 session with a running zoom and show that a second
//! ordinary request is refused while the STOP is admitted, written, and
//! applied.

#![allow(clippy::expect_used)]

#[cfg(any(
    feature = "blocking",
    all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]
#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

#[cfg(any(
    feature = "blocking",
    all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]
mod wire {
    pub const ZOOM_TELE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xff];
    pub const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
    pub const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
    pub const ACK_SOCKET_TWO: &[u8] = &[0x90, 0x42, 0xff];
    pub const COMPLETE_SOCKET_TWO: &[u8] = &[0x90, 0x52, 0xff];
}

#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
mod asynchronous {
    use std::{
        future::Future,
        num::NonZeroUsize,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
        time::{Duration, Instant},
    };

    use grafton_visca::{
        completion::AppliedOnly,
        profile::ProfileSpec,
        request::builtin::{ZoomDrive, ZoomStop},
        transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
        Error, Executor, Session, SessionConfig,
    };

    use super::{profile_fixtures::NonDefaultCompileTimeProfile as Raw, wire::*};

    #[derive(Debug)]
    struct ScriptedTransport {
        config: TransportConfig,
        responses: flume::Receiver<Vec<u8>>,
        probe: Probe,
    }

    #[derive(Clone, Debug)]
    struct Probe {
        response_tx: flume::Sender<Vec<u8>>,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
        reads: Arc<AtomicUsize>,
    }

    impl Probe {
        fn push(&self, bytes: &[u8]) {
            self.response_tx
                .send(bytes.to_vec())
                .expect("owner response channel remains connected");
        }

        fn writes(&self) -> Vec<Vec<u8>> {
            self.writes.lock().expect("writes lock").clone()
        }

        async fn await_writes<E: Executor>(&self, executor: &E, count: usize) {
            let deadline = Instant::now() + Duration::from_secs(5);
            while self.writes().len() < count {
                assert!(
                    Instant::now() < deadline,
                    "timed out awaiting {count} writes"
                );
                executor.sleep(Duration::from_millis(1)).await;
            }
        }

        async fn await_reads<E: Executor>(&self, executor: &E, count: usize) {
            let deadline = Instant::now() + Duration::from_secs(5);
            while self.reads.load(Ordering::Acquire) < count {
                assert!(
                    Instant::now() < deadline,
                    "timed out awaiting {count} reads"
                );
                executor.sleep(Duration::from_millis(1)).await;
            }
        }
    }

    impl HasTransportConfig for ScriptedTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl AsyncTransport for ScriptedTransport {
        fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
            self.probe
                .writes
                .lock()
                .expect("writes lock")
                .push(bytes.to_vec());
            async { Ok(()) }
        }

        async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
            let bytes = self
                .responses
                .recv_async()
                .await
                .map_err(|_| Error::connection_closed(None))?;
            dst[..bytes.len()].copy_from_slice(&bytes);
            self.probe.reads.fetch_add(1, Ordering::AcqRel);
            Ok(bytes.len())
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    async fn saturated_session<E: Executor>(executor: &E) -> (Session, Probe) {
        let (response_tx, responses) = flume::unbounded();
        let probe = Probe {
            response_tx,
            writes: Arc::default(),
            reads: Arc::default(),
        };
        let transport = ScriptedTransport {
            config: TransportConfig::default(),
            responses,
            probe: probe.clone(),
        };
        let config = SessionConfig::new(
            ProfileSpec::from_compile_time::<Raw>().expect("raw three-axis profile"),
        )
        .with_admission_capacity(NonZeroUsize::MIN);
        let session = Session::open(transport, config, executor.clone())
            .await
            .expect("owner session");
        (session, probe)
    }

    async fn stop_passes_a_saturated_session<E: Executor>(executor: E) {
        let (session, probe) = saturated_session(&executor).await;
        let camera = session.camera::<Raw>().expect("raw camera");

        // The running zoom holds the session's only ordinary slot.
        let zoom = camera
            .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
            .await
            .expect("ordinary zoom admitted");
        probe.await_writes(&executor, 1).await;
        probe.push(ACK_SOCKET_ONE);
        probe.await_reads(&executor, 1).await;

        let refused = camera
            .submit::<AppliedOnly, _>(&ZoomDrive::Wide)
            .await
            .expect_err("ordinary admission is full");
        assert!(matches!(
            refused,
            Error::RuntimeQueueFull { capacity: 1, .. }
        ));

        let mut stop = camera
            .submit::<AppliedOnly, _>(&ZoomStop)
            .await
            .expect("the typed STOP uses camera 1's control reserve");
        probe.await_writes(&executor, 2).await;
        probe.push(ACK_SOCKET_TWO);
        probe.push(COMPLETE_SOCKET_TWO);
        stop.applied().await.expect("the STOP applies");
        assert_eq!(probe.writes(), vec![ZOOM_TELE.to_vec(), ZOOM_STOP.to_vec()]);

        let metrics = session.metrics().await.expect("metrics");
        assert_eq!(metrics.control_reserve_admitted, 1);
        assert_eq!(metrics.control_reserve_rejected, 0);
        assert_eq!(metrics.admission_rejected, 1);

        zoom.detach();
        session.shutdown().await.expect("owner shutdown");
    }

    /// The dynamic projection reaches the same reserve.
    #[cfg(feature = "dyn-api")]
    async fn dyn_stop_passes_a_saturated_session<E: Executor>(executor: E) {
        use grafton_visca::dynapi::DynSessionCamera;

        let (session, probe) = saturated_session(&executor).await;
        let camera = DynSessionCamera::from_session(&session).expect("dynamic camera");
        let zoom = camera
            .submit_applied(&ZoomDrive::Tele)
            .await
            .expect("ordinary zoom admitted");
        probe.await_writes(&executor, 1).await;
        probe.push(ACK_SOCKET_ONE);
        probe.await_reads(&executor, 1).await;

        assert!(matches!(
            camera.submit_applied(&ZoomDrive::Wide).await,
            Err(Error::RuntimeQueueFull { capacity: 1, .. })
        ));
        let mut stop = camera
            .submit_applied(&ZoomStop)
            .await
            .expect("the typed STOP uses camera 1's control reserve");
        probe.await_writes(&executor, 2).await;
        probe.push(ACK_SOCKET_TWO);
        probe.push(COMPLETE_SOCKET_TWO);
        stop.applied().await.expect("the STOP applies");

        zoom.detach();
        session.shutdown().await.expect("owner shutdown");
    }

    async fn contract<E: Executor>(executor: E) {
        stop_passes_a_saturated_session(executor.clone()).await;
        #[cfg(feature = "dyn-api")]
        dyn_stop_passes_a_saturated_session(executor).await;
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_stop_passes_a_saturated_session() {
        contract(grafton_visca::TokioRuntime::from_current().expect("Tokio runtime")).await;
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_stop_passes_a_saturated_session() {
        smol::block_on(contract(grafton_visca::SmolRuntime::new()));
    }
}

#[cfg(feature = "blocking")]
mod blocking {
    use std::{
        collections::VecDeque,
        num::NonZeroUsize,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use grafton_visca::{
        blocking::{Session, SessionConfig},
        command::CommandKind,
        completion::AppliedOnly,
        profile::ProfileSpec,
        request::builtin::{ZoomDrive, ZoomStop},
        transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
        Error,
    };

    use super::{profile_fixtures::NonDefaultCompileTimeProfile as Raw, wire::*};

    #[derive(Debug)]
    struct ScriptedTransport {
        config: TransportConfig,
        responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl HasTransportConfig for ScriptedTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl BlockingTransport for ScriptedTransport {
        fn send_with_timeout(
            &mut self,
            bytes: &[u8],
            _kind: CommandKind,
            _timeout: Duration,
        ) -> Result<(), Error> {
            self.writes
                .lock()
                .expect("writes lock")
                .push(bytes.to_vec());
            Ok(())
        }

        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            timeout: Duration,
        ) -> Result<usize, Error> {
            let next = self.responses.lock().expect("responses lock").pop_front();
            let Some(bytes) = next else {
                std::thread::sleep(timeout.min(Duration::from_millis(1)));
                return Err(Error::io_timeout());
            };
            dst[..bytes.len()].copy_from_slice(&bytes);
            Ok(bytes.len())
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    #[test]
    fn blocking_stop_passes_a_saturated_session() {
        let responses = Arc::new(Mutex::new(VecDeque::new()));
        let writes = Arc::new(Mutex::new(Vec::new()));
        let push = |bytes: &[u8]| {
            responses
                .lock()
                .expect("responses lock")
                .push_back(bytes.to_vec());
        };
        let transport = ScriptedTransport {
            config: TransportConfig::default(),
            responses: Arc::clone(&responses),
            writes: Arc::clone(&writes),
        };
        let config = SessionConfig::new(
            ProfileSpec::from_compile_time::<Raw>().expect("raw three-axis profile"),
        )
        .with_admission_capacity(NonZeroUsize::MIN);
        let session = Session::open(transport, config).expect("owner session");
        let camera = session.camera::<Raw>().expect("raw camera");

        // The running zoom holds the session's only ordinary slot. A short
        // wait pumps its ACK, so the zoom is executing on socket one before
        // the STOP is written.
        let mut zoom = camera
            .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
            .expect("ordinary zoom admitted");
        push(ACK_SOCKET_ONE);
        assert!(matches!(
            zoom.applied_with_timeout(Duration::from_millis(20)),
            Err(Error::ObservationTimeout { .. })
        ));

        let refused = camera
            .submit::<AppliedOnly, _>(&ZoomDrive::Wide)
            .expect_err("ordinary admission is full");
        assert!(matches!(
            refused,
            Error::RuntimeQueueFull { capacity: 1, .. }
        ));

        let mut stop = camera
            .submit::<AppliedOnly, _>(&ZoomStop)
            .expect("the typed STOP uses camera 1's control reserve");
        push(ACK_SOCKET_TWO);
        push(COMPLETE_SOCKET_TWO);
        stop.applied().expect("the STOP applies");
        assert_eq!(
            writes.lock().expect("writes lock").clone(),
            vec![ZOOM_TELE.to_vec(), ZOOM_STOP.to_vec()]
        );

        let metrics = session.metrics().expect("metrics");
        assert_eq!(metrics.control_reserve_admitted, 1);
        assert_eq!(metrics.admission_rejected, 1);

        zoom.detach();
        drop(stop);
        session.shutdown().expect("owner shutdown");
    }
}
