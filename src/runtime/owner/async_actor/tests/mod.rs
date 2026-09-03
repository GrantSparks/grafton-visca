#![allow(
    clippy::await_holding_lock,
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    unused_qualifications
)]

use std::sync::{atomic::Ordering, Mutex};

use crate::runtime::Runtime;
use crate::{
    runtime::engine::{
        CancellationPolicy, ControlPolicy, DecodedResponse, EncodedMessage, EnvelopeKind,
        InquiryRoute, ProtocolPolicy, ReplyShape, RequestContext, RetryPolicy, RuntimeRequest,
        TargetPolicy, TimeoutPolicy, TransportKind,
    },
    CameraId, ViscaSocket,
};

#[cfg(feature = "runtime-tokio")]
use crate::runtime::engine::{EnvelopeSequence, SequenceWidth};

#[cfg(feature = "runtime-tokio")]
use crate::runtime::engine::CancellationObservation;
#[cfg(feature = "runtime-tokio")]
use crate::runtime::owner::{canonical_owner_trace, CANONICAL_OWNER_TRACE};
#[cfg(feature = "runtime-smol")]
use crate::runtime::SmolRuntime;
#[cfg(feature = "runtime-tokio")]
use crate::runtime::TokioRuntime;

use super::*;

mod fairness;
mod faults;
#[cfg(feature = "runtime-tokio")]
mod lifecycle;
mod receipts;
#[cfg(feature = "runtime-tokio")]
mod release_boundary;

#[cfg(feature = "runtime-tokio")]
#[derive(Clone)]
struct ManualRuntime {
    executor: TokioRuntime,
    now: Arc<Mutex<Instant>>,
    sleep_wakers: Arc<Mutex<Vec<std::task::Waker>>>,
    sleep_started: Option<flume::Sender<Duration>>,
    advance_after_next_now: Arc<Mutex<Option<Duration>>>,
    polling_sleeps: bool,
}

#[cfg(feature = "runtime-tokio")]
impl ManualRuntime {
    fn new(now: Instant) -> Self {
        Self::with_sleep_behavior(now, false)
    }

    /// Makes a timer ready when this manual clock has advanced past it and
    /// its future is polled again. Tests that manually poll an owner use
    /// this to control an already-constructed timer race exactly.
    fn with_polling_sleeps(now: Instant) -> Self {
        Self::with_sleep_behavior(now, true)
    }

    /// Returns a manual clock plus a one-shot-friendly proof that an actor
    /// actually polled a sleep.  Raw-boundary tests use it to inject input
    /// *during* a clamped idle sleep, without trusting a wall-clock delay.
    fn with_polling_sleeps_and_sleep_barrier(now: Instant) -> (Self, flume::Receiver<Duration>) {
        let mut runtime = Self::with_polling_sleeps(now);
        let (sleep_started, observed) = flume::unbounded();
        runtime.sleep_started = Some(sleep_started);
        (runtime, observed)
    }

    fn with_sleep_behavior(now: Instant, polling_sleeps: bool) -> Self {
        Self {
            executor: TokioRuntime::from_current().unwrap(),
            now: Arc::new(Mutex::new(now)),
            sleep_wakers: Arc::new(Mutex::new(Vec::new())),
            sleep_started: None,
            advance_after_next_now: Arc::new(Mutex::new(None)),
            polling_sleeps,
        }
    }

    fn advance(&self, duration: Duration) {
        self.advance_silently(duration);
        // A manually advanced deadline must also wake an actor currently
        // parked inside its executor-neutral sleep future.  Tests use this
        // as a deterministic sleep-entered/read-consumed barrier rather
        // than a wall-clock delay.
        let wakers = std::mem::take(&mut *self.sleep_wakers.lock().unwrap());
        for waker in wakers {
            waker.wake();
        }
    }

    /// Advance the virtual clock without waking a parked timer.  A focused
    /// boundary race can then make a receive ready at H and use that
    /// receive's own waker to poll the already-due timer in the same
    /// selection, reproducing the real all-ready linearization exactly.
    fn advance_silently(&self, duration: Duration) {
        let mut now = self.now.lock().unwrap();
        *now = now.checked_add(duration).unwrap();
    }

    /// Make exactly the next clock sample return the current instant, then
    /// advance the virtual clock for the following sample.  This models a
    /// receive completing just before H while the executor resumes the
    /// actor's boundary handler just after H, without a scheduler sleep.
    fn advance_after_next_now(&self, duration: Duration) {
        *self.advance_after_next_now.lock().unwrap() = Some(duration);
    }
}

#[cfg(feature = "runtime-tokio")]
impl crate::executor::Executor for ManualRuntime {
    type Join<T>
        = <TokioRuntime as crate::executor::Executor>::Join<T>
    where
        T: Send + 'static;

    type Detach = <TokioRuntime as crate::executor::Executor>::Detach;

    fn spawn_with_detach<F>(&self, future: F) -> (Self::Join<F::Output>, Self::Detach)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        crate::executor::Executor::spawn_with_detach(&self.executor, future)
    }

    fn block_on<F: Future>(&self, future: F) -> F::Output {
        crate::executor::Executor::block_on(&self.executor, future)
    }

    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
        let deadline = {
            let now = *self.now.lock().unwrap();
            now.checked_add(duration).unwrap_or(now)
        };
        let now = Arc::clone(&self.now);
        let sleep_wakers = Arc::clone(&self.sleep_wakers);
        let sleep_started = self.sleep_started.clone();
        let polling_sleeps = self.polling_sleeps;
        let mut announced = false;
        std::future::poll_fn(move |context| {
            if !announced {
                if let Some(observed) = sleep_started.as_ref() {
                    let _ = observed.try_send(duration);
                }
                announced = true;
            }
            if polling_sleeps && *now.lock().unwrap() >= deadline {
                std::task::Poll::Ready(())
            } else {
                sleep_wakers.lock().unwrap().push(context.waker().clone());
                std::task::Poll::Pending
            }
        })
    }

    fn timeout<'a, F, T>(
        &'a self,
        duration: Duration,
        future: F,
    ) -> impl Future<Output = Result<T, Error>> + Send + 'a
    where
        F: Future<Output = T> + Send + 'a,
        T: Send + 'a,
    {
        crate::executor::Executor::timeout(&self.executor, duration, future)
    }

    fn now(&self) -> Instant {
        let mut now = self.now.lock().unwrap();
        let observed = *now;
        if let Some(duration) = self.advance_after_next_now.lock().unwrap().take() {
            *now = now.checked_add(duration).unwrap();
        }
        observed
    }
}

#[cfg(feature = "runtime-tokio")]
impl Runtime for ManualRuntime {
    type TcpTransport = <TokioRuntime as Runtime>::TcpTransport;
    type UdpTransport = <TokioRuntime as Runtime>::UdpTransport;
    #[cfg(feature = "transport-serial-tokio")]
    type SerialTransport = std::convert::Infallible;

    async fn connect_tcp(
        &self,
        _addr: &str,
        _cfg: crate::transport::builder::TransportConfig,
    ) -> Result<Self::TcpTransport, Error> {
        Err(Error::NotSupported)
    }

    async fn connect_udp(
        &self,
        _addr: &str,
        _cfg: crate::transport::builder::TransportConfig,
    ) -> Result<Self::UdpTransport, Error> {
        Err(Error::NotSupported)
    }

    fn now(&self) -> Instant {
        crate::executor::Executor::now(self)
    }
}

type RecordedWrites = Arc<Mutex<Vec<(RequestId, Vec<u8>, bool)>>>;

fn policy(capacity: usize) -> OwnerPolicy {
    OwnerPolicy::single_target(
        ProtocolPolicy {
            capacity,
            envelope: EnvelopeKind::Raw,
            transport: TransportKind::Datagram,
            inquiry_capacity: capacity,
            command_spacing: Duration::ZERO,
            inquiry_spacing: Duration::ZERO,
            inquiry_cooldown: Duration::ZERO,
            raw_inquiry_release_hold: Duration::from_secs(1),
            raw_release_grace: Duration::from_millis(100),
            strict_unconfirmed_poison: false,
        },
        CameraId::CAMERA_1,
        TargetPolicy {
            command_sockets: 2,
            cancellation: CancellationPolicy::Supported,
        },
    )
    .unwrap()
}

// Sony's envelope sequence is the correlation key that permits more than
// one command to wait for an ACK before any socket has been assigned.
// Keep this separate from the raw policy used by the rest of the actor
// fixtures so each test states which wire contract it exercises.
#[cfg(feature = "runtime-tokio")]
fn sony_policy(capacity: usize) -> OwnerPolicy {
    let mut owner = policy(capacity);
    owner.protocol.envelope = EnvelopeKind::Sony;
    owner
}

// Used only by the runtime-tokio stream tests below; dead on the runtime-smol leg (#636).
#[allow(dead_code)]
fn stream_policy(capacity: usize) -> OwnerPolicy {
    let mut owner = policy(capacity);
    owner.protocol.transport = TransportKind::Stream;
    owner
}

/// Two independently registered raw targets used to make an unrelated
/// write park across target 1's exact ambiguity release.  The regression
/// must exercise `drive()` itself, rather than a synthetic Wake, because
/// only an awaited transmission completion recursively enters the engine
/// between source selections.
#[cfg(feature = "runtime-tokio")]
fn two_target_raw_policy(transport: TransportKind) -> OwnerPolicy {
    let protocol = ProtocolPolicy {
        capacity: 3,
        envelope: EnvelopeKind::Raw,
        transport,
        inquiry_capacity: 1,
        command_spacing: Duration::ZERO,
        inquiry_spacing: Duration::ZERO,
        inquiry_cooldown: Duration::ZERO,
        raw_inquiry_release_hold: Duration::from_secs(1),
        raw_release_grace: Duration::from_millis(100),
        strict_unconfirmed_poison: false,
    };
    let target = TargetPolicy {
        command_sockets: 2,
        cancellation: CancellationPolicy::Supported,
    };
    let mut targets = [None; 9];
    targets[usize::from(CameraId::CAMERA_1.id())] = Some(target);
    targets[usize::from(CameraId::CAMERA_2.id())] = Some(target);
    OwnerPolicy::with_targets(protocol, targets).unwrap()
}

fn command() -> RuntimeRequest {
    command_for(CameraId::CAMERA_1)
}

fn command_for(target: CameraId) -> RuntimeRequest {
    RuntimeRequest::Command {
        wire: Arc::new(
            EncodedMessage::new(&[0x80_u8.saturating_add(target.id()), 0x01, 0x04, 0x00, 0xff])
                .unwrap(),
        ),
        context: RequestContext {
            target,
            timeout: TimeoutPolicy {
                ack: Duration::from_secs(5),
                completion: Duration::from_secs(5),
                inquiry: Duration::from_secs(5),
                cancellation: Duration::from_secs(1),
                ambiguity: Duration::from_secs(1),
            },
            retry: RetryPolicy::NEVER,
            control: ControlPolicy::default(),
            cancellation: CancellationPolicy::Supported,
            reply_shape: ReplyShape::AckThenCompletion,
        },
        applied_state: None,
    }
}

/// A command whose deadlines are all short, so a liveness test that leaves it
/// in flight settles quickly on the fix's success path (#675).
fn command_with_short_deadlines() -> RuntimeRequest {
    RuntimeRequest::Command {
        wire: Arc::new(EncodedMessage::new(&[0x81, 0x01, 0x04, 0x00, 0xff]).unwrap()),
        context: RequestContext {
            target: CameraId::CAMERA_1,
            timeout: TimeoutPolicy {
                ack: Duration::from_millis(100),
                completion: Duration::from_millis(100),
                inquiry: Duration::from_millis(100),
                cancellation: Duration::from_millis(100),
                ambiguity: Duration::from_millis(100),
            },
            retry: RetryPolicy::NEVER,
            control: ControlPolicy::default(),
            cancellation: CancellationPolicy::Supported,
            reply_shape: ReplyShape::AckThenCompletion,
        },
        applied_state: None,
    }
}

/// A raw command with an individually chosen completion deadline and a
/// wire marker visible in the production transport script.  The marker is
/// deliberately carried in the otherwise inert fixture wire so assertions
/// can prove a successor did not write before the retained frame settled.
#[cfg(feature = "runtime-tokio")]
fn raw_command_with_completion_deadline(marker: u8, completion: Duration) -> RuntimeRequest {
    RuntimeRequest::Command {
        wire: Arc::new(EncodedMessage::new(&[0x81, 0x01, 0x04, marker, 0xff]).unwrap()),
        context: RequestContext {
            target: CameraId::CAMERA_1,
            timeout: TimeoutPolicy {
                ack: Duration::from_secs(5),
                completion,
                inquiry: Duration::from_secs(5),
                cancellation: Duration::from_secs(1),
                ambiguity: Duration::from_secs(1),
            },
            retry: RetryPolicy::NEVER,
            control: ControlPolicy::default(),
            cancellation: CancellationPolicy::Supported,
            reply_shape: ReplyShape::AckThenCompletion,
        },
        applied_state: None,
    }
}

fn inquiry() -> RuntimeRequest {
    inquiry_for(CameraId::CAMERA_1)
}

/// An inquiry whose successful write immediately reaches its response
/// deadline. Boundary tests use this to create the genuine late-reply
/// hold that remains after timeout (#712).
#[cfg(feature = "runtime-tokio")]
fn timed_out_inquiry() -> RuntimeRequest {
    let mut request = inquiry();
    let RuntimeRequest::Inquiry { context, .. } = &mut request else {
        unreachable!("inquiry helper always constructs an inquiry");
    };
    context.timeout.inquiry = Duration::ZERO;
    request
}

fn inquiry_for(target: CameraId) -> RuntimeRequest {
    RuntimeRequest::Inquiry {
        wire: Arc::new(
            EncodedMessage::new(&[0x80_u8.saturating_add(target.id()), 0x09, 0x04, 0x00, 0xff])
                .unwrap(),
        ),
        context: RequestContext {
            target,
            timeout: TimeoutPolicy {
                ack: Duration::from_secs(5),
                completion: Duration::from_secs(5),
                inquiry: Duration::from_secs(5),
                cancellation: Duration::from_secs(1),
                ambiguity: Duration::from_secs(1),
            },
            retry: RetryPolicy::NEVER,
            control: ControlPolicy::default(),
            cancellation: CancellationPolicy::Supported,
            reply_shape: ReplyShape::AckThenCompletion,
        },
        route: InquiryRoute::UNKNOWN,
    }
}

/// Wrap decoded frames as one nonzero-length read for the fake driver.
fn batch(frames: Vec<DecodedFrame>) -> Result<AsyncReceive, Error> {
    Ok(AsyncReceive::Frames(frames))
}

fn ack(socket: ViscaSocket) -> DecodedFrame {
    DecodedFrame {
        target: CameraId::CAMERA_1,
        sequence: None,
        response: DecodedResponse::Ack {
            socket: Some(socket),
        },
    }
}

fn completion(socket: ViscaSocket) -> DecodedFrame {
    DecodedFrame {
        target: CameraId::CAMERA_1,
        sequence: None,
        response: DecodedResponse::Completion {
            socket: Some(socket),
        },
    }
}

#[cfg(feature = "runtime-tokio")]
fn sequenced(sequence: u32, response: DecodedResponse) -> DecodedFrame {
    DecodedFrame {
        target: CameraId::CAMERA_1,
        sequence: Some(EnvelopeSequence {
            value: sequence,
            width: SequenceWidth::Full32,
        }),
        response,
    }
}

#[derive(Debug)]
struct FakeAsyncDriver {
    writes: RecordedWrites,
    started: flume::Sender<RequestId>,
    gates: flume::Receiver<Result<TransmissionMeta, Error>>,
    frames: flume::Receiver<Result<AsyncReceive, Error>>,
}

impl AsyncOwnerDriver for FakeAsyncDriver {
    fn write(
        &mut self,
        write: WireWrite<'_>,
    ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
        let id = write.request;
        self.writes
            .lock()
            .unwrap()
            .push((write.request, write.bytes.to_vec(), write.cancellation));
        let started = self.started.clone();
        let gates = self.gates.clone();
        async move {
            let _ = started.send_async(id).await;
            gates
                .recv_async()
                .await
                .unwrap_or(Err(Error::RuntimeShutdown))
        }
    }

    fn receive(
        &mut self,
        _buffers: &mut super::super::OwnerBuffers,
        _frame_limit: usize,
    ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
        let frames = self.frames.clone();
        async move {
            frames
                .recv_async()
                .await
                .unwrap_or(Err(Error::RuntimeShutdown))
        }
    }
}

#[cfg(feature = "runtime-tokio")]
#[derive(Debug)]
struct PanickingReceiveDriver;

/// A driver that reports the Tokio runtime executing the real owner task
/// before ending the session.
#[cfg(feature = "runtime-tokio")]
#[derive(Debug)]
struct RuntimeAffinityDriver {
    observed: flume::Sender<tokio::runtime::Id>,
}

#[cfg(feature = "runtime-tokio")]
impl AsyncOwnerDriver for RuntimeAffinityDriver {
    #[allow(clippy::manual_async_fn)]
    fn write(
        &mut self,
        _write: WireWrite<'_>,
    ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
        async { Ok(TransmissionMeta { sequence: None }) }
    }

    fn receive(
        &mut self,
        _buffers: &mut super::super::OwnerBuffers,
        _frame_limit: usize,
    ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
        let observed = self.observed.clone();
        async move {
            let _ = observed.try_send(tokio::runtime::Handle::current().id());
            Ok(AsyncReceive::Closed)
        }
    }
}

#[cfg(feature = "runtime-tokio")]
impl AsyncOwnerDriver for PanickingReceiveDriver {
    #[allow(clippy::manual_async_fn)]
    fn write(
        &mut self,
        _write: WireWrite<'_>,
    ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
        async { Ok(TransmissionMeta { sequence: None }) }
    }

    #[allow(clippy::manual_async_fn)]
    fn receive(
        &mut self,
        _buffers: &mut super::super::OwnerBuffers,
        _frame_limit: usize,
    ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
        async {
            panic!("test driver panic before terminal publication");
        }
    }
}

#[cfg(feature = "runtime-tokio")]
#[derive(Debug)]
struct PanickingAfterAdmissionDriver {
    panic_signal: flume::Receiver<()>,
}

#[cfg(feature = "runtime-tokio")]
impl AsyncOwnerDriver for PanickingAfterAdmissionDriver {
    #[allow(clippy::manual_async_fn)]
    fn write(
        &mut self,
        _write: WireWrite<'_>,
    ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
        async { Ok(TransmissionMeta { sequence: None }) }
    }

    #[allow(clippy::manual_async_fn)]
    fn receive(
        &mut self,
        _buffers: &mut super::super::OwnerBuffers,
        _frame_limit: usize,
    ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
        let panic_signal = self.panic_signal.clone();
        async move {
            panic_signal
                .recv_async()
                .await
                .expect("the panic signal must remain connected");
            panic!("test driver panic after admission");
        }
    }
}

struct Harness {
    driver: FakeAsyncDriver,
    started: flume::Receiver<RequestId>,
    gates: flume::Sender<Result<TransmissionMeta, Error>>,
    frames: flume::Sender<Result<AsyncReceive, Error>>,
    writes: RecordedWrites,
}

fn harness() -> Harness {
    let (started_tx, started) = flume::bounded(8);
    let (gate_tx, gates) = flume::bounded(8);
    let (frame_tx, frames) = flume::bounded(8);
    let writes = Arc::new(Mutex::new(Vec::new()));
    Harness {
        driver: FakeAsyncDriver {
            writes: Arc::clone(&writes),
            started: started_tx,
            gates,
            frames,
        },
        started,
        gates: gate_tx,
        frames: frame_tx,
        writes,
    }
}

fn prepared_focus(profile: &crate::ProfileSpec) -> crate::prepared::PreparedCommand {
    crate::prepared::prepare_command(
        &crate::request::builtin::FocusModeCommand::Manual,
        CameraId::CAMERA_1,
        profile,
        crate::OperationalTuning::new(),
        crate::prepared::ClassSelection::Request,
    )
    .unwrap()
}

fn prepared_zoom(
    profile: &crate::ProfileSpec,
) -> crate::prepared::PreparedOperation<completion::Targeted> {
    crate::prepared::prepare_operation::<completion::Targeted, _>(
        &crate::request::builtin::ZoomTarget::new(crate::types::ZoomPosition::new(0x0100).unwrap()),
        CameraId::CAMERA_1,
        profile,
        crate::OperationalTuning::new(),
        crate::prepared::ClassSelection::Request,
    )
    .unwrap()
}

async fn typed_async_receipt_matrix<R>(runtime: R)
where
    R: Runtime,
{
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(2), runtime.clone()).unwrap();
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let frames = harness.frames.clone();
    let writes = Arc::clone(&harness.writes);
    let client = async {
        let success = handle
            .submit_command(prepared_focus(&profile))
            .await
            .unwrap();
        let _ = started.recv_async().await.unwrap();
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![ack(ViscaSocket::S1)]))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![completion(ViscaSocket::S1)]))
            .await
            .unwrap();
        assert_eq!(handle.snapshot().await.unwrap().active, 0);
        success
            .wait_with_timeout(handle.receipt_control(), Duration::ZERO)
            .await
            .unwrap();

        let failed = handle
            .submit_command(prepared_focus(&profile))
            .await
            .unwrap();
        let _ = started.recv_async().await.unwrap();
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![ack(ViscaSocket::S1)]))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![DecodedFrame {
                target: CameraId::CAMERA_1,
                sequence: None,
                response: DecodedResponse::Error {
                    socket: Some(ViscaSocket::S1),
                    code: 0x02,
                },
            }]))
            .await
            .unwrap();
        assert_eq!(handle.snapshot().await.unwrap().active, 0);
        assert!(matches!(
            failed.wait(handle.receipt_control()).await,
            Err(Error::SyntaxError)
        ));

        let operation = handle
            .submit_operation(prepared_zoom(&profile))
            .await
            .unwrap();
        let _ = started.recv_async().await.unwrap();
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![ack(ViscaSocket::S1)]))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![completion(ViscaSocket::S1)]))
            .await
            .unwrap();
        assert_eq!(handle.snapshot().await.unwrap().active, 0);
        let cancellation = operation.cancel().await.unwrap();
        assert_eq!(
            cancellation
                .outcome(handle.receipt_control(), Duration::ZERO)
                .await
                .unwrap(),
            CancellationOutcome::Completed
        );

        let detached = handle
            .submit_command(prepared_focus(&profile))
            .await
            .unwrap();
        let _ = started.recv_async().await.unwrap();
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        assert!(matches!(
            detached
                .wait_with_timeout(handle.receipt_control(), Duration::ZERO)
                .await,
            Err(Error::Timeout)
        ));
        assert_eq!(
            writes
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, _, cancellation)| *cancellation)
                .count(),
            0,
            "observer timeout never sends cancellation"
        );
        frames
            .send_async(batch(vec![ack(ViscaSocket::S1)]))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![completion(ViscaSocket::S1)]))
            .await
            .unwrap();
        assert_eq!(handle.snapshot().await.unwrap().active, 0);

        handle.shutdown().await.unwrap();
    };
    let ((), snapshot) = future::zip(client, actor.run(harness.driver)).await;
    assert_eq!(snapshot.active, 0);
}

async fn targeted_settlement_matrix<R>(runtime: R)
where
    R: Runtime,
{
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(3), runtime).unwrap();
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let frames = harness.frames.clone();
    let writes = Arc::clone(&harness.writes);
    let client = async {
        let operation = handle
            .submit_operation(prepared_zoom(&profile))
            .await
            .unwrap();
        let _ = started.recv_async().await.unwrap();
        gates
            .send_async(Ok(TransmissionMeta { sequence: None }))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![ack(ViscaSocket::S1)]))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![completion(ViscaSocket::S1)]))
            .await
            .unwrap();
        assert_eq!(handle.snapshot().await.unwrap().active, 0);

        let settlement = operation
            .settled_with_timeout(handle.receipt_control(), Duration::from_secs(1))
            .erase()
            .wait();
        let replies = async {
            for _ in 0..2 {
                let _ = started.recv_async().await.unwrap();
                gates
                    .send_async(Ok(TransmissionMeta { sequence: None }))
                    .await
                    .unwrap();
                frames
                    .send_async(batch(vec![DecodedFrame {
                        target: CameraId::CAMERA_1,
                        sequence: None,
                        response: DecodedResponse::InquiryReply {
                            route: None,
                            payload: smallvec::smallvec![0x0, 0x1, 0x0, 0x0],
                        },
                    }]))
                    .await
                    .unwrap();
            }
        };
        let (settled, ()) = future::zip(settlement, replies).await;
        assert!(settled.is_ok());
        assert_eq!(handle.snapshot().await.unwrap().active, 0);
        let writes = writes.lock().unwrap();
        assert_eq!(writes.len(), 3);
        assert_eq!(writes[1].1, writes[2].1);
        assert_eq!(writes[1].1, vec![0x81, 0x09, 0x04, 0x47, 0xff]);
        drop(writes);
        handle.shutdown().await.unwrap();
    };
    let ((), snapshot) = future::zip(client, actor.run(harness.driver)).await;
    assert_eq!(snapshot.active, 0);
}

/// Byte-stream transport whose reads are handed over one chunk at a time,
/// so a test can split a single VISCA reply across two reads.
#[cfg(feature = "runtime-tokio")]
#[derive(Debug)]
struct ChunkedStreamTransport {
    config: crate::transport::builder::TransportConfig,
    chunks: flume::Receiver<Vec<u8>>,
    sent: flume::Sender<Vec<u8>>,
}

#[cfg(feature = "runtime-tokio")]
impl crate::transport::HasTransportConfig for ChunkedStreamTransport {
    fn transport_config(&self) -> &crate::transport::builder::TransportConfig {
        &self.config
    }
}

#[cfg(feature = "runtime-tokio")]
impl crate::transport::AsyncTransport for ChunkedStreamTransport {
    async fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.sent
            .send_async(bytes.to_vec())
            .await
            .map_err(|_| Error::RuntimeShutdown)
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        // An exhausted script parks instead of reporting end of stream, so
        // the test controls exactly when the transport closes.
        let Ok(chunk) = self.chunks.recv_async().await else {
            return future::pending().await;
        };
        let len = chunk.len().min(dst.len());
        dst[..len].copy_from_slice(&chunk[..len]);
        Ok(len)
    }

    fn send_semantics(&self) -> crate::transport::SendSemantics {
        crate::transport::SendSemantics::Stream
    }

    fn addressing_mode_hint(&self) -> Option<crate::transport::builder::AddressingMode> {
        Some(self.config.addressing)
    }
}

/// One driver-level script for the raw-correlation boundary fixtures.
/// `Tail` only produces a frame while an earlier `Prefix` remains buffered,
/// which models the production framer closely enough to distinguish a
/// completed boundary frame from a tail delivered after an orphan reset.
#[cfg(feature = "runtime-tokio")]
#[derive(Debug)]
enum ScriptedRawRead {
    Prefix,
    Tail(DecodedFrame),
    Complete(DecodedFrame),
    NoData,
    NoDataThenComplete(DecodedFrame),
    Empty,
    Fault(Error),
    RepeatingFault(Error),
}

#[cfg(feature = "runtime-tokio")]
#[derive(Debug, Clone, Copy, Default)]
struct ScriptedRawOptions {
    refuse_discard: bool,
    park_target_two_write: bool,
}

#[cfg(feature = "runtime-tokio")]
#[derive(Debug)]
struct ScriptedRawDriver {
    reads: flume::Receiver<ScriptedRawRead>,
    reads_observed: flume::Sender<()>,
    receive_polled: flume::Sender<()>,
    after_no_data: Arc<Mutex<Option<DecodedFrame>>>,
    repeating_fault: Arc<Mutex<Option<Error>>>,
    writes: flume::Sender<RequestId>,
    target_two_gate: flume::Receiver<Result<TransmissionMeta, Error>>,
    buffered: Arc<std::sync::atomic::AtomicBool>,
    discards: Arc<std::sync::atomic::AtomicUsize>,
    options: ScriptedRawOptions,
    prefix_kind: crate::protocol::framer::RawIncompletePrefix,
}

#[cfg(feature = "runtime-tokio")]
impl AsyncOwnerDriver for ScriptedRawDriver {
    fn write(
        &mut self,
        write: WireWrite<'_>,
    ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
        let request = write.request;
        // The raw request fixture encodes Camera 2 as the first wire byte
        // (0x80 + target id). Read it while `WireWrite` remains borrowed.
        let target_two = write.bytes.first() == Some(&0x82);
        let writes = self.writes.clone();
        let target_two_gate = self.target_two_gate.clone();
        let park_target_two_write = self.options.park_target_two_write;
        async move {
            writes
                .send_async(request)
                .await
                .map_err(|_| Error::RuntimeShutdown)?;
            if park_target_two_write && target_two {
                target_two_gate
                    .recv_async()
                    .await
                    .unwrap_or(Err(Error::RuntimeShutdown))
            } else {
                Ok(TransmissionMeta { sequence: None })
            }
        }
    }

    fn receive(
        &mut self,
        _buffers: &mut super::super::OwnerBuffers,
        _frame_limit: usize,
    ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
        let reads = self.reads.clone();
        let reads_observed = self.reads_observed.clone();
        let receive_polled = self.receive_polled.clone();
        let after_no_data = Arc::clone(&self.after_no_data);
        let repeating_fault = Arc::clone(&self.repeating_fault);
        let buffered = Arc::clone(&self.buffered);
        async move {
            let _ = receive_polled.try_send(());
            if let Some(frame) = after_no_data.lock().unwrap().take() {
                let _ = reads_observed.try_send(());
                return Ok(AsyncReceive::Frames(vec![frame]));
            }
            if let Some(error) = repeating_fault.lock().unwrap().as_ref().cloned() {
                let _ = reads_observed.try_send(());
                return Ok(AsyncReceive::Fault(error));
            }
            let read = reads
                .recv_async()
                .await
                .map_err(|_| Error::RuntimeShutdown)?;
            let _ = reads_observed.try_send(());
            Ok(match read {
                ScriptedRawRead::Prefix => {
                    buffered.store(true, Ordering::Release);
                    AsyncReceive::Frames(Vec::new())
                }
                ScriptedRawRead::Tail(frame) => {
                    if buffered.swap(false, Ordering::AcqRel) {
                        AsyncReceive::Frames(vec![frame])
                    } else {
                        AsyncReceive::Frames(Vec::new())
                    }
                }
                ScriptedRawRead::Complete(frame) => {
                    buffered.store(false, Ordering::Release);
                    AsyncReceive::Frames(vec![frame])
                }
                ScriptedRawRead::NoData => AsyncReceive::NoData,
                ScriptedRawRead::NoDataThenComplete(frame) => {
                    *after_no_data.lock().unwrap() = Some(frame);
                    AsyncReceive::NoData
                }
                ScriptedRawRead::Empty => AsyncReceive::Frames(Vec::new()),
                ScriptedRawRead::Fault(error) => AsyncReceive::Fault(error),
                ScriptedRawRead::RepeatingFault(error) => {
                    *repeating_fault.lock().unwrap() = Some(error.clone());
                    AsyncReceive::Fault(error)
                }
            })
        }
    }

    fn has_buffered_stream_input(&mut self) -> Result<bool, Error> {
        Ok(self.buffered.load(Ordering::Acquire))
    }

    fn buffered_stream_input(&mut self) -> Result<Option<RawPrefixEvidence>, Error> {
        Ok(self
            .buffered
            .load(Ordering::Acquire)
            .then_some(RawPrefixEvidence::Incomplete {
                target: CameraId::CAMERA_1,
                // The fixture defaults to an exact named terminal, keeping
                // its original stale-prefix tests about discard mechanics.
                // A focused actor test may override this with ambiguous
                // evidence to exercise the local deferral budget.
                kind: self.prefix_kind,
            }))
    }

    fn discard_buffered_stream_input(&mut self) -> Result<(), Error> {
        self.discards.fetch_add(1, Ordering::Relaxed);
        if !self.options.refuse_discard {
            self.buffered.store(false, Ordering::Release);
        }
        Ok(())
    }
}

#[cfg(feature = "runtime-tokio")]
fn raw_inquiry_reply(payload: u8) -> DecodedFrame {
    DecodedFrame {
        target: CameraId::CAMERA_1,
        sequence: None,
        response: DecodedResponse::InquiryReply {
            route: None,
            payload: smallvec::smallvec![payload],
        },
    }
}

#[cfg(feature = "runtime-tokio")]
struct ScriptedRawHarness {
    driver: Option<ScriptedRawDriver>,
    reads: flume::Sender<ScriptedRawRead>,
    reads_observed: flume::Receiver<()>,
    receive_polled: flume::Receiver<()>,
    repeating_fault: Arc<Mutex<Option<Error>>>,
    writes: flume::Receiver<RequestId>,
    target_two_gate: flume::Sender<Result<TransmissionMeta, Error>>,
    buffered: Arc<std::sync::atomic::AtomicBool>,
    discards: Arc<std::sync::atomic::AtomicUsize>,
}

#[cfg(feature = "runtime-tokio")]
fn scripted_raw_harness(options: ScriptedRawOptions, read_capacity: usize) -> ScriptedRawHarness {
    let (read_tx, reads) = flume::bounded(read_capacity);
    let (read_observed_tx, reads_observed) = flume::unbounded();
    let (receive_polled_tx, receive_polled) = flume::unbounded();
    let (writes, write_rx) = flume::bounded(16);
    let (target_two_gate, target_two_gates) = flume::bounded(1);
    let after_no_data = Arc::new(Mutex::new(None));
    let repeating_fault = Arc::new(Mutex::new(None));
    let buffered = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let discards = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    ScriptedRawHarness {
        driver: Some(ScriptedRawDriver {
            reads,
            reads_observed: read_observed_tx,
            receive_polled: receive_polled_tx,
            after_no_data,
            repeating_fault: Arc::clone(&repeating_fault),
            writes,
            target_two_gate: target_two_gates,
            buffered: Arc::clone(&buffered),
            discards: Arc::clone(&discards),
            options,
            prefix_kind: crate::protocol::framer::RawIncompletePrefix::NamedCompletionOrError(
                ViscaSocket::S1,
            ),
        }),
        reads: read_tx,
        reads_observed,
        receive_polled,
        repeating_fault,
        writes: write_rx,
        target_two_gate,
        buffered,
        discards,
    }
}

#[cfg(feature = "runtime-tokio")]
fn boundary_stream_harness(refuse_discard: bool) -> ScriptedRawHarness {
    scripted_raw_harness(
        ScriptedRawOptions {
            refuse_discard,
            ..ScriptedRawOptions::default()
        },
        128,
    )
}

/// Minimal raw receive/write script used to prove the exact-release probe
/// on both stream and datagram policies. The read-consumed channel is an
/// actor-side barrier: a test never infers consumption from a scheduler
/// yield or wall-clock sleep.
#[cfg(feature = "runtime-tokio")]
fn raw_release_probe_harness() -> ScriptedRawHarness {
    scripted_raw_harness(ScriptedRawOptions::default(), 16)
}

/// A raw driver whose target-2 write deliberately parks.  It models the
/// production `drive()` seam: target 1 has an expired correlation
/// tombstone, but the actor is inside an unrelated transport write when H
/// is crossed.  The read-consumed channel makes the return-to-coordinator
/// ordering observable without relying on task scheduling.
#[cfg(feature = "runtime-tokio")]
fn parked_write_raw_release_harness() -> ScriptedRawHarness {
    scripted_raw_harness(
        ScriptedRawOptions {
            park_target_two_write: true,
            ..ScriptedRawOptions::default()
        },
        16,
    )
}

#[cfg(feature = "runtime-tokio")]
async fn establish_raw_inquiry_tombstone_with_probe_driver(
    handle: &AsyncOwnerHandle,
    harness: &ScriptedRawHarness,
) -> ReceiptCore {
    let predecessor = handle.submit(timed_out_inquiry()).await.unwrap();
    assert_eq!(harness.writes.recv_async().await.unwrap(), predecessor.id);
    assert!(matches!(
        predecessor.terminal().await.unwrap(),
        RuntimeOutcome::Failed(Error::Timeout)
    ));

    let successor = handle.submit(inquiry()).await.unwrap();
    assert!(
        harness.writes.try_recv().is_err(),
        "the raw tombstone must retain the successor before its boundary"
    );
    successor
}

/// Exercises the H-δ idle path that used to let a boundary-first Wake
/// release B before an already-ready stale A was even framed.  The
/// read-consumed and sleep-entered barriers make the ordering independent
/// of Tokio task timing: stale A is injected only after NoData has started
/// its clamp-to-H sleep, then H is advanced explicitly.
#[cfg(feature = "runtime-tokio")]
async fn assert_raw_release_probe_precedes_stale_frame_after_idle_sleep(policy: OwnerPolicy) {
    let initial = Instant::now();
    let (runtime, sleeps) = ManualRuntime::with_polling_sleeps_and_sleep_barrier(initial);
    let (handle, actor) = AsyncOwnerActor::new(policy, runtime.clone()).unwrap();
    let mut harness = raw_release_probe_harness();
    let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
    let successor = establish_raw_inquiry_tombstone_with_probe_driver(&handle, &harness).await;

    // The predecessor installed its one-second raw inquiry tombstone at
    // `initial`.  At H−1ms, consume an idle read; its escalating pause is
    // clamped to exactly the release boundary.
    runtime.advance(Duration::from_millis(999));
    harness
        .reads
        .send_async(ScriptedRawRead::NoData)
        .await
        .unwrap();
    harness.reads_observed.recv_async().await.unwrap();
    loop {
        if sleeps.recv_async().await.unwrap() == Duration::from_millis(1) {
            break;
        }
    }

    // A is now transport-ready but cannot have been framed while the
    // actor is parked in the clamped idle sleep.  A pre-fix boundary-first
    // Wake would dispatch B at H before consuming this message.
    harness
        .reads
        .send_async(ScriptedRawRead::Complete(raw_inquiry_reply(0xa1)))
        .await
        .unwrap();
    assert!(
        harness.reads_observed.try_recv().is_err(),
        "the stale frame must remain unframed until H wakes the actor"
    );
    runtime.advance(Duration::from_millis(1));

    enum BoundaryOrder {
        StaleRead,
        SuccessorWrite,
    }
    let first_after_h = future::or(
        async {
            harness.reads_observed.recv_async().await.unwrap();
            BoundaryOrder::StaleRead
        },
        async {
            let _ = harness.writes.recv_async().await.unwrap();
            BoundaryOrder::SuccessorWrite
        },
    )
    .await;
    assert!(
        matches!(first_after_h, BoundaryOrder::StaleRead),
        "the exact raw-release probe must consume stale A before B writes"
    );

    assert_eq!(
        harness.writes.recv_async().await.unwrap(),
        successor.id,
        "only the post-A due pass may dispatch B"
    );
    let snapshot = handle.snapshot().await.unwrap();
    assert!(snapshot.diagnostics.iter().any(|event| matches!(
        event,
        DiagnosticEvent::Ignored(IgnoreReason::UnmatchedFrame)
    )));

    harness
        .reads
        .send_async(ScriptedRawRead::Complete(raw_inquiry_reply(0xb2)))
        .await
        .unwrap();
    assert!(matches!(
        successor.terminal().await.unwrap(),
        RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
    ));

    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}

/// A read's timestamp, rather than the later time at which the actor
/// handles its event, defines whether it discharged the exact-H probe. A
/// pre-H `NoData` can resume after H with an old frame already waiting; it
/// must cause another receive-first pass, not fence the due Wake.
#[cfg(feature = "runtime-tokio")]
async fn assert_pre_h_idle_read_resumed_at_h_requires_fresh_probe(policy: OwnerPolicy) {
    let initial = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(initial);
    let (handle, actor) = AsyncOwnerActor::new(policy, runtime.clone()).unwrap();
    let mut harness = raw_release_probe_harness();
    let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
    let successor = establish_raw_inquiry_tombstone_with_probe_driver(&handle, &harness).await;

    // Put the actor in an ordinary H-δ boundary selection with a receive
    // already pending.  This avoids any scheduler sleep assumption: the
    // driver's poll notification is the exact barrier.
    while harness.receive_polled.try_recv().is_ok() {}
    runtime.advance(Duration::from_millis(999));
    harness
        .reads
        .send_async(ScriptedRawRead::Empty)
        .await
        .unwrap();
    harness.reads_observed.recv_async().await.unwrap();
    harness.receive_polled.recv_async().await.unwrap();

    // The scripted receive is stamped at H-ε, then `now()` advances the
    // virtual clock before the selected event is handled.  It leaves a
    // complete stale A ready for the next receive.  Keying the fence on
    // `selected_at` would let B write first; keying it on `received_at`
    // makes that next probe consume A first.
    runtime.advance_after_next_now(Duration::from_millis(1));
    harness
        .reads
        .try_send(ScriptedRawRead::NoDataThenComplete(raw_inquiry_reply(0xa1)))
        .unwrap();
    harness.reads_observed.recv_async().await.unwrap();

    enum BoundaryOrder {
        StaleRead,
        SuccessorWrite,
    }
    let first_after_h = future::or(
        async {
            harness.reads_observed.recv_async().await.unwrap();
            BoundaryOrder::StaleRead
        },
        async {
            let _ = harness.writes.recv_async().await.unwrap();
            BoundaryOrder::SuccessorWrite
        },
    )
    .await;
    assert!(
        matches!(first_after_h, BoundaryOrder::StaleRead),
        "a pre-H NoData resumed at H must not fence the stale-frame probe"
    );
    assert_eq!(harness.writes.recv_async().await.unwrap(), successor.id);
    assert!(handle
        .snapshot()
        .await
        .unwrap()
        .diagnostics
        .iter()
        .any(|event| matches!(
            event,
            DiagnosticEvent::Ignored(IgnoreReason::UnmatchedFrame)
        )));

    harness
        .reads
        .send_async(ScriptedRawRead::Complete(raw_inquiry_reply(0xb2)))
        .await
        .unwrap();
    assert!(matches!(
        successor.terminal().await.unwrap(),
        RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
    ));

    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}

/// A custom async driver may report an idle read timeout through its
/// `Fault` result instead of `NoData`.  That path is normalized by the
/// owner, so an exact-H timeout must install the same no-input fence.  The
/// driver repeats the idle fault forever after its first result: without
/// the fence a left-biased receive probe hot-loops and B never writes.
#[cfg(feature = "runtime-tokio")]
async fn assert_raw_release_idle_fault_fences_once(policy: OwnerPolicy) {
    let initial = Instant::now();
    let (runtime, sleeps) = ManualRuntime::with_polling_sleeps_and_sleep_barrier(initial);
    let (handle, actor) = AsyncOwnerActor::new(policy, runtime.clone()).unwrap();
    let mut harness = raw_release_probe_harness();
    let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
    let successor = establish_raw_inquiry_tombstone_with_probe_driver(&handle, &harness).await;

    runtime.advance(Duration::from_millis(999));
    harness
        .reads
        .send_async(ScriptedRawRead::NoData)
        .await
        .unwrap();
    harness.reads_observed.recv_async().await.unwrap();
    loop {
        if sleeps.recv_async().await.unwrap() == Duration::from_millis(1) {
            break;
        }
    }

    // This fault is the documented non-consuming idle condition, not a
    // transient ConnectionRefused-style error.  It is queued before H so
    // the exact coordinator consumes it on the first post-H probe.
    harness
        .reads
        .try_send(ScriptedRawRead::RepeatingFault(Error::Timeout))
        .unwrap();
    runtime.advance(Duration::from_millis(1));
    harness.reads_observed.recv_async().await.unwrap();

    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), harness.writes.recv_async())
            .await
            .expect("an exact idle fault must fence once rather than hot-loop")
            .unwrap(),
        successor.id
    );

    // Stop the intentionally adversarial fixture before terminating its
    // owner; real idle transports become pending between reads, whereas
    // this test's value is specifically that it would otherwise stay
    // perpetually ready if the fence were removed.
    *harness.repeating_fault.lock().unwrap() = None;
    handle.shutdown().await.unwrap();
    // The actor may have started its ordinary post-write idle pacing sleep
    // just before the fixture was cleared; wake that sleep so the queued
    // shutdown boundary is observed without depending on wall clock.
    runtime.advance(Duration::from_secs(1));
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}

/// A parked target-2 write may finish exactly as target 1's raw tombstone
/// expires.  `finish_write` used to run an ordinary due/dispatch tail
/// recursively from `drive()`, allowing B to write before the actor ever
/// returned to receive stale A.  The async no-due completion path must
/// instead return to the top-level raw coordinator, where stale A wins the
/// left-biased input probe before B's due pass.
#[cfg(feature = "runtime-tokio")]
async fn assert_parked_cross_target_write_returns_to_raw_coordinator(policy: OwnerPolicy) {
    let initial = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(initial);
    let (handle, actor) = AsyncOwnerActor::new(policy, runtime.clone()).unwrap();
    let mut harness = parked_write_raw_release_harness();
    let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));

    let predecessor = handle.submit(timed_out_inquiry()).await.unwrap();
    assert_eq!(harness.writes.recv_async().await.unwrap(), predecessor.id);
    assert!(matches!(
        predecessor.terminal().await.unwrap(),
        RuntimeOutcome::Failed(Error::Timeout)
    ));

    let successor = handle.submit(inquiry()).await.unwrap();
    assert!(
        harness.writes.try_recv().is_err(),
        "target-1's raw tombstone must hold B before H"
    );

    // C is independent target-2 work.  Its driver write begins at H-δ
    // and is held there while stale A becomes transport-ready; therefore
    // no receive can consume A before C's `finish_write` executes at H.
    runtime.advance(Duration::from_millis(999));
    let parked = handle
        .submit(command_for(CameraId::CAMERA_2))
        .await
        .unwrap();
    assert_eq!(harness.writes.recv_async().await.unwrap(), parked.id);
    harness
        .reads
        .try_send(ScriptedRawRead::Complete(raw_inquiry_reply(0xa1)))
        .unwrap();
    runtime.advance_silently(Duration::from_millis(1));
    harness
        .target_two_gate
        .try_send(Ok(TransmissionMeta { sequence: None }))
        .unwrap();

    enum BoundaryOrder {
        StaleRead,
        SuccessorWrite,
    }
    let first_after_parked_write = future::or(
        async {
            harness.reads_observed.recv_async().await.unwrap();
            BoundaryOrder::StaleRead
        },
        async {
            let _ = harness.writes.recv_async().await.unwrap();
            BoundaryOrder::SuccessorWrite
        },
    )
    .await;
    assert!(
        matches!(first_after_parked_write, BoundaryOrder::StaleRead),
        "a parked write completion crossing H must return to the raw receive probe before B writes"
    );
    assert_eq!(harness.writes.recv_async().await.unwrap(), successor.id);
    assert!(handle
        .snapshot()
        .await
        .unwrap()
        .diagnostics
        .iter()
        .any(|event| matches!(
            event,
            DiagnosticEvent::Ignored(IgnoreReason::UnmatchedFrame)
        )));

    harness
        .reads
        .send_async(ScriptedRawRead::Complete(raw_inquiry_reply(0xb2)))
        .await
        .unwrap();
    assert!(matches!(
        successor.terminal().await.unwrap(),
        RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
    ));

    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}

/// Reproduces the other temporal hole: an ordinary boundary-first
/// selection begins at H−δ, then a transient receive fault wakes that
/// selection after H alongside the due timer.  The fault itself cannot
/// authorize a raw release; stale A behind it must be read before B can
/// write.  Both read barriers must therefore beat B's write barrier.
#[cfg(feature = "runtime-tokio")]
async fn assert_raw_release_fault_then_stale_frame_stays_input_first(policy: OwnerPolicy) {
    let initial = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(initial);
    let (handle, actor) = AsyncOwnerActor::new(policy, runtime.clone()).unwrap();
    let mut harness = raw_release_probe_harness();
    let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
    let successor = establish_raw_inquiry_tombstone_with_probe_driver(&handle, &harness).await;

    // The actor is normally already blocked in receive after B's admission.
    // Drain its old poll notification, then use an empty nonzero receive to
    // enter `YieldBoundaries` without the idle-read sleep path.
    while harness.receive_polled.try_recv().is_ok() {}
    runtime.advance(Duration::from_millis(999));
    harness
        .reads
        .send_async(ScriptedRawRead::Empty)
        .await
        .unwrap();
    harness.reads_observed.recv_async().await.unwrap();
    harness.receive_polled.recv_async().await.unwrap();

    // Move virtual time to H without waking its timer, then make the
    // already-pending receive ready with a transient fault followed by
    // stale A.  That receive wake polls the due timer in the same
    // all-ready boundary-first selection; no wall-clock timing is involved.
    runtime.advance_silently(Duration::from_millis(1));
    harness
        .reads
        .try_send(ScriptedRawRead::Fault(Error::Io(Arc::new(
            std::io::Error::from(std::io::ErrorKind::ConnectionRefused),
        ))))
        .unwrap();
    harness
        .reads
        .try_send(ScriptedRawRead::Complete(raw_inquiry_reply(0xa1)))
        .unwrap();

    enum BoundaryOrder {
        Read,
        SuccessorWrite,
    }
    for description in ["the fault", "stale A"] {
        let next = future::or(
            async {
                harness.reads_observed.recv_async().await.unwrap();
                BoundaryOrder::Read
            },
            async {
                let _ = harness.writes.recv_async().await.unwrap();
                BoundaryOrder::SuccessorWrite
            },
        )
        .await;
        assert!(
            matches!(next, BoundaryOrder::Read),
            "{description} must be consumed before the due release can write B"
        );
    }
    assert_eq!(harness.writes.recv_async().await.unwrap(), successor.id);
    let snapshot = handle.snapshot().await.unwrap();
    assert!(snapshot.diagnostics.iter().any(|event| matches!(
        event,
        DiagnosticEvent::Ignored(IgnoreReason::UnmatchedFrame)
    )));

    harness
        .reads
        .send_async(ScriptedRawRead::Complete(raw_inquiry_reply(0xb2)))
        .await
        .unwrap();
    assert!(matches!(
        successor.terminal().await.unwrap(),
        RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
    ));

    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}

#[cfg(feature = "runtime-tokio")]
async fn terminal_within_test_deadline(
    receipt: &ReceiptCore,
    context: &'static str,
) -> RuntimeOutcome {
    tokio::time::timeout(Duration::from_secs(1), receipt.terminal())
        .await
        .expect(context)
        .expect("the receipt observation channel remains live")
}

#[cfg(feature = "runtime-tokio")]
async fn establish_raw_inquiry_tombstone(
    handle: &AsyncOwnerHandle,
    harness: &ScriptedRawHarness,
) -> ReceiptCore {
    let predecessor = handle.submit(timed_out_inquiry()).await.unwrap();
    assert_eq!(harness.writes.recv_async().await.unwrap(), predecessor.id);
    assert!(matches!(
        terminal_within_test_deadline(
            &predecessor,
            "the predecessor terminalizes at its inquiry deadline",
        )
        .await,
        RuntimeOutcome::Failed(Error::Timeout)
    ));

    let successor = handle.submit(inquiry()).await.unwrap();
    assert!(
        harness.writes.try_recv().is_err(),
        "the raw tombstone must retain the successor before its boundary"
    );
    successor
}

/// Start two raw commands on the same target and assign their distinct
/// camera sockets through the real async transport/framer. X remains live
/// on S1 while Y's shorter completion deadline will enter its exact-S2
/// ambiguity quarantine. Z is queued so the boundary test can prove when
/// dispatch becomes legal.
#[cfg(feature = "runtime-tokio")]
async fn establish_production_two_socket_boundary(
    handle: &AsyncOwnerHandle,
    chunks: &flume::Sender<Vec<u8>>,
    sent: &flume::Receiver<Vec<u8>>,
) -> (ReceiptCore, ReceiptCore, ReceiptCore) {
    let x = handle
        .submit(raw_command_with_completion_deadline(
            0x11,
            Duration::from_secs(30),
        ))
        .await
        .unwrap();
    assert_eq!(
        sent.recv_async().await.unwrap(),
        vec![0x81, 0x01, 0x04, 0x11, 0xff],
        "X must be the first physical write",
    );
    chunks.send_async(vec![0x90, 0x41, 0xff]).await.unwrap();
    let _ = handle.snapshot().await.unwrap();

    let y = handle
        .submit(raw_command_with_completion_deadline(
            0x22,
            Duration::from_secs(5),
        ))
        .await
        .unwrap();
    assert_eq!(
        sent.recv_async().await.unwrap(),
        vec![0x81, 0x01, 0x04, 0x22, 0xff],
        "X's ACK must free the second raw camera socket",
    );
    chunks.send_async(vec![0x90, 0x42, 0xff]).await.unwrap();
    let _ = handle.snapshot().await.unwrap();

    let z = handle
        .submit(raw_command_with_completion_deadline(
            0x33,
            Duration::from_secs(30),
        ))
        .await
        .unwrap();
    assert!(
        sent.try_recv().is_err(),
        "both raw camera sockets are occupied before Y's quarantine releases",
    );
    (x, y, z)
}

/// Let Y first cross its completion deadline into the exact-S2 quarantine,
/// leaving only the one-second ambiguity release for the test to race.
#[cfg(feature = "runtime-tokio")]
async fn enter_production_s2_quarantine(handle: &AsyncOwnerHandle, runtime: &ManualRuntime) {
    runtime.advance(Duration::from_secs(5));
    let _ = handle.snapshot().await.unwrap();
    // The first control may consume its one documented allowance ahead of
    // the mature timer. The next selection makes that timer precede another
    // control, so this second round-trip is an observable barrier: Y has
    // entered its S2 ambiguity quarantine before a test installs bytes for
    // the later exact-release boundary.
    let _ = handle.snapshot().await.unwrap();
}

/// A command whose retry policy allows one more attempt.
#[cfg(feature = "runtime-tokio")]
fn retrying_command() -> RuntimeRequest {
    let mut request = command();
    if let RuntimeRequest::Command { context, .. } = &mut request {
        context.retry = RetryPolicy {
            max_retries: 2,
            initial_backoff: Duration::ZERO,
            maximum_backoff: Duration::ZERO,
            total_budget: Duration::ZERO,
            ack_timeout: true,
            completion_timeout: true,
            inquiry_timeout: true,
            buffer_full: true,
            movement_not_executable: true,
            builtin_inquiry_syntax: true,
        };
    }
    request
}

// ---------------------------------------------------------------------
// #625: a repeatedly-failing transport must not livelock the actor.
// ---------------------------------------------------------------------

/// A driver whose read always fails immediately, so the receive branch of
/// the event race is permanently ready. This is the shape a disconnected
/// USB-serial adapter has: `EIO`/`ENXIO` surface as `ErrorKind::Other`,
/// which classifies as a transient fault.
#[derive(Debug)]
struct AlwaysFailingReceive {
    reads: Arc<std::sync::atomic::AtomicUsize>,
}

impl AsyncOwnerDriver for AlwaysFailingReceive {
    // The private driver trait requires an explicitly `Send` future.
    #[allow(clippy::manual_async_fn)]
    fn write(
        &mut self,
        _write: WireWrite<'_>,
    ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
        async { Ok(TransmissionMeta { sequence: None }) }
    }

    fn receive(
        &mut self,
        _buffers: &mut super::super::OwnerBuffers,
        _frame_limit: usize,
    ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
        self.reads.fetch_add(1, Ordering::Relaxed);
        async {
            Ok(AsyncReceive::Fault(Error::Io(Arc::new(
                std::io::Error::other("simulated adapter unplugged"),
            ))))
        }
    }
}

/// Alternates a transient transport fault with a clean no-data receive.
/// A no-data read paces the actor but is not a successful read, so it must
/// not break the fault run that this driver deliberately keeps within the
/// five-second recovery window.
#[cfg(feature = "runtime-tokio")]
#[derive(Debug)]
struct AlternatingFaultNoData {
    reads: Arc<std::sync::atomic::AtomicUsize>,
    faults: Arc<std::sync::atomic::AtomicUsize>,
}

#[cfg(feature = "runtime-tokio")]
impl AsyncOwnerDriver for AlternatingFaultNoData {
    #[allow(clippy::manual_async_fn)]
    fn write(
        &mut self,
        _write: WireWrite<'_>,
    ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
        async { Ok(TransmissionMeta { sequence: None }) }
    }

    fn receive(
        &mut self,
        _buffers: &mut super::super::OwnerBuffers,
        _frame_limit: usize,
    ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
        let reads = Arc::clone(&self.reads);
        let faults = Arc::clone(&self.faults);
        async move {
            if reads.fetch_add(1, Ordering::Relaxed).is_multiple_of(2) {
                faults.fetch_add(1, Ordering::Relaxed);
                Ok(AsyncReceive::Fault(Error::TransportError(
                    "simulated intermittent adapter fault".into(),
                )))
            } else {
                Ok(AsyncReceive::NoData)
            }
        }
    }
}

// ---------------------------------------------------------------------
// #626: a boundary request racing teardown must never hang.
// ---------------------------------------------------------------------

/// Driver whose writes complete immediately and whose reads come from one
/// channel the test controls. Writes never stall, so the actor is only ever
/// waiting on its own event race.
#[cfg(feature = "runtime-tokio")]
#[derive(Debug)]
struct UngatedDriver {
    receives: flume::Receiver<Result<AsyncReceive, Error>>,
}

#[cfg(feature = "runtime-tokio")]
impl AsyncOwnerDriver for UngatedDriver {
    // The private driver trait requires an explicitly `Send` future.
    #[allow(clippy::manual_async_fn)]
    fn write(
        &mut self,
        _write: WireWrite<'_>,
    ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
        async { Ok(TransmissionMeta { sequence: None }) }
    }

    fn receive(
        &mut self,
        _buffers: &mut super::super::OwnerBuffers,
        _frame_limit: usize,
    ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
        let receives = self.receives.clone();
        async move {
            // An exhausted script parks instead of reporting a close, so
            // the test decides exactly when the transport ends.
            match receives.recv_async().await {
                Ok(next) => next,
                Err(_) => future::pending().await,
            }
        }
    }
}

// ---------------------------------------------------------------------
// #637: the owners agree on a malformed datagram.
// ---------------------------------------------------------------------

/// A datagram transport whose reads are handed over one datagram at a time.
#[cfg(feature = "runtime-tokio")]
#[derive(Debug)]
struct ScriptedDatagramTransport {
    config: crate::transport::builder::TransportConfig,
    datagrams: flume::Receiver<Vec<u8>>,
    sent: flume::Sender<Vec<u8>>,
}

#[cfg(feature = "runtime-tokio")]
impl crate::transport::HasTransportConfig for ScriptedDatagramTransport {
    fn transport_config(&self) -> &crate::transport::builder::TransportConfig {
        &self.config
    }
}

#[cfg(feature = "runtime-tokio")]
impl crate::transport::AsyncTransport for ScriptedDatagramTransport {
    async fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.sent
            .send_async(bytes.to_vec())
            .await
            .map_err(|_| Error::RuntimeShutdown)
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        let Ok(datagram) = self.datagrams.recv_async().await else {
            return future::pending().await;
        };
        let len = datagram.len().min(dst.len());
        dst[..len].copy_from_slice(&datagram[..len]);
        Ok(len)
    }

    fn send_semantics(&self) -> crate::transport::SendSemantics {
        crate::transport::SendSemantics::Datagram
    }
}

/// Production-adapter fixture that alternates a failed read with an
/// oversized datagram whose copied prefix happens to be a valid ACK. The
/// adapter must classify the latter as a consumed bad datagram, not a
/// transient transport fault.
#[cfg(feature = "runtime-tokio")]
#[derive(Debug)]
struct AlternatingFaultAndTruncatedDatagrams {
    config: crate::transport::builder::TransportConfig,
    reads: Arc<std::sync::atomic::AtomicUsize>,
    faults: Arc<std::sync::atomic::AtomicUsize>,
    truncated: Arc<std::sync::atomic::AtomicUsize>,
}

#[cfg(feature = "runtime-tokio")]
impl AlternatingFaultAndTruncatedDatagrams {
    fn next_receive(&self, dst: &mut [u8]) -> Result<crate::transport::ReceiveOutcome, Error> {
        if self.reads.fetch_add(1, Ordering::Relaxed).is_multiple_of(2) {
            self.faults.fetch_add(1, Ordering::Relaxed);
            return Err(Error::TransportError(
                "simulated intermittent datagram adapter fault".into(),
            ));
        }
        let prefix = [0x90, 0x41, 0xff];
        dst[..prefix.len()].copy_from_slice(&prefix);
        self.truncated.fetch_add(1, Ordering::Relaxed);
        Ok(crate::transport::ReceiveOutcome::Truncated {
            copied: prefix.len(),
        })
    }
}

#[cfg(feature = "runtime-tokio")]
impl crate::transport::HasTransportConfig for AlternatingFaultAndTruncatedDatagrams {
    fn transport_config(&self) -> &crate::transport::builder::TransportConfig {
        &self.config
    }
}

#[cfg(feature = "runtime-tokio")]
impl crate::transport::AsyncTransport for AlternatingFaultAndTruncatedDatagrams {
    async fn send(&mut self, _bytes: &[u8]) -> Result<(), Error> {
        Ok(())
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        self.next_receive(dst).map(|outcome| outcome.copied_len())
    }

    async fn recv_into_with_outcome(
        &mut self,
        dst: &mut [u8],
    ) -> Result<crate::transport::ReceiveOutcome, Error> {
        self.next_receive(dst)
    }

    fn send_semantics(&self) -> crate::transport::SendSemantics {
        crate::transport::SendSemantics::Datagram
    }
}

/// A datagram transport whose every send reports the connection as closed.
#[cfg(feature = "runtime-tokio")]
#[derive(Debug)]
struct ClosedSendDatagramTransport {
    config: crate::transport::builder::TransportConfig,
}

#[cfg(feature = "runtime-tokio")]
impl crate::transport::HasTransportConfig for ClosedSendDatagramTransport {
    fn transport_config(&self) -> &crate::transport::builder::TransportConfig {
        &self.config
    }
}

#[cfg(feature = "runtime-tokio")]
impl crate::transport::AsyncTransport for ClosedSendDatagramTransport {
    async fn send(&mut self, _bytes: &[u8]) -> Result<(), Error> {
        Err(Error::ConnectionClosed {
            reason: Some("socket closed".into()),
        })
    }

    async fn recv_into(&mut self, _dst: &mut [u8]) -> Result<usize, Error> {
        future::pending().await
    }

    fn send_semantics(&self) -> crate::transport::SendSemantics {
        crate::transport::SendSemantics::Datagram
    }
}

// ---- #675: async arbitration liveness ----
//
// These run the actor on a spawned task (a real worker thread on the
// multi-thread tokio runtime and on smol's global executor) so an adversarial
// transport that never yields the CPU cannot wedge the single test thread —
// a regression fails the bounded wait cleanly instead of hanging the suite.

/// A peer that produces a valid frame on every poll. `receive` returns
/// immediately, so without the fairness ceiling it wins the left-biased
/// receive-first selection forever and starves every boundary source.
#[derive(Debug, Default)]
struct BabblingDriver;

impl AsyncOwnerDriver for BabblingDriver {
    #[allow(clippy::manual_async_fn)]
    fn write(
        &mut self,
        _write: WireWrite<'_>,
    ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
        async { Ok(TransmissionMeta { sequence: None }) }
    }

    #[allow(clippy::manual_async_fn)]
    fn receive(
        &mut self,
        _buffers: &mut super::super::OwnerBuffers,
        _frame_limit: usize,
    ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
        async {
            Ok(AsyncReceive::Frames(vec![DecodedFrame {
                target: CameraId::CAMERA_1,
                sequence: None,
                response: DecodedResponse::Unknown,
            }]))
        }
    }
}

/// A frame flood whose adapter reports retained stream input after every
/// bounded batch. Production stream adapters do this whenever a read contains
/// more complete frames than `frames_per_receive`; resetting fairness on this
/// outcome used to starve every actor boundary indefinitely.
#[derive(Debug, Default)]
struct BufferedBabblingDriver;

impl AsyncOwnerDriver for BufferedBabblingDriver {
    #[allow(clippy::manual_async_fn)]
    fn write(
        &mut self,
        _write: WireWrite<'_>,
    ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
        async { Ok(TransmissionMeta { sequence: None }) }
    }

    #[allow(clippy::manual_async_fn)]
    fn receive(
        &mut self,
        _buffers: &mut super::super::OwnerBuffers,
        _frame_limit: usize,
    ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
        async {
            Ok(AsyncReceive::Frames(vec![DecodedFrame {
                target: CameraId::CAMERA_1,
                sequence: None,
                response: DecodedResponse::Unknown,
            }]))
        }
    }

    fn has_buffered_stream_input(&mut self) -> Result<bool, Error> {
        Ok(true)
    }
}

/// A babbling peer with an external test-only escape hatch. The watchdog
/// uses `stop` only after declaring the single-thread liveness check
/// failed, so a regressed actor can be released rather than wedging the
/// whole test process.
#[derive(Debug)]
struct CountingBabblingDriver {
    reads: Arc<std::sync::atomic::AtomicU64>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    /// `true` models the empty batch the stream adapter returns after it
    /// discards one or more delimited malformed frames.
    empty_batches: bool,
}

impl AsyncOwnerDriver for CountingBabblingDriver {
    #[allow(clippy::manual_async_fn)]
    fn write(
        &mut self,
        _write: WireWrite<'_>,
    ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
        async { Ok(TransmissionMeta { sequence: None }) }
    }

    fn receive(
        &mut self,
        _buffers: &mut super::super::OwnerBuffers,
        _frame_limit: usize,
    ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
        let reads = Arc::clone(&self.reads);
        let stop = Arc::clone(&self.stop);
        let empty_batches = self.empty_batches;
        async move {
            reads.fetch_add(1, Ordering::Relaxed);
            if stop.load(Ordering::Acquire) {
                Ok(AsyncReceive::Closed)
            } else if empty_batches {
                Ok(AsyncReceive::Frames(Vec::new()))
            } else {
                Ok(AsyncReceive::Frames(vec![DecodedFrame {
                    target: CameraId::CAMERA_1,
                    sequence: None,
                    response: DecodedResponse::Unknown,
                }]))
            }
        }
    }
}

/// A peer that accepts the write but never completes it, and never delivers a
/// read. Without a write timeout the actor parks in the write and `close()`
/// never returns.
#[derive(Debug, Default)]
struct StallingWriteDriver;

impl AsyncOwnerDriver for StallingWriteDriver {
    #[allow(clippy::manual_async_fn)]
    fn write(
        &mut self,
        _write: WireWrite<'_>,
    ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
        async { future::pending().await }
    }

    #[allow(clippy::manual_async_fn)]
    fn receive(
        &mut self,
        _buffers: &mut super::super::OwnerBuffers,
        _frame_limit: usize,
    ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
        async { future::pending().await }
    }
}

/// A peer that reports "no data" on every poll, immediately. Without the
/// idle-read pace this spins the actor at hundreds of thousands of reads a
/// second.
#[derive(Debug)]
struct NoDataDriver {
    reads: Arc<std::sync::atomic::AtomicU64>,
}

impl AsyncOwnerDriver for NoDataDriver {
    #[allow(clippy::manual_async_fn)]
    fn write(
        &mut self,
        _write: WireWrite<'_>,
    ) -> impl Future<Output = Result<TransmissionMeta, Error>> + Send {
        async { Ok(TransmissionMeta { sequence: None }) }
    }

    fn receive(
        &mut self,
        _buffers: &mut super::super::OwnerBuffers,
        _frame_limit: usize,
    ) -> impl Future<Output = Result<AsyncReceive, Error>> + Send {
        let reads = Arc::clone(&self.reads);
        async move {
            reads.fetch_add(1, Ordering::Relaxed);
            Ok(AsyncReceive::NoData)
        }
    }
}

// Shared assertions, generic over the runtime used for the *test's* bounded
// waits. The actor itself runs on a dedicated OS thread with its own runtime
// instance (see the harnesses below), so an adversarial peer that never
// yields cannot wedge the test thread, and the actor's own timers are driven
// by that thread rather than the one running these bounds.

async fn assert_babble_never_starves_boundaries<R>(
    runtime: R,
    handle: AsyncOwnerHandle,
    terminated: flume::Receiver<OwnerSnapshot>,
) where
    R: Runtime,
{
    // Admission is a boundary source; it must be serviced within a bound
    // despite the continuous flood of valid frames.
    let receipt = Executor::timeout(&runtime, Duration::from_secs(5), handle.submit(command()))
        .await
        .expect("a babbling peer must not starve admission")
        .expect("admission rejected");
    // Shutdown enters its one-slot lane; the actor must then terminate — the
    // `close()` liveness the P0 is about — within a bound.
    handle.shutdown().await.unwrap();
    let snapshot = Executor::timeout(&runtime, Duration::from_secs(5), terminated.recv_async())
        .await
        .expect("a babbling peer must not starve shutdown/close")
        .expect("actor terminated");
    assert_eq!(snapshot.state, SessionState::Shutdown);
    drop(receipt);
}

async fn assert_stalled_write_never_parks_close<R>(
    runtime: R,
    handle: AsyncOwnerHandle,
    terminated: flume::Receiver<OwnerSnapshot>,
) where
    R: Runtime,
{
    // Short request deadlines so the fix's success path (the write is
    // abandoned at its timeout, then the request settles on its own
    // deadline) completes quickly and is cleanly separated from the broken
    // "never returns" case the bound catches.
    let receipt = Executor::timeout(
        &runtime,
        Duration::from_secs(5),
        handle.submit(command_with_short_deadlines()),
    )
    .await
    .expect("admission must resolve before the stalled write")
    .expect("admission rejected");
    // The write is abandoned at its 50 ms timeout, unparking the actor, so
    // shutdown/close is serviced rather than blocked behind the stalled peer.
    handle.shutdown().await.unwrap();
    let snapshot = Executor::timeout(&runtime, Duration::from_secs(5), terminated.recv_async())
        .await
        .expect("a stalled write must not park close")
        .expect("actor terminated");
    assert_eq!(snapshot.state, SessionState::Shutdown);
    drop(receipt);
}

async fn assert_nodata_never_hot_spins<R>(
    runtime: R,
    handle: AsyncOwnerHandle,
    terminated: flume::Receiver<OwnerSnapshot>,
    reads: Arc<std::sync::atomic::AtomicU64>,
) where
    R: Runtime,
{
    // Let the idle transport run for a bounded wall-clock window.
    Executor::sleep(&runtime, Duration::from_millis(500)).await;
    let observed = reads.load(Ordering::Relaxed);
    handle.shutdown().await.unwrap();
    let _ = Executor::timeout(&runtime, Duration::from_secs(5), terminated.recv_async()).await;
    // The escalating idle pause caps at 250 ms, so a correctly paced actor
    // does single digits of reads here; the unbounded spin does hundreds of
    // thousands.
    assert!(
        observed < 2_000,
        "idle no-data receive hot-spun: {observed} reads in 500 ms"
    );
}

// Per-runtime harnesses: run `actor` on its own OS thread with its own
// runtime instance, returning the terminal snapshot on a channel. tokio
// timers are driven by that thread's runtime; smol timers by that thread's
// `block_on`, which reacts on async-io whenever the actor parks.
#[cfg(feature = "runtime-tokio")]
fn run_isolated_tokio_actor<D>(
    owner_policy: OwnerPolicy,
    driver: D,
) -> (
    AsyncOwnerHandle,
    flume::Receiver<OwnerSnapshot>,
    std::thread::JoinHandle<()>,
)
where
    D: AsyncOwnerDriver + Send + 'static,
{
    let actor_rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let actor_runtime = TokioRuntime::from_handle(actor_rt.handle().clone());
    let (handle, actor) = AsyncOwnerActor::new(owner_policy, actor_runtime).unwrap();
    let (tx, rx) = flume::bounded::<OwnerSnapshot>(1);
    let join = std::thread::spawn(move || {
        let snapshot = actor_rt.block_on(actor.run(driver));
        let _ = tx.send(snapshot);
    });
    (handle, rx, join)
}

#[cfg(feature = "runtime-smol")]
fn run_isolated_smol_actor<D>(
    owner_policy: OwnerPolicy,
    driver: D,
) -> (
    AsyncOwnerHandle,
    flume::Receiver<OwnerSnapshot>,
    std::thread::JoinHandle<()>,
)
where
    D: AsyncOwnerDriver + Send + 'static,
{
    let (handle, actor) = AsyncOwnerActor::new(owner_policy, SmolRuntime::new()).unwrap();
    let (tx, rx) = flume::bounded::<OwnerSnapshot>(1);
    let join = std::thread::spawn(move || {
        let snapshot = smol::block_on(actor.run(driver));
        let _ = tx.send(snapshot);
    });
    (handle, rx, join)
}

/// The fairness ceiling must surrender the executor, not merely reverse
/// polling order. This puts the actor, a caller admission, a control
/// request, and a timer on one Tokio current-thread runtime. The outer
/// watchdog lives on a separate OS thread so the pre-fix hot loop cannot
/// hang the test binary; it asks the test driver to close only after the
/// liveness deadline has already failed.
#[cfg(feature = "runtime-tokio")]
fn tokio_current_thread_ready_receive_yields_to_boundaries(
    empty_batches: bool,
    include_cancellation: bool,
    scenario: &'static str,
) {
    const WATCHDOG: Duration = Duration::from_secs(2);

    let reads = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (finished, result) = flume::bounded(1);
    let worker_reads = Arc::clone(&reads);
    let worker_stop = Arc::clone(&stop);
    let worker = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let outcome: Result<(), String> = runtime.block_on(async move {
            let actor_runtime = TokioRuntime::from_current().map_err(|error| error.to_string())?;
            let owner_policy = policy(1);
            let fairness_ceiling = owner_policy.limits.frames_per_receive.max(1) as u64;
            let ready_reads = if empty_batches { 1 } else { fairness_ceiling };
            let (handle, actor) = AsyncOwnerActor::new(owner_policy, actor_runtime)
                .map_err(|error| error.to_string())?;
            let actor_task = tokio::spawn(actor.run(CountingBabblingDriver {
                reads: Arc::clone(&worker_reads),
                stop: Arc::clone(&worker_stop),
                empty_batches,
            }));

            // Do not enqueue boundary work until the actor has reached its
            // first forced (valid batch) or boundary-first (empty batch)
            // turn. Before the cooperative yield this loop is never polled
            // again; after it, all work queues on the same executor.
            while worker_reads.load(Ordering::Acquire) < ready_reads {
                tokio::task::yield_now().await;
            }

            let caller_handle = handle.clone();
            let caller = tokio::spawn(async move {
                caller_handle
                    .submit(command())
                    .await
                    .map_err(|error| error.to_string())
            });
            let control_handle = handle.clone();
            let control = tokio::spawn(async move {
                control_handle
                    .snapshot()
                    .await
                    .map_err(|error| error.to_string())
            });
            let timer = tokio::spawn(async {
                tokio::time::sleep(Duration::from_millis(1)).await;
            });

            let (caller, control, timer) = tokio::join!(caller, control, timer);
            let receipt = caller.map_err(|error| format!("caller task failed: {error}"))??;
            let snapshot = control.map_err(|error| format!("control task failed: {error}"))??;
            timer.map_err(|error| format!("timer task failed: {error}"))?;
            if snapshot.state != SessionState::Running {
                return Err(format!(
                    "control observed an unexpected owner state: {:?}",
                    snapshot.state
                ));
            }

            if include_cancellation {
                let cancellation = handle
                    .cancel_test(receipt)
                    .await
                    .map_err(|error| format!("{error:?}"))?;
                drop(cancellation);
            } else {
                drop(receipt);
            }

            handle.shutdown().await.map_err(|error| error.to_string())?;
            let terminal = actor_task
                .await
                .map_err(|error| format!("actor task failed: {error}"))?;
            if terminal.state != SessionState::Shutdown {
                return Err(format!(
                    "actor ended in an unexpected state: {:?}",
                    terminal.state
                ));
            }
            Ok(())
        });
        let _ = finished.send(outcome);
    });

    match result.recv_timeout(WATCHDOG) {
        Ok(Ok(())) => worker.join().unwrap(),
        Ok(Err(error)) => {
            worker.join().unwrap();
            panic!("{scenario} single-thread liveness scenario failed: {error}");
        }
        Err(flume::RecvTimeoutError::Timeout) => {
            // The old implementation remains inside the ready receive loop.
            // Let this test-only driver turn that loop into a terminal read,
            // then join if it unwinds as expected; never wait indefinitely.
            stop.store(true, Ordering::Release);
            if result.recv_timeout(WATCHDOG).is_ok() {
                worker.join().unwrap();
            } else {
                drop(worker);
            }
            panic!(
                "{scenario} monopolized Tokio's current-thread runtime before caller, control, cancellation, or timer work could run"
            );
        }
        Err(flume::RecvTimeoutError::Disconnected) => {
            worker.join().unwrap();
            panic!("{scenario} single-thread liveness worker exited without a result");
        }
    }
}
