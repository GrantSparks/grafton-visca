use super::*;
#[test]
fn admission_rejection_ingress_total_saturates() {
    let ingress = AdmissionRejectionIngress::new(2);
    let rejection = PreAdmissionRejection {
        target: CameraId::CAMERA_1,
        lane: RequestLane::Command,
        error: crate::ErrorKind::BufferFull,
    };
    ingress.total.store(u64::MAX - 1, Ordering::Release);

    assert!(ingress.record(rejection));
    assert_eq!(ingress.total(), u64::MAX);
    assert!(!ingress.record(rejection));
    assert_eq!(ingress.total(), u64::MAX);
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_typed_receipts_are_completion_first_and_timeout_only_detaches() {
    typed_async_receipt_matrix(TokioRuntime::from_current().unwrap()).await;
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_targeted_settlement_uses_ordinary_prepared_inquiries() {
    targeted_settlement_matrix(TokioRuntime::from_current().unwrap()).await;
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn async_manual_clock_rejects_settlement_query_at_deadline() {
    let now = Instant::now();
    let runtime = ManualRuntime::new(now);
    let (handle, actor) = AsyncOwnerActor::new(policy(2), runtime.clone()).unwrap();
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let frames = harness.frames.clone();
    let writes = Arc::clone(&harness.writes);
    let actor_task = tokio::spawn(actor.run(harness.driver));
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();

    let operation = handle
        .submit_operation(prepared_zoom(&profile))
        .await
        .unwrap();
    assert_eq!(started.recv_async().await.unwrap(), operation.core.id());
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
    assert_eq!(Runtime::now(&runtime), now);

    let error = operation
        .settled_with_timeout(handle.receipt_control(), Duration::ZERO)
        .erase()
        .wait()
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Timeout));
    assert_eq!(
        writes.lock().unwrap().len(),
        1,
        "no position inquiry is written at the exact owner deadline"
    );

    handle.shutdown().await.unwrap();
    let snapshot = actor_task.await.unwrap();
    assert_eq!(snapshot.metrics.admitted, 1);
    assert_eq!(snapshot.active, 0);
}
/// A caller deadline before actor admission is a rejected boundary, not an
/// observer timeout. In particular, starting the actor after the caller
/// timed out must neither create engine state nor transmit the stale work,
/// and dropping that boundary must return its shared capacity permit.
#[cfg(feature = "runtime-tokio")]
#[tokio::test(start_paused = true)]
async fn expired_pre_admission_boundary_never_writes_and_releases_capacity() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
    let deadline = handle.deadline_after(Duration::from_millis(1)).unwrap();
    let expiring_handle = handle.clone();
    let expiring = tokio::spawn(async move {
        expiring_handle
            .submit_with_timeout_until(inquiry(), Duration::from_secs(5), deadline)
            .await
    });

    tokio::task::yield_now().await;
    assert_eq!(
        handle.permits.available(),
        0,
        "the queued boundary owns capacity until the actor observes its expiry"
    );
    tokio::time::advance(Duration::from_millis(1)).await;
    assert!(matches!(expiring.await.unwrap(), Err(Error::Timeout)));

    // Only now let the actor consume the expired boundary. A pre-fix actor
    // staged it and wrote it after this point because the caller had merely
    // dropped its reply receiver.
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let writes = Arc::clone(&harness.writes);
    let actor_task = tokio::spawn(actor.run(harness.driver));
    let snapshot = handle.snapshot().await.unwrap();
    assert_eq!(snapshot.metrics.admitted, 0);
    assert_eq!(snapshot.metrics.admission_rejected, 1);
    assert_eq!(snapshot.metrics.writes, 0);
    assert_eq!(snapshot.active, 0);
    assert_eq!(
        snapshot.diagnostics,
        vec![DiagnosticEvent::AdmissionRejected {
            target: CameraId::CAMERA_1,
            lane: super::super::RequestLane::Inquiry,
            error: crate::ErrorKind::Timeout,
        }],
        "the caller-expiry winner records exactly one pre-admission rejection"
    );
    assert!(
        writes.lock().unwrap().is_empty(),
        "expired work was not written"
    );
    assert_eq!(
        handle.permits.available(),
        handle.permits.capacity(),
        "dropping the stale boundary returns its capacity permit"
    );

    // Reusing the only slot proves that no invisible pending admission is
    // retaining capacity after the caller saw `Timeout`.
    let receipt = handle.try_submit(inquiry()).unwrap().await.unwrap();
    assert_eq!(started.recv_async().await.unwrap(), receipt.id);
    assert_eq!(writes.lock().unwrap().len(), 1);
    drop(receipt);
    gates
        .send_async(Ok(TransmissionMeta { sequence: None }))
        .await
        .unwrap();
    handle.shutdown().await.unwrap();
    let terminal = actor_task.await.unwrap();
    assert_eq!(terminal.state, SessionState::Shutdown);
}
/// A deadline already reached before a boundary exists is still a rejected
/// submission. It must use the same bounded handle-side telemetry path as
/// a capacity rejection without allocating admission state.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn immediate_pre_admission_deadline_is_telemetrized() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, mut actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
    let error = handle
        .submit_with_timeout_until(inquiry(), Duration::from_secs(1), handle.now())
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Timeout));

    actor.flush_pre_admission_rejections(true);
    let metrics = actor.state.metrics_snapshot();
    assert_eq!(metrics.admission_rejected, 1);
    assert_eq!(metrics.admitted, 0);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.pending, 0);
    assert_eq!(metrics.writes, 0);
    assert_eq!(
        actor.state.diagnostics().copied().collect::<Vec<_>>(),
        vec![DiagnosticEvent::AdmissionRejected {
            target: CameraId::CAMERA_1,
            lane: super::super::RequestLane::Inquiry,
            error: crate::ErrorKind::Timeout,
        }]
    );
}
/// If the actor sees an expired boundary before its caller polls the
/// deadline, it is the one authoritative telemetry writer. The caller
/// observes the reply and must not create a second rejection event.
#[cfg(feature = "runtime-tokio")]
#[tokio::test(start_paused = true)]
async fn actor_expired_pre_admission_boundary_is_telemetrized_once() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, mut actor) = AsyncOwnerActor::new(policy(1), runtime.clone()).unwrap();
    let deadline = handle.deadline_after(Duration::from_millis(1)).unwrap();
    let validity = AdmissionValidity::until(deadline);
    let (completion, reply) = handle
        .enqueue_admission(inquiry(), Some(validity))
        .expect("a future deadline can enter the bounded admission lane");
    drop(completion);
    tokio::time::advance(Duration::from_millis(1)).await;

    let boundary = actor
        .admissions
        .try_recv()
        .expect("the actor owns the queued boundary");
    let harness = harness();
    let writes = Arc::clone(&harness.writes);
    let mut driver = harness.driver;
    actor
        .handle_admission(boundary, &mut driver, &runtime, Executor::now(&runtime))
        .await;

    assert!(matches!(reply.recv_async().await, Ok(Err(Error::Timeout))));
    let metrics = actor.state.metrics_snapshot();
    assert_eq!(metrics.admission_rejected, 1);
    assert_eq!(metrics.admitted, 0);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.pending, 0);
    assert_eq!(metrics.writes, 0);
    assert!(writes.lock().unwrap().is_empty());
    assert_eq!(
        actor.state.diagnostics().copied().collect::<Vec<_>>(),
        vec![DiagnosticEvent::AdmissionRejected {
            target: CameraId::CAMERA_1,
            lane: super::super::RequestLane::Inquiry,
            error: crate::ErrorKind::Timeout,
        }]
    );
}
/// The owner task follows `TokioRuntime::from_handle`, even when both the
/// runtime value and actor future are constructed and awaited on a distinct
/// Tokio runtime. This is the affinity that keeps a selected transport,
/// actor timers and I/O together.
#[cfg(feature = "runtime-tokio")]
#[test]
fn tokio_from_handle_runs_the_owner_on_the_selected_runtime() {
    let selected = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let selected_handle = selected.handle().clone();
    let ambient = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    ambient.block_on(async move {
        let runtime = TokioRuntime::from_handle(selected_handle.clone());
        let (observed, receiver) = flume::bounded(1);
        let (_owner, actor) = AsyncOwnerActor::new(policy(1), runtime.clone()).unwrap();
        let actor_task = crate::executor::Executor::spawn(
            &runtime,
            actor.run(RuntimeAffinityDriver { observed }),
        );

        assert_eq!(receiver.recv_async().await.unwrap(), selected_handle.id());
        assert_eq!(actor_task.await.unwrap().state, SessionState::Closed);
    });
}
#[cfg(feature = "runtime-smol")]
#[test]
fn smol_typed_receipts_are_completion_first_and_timeout_only_detaches() {
    smol::block_on(typed_async_receipt_matrix(SmolRuntime::new()));
}
#[cfg(feature = "runtime-smol")]
#[test]
fn smol_targeted_settlement_uses_ordinary_prepared_inquiries() {
    smol::block_on(targeted_settlement_matrix(SmolRuntime::new()));
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn async_admission_precedes_write_and_matches_blocking_terminal_trace() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(2), runtime).unwrap();
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let frames = harness.frames.clone();
    let writes = Arc::clone(&harness.writes);
    let actor_task = tokio::spawn(actor.run(harness.driver));

    // The actor is intentionally stalled in the first write. Admission must
    // still resolve because its effect precedes Transmit in source order.
    let receipt = tokio::time::timeout(Duration::from_secs(1), handle.submit(command()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(started.recv_async().await.unwrap(), receipt.id);
    assert_eq!(writes.lock().unwrap().len(), 1);
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
    assert!(matches!(
        receipt.terminal().await.unwrap(),
        RuntimeOutcome::Applied
    ));
    handle.shutdown().await.unwrap();
    let snapshot = actor_task.await.unwrap();
    assert_eq!(snapshot.state, SessionState::Shutdown);
    assert_eq!(snapshot.active, 0);
    assert_eq!(
        canonical_owner_trace(snapshot.diagnostics.iter().copied()),
        CANONICAL_OWNER_TRACE
    );
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn capacity_rejection_is_pre_identity_and_cancel_full_waits_without_drop() {
    let runtime = TokioRuntime::from_current().unwrap();
    // Raw VISCA admits only one unacknowledged command per target because
    // the camera has not supplied a socket to correlate a second ACK. The
    // capacity/cancellation assertion is about the bounded owner lanes,
    // so use Sony's explicit sequence key for genuine pre-ACK pipelining.
    let (handle, actor) = AsyncOwnerActor::new(sony_policy(2), runtime).unwrap();
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let actor_task = tokio::spawn(actor.run(harness.driver));

    let first = handle.submit(command()).await.unwrap();
    assert_eq!(started.recv_async().await.unwrap(), first.id);
    gates
        .send_async(Ok(TransmissionMeta { sequence: Some(1) }))
        .await
        .unwrap();
    let second = handle.submit(command()).await.unwrap();
    assert_eq!(started.recv_async().await.unwrap(), second.id);
    let error = handle.try_submit(command()).err().unwrap();
    assert!(matches!(error, Error::RuntimeQueueFull { capacity: 2 }));

    let cancel_handle = handle.clone();
    let first_cancel = tokio::spawn(async move { cancel_handle.cancel_test(first).await });
    tokio::task::yield_now().await;
    let cancel_handle = handle.clone();
    let mut second_cancel = tokio::spawn(async move { cancel_handle.cancel_test(second).await });
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut second_cancel)
            .await
            .is_err(),
        "the full dedicated lane applies backpressure"
    );

    gates
        .send_async(Ok(TransmissionMeta { sequence: Some(2) }))
        .await
        .unwrap();
    let first_observation = first_cancel.await.unwrap().unwrap();
    let second_observation = second_cancel.await.unwrap().unwrap();
    drop((first_observation, second_observation));

    let (left, right) = tokio::join!(handle.shutdown(), handle.shutdown());
    left.unwrap();
    right.unwrap();
    let snapshot = actor_task.await.unwrap();
    assert_eq!(snapshot.active, 0);
    assert_eq!(snapshot.metrics.admitted, 2);
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn ordinary_async_submit_is_fail_fast_at_capacity() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let actor_task = tokio::spawn(actor.run(harness.driver));

    let first = handle.submit(command()).await.unwrap();
    assert_eq!(started.recv_async().await.unwrap(), first.id);
    let error = tokio::time::timeout(Duration::from_millis(20), handle.submit(command()))
        .await
        .expect("capacity failure must not register a waiter")
        .unwrap_err();
    assert!(matches!(error, Error::RuntimeQueueFull { capacity: 1 }));
    drop(first);
    gates
        .send_async(Ok(TransmissionMeta { sequence: None }))
        .await
        .unwrap();
    handle.shutdown().await.unwrap();
    let snapshot = actor_task.await.unwrap();
    assert_eq!(snapshot.metrics.admitted, 1);
}
/// A handle-side capacity rejection happens before an admission boundary
/// can allocate an observer, request ID, or pending slot. It still belongs
/// to the stable pre-admission telemetry contract, so the actor receives a
/// compact bounded ingress event without turning the fail-fast call into a
/// wait for the owner.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn fail_fast_capacity_rejection_records_metrics_and_diagnostic() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let writes = Arc::clone(&harness.writes);
    let actor_task = tokio::spawn(actor.run(harness.driver));
    let diagnostics = handle.subscribe_diagnostics(16).await.unwrap();

    let first = handle.submit(command()).await.unwrap();
    assert_eq!(started.recv_async().await.unwrap(), first.id);
    assert_eq!(writes.lock().unwrap().len(), 1);

    let error = tokio::time::timeout(Duration::from_millis(20), handle.submit(command()))
        .await
        .expect("capacity failure must not register an admission waiter")
        .unwrap_err();
    assert!(matches!(error, Error::RuntimeQueueFull { capacity: 1 }));

    // The first write is deliberately held so the rejected work cannot be
    // confused with a newly admitted request. Release it only after the
    // fail-fast result, then query the actor-owned metric snapshot.
    gates
        .send_async(Ok(TransmissionMeta { sequence: None }))
        .await
        .unwrap();
    let metrics = handle.metrics().await.unwrap();
    assert_eq!(metrics.admitted, 1);
    assert_eq!(metrics.admission_rejected, 1);
    assert_eq!(metrics.active, 1);
    assert_eq!(metrics.pending, 0);
    assert_eq!(metrics.writes, 1);
    assert_eq!(
        writes.lock().unwrap().len(),
        1,
        "a rejected admission must not reach transport I/O"
    );

    let rejected = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let event = diagnostics.recv_async().await.unwrap();
            if matches!(event, DiagnosticEvent::AdmissionRejected { .. }) {
                break event;
            }
        }
    })
    .await
    .expect("the bounded ingress must emit one rejection diagnostic");
    assert_eq!(
        rejected,
        DiagnosticEvent::AdmissionRejected {
            target: CameraId::CAMERA_1,
            lane: super::super::RequestLane::Command,
            error: crate::ErrorKind::BufferFull,
        }
    );

    drop(first);
    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
/// The handle-side ingress is runtime-neutral. Exercise the same
/// fail-fast capacity path under smol without requiring a transport task:
/// the actor remains the sole writer when it drains the compact event.
#[cfg(feature = "runtime-smol")]
#[test]
fn smol_fail_fast_capacity_rejection_records_telemetry() {
    let (handle, mut actor) = AsyncOwnerActor::new(policy(1), SmolRuntime::new()).unwrap();
    let held = handle
        .permits
        .try_acquire()
        .expect("the test reserves the only admission slot");

    let error = match handle.enqueue_admission(inquiry(), None) {
        Ok(_) => panic!("a full admission pool must fail fast"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::RuntimeQueueFull { capacity: 1 }));

    actor.flush_pre_admission_rejections(true);
    let metrics = actor.state.metrics_snapshot();
    assert_eq!(metrics.admission_rejected, 1);
    assert_eq!(metrics.admitted, 0);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.pending, 0);
    assert_eq!(metrics.writes, 0);
    assert_eq!(
        actor.state.diagnostics().copied().collect::<Vec<_>>(),
        vec![DiagnosticEvent::AdmissionRejected {
            target: CameraId::CAMERA_1,
            lane: super::super::RequestLane::Inquiry,
            error: crate::ErrorKind::BufferFull,
        }]
    );
    drop(held);
}
/// The ingress is deliberately bounded. If a burst outruns its one-slot
/// diagnostic staging queue, metrics keep the exact rejection total and
/// the already-public dropped-diagnostics counter makes the evicted event
/// visible instead of silently inventing another loss class.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn pre_admission_rejection_ingress_reports_bounded_diagnostic_loss() {
    let runtime = TokioRuntime::from_current().unwrap();
    let mut owner_policy = policy(1);
    owner_policy.limits.diagnostics = 1;
    let (_handle, mut actor) = AsyncOwnerActor::new(owner_policy, runtime).unwrap();
    let first = PreAdmissionRejection {
        target: CameraId::CAMERA_1,
        lane: super::super::RequestLane::Command,
        error: crate::ErrorKind::BufferFull,
    };
    let second = PreAdmissionRejection {
        target: CameraId::CAMERA_1,
        lane: super::super::RequestLane::Inquiry,
        error: crate::ErrorKind::IoClosed,
    };

    assert!(actor.admission_rejections.record(first));
    assert!(
        !actor.admission_rejections.record(second),
        "one bounded actor wake coalesces the burst"
    );
    actor.flush_pre_admission_rejections(true);

    let metrics = actor.state.metrics();
    assert_eq!(metrics.admission_rejected, 2);
    assert_eq!(metrics.dropped_diagnostics, 1);
    assert_eq!(
        actor.state.diagnostics().copied().collect::<Vec<_>>(),
        vec![DiagnosticEvent::AdmissionRejected {
            target: CameraId::CAMERA_1,
            lane: super::super::RequestLane::Inquiry,
            error: crate::ErrorKind::IoClosed,
        }],
        "the bounded queue retains the newest rejection fact"
    );
}
/// A receiver can disappear in the narrow interval after the handle passes
/// its lifecycle check but before its boundary send. That is still a
/// rejection before authoritative admission, so it must use the same
/// metric/diagnostic ingress as fail-fast capacity exhaustion.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn disconnected_pre_boundary_admission_is_telemetrized() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, mut actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
    let (replacement_sender, replacement_receiver) = flume::bounded(1);
    drop(replacement_sender);
    let original_receiver = std::mem::replace(&mut actor.admissions, replacement_receiver);
    drop(original_receiver);

    let error = match handle.try_submit(command()) {
        Ok(_) => panic!("a disconnected admission receiver must reject"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        Error::InvalidState(message)
            if message.contains("without publishing a terminal result")
    ));

    actor.flush_pre_admission_rejections(true);
    assert_eq!(actor.state.metrics().admission_rejected, 1);
    assert_eq!(
        actor.state.diagnostics().copied().collect::<Vec<_>>(),
        vec![DiagnosticEvent::AdmissionRejected {
            target: CameraId::CAMERA_1,
            lane: super::super::RequestLane::Command,
            error: crate::ErrorKind::NotExecutable,
        }]
    );
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn async_cancellation_returns_after_recording_and_retains_terminal() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let frames = harness.frames.clone();
    let actor_task = tokio::spawn(actor.run(harness.driver));
    let operation = handle.submit(command()).await.unwrap();
    assert_eq!(started.recv_async().await.unwrap(), operation.id);
    gates
        .send_async(Ok(TransmissionMeta { sequence: None }))
        .await
        .unwrap();
    frames
        .send_async(batch(vec![ack(ViscaSocket::S1)]))
        .await
        .unwrap();
    let cancel_handle = handle.clone();
    let cancel_task = tokio::spawn(async move { cancel_handle.cancel_test(operation).await });
    let _cancel_write = started.recv_async().await.unwrap();
    gates
        .send_async(Ok(TransmissionMeta { sequence: None }))
        .await
        .unwrap();
    let cancellation = cancel_task.await.unwrap().unwrap();
    frames
        .send_async(batch(vec![DecodedFrame {
            target: CameraId::CAMERA_1,
            sequence: None,
            response: DecodedResponse::Error {
                socket: Some(ViscaSocket::S1),
                code: 0x04,
            },
        }]))
        .await
        .unwrap();
    assert!(matches!(
        cancellation.recv_test().await.unwrap(),
        CancellationObservation::Cancelled
    ));
    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().active, 0);
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn receipt_control_is_bound_to_its_originating_owner_and_clock() {
    let now = Instant::now();
    let first_runtime = ManualRuntime::new(now);
    let second_runtime = ManualRuntime::new(now.checked_add(Duration::from_secs(60)).unwrap());
    let (first, first_actor) = AsyncOwnerActor::new(policy(1), first_runtime).unwrap();
    let (second, second_actor) = AsyncOwnerActor::new(policy(1), second_runtime).unwrap();
    let first_harness = harness();
    let second_harness = harness();
    let first_started = first_harness.started.clone();
    let second_started = second_harness.started.clone();
    let first_gates = first_harness.gates.clone();
    let second_gates = second_harness.gates.clone();
    let first_task = tokio::spawn(first_actor.run(first_harness.driver));
    let second_task = tokio::spawn(second_actor.run(second_harness.driver));
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();

    let first_receipt = first
        .submit_command(prepared_focus(&profile))
        .await
        .unwrap();
    let second_receipt = second
        .submit_command(prepared_focus(&profile))
        .await
        .unwrap();
    let _ = first_started.recv_async().await.unwrap();
    let _ = second_started.recv_async().await.unwrap();
    assert!(matches!(
        first_receipt
            .wait_with_timeout(second.receipt_control(), Duration::ZERO)
            .await,
        Err(Error::InvalidState(_))
    ));
    second_receipt.detach();
    first_gates
        .send_async(Ok(TransmissionMeta { sequence: None }))
        .await
        .unwrap();
    second_gates
        .send_async(Ok(TransmissionMeta { sequence: None }))
        .await
        .unwrap();
    first.shutdown().await.unwrap();
    second.shutdown().await.unwrap();
    assert_eq!(first_task.await.unwrap().state, SessionState::Shutdown);
    assert_eq!(second_task.await.unwrap().state, SessionState::Shutdown);
}
#[cfg(feature = "runtime-smol")]
#[test]
fn smol_actor_has_the_same_admit_write_terminal_order() {
    smol::block_on(async {
        let runtime = SmolRuntime::new();
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
        let harness = harness();
        let started = harness.started.clone();
        let gates = harness.gates.clone();
        let frames = harness.frames.clone();
        let task = smol::spawn(actor.run(harness.driver));

        let receipt = handle.submit(command()).await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), receipt.id);
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
        assert!(matches!(
            receipt.terminal().await.unwrap(),
            RuntimeOutcome::Applied
        ));
        handle.shutdown().await.unwrap();
        let snapshot = task.await;
        assert_eq!(snapshot.state, SessionState::Shutdown);
        assert_eq!(snapshot.active, 0);
    });
}
