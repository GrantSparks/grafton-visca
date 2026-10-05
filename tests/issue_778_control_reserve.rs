//! A typed STOP is admitted while ordinary admission is saturated (D26, #778).
//!
//! `SessionConfig::admission_capacity` bounds ordinary requests. Each
//! registered camera also reserves one admission slot per typed STOP its
//! profile supports, which only an urgent typed STOP may use. These tests
//! saturate a capacity-1 session with a running zoom and show that a second
//! ordinary request is refused while the STOP is admitted, written, and
//! applied.
//!
//! The zoom must be acknowledged before the STOP is submitted. A raw STOP that
//! crosses a still-unacknowledged command creates the documented two-candidate
//! state (#714), in which neither ACK binds and the STOP may end
//! `UnsequencedCommandUnconfirmed`. Each test therefore waits until the owner
//! has read the zoom's ACK, never for an elapsed time.

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
mod scenario {
    use std::{num::NonZeroUsize, time::Duration};

    use grafton_visca::{profile::ProfileSpec, OperationalTuning, SessionConfig};

    pub use super::profile_fixtures::NonDefaultCompileTimeProfile as Raw;

    pub const ZOOM_TELE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xff];
    pub const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff];
    pub const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
    pub const ACK_SOCKET_TWO: &[u8] = &[0x90, 0x42, 0xff];
    pub const COMPLETE_SOCKET_TWO: &[u8] = &[0x90, 0x52, 0xff];

    /// How long a scripted reply may take to reach the owner and how long a
    /// test waits for an owner write or read before failing.
    pub const OWNER_PROGRESS_BOUND: Duration = Duration::from_secs(5);

    /// A capacity-1 session for the raw three-axis fixture.
    ///
    /// Every scripted ACK is pushed only after the owner has written its
    /// frame, so the test never races the camera. The fixture's 100 ms ACK
    /// deadline would still race the owner's scheduling on a loaded host; the
    /// deadline is not what these tests exercise, so it is widened to
    /// [`OWNER_PROGRESS_BOUND`].
    pub fn saturated_config() -> SessionConfig {
        SessionConfig::new(ProfileSpec::from_compile_time::<Raw>().expect("raw three-axis profile"))
            .with_admission_capacity(NonZeroUsize::MIN)
            .with_tuning(OperationalTuning::new().ack_timeout(OWNER_PROGRESS_BOUND))
    }
}

#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
mod asynchronous {
    use std::{
        future::Future,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
        time::{Duration, Instant},
    };

    use grafton_visca::{
        completion::AppliedOnly,
        request::builtin::{ZoomDrive, ZoomStop},
        transport::{
            AsyncTransport, HasTransportConfig, ReceiveOutcome, SendSemantics, TransportConfig,
        },
        Error, Executor, Session,
    };

    use super::scenario::*;

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
            let deadline = Instant::now() + OWNER_PROGRESS_BOUND;
            while self.writes().len() < count {
                assert!(
                    Instant::now() < deadline,
                    "timed out awaiting {count} writes"
                );
                executor.sleep(Duration::from_millis(1)).await;
            }
        }

        /// Waits until the owner has read `count` scripted frames. The actor
        /// applies a frame in the turn that read it, so once this returns the
        /// frame's correlation precedes any later boundary request.
        async fn await_reads<E: Executor>(&self, executor: &E, count: usize) {
            let deadline = Instant::now() + OWNER_PROGRESS_BOUND;
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

        async fn recv_into(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome, Error> {
            let bytes = self
                .responses
                .recv_async()
                .await
                .map_err(|_| Error::connection_closed(None))?;
            self.probe.reads.fetch_add(1, Ordering::AcqRel);
            Ok(ReceiveOutcome::copy_message(&bytes, dst))
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
        let session = Session::open(transport, saturated_config(), executor.clone())
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
        session.shutdown().expect("owner shutdown");
    }

    /// The dynamic projection reaches the same reserve.
    #[cfg(feature = "dyn-api")]
    async fn dyn_stop_passes_a_saturated_session<E: Executor>(executor: E) {
        let (session, probe) = saturated_session(&executor).await;
        let camera = session.camera_dyn().expect("dynamic camera");
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
        session.shutdown().expect("owner shutdown");
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
        sync::{Arc, Condvar, Mutex, MutexGuard},
        time::Duration,
    };

    use grafton_visca::{
        blocking::Session,
        command::CommandKind,
        completion::AppliedOnly,
        request::builtin::{ZoomDrive, ZoomStop},
        transport::{
            BlockingTransport, HasTransportConfig, ReceiveOutcome, SendSemantics, TransportConfig,
        },
        Error,
    };

    use super::scenario::*;

    /// The scripted camera's side of the wire, shared with the test thread.
    ///
    /// Submission returns at admission and the owner worker writes and reads
    /// afterwards, so the test waits here for the owner's progress.
    #[derive(Debug, Default)]
    struct Wire {
        state: Mutex<WireState>,
        changed: Condvar,
    }

    #[derive(Debug, Default)]
    struct WireState {
        responses: VecDeque<Vec<u8>>,
        writes: Vec<Vec<u8>>,
        reads: usize,
    }

    impl Wire {
        fn lock(&self) -> MutexGuard<'_, WireState> {
            self.state.lock().expect("wire lock")
        }

        fn update(&self, change: impl FnOnce(&mut WireState)) {
            change(&mut self.lock());
            self.changed.notify_all();
        }

        fn push(&self, bytes: &[u8]) {
            self.update(|state| state.responses.push_back(bytes.to_vec()));
        }

        fn writes(&self) -> Vec<Vec<u8>> {
            self.lock().writes.clone()
        }

        /// Waits, bounded, until `ready` holds for the wire state.
        fn await_owner(&self, what: &str, ready: impl Fn(&WireState) -> bool) {
            let (_state, timeout) = self
                .changed
                .wait_timeout_while(self.lock(), OWNER_PROGRESS_BOUND, |state| !ready(state))
                .expect("wire lock");
            assert!(!timeout.timed_out(), "timed out awaiting {what}");
        }

        fn await_writes(&self, count: usize) {
            self.await_owner("owner writes", |state| state.writes.len() >= count);
        }

        /// Waits until the owner worker has read `count` scripted frames. The
        /// worker applies a frame in the turn that read it, before it serves
        /// another boundary request, so once this returns the frame's
        /// correlation precedes any later submission.
        fn await_reads(&self, count: usize) {
            self.await_owner("owner reads", |state| state.reads >= count);
        }
    }

    #[derive(Debug)]
    struct ScriptedTransport {
        config: TransportConfig,
        wire: Arc<Wire>,
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
            self.wire.update(|state| state.writes.push(bytes.to_vec()));
            Ok(())
        }

        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            timeout: Duration,
        ) -> Result<ReceiveOutcome, Error> {
            let (mut state, _) = self
                .wire
                .changed
                .wait_timeout_while(self.wire.lock(), timeout, |state| {
                    state.responses.is_empty()
                })
                .expect("wire lock");
            let Some(bytes) = state.responses.pop_front() else {
                return Err(Error::io_timeout());
            };
            state.reads += 1;
            drop(state);
            self.wire.changed.notify_all();
            Ok(ReceiveOutcome::copy_message(&bytes, dst))
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    #[test]
    fn blocking_stop_passes_a_saturated_session() {
        let wire = Arc::new(Wire::default());
        let transport = ScriptedTransport {
            config: TransportConfig::default(),
            wire: Arc::clone(&wire),
        };
        let session = Session::open(transport, saturated_config()).expect("owner session");
        let camera = session.camera::<Raw>().expect("raw camera");

        // The running zoom holds the session's only ordinary slot. Its ACK is
        // answered after the owner writes it and read before the STOP is
        // submitted, so the zoom is executing on socket one when the STOP is
        // written.
        let mut zoom = camera
            .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
            .expect("ordinary zoom admitted");
        wire.await_writes(1);
        wire.push(ACK_SOCKET_ONE);
        wire.await_reads(1);
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
        wire.await_writes(2);
        wire.push(ACK_SOCKET_TWO);
        wire.push(COMPLETE_SOCKET_TWO);
        stop.applied().expect("the STOP applies");
        assert_eq!(wire.writes(), vec![ZOOM_TELE.to_vec(), ZOOM_STOP.to_vec()]);

        let metrics = session.metrics().expect("metrics");
        assert_eq!(metrics.control_reserve_admitted, 1);
        assert_eq!(metrics.admission_rejected, 1);

        zoom.detach();
        drop(stop);
        session.shutdown().expect("owner shutdown");
    }
}
