//! The raw-release coordinator's single retained-boundary slot (#775).
//!
//! An admission or cancellation selected while a raw correlation release is
//! due is retained, not applied, until that release resolves. The actor has
//! exactly one slot for it, so while the slot is occupied a second boundary
//! must stay in its bounded channel. Before #775 a second boundary could be
//! dequeued too: debug builds hit an assertion and optimized builds silently
//! replaced the first, dropping its reply.
//!
//! Every case here parks the owner inside a retained-prefix release grace
//! with one boundary already retained, then offers a second boundary. The
//! assertions check that each caller receives its own reply, rather than only
//! that the actor did not panic, so they catch the optimized-build overwrite
//! as well as the debug assertion.

use super::*;

const HOLD: Duration = Duration::from_secs(1);
const GRACE: Duration = Duration::from_millis(100);

/// An owner that is due to release A's raw correlation hold at H but is held
/// in the release grace by a retained source-only prefix. Receive stays
/// pending (no scripted read is queued), so only the grace timer can resolve
/// the release.
struct RetainedRelease {
    handle: AsyncOwnerHandle,
    actor: Option<AsyncOwnerActor<ManualRuntime>>,
    driver: Option<ScriptedRawDriver>,
    runtime: ManualRuntime,
    sleeps: flume::Receiver<Duration>,
    harness: ScriptedRawHarness,
    /// A, timed out with its terminal still buffered in its observer.
    predecessor: Option<ReceiptCore>,
    /// Commands admitted before H: the first is on the wire awaiting its ACK
    /// and any others are queued behind it. All are live, cancellable engine
    /// work without a buffered terminal, so cancelling one is a live
    /// cancellation that the coordinator must defer.
    live: Vec<ReceiptCore>,
}

impl RetainedRelease {
    async fn new(capacity: usize, live_commands: usize) -> Self {
        // Admission capacity covers every boundary a case offers, while one
        // raw inquiry slot keeps successors behind A's correlation hold.
        let mut owner_policy = stream_policy(capacity);
        owner_policy.protocol.inquiry_capacity = 1;
        Self::build(owner_policy, live_commands, None).await
    }

    /// `second_hold` additionally gives camera 2 a broad raw hold that
    /// expires that long after start, so it can join the due release set
    /// while the camera 1 release is still in its grace.
    async fn build(
        owner_policy: OwnerPolicy,
        live_commands: usize,
        second_hold: Option<Duration>,
    ) -> Self {
        let (runtime, sleeps) =
            ManualRuntime::with_polling_sleeps_and_sleep_barrier(Instant::now());
        let (handle, mut actor) = AsyncOwnerActor::new(owner_policy, runtime.clone()).unwrap();
        let mut harness = boundary_stream_harness(false);
        let mut driver = harness.driver.take().unwrap();
        // A source-only prefix is identity-weak, so the engine grants it a
        // grace deadline instead of discarding it at H.
        driver.prefix_kind = crate::protocol::framer::RawIncompletePrefix::SourceOnly;

        // A's completion is left unread: its buffered terminal is what a
        // cancellation of A observes.
        let a_request = timed_out_inquiry();
        let a_timeout = a_request.context().timeout.inquiry;
        let (a_completion, a_admitted) = handle.core.enqueue_admission(a_request, None).unwrap();
        let a_boundary = actor.core.receivers.admissions.try_recv().unwrap();
        actor
            .handle_event(
                OwnerEvent::Admission(Ok(a_boundary)),
                &mut driver,
                &runtime,
                Executor::now(&runtime),
                false,
            )
            .await;
        let a = a_admitted.recv_async().await.unwrap().unwrap();
        assert_eq!(harness.writes.recv_async().await.unwrap(), a);
        let predecessor = ReceiptCore::new(
            a,
            CameraId::CAMERA_1,
            a_completion,
            a_timeout,
            Arc::clone(&handle.core.origin),
        );

        if let Some(second_hold) = second_hold {
            let (_completion, admitted) = handle
                .core
                .enqueue_admission(no_reply_command_for(CameraId::CAMERA_2, second_hold), None)
                .unwrap();
            let boundary = actor.core.receivers.admissions.try_recv().unwrap();
            actor
                .handle_event(
                    OwnerEvent::Admission(Ok(boundary)),
                    &mut driver,
                    &runtime,
                    Executor::now(&runtime),
                    false,
                )
                .await;
            let id = admitted.recv_async().await.unwrap().unwrap();
            assert_eq!(harness.writes.recv_async().await.unwrap(), id);
        }

        let mut live_receipts = Vec::with_capacity(live_commands);
        for _ in 0..live_commands {
            let request = command();
            let timeout = request.context().timeout.completion;
            let (completion, admitted) = handle.core.enqueue_admission(request, None).unwrap();
            let boundary = actor.core.receivers.admissions.try_recv().unwrap();
            actor
                .handle_event(
                    OwnerEvent::Admission(Ok(boundary)),
                    &mut driver,
                    &runtime,
                    Executor::now(&runtime),
                    false,
                )
                .await;
            let id = admitted.recv_async().await.unwrap().unwrap();
            live_receipts.push(ReceiptCore::new(
                id,
                CameraId::CAMERA_1,
                completion,
                timeout,
                Arc::clone(&handle.core.origin),
            ));
        }

        // Only the first command can be on the wire before its ACK; any
        // later one stays queued in the engine. Both kinds are live work.
        if let Some(first) = live_receipts.first() {
            assert_eq!(harness.writes.try_recv().unwrap(), first.id);
        }
        assert!(harness.writes.try_recv().is_err());

        driver.buffered.store(true, Ordering::Release);
        runtime.advance(HOLD);
        let at_h = Executor::now(&runtime);
        assert_eq!(
            actor
                .handle_event(OwnerEvent::Wake, &mut driver, &runtime, at_h, false)
                .await,
            TurnOutcome::ContinueBuffered,
            "the retained prefix holds the release in its grace"
        );
        assert_eq!(
            actor.core.coordinator.release().await_until(),
            at_h.checked_add(GRACE)
        );

        Self {
            handle,
            actor: Some(actor),
            driver: Some(driver),
            runtime,
            sleeps,
            harness,
            predecessor: Some(predecessor),
            live: live_receipts,
        }
    }

    /// Start the actor with the boundaries the test has already queued. The
    /// first one is selected at H and retained into the slot.
    fn start(&mut self) -> tokio::task::JoinHandle<OwnerSnapshot> {
        let actor = self.actor.take().unwrap();
        let driver = self.driver.take().unwrap();
        tokio::spawn(actor.run(driver))
    }

    /// Wait until the actor has polled the release-grace timer.
    ///
    /// The grace wake is polled only when no earlier source was ready, so the
    /// first such poll proves the retained boundary was taken and the actor
    /// is now parked on the following turn. The 5 s read-timeout sleeps
    /// announced on each receive poll are skipped.
    async fn parked_in_grace(&self) {
        loop {
            let pause = tokio::time::timeout(Duration::from_secs(1), self.sleeps.recv_async())
                .await
                .expect("the owner must park in the raw-release grace")
                .unwrap();
            if pause == GRACE {
                return;
            }
        }
    }

    /// End the grace. The retained prefix is discarded and the release runs.
    fn release(&self) {
        self.runtime.advance(GRACE);
    }

    async fn next_write(&self, context: &'static str) -> RequestId {
        tokio::time::timeout(Duration::from_secs(1), self.harness.writes.recv_async())
            .await
            .expect(context)
            .unwrap()
    }

    fn enqueue_inquiry(&self) -> (TerminalObserver, flume::Receiver<Result<RequestId, Error>>) {
        self.handle.core.enqueue_admission(inquiry(), None).unwrap()
    }

    /// Requests cancellation on a spawned task, which hands the receipt back
    /// with the answer so its terminal slot stays observable.
    fn spawn_cancel(&self, receipt: ReceiptCore) -> CancelTask {
        let handle = self.handle.clone();
        tokio::spawn(async move {
            let answer = handle.cancel_test(&receipt).await;
            (answer, receipt)
        })
    }

    /// Wait for the cancellation lane to hold the sent boundary.
    async fn cancellation_queued(&self) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while self.handle.core.cancellations.is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the cancellation must wait in its lane, not be dequeued");
    }

    async fn shutdown(&self, actor_task: tokio::task::JoinHandle<OwnerSnapshot>) -> OwnerSnapshot {
        self.handle.shutdown().await.unwrap();
        let snapshot = actor_task.await.unwrap();
        assert_eq!(snapshot.state, SessionState::Shutdown);
        assert_eq!(snapshot.active, 0);
        assert_eq!(
            self.handle.core.permits.available(),
            self.handle.core.permits.capacity(),
            "every admission permit is returned"
        );
        snapshot
    }
}

async fn admitted_within(
    reply: &flume::Receiver<Result<RequestId, Error>>,
    context: &'static str,
) -> Result<RequestId, Error> {
    tokio::time::timeout(Duration::from_secs(1), reply.recv_async())
        .await
        .expect(context)
        .expect("the admission reply must be answered, not dropped")
}

type CancelTask = tokio::task::JoinHandle<(Result<CancellationObserver, Error>, ReceiptCore)>;

async fn cancelled_within(
    task: CancelTask,
    context: &'static str,
) -> (Result<CancellationObserver, Error>, ReceiptCore) {
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .expect(context)
        .expect("the cancelling task must not panic")
}

/// The issue's reproduction: B and C are both ready at H while receive is
/// pending. B is retained; C must wait in its channel and then be admitted in
/// its own right.
#[tokio::test]
async fn two_live_admissions_at_h_each_receive_their_own_reply() {
    let mut owner = RetainedRelease::new(3, 0).await;
    let (_b_completion, b_admitted) = owner.enqueue_inquiry();
    let (c_completion, c_admitted) = owner.enqueue_inquiry();
    let actor_task = owner.start();
    owner.parked_in_grace().await;
    assert!(
        b_admitted.is_empty() && c_admitted.is_empty(),
        "neither admission may be applied before the release resolves"
    );
    assert_eq!(
        owner.handle.core.admissions.len(),
        1,
        "C stays queued while B occupies the slot"
    );

    owner.release();
    let b = admitted_within(&b_admitted, "B is admitted after the release")
        .await
        .unwrap();
    let c = admitted_within(&c_admitted, "C is admitted after B")
        .await
        .unwrap();
    assert_ne!(b, c);
    assert_eq!(
        owner.next_write("B dispatches first").await,
        b,
        "the retained boundary keeps its place ahead of the queued one"
    );
    assert_eq!(owner.harness.discards.load(Ordering::Relaxed), 1);

    let snapshot = owner.shutdown(actor_task).await;
    assert_eq!(
        snapshot.metrics.admitted, 3,
        "A, B, and C were all admitted"
    );
    assert!(
        matches!(
            c_completion.recv_async().await.unwrap(),
            RuntimeOutcome::Failed(Error::RuntimeShutdown)
        ),
        "C, still queued behind B's inquiry, terminalizes at shutdown"
    );
}

/// A live cancellation that arrives while an admission is retained waits for
/// that admission, then reaches the engine exactly once.
#[tokio::test]
async fn admission_then_live_cancellation_are_both_answered() {
    let mut owner = RetainedRelease::new(3, 1).await;
    let x = owner.live.pop().unwrap();
    let x_id = x.id;
    let (_b_completion, b_admitted) = owner.enqueue_inquiry();
    let actor_task = owner.start();
    owner.parked_in_grace().await;

    let cancel = owner.spawn_cancel(x);
    owner.cancellation_queued().await;
    tokio::task::yield_now().await;
    assert_eq!(
        owner.handle.core.cancellations.len(),
        1,
        "the cancellation stays queued while B occupies the slot"
    );
    assert!(b_admitted.is_empty());

    owner.release();
    let b = admitted_within(&b_admitted, "B is admitted after the release")
        .await
        .unwrap();
    assert_eq!(owner.next_write("B dispatches at the release").await, b);
    let (answer, _x) = cancelled_within(cancel, "the cancellation is answered").await;
    drop(answer.expect("the live command accepts cancellation intent"));

    owner.shutdown(actor_task).await;
    assert_no_rewrite(&owner, x_id);
}

/// The reverse order: a live cancellation is retained first, and an
/// admission that arrives during the grace waits behind it.
#[tokio::test]
async fn live_cancellation_then_admission_are_both_answered() {
    let mut owner = RetainedRelease::new(3, 1).await;
    let x = owner.live.pop().unwrap();
    let x_id = x.id;
    let cancel = owner.spawn_cancel(x);
    owner.cancellation_queued().await;
    let actor_task = owner.start();
    owner.parked_in_grace().await;
    assert!(
        owner.handle.core.cancellations.is_empty(),
        "the cancellation was taken into the slot"
    );

    let (_b_completion, b_admitted) = owner.enqueue_inquiry();
    tokio::task::yield_now().await;
    assert_eq!(
        owner.handle.core.admissions.len(),
        1,
        "B stays queued while the cancellation occupies the slot"
    );

    owner.release();
    let (answer, _x) = cancelled_within(cancel, "the cancellation is answered").await;
    drop(answer.expect("the live command accepts cancellation intent"));
    let b = admitted_within(&b_admitted, "B is admitted after the cancellation")
        .await
        .unwrap();
    assert_eq!(owner.next_write("B dispatches after the release").await, b);

    owner.shutdown(actor_task).await;
    assert_no_rewrite(&owner, x_id);
}

/// Successive live cancellations: the single-capacity cancellation lane
/// refills as soon as the first is dequeued, so the second is offered to the
/// actor while the first is retained.
#[tokio::test]
async fn successive_live_cancellations_are_both_answered() {
    let mut owner = RetainedRelease::new(3, 2).await;
    let y = owner.live.pop().unwrap();
    let x = owner.live.pop().unwrap();
    let (x_id, y_id) = (x.id, y.id);
    let cancel_x = owner.spawn_cancel(x);
    owner.cancellation_queued().await;
    let actor_task = owner.start();
    owner.parked_in_grace().await;
    assert!(owner.handle.core.cancellations.is_empty());

    let cancel_y = owner.spawn_cancel(y);
    owner.cancellation_queued().await;
    tokio::task::yield_now().await;
    assert_eq!(
        owner.handle.core.cancellations.len(),
        1,
        "Y's cancellation stays queued while X's occupies the slot"
    );

    owner.release();
    let (answer, _x) = cancelled_within(cancel_x, "X's cancellation is answered").await;
    drop(answer.expect("X accepts cancellation intent"));
    let (answer, _y) = cancelled_within(cancel_y, "Y's cancellation is answered").await;
    drop(answer.expect("Y accepts cancellation intent"));

    owner.shutdown(actor_task).await;
    assert_no_rewrite(&owner, x_id);
    assert_no_rewrite(&owner, y_id);
}

/// An accepted cancellation intent must never be followed by a retry of the
/// request it cancelled.
fn assert_no_rewrite(owner: &RetainedRelease, id: RequestId) {
    let later: Vec<_> = owner.harness.writes.try_iter().collect();
    assert!(
        !later.contains(&id),
        "{id:?} was written again after its cancellation: {later:?}"
    );
}

/// A cancellation whose target already has a buffered terminal needs no
/// engine turn, but it is still a cancellation boundary: it waits behind the
/// retained admission and is then answered `Ok` without installing an intent,
/// leaving the buffered terminal outcome to decide the cancellation (#777).
#[tokio::test]
async fn buffered_terminal_cancellation_waits_behind_retained_admission() {
    let mut owner = RetainedRelease::new(3, 0).await;
    let a = owner.predecessor.take().unwrap();
    let a_id = a.id;
    let (_b_completion, b_admitted) = owner.enqueue_inquiry();
    let actor_task = owner.start();
    owner.parked_in_grace().await;

    let (observer, cell) = CancellationObserver::pair();
    let (reply, cancelled) = flume::bounded(1);
    owner
        .handle
        .core
        .cancellations
        .send_async(CancellationBoundary {
            request: CancellationRequest {
                id: a_id,
                observer: cell,
            },
            reply,
        })
        .await
        .unwrap();
    tokio::task::yield_now().await;
    assert_eq!(
        owner.handle.core.cancellations.len(),
        1,
        "the cancellation stays queued while B occupies the slot"
    );
    assert!(cancelled.is_empty());

    owner.release();
    let b = admitted_within(&b_admitted, "B is admitted after the release")
        .await
        .unwrap();
    assert_eq!(owner.next_write("B dispatches at the release").await, b);
    tokio::time::timeout(Duration::from_secs(1), cancelled.recv_async())
        .await
        .expect("the buffered cancellation is answered")
        .unwrap()
        .expect("a concluded request answers its cancellation with Ok");
    assert!(matches!(
        a.completion.try_recv(),
        Some(RuntimeOutcome::Failed(Error::Timeout { .. }))
    ));
    assert!(
        observer.try_recv().is_none(),
        "no cancellation intent was installed, so none can fail"
    );
    assert_no_rewrite(&owner, a_id);

    owner.shutdown(actor_task).await;
}

/// The pre-admission claim is taken when a boundary is selected, before it is
/// retained, so the retained admission is authoritative even if its caller's
/// deadline passes during the grace. A queued admission whose deadline passes
/// while it waits behind the slot is rejected explicitly when it is reached.
#[tokio::test]
async fn admission_deadlines_are_claimed_at_selection_not_at_release() {
    let mut owner = RetainedRelease::new(3, 0).await;
    let at_h = Executor::now(&owner.runtime);
    let deadline = at_h.checked_add(GRACE / 2).unwrap();
    let (_b_completion, b_admitted) = owner
        .handle
        .core
        .enqueue_admission(inquiry(), Some(AdmissionValidity::until(deadline)))
        .unwrap();
    let actor_task = owner.start();
    owner.parked_in_grace().await;

    let (_c_completion, c_admitted) = owner
        .handle
        .core
        .enqueue_admission(inquiry(), Some(AdmissionValidity::until(deadline)))
        .unwrap();
    tokio::task::yield_now().await;
    assert_eq!(owner.handle.core.admissions.len(), 1);

    owner.release();
    let b = admitted_within(&b_admitted, "retained B keeps its claimed admission")
        .await
        .expect("B was claimed before its deadline");
    assert_eq!(owner.next_write("B dispatches at the release").await, b);
    // C's admission deadline passed before the owner accepted it, so it
    // never existed and may be resubmitted (D20, #783).
    assert_eq!(
        admitted_within(&c_admitted, "queued C is answered")
            .await
            .expect_err("C's admission deadline passed")
            .failure_context(),
        Error::admission_timeout().failure_context()
    );

    let snapshot = owner.shutdown(actor_task).await;
    assert_eq!(snapshot.metrics.admitted, 2, "only A and B were admitted");
}

/// Shutdown keeps its priority while a boundary is retained. The retained
/// admission and the one queued behind it both receive the terminal error,
/// and every permit is returned.
#[tokio::test]
async fn shutdown_answers_retained_and_queued_boundaries() {
    let mut owner = RetainedRelease::new(3, 0).await;
    let (_b_completion, b_admitted) = owner.enqueue_inquiry();
    let actor_task = owner.start();
    owner.parked_in_grace().await;
    let (_c_completion, c_admitted) = owner.enqueue_inquiry();
    tokio::task::yield_now().await;
    assert_eq!(owner.handle.core.admissions.len(), 1);

    let snapshot = owner.shutdown(actor_task).await;
    assert!(matches!(
        admitted_within(&b_admitted, "retained B is answered").await,
        Err(Error::RuntimeShutdown)
    ));
    assert!(matches!(
        admitted_within(&c_admitted, "queued C is answered").await,
        Err(Error::RuntimeShutdown)
    ));
    assert_eq!(snapshot.metrics.admitted, 1, "only A was admitted");
    assert!(
        owner.harness.writes.try_recv().is_err(),
        "neither boundary reached the wire"
    );
}

/// A second hold can join the due release set while the first boundary is
/// retained. The grown set needs its own receive proof and grace, and the
/// slot stays occupied (with the next boundary still queued) throughout.
#[tokio::test]
async fn release_set_growth_keeps_the_retained_boundary_and_its_queue() {
    let mut owner_policy = two_target_raw_policy(TransportKind::Stream);
    owner_policy.protocol.raw_inquiry_release_hold = HOLD;
    let mut owner =
        RetainedRelease::build(owner_policy, 0, Some(HOLD.checked_add(GRACE / 2).unwrap())).await;
    let (_b_completion, b_admitted) = owner.enqueue_inquiry();
    let actor_task = owner.start();
    owner.parked_in_grace().await;
    let (_c_completion, c_admitted) = owner.enqueue_inquiry();
    tokio::task::yield_now().await;

    // Camera 2's hold becomes due inside camera 1's grace. Ending that first
    // grace now meets a grown release set, which earns a fresh grace rather
    // than releasing on the old proof.
    owner.runtime.advance(GRACE / 2);
    owner.release();
    owner.parked_in_grace().await;
    assert!(b_admitted.is_empty(), "B is still retained");
    assert_eq!(
        owner.handle.core.admissions.len(),
        1,
        "C is still queued behind the retained boundary"
    );
    assert!(owner.harness.writes.try_recv().is_err());

    owner.release();
    let b = admitted_within(&b_admitted, "B is admitted after the grown release")
        .await
        .unwrap();
    let c = admitted_within(&c_admitted, "C is admitted after B")
        .await
        .unwrap();
    assert_ne!(b, c);
    assert_eq!(owner.next_write("B dispatches first").await, b);

    owner.shutdown(actor_task).await;
}
