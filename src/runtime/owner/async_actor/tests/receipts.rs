/// Dropping a pending command, inquiry, or operation must not turn queued
/// boundary work into unobservable hardware activity (#964). The same
/// serialized owner boundary is exercised on both supported async runtimes.
async fn abandoned_admission_matrix<R: Runtime>(runtime: R) {
    let (handle, actor) = AsyncOwnerActor::new(policy(5), runtime).unwrap();
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let frames = harness.frames.clone();
    let writes = Arc::clone(&harness.writes);
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBrc300>().unwrap();
    let client = async {
        let first = handle.submit(command()).await.unwrap();
        assert_eq!(started.recv_async().await.unwrap(), first.id);

        // Poll each public-class seam until it has queued its admission, then
        // abandon the future while the actor is blocked in the first write.
        {
            let pending = handle.submit_command(prepared_focus(&profile));
            let mut pending = std::pin::pin!(pending);
            assert!(future::poll_once(pending.as_mut()).await.is_none());
        }
        {
            let pending = handle.submit(inquiry());
            let mut pending = std::pin::pin!(pending);
            assert!(future::poll_once(pending.as_mut()).await.is_none());
        }
        {
            let pending = handle.submit_operation(prepared_zoom(&profile));
            let mut pending = std::pin::pin!(pending);
            assert!(future::poll_once(pending.as_mut()).await.is_none());
        }
        {
            let deadline = handle.deadline_after(Duration::from_secs(1)).unwrap();
            let pending = handle.submit_with_timeout_until::<RuntimeOutcome>(
                inquiry().into(),
                Duration::from_secs(5),
                deadline,
            );
            let mut pending = std::pin::pin!(pending);
            assert!(future::poll_once(pending.as_mut()).await.is_none());
        }
        assert_eq!(handle.core.admissions.len(), 4);
        assert_eq!(handle.core.permits.available(), 0);

        gates.send_async(Ok(TransmissionMeta::Raw)).await.unwrap();
        frames
            .send_async(batch(vec![ack(ViscaSocket::S1)]))
            .await
            .unwrap();
        frames
            .send_async(batch(vec![completion(ViscaSocket::S1)]))
            .await
            .unwrap();
        assert!(matches!(
            first.terminal().await.unwrap(),
            RuntimeOutcome::Command(CommandOutcome::Applied)
        ));
        assert_eq!(
            writes.lock().unwrap().len(),
            1,
            "abandoned work never writes"
        );
        let metrics = handle.metrics().await.unwrap();
        assert_eq!(metrics.admitted, 1);
        assert_eq!(metrics.admission_rejected, 4);
        assert_eq!(metrics.dropped_observer_events, 0);
        assert_eq!(metrics.writes, 1);
        assert_eq!(metrics.active, 0);
        assert_eq!(metrics.pending, 0);
        assert_eq!(writes.lock().unwrap().len(), 1);
        assert_eq!(
            handle.core.permits.available(),
            handle.core.permits.capacity()
        );
        handle.shutdown().unwrap();
    };
    let ((), snapshot) = future::zip(client, actor.run(harness.driver)).await;
    assert_eq!(snapshot.active, 0);
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_abandoned_admissions_never_write_and_release_capacity() {
    abandoned_admission_matrix(TokioRuntime::from_current().unwrap()).await;
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_abandoned_admissions_never_write_and_release_capacity() {
    smol::block_on(abandoned_admission_matrix(SmolRuntime::new()));
}

/// An outer async timeout drops an unbounded submission future. Starting
/// the owner afterwards cannot turn that elapsed call into a camera move.
#[cfg(feature = "runtime-tokio")]
#[tokio::test(start_paused = true)]
async fn timed_out_operation_before_admission_never_writes() {
    let (handle, mut actor) =
        AsyncOwnerActor::new(policy(1), TokioRuntime::from_current().unwrap()).unwrap();
    let diagnostics = observe_diagnostics(&mut actor);
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBrc300>().unwrap();
    assert!(tokio::time::timeout(
        Duration::from_millis(1),
        handle.submit_operation(prepared_zoom(&profile)),
    )
    .await
    .is_err());
    assert_eq!(handle.core.admissions.len(), 1);
    let harness = harness();
    let writes = Arc::clone(&harness.writes);
    let actor_task = tokio::spawn(actor.run(harness.driver));
    let metrics = handle.metrics().await.unwrap();
    assert_eq!(metrics.admitted, 0);
    assert_eq!(metrics.admission_rejected, 1);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.pending, 0);
    assert_eq!(metrics.writes, 0);
    assert_eq!(metrics.dropped_observer_events, 0);
    assert_eq!(
        handle.core.permits.available(),
        handle.core.permits.capacity()
    );
    assert!(writes.lock().unwrap().is_empty());
    assert_eq!(
        diagnostics.events(),
        vec![DiagnosticEvent::AdmissionRejected {
            target: crate::CameraId::CAMERA_1,
            lane: crate::observability::DiagnosticLane::Command,
            error: crate::ErrorKind::Cancelled,
        }]
    );
    handle.shutdown().unwrap();
    actor_task.await.unwrap();
}

/// Channel disconnect is observed after queued messages. Abandoning the
/// session must not drain those messages as fresh hardware commands (#964).
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn dropping_last_handle_does_not_write_abandoned_admissions() {
    let (handle, actor) =
        AsyncOwnerActor::new(policy(4), TokioRuntime::from_current().unwrap()).unwrap();
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let writes = Arc::clone(&harness.writes);
    let actor_task = tokio::spawn(actor.run(harness.driver));
    let first = handle.submit(command()).await.unwrap();
    assert_eq!(started.recv_async().await.unwrap(), first.id);
    for request in [command(), inquiry(), command()] {
        let pending = handle.submit(request);
        let mut pending = std::pin::pin!(pending);
        assert!(future::poll_once(pending.as_mut()).await.is_none());
    }
    assert_eq!(handle.core.admissions.len(), 3);
    drop(handle);
    gates.send_async(Ok(TransmissionMeta::Raw)).await.unwrap();
    let snapshot = actor_task.await.unwrap();
    assert_eq!(snapshot.metrics.admitted, 1);
    assert_eq!(snapshot.metrics.admission_rejected, 3);
    assert_eq!(snapshot.metrics.dropped_observer_events, 0);
    assert_eq!(snapshot.metrics.writes, 1);
    assert_eq!(writes.lock().unwrap().len(), 1);
    assert_eq!(snapshot.active, 0);
    drop(first);
}

use super::*;
#[cfg(feature = "runtime-tokio")]
use crate::runtime::owner::boundary::PreAdmissionRejection;
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
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBrc300>().unwrap();

    let mut operation = handle
        .submit_operation(prepared_zoom(&profile))
        .await
        .unwrap();
    assert_eq!(
        started.recv_async().await.unwrap(),
        operation.observation.id()
    );
    gates.send_async(Ok(TransmissionMeta::Raw)).await.unwrap();
    frames
        .send_async(batch(vec![ack(ViscaSocket::S1)]))
        .await
        .unwrap();
    frames
        .send_async(batch(vec![completion(ViscaSocket::S1)]))
        .await
        .unwrap();
    assert_eq!(handle.snapshot().await.unwrap().active, 0);
    assert_eq!(crate::executor::Executor::now(&runtime), now);

    let error = operation.settled(Some(Duration::ZERO)).await.unwrap_err();
    assert!(matches!(error, Error::ObservationTimeout { .. }));
    assert_eq!(
        writes.lock().unwrap().len(),
        1,
        "no position inquiry is written at the exact owner deadline"
    );

    handle.shutdown().unwrap();
    let snapshot = actor_task.await.unwrap();
    assert_eq!(snapshot.metrics.admitted, 1);
    assert_eq!(snapshot.active, 0);
}

/// #744: the async owner exercises the same raw tombstone rule as the
/// blocking owner. A stop admitted through a lost-ACK `PreAck` hold is the
/// unique live command candidate, so its ACK must bind and confirm it rather
/// than being discarded as stale predecessor traffic.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn async_urgent_stop_binds_its_ack_under_a_preack_hold() {
    let start = Instant::now();
    let (runtime, sleep_started) = ManualRuntime::with_polling_sleeps_and_sleep_barrier(start);
    let (handle, mut actor) = AsyncOwnerActor::new(policy(2), runtime.clone()).unwrap();
    let diagnostics = observe_diagnostics(&mut actor);
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let frames = harness.frames.clone();
    let writes = Arc::clone(&harness.writes);
    let actor_task = tokio::spawn(actor.run(harness.driver));

    let predecessor = handle
        .submit(command_with_ack_timeout(Duration::from_millis(10)))
        .await
        .unwrap();
    assert_eq!(started.recv_async().await.unwrap(), predecessor.id);
    gates.send_async(Ok(TransmissionMeta::Raw)).await.unwrap();

    let armed_ack_deadline = loop {
        let sleep = sleep_started.recv_async().await.unwrap();
        if sleep == Duration::from_millis(10) {
            break sleep;
        }
    };
    assert_eq!(
        armed_ack_deadline,
        Duration::from_millis(10),
        "the owner arms the predecessor ACK deadline before the manual clock advances"
    );
    runtime.advance(Duration::from_millis(10));
    assert!(matches!(
        terminal_within_test_deadline(&predecessor, "lost ACK terminalizes the predecessor").await,
        RuntimeOutcome::Command(CommandOutcome::Failed(Error::UnsequencedCommandUnconfirmed))
    ));

    let stop = handle.submit(urgent_stop_command()).await.unwrap();
    assert_eq!(started.recv_async().await.unwrap(), stop.id);
    assert_eq!(
        writes
            .lock()
            .unwrap()
            .last()
            .map(|(_, bytes, _)| bytes.clone()),
        Some(vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xff]),
        "the emergency stop writes through the predecessor hold"
    );
    gates.send_async(Ok(TransmissionMeta::Raw)).await.unwrap();
    frames
        .send_async(batch(vec![ack(ViscaSocket::S1)]))
        .await
        .unwrap();
    frames
        .send_async(batch(vec![completion(ViscaSocket::S1)]))
        .await
        .unwrap();

    assert!(matches!(
        terminal_within_test_deadline(&stop, "the urgent stop owns its ACK and completion").await,
        RuntimeOutcome::Command(CommandOutcome::Applied)
    ));
    handle.metrics().await.unwrap();
    assert!(
        !diagnostics.events().iter().any(|event| matches!(
            event,
            DiagnosticEvent::Ignored {
                reason: crate::observability::DiagnosticIgnoreReason::UnmatchedFrame
            }
        )),
        "the stop ACK must not be discarded by the predecessor tombstone"
    );

    handle.shutdown().unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}

/// A caller deadline before actor admission is a rejected boundary, not an
/// observer timeout. In particular, starting the actor after the caller
/// timed out must neither create engine state nor transmit the stale work,
/// and dropping that boundary must return its shared capacity permit.
#[cfg(feature = "runtime-tokio")]
#[tokio::test(start_paused = true)]
async fn expired_pre_admission_boundary_never_writes_and_releases_capacity() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, mut actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
    let diagnostics = observe_diagnostics(&mut actor);
    let deadline = handle.deadline_after(Duration::from_millis(1)).unwrap();
    let expiring_handle = handle.clone();
    let expiring = tokio::spawn(async move {
        expiring_handle
            .submit_with_timeout_until::<RuntimeOutcome>(
                inquiry().into(),
                Duration::from_secs(5),
                deadline,
            )
            .await
    });

    tokio::task::yield_now().await;
    assert_eq!(
        handle.core.permits.available(),
        0,
        "the queued boundary owns capacity until the actor observes its expiry"
    );
    tokio::time::advance(Duration::from_millis(1)).await;
    assert!(matches!(
        expiring.await.unwrap(),
        Err(Error::Timeout { .. })
    ));

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
        diagnostics.events(),
        vec![DiagnosticEvent::AdmissionRejected {
            target: crate::CameraId::CAMERA_1,
            lane: crate::observability::DiagnosticLane::Inquiry,
            error: crate::ErrorKind::Timeout,
        }],
        "the caller-expiry winner records exactly one pre-admission rejection"
    );
    assert!(
        writes.lock().unwrap().is_empty(),
        "expired work was not written"
    );
    assert_eq!(
        handle.core.permits.available(),
        handle.core.permits.capacity(),
        "dropping the stale boundary returns its capacity permit"
    );

    // Reusing the only slot proves that no invisible pending admission is
    // retaining capacity after the caller saw `Timeout`.
    let receipt = handle.try_submit(inquiry()).unwrap().await.unwrap();
    assert_eq!(started.recv_async().await.unwrap(), receipt.id);
    assert_eq!(writes.lock().unwrap().len(), 1);
    drop(receipt);
    gates.send_async(Ok(TransmissionMeta::Raw)).await.unwrap();
    handle.shutdown().unwrap();
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
    let diagnostics = observe_diagnostics(&mut actor);
    let error = handle
        .submit_with_timeout_until::<RuntimeOutcome>(
            inquiry().into(),
            Duration::from_secs(1),
            handle.now(),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Timeout { .. }));

    actor.core.flush_pre_admission_rejections(true);
    let metrics = actor.core.state.metrics_snapshot(0);
    assert_eq!(metrics.admission_rejected, 1);
    assert_eq!(metrics.admitted, 0);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.pending, 0);
    assert_eq!(metrics.writes, 0);
    assert_eq!(
        diagnostics.events(),
        vec![DiagnosticEvent::AdmissionRejected {
            target: crate::CameraId::CAMERA_1,
            lane: crate::observability::DiagnosticLane::Inquiry,
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
    let diagnostics = observe_diagnostics(&mut actor);
    let deadline = handle.deadline_after(Duration::from_millis(1)).unwrap();
    let validity = crate::runtime::owner::boundary::AdmissionValidity::until(deadline);
    let (completion, reply) = handle
        .core
        .enqueue_admission::<RuntimeOutcome>(inquiry(), Some(validity))
        .expect("a future deadline can enter the bounded admission lane");
    drop(completion);
    tokio::time::advance(Duration::from_millis(1)).await;

    let boundary = actor
        .core
        .receivers
        .admissions
        .try_recv()
        .expect("the actor owns the queued boundary");
    let harness = harness();
    let writes = Arc::clone(&harness.writes);
    let mut driver = harness.driver;
    actor
        .handle_event(
            OwnerEvent::Admission(Ok(boundary)),
            &mut driver,
            Executor::now(&runtime),
            false,
        )
        .await;

    assert!(matches!(
        reply.recv_async().await,
        Ok(Err(Error::Timeout { .. }))
    ));
    let metrics = actor.core.state.metrics_snapshot(0);
    assert_eq!(metrics.admission_rejected, 1);
    assert_eq!(metrics.admitted, 0);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.pending, 0);
    assert_eq!(metrics.writes, 0);
    assert!(writes.lock().unwrap().is_empty());
    assert_eq!(
        diagnostics.events(),
        vec![DiagnosticEvent::AdmissionRejected {
            target: crate::CameraId::CAMERA_1,
            lane: crate::observability::DiagnosticLane::Inquiry,
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
    let (handle, mut actor) = AsyncOwnerActor::new(policy(2), runtime).unwrap();
    let diagnostics = observe_diagnostics(&mut actor);
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
    gates.send_async(Ok(TransmissionMeta::Raw)).await.unwrap();
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
        RuntimeOutcome::Command(CommandOutcome::Applied)
    ));
    handle.shutdown().unwrap();
    let snapshot = actor_task.await.unwrap();
    assert_eq!(snapshot.state, SessionState::Shutdown);
    assert_eq!(snapshot.active, 0);
    assert_eq!(
        canonical_owner_trace(diagnostics.events().iter().copied()),
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
        .send_async(Ok(TransmissionMeta::Sony { sequence: 1 }))
        .await
        .unwrap();
    let second = handle.submit(command()).await.unwrap();
    assert_eq!(started.recv_async().await.unwrap(), second.id);
    let error = handle.try_submit(command()).err().unwrap();
    assert!(matches!(error, Error::RuntimeQueueFull { capacity: 2 }));

    let cancel_handle = handle.clone();
    let first_cancel = tokio::spawn(async move {
        let observer = cancel_handle.cancel_test(&first).await;
        (observer, first)
    });
    tokio::task::yield_now().await;
    let cancel_handle = handle.clone();
    let mut second_cancel = tokio::spawn(async move {
        let observer = cancel_handle.cancel_test(&second).await;
        (observer, second)
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut second_cancel)
            .await
            .is_err(),
        "the full dedicated lane applies backpressure"
    );

    gates
        .send_async(Ok(TransmissionMeta::Sony { sequence: 2 }))
        .await
        .unwrap();
    let (first_observation, first) = first_cancel.await.unwrap();
    let (second_observation, second) = second_cancel.await.unwrap();
    drop((first_observation.unwrap(), second_observation.unwrap()));
    drop((first, second));

    let (left, right) = (handle.shutdown(), handle.shutdown());
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
    gates.send_async(Ok(TransmissionMeta::Raw)).await.unwrap();
    handle.shutdown().unwrap();
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
    gates.send_async(Ok(TransmissionMeta::Raw)).await.unwrap();
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
            target: crate::CameraId::CAMERA_1,
            lane: crate::observability::DiagnosticLane::Command,
            error: crate::ErrorKind::BufferFull,
        }
    );

    drop(first);
    handle.shutdown().unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
/// The handle-side ingress is runtime-neutral. Exercise the same
/// fail-fast capacity path under smol without requiring a transport task:
/// the actor remains the sole writer when it drains the compact event.
#[cfg(feature = "runtime-smol")]
#[test]
fn smol_fail_fast_capacity_rejection_records_telemetry() {
    let (handle, mut actor) = AsyncOwnerActor::new(policy(1), SmolRuntime::new()).unwrap();
    let diagnostics = observe_diagnostics(&mut actor);
    let held = handle
        .core
        .permits
        .try_acquire_ordinary()
        .expect("the test reserves the only admission slot");

    let error = match handle
        .core
        .enqueue_admission::<RuntimeOutcome>(inquiry(), None)
    {
        Ok(_) => panic!("a full admission pool must fail fast"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::RuntimeQueueFull { capacity: 1 }));

    actor.core.flush_pre_admission_rejections(true);
    let metrics = actor.core.state.metrics_snapshot(0);
    assert_eq!(metrics.admission_rejected, 1);
    assert_eq!(metrics.admitted, 0);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.pending, 0);
    assert_eq!(metrics.writes, 0);
    assert_eq!(
        diagnostics.events(),
        vec![DiagnosticEvent::AdmissionRejected {
            target: crate::CameraId::CAMERA_1,
            lane: crate::observability::DiagnosticLane::Inquiry,
            error: crate::ErrorKind::BufferFull,
        }]
    );
    drop(held);
}
/// The ingress is deliberately bounded. If a burst outruns its one-slot
/// diagnostic staging queue, metrics keep the exact rejection total and
/// the staging-loss counter distinguishes the evicted event from a full
/// subscriber queue.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn pre_admission_rejection_ingress_reports_bounded_diagnostic_loss() {
    let runtime = TokioRuntime::from_current().unwrap();
    let mut owner_policy = policy(1);
    owner_policy.limits.staged_diagnostics = 1;
    let (_handle, mut actor) = AsyncOwnerActor::new(owner_policy, runtime).unwrap();
    let diagnostics = observe_diagnostics(&mut actor);
    let first = PreAdmissionRejection {
        target: crate::target::TargetSlot::CAMERA_1,
        lane: crate::runtime::engine::Lane::Command,
        error: crate::ErrorKind::BufferFull,
    };
    let second = PreAdmissionRejection {
        target: crate::target::TargetSlot::CAMERA_1,
        lane: crate::runtime::engine::Lane::Inquiry,
        error: crate::ErrorKind::IoClosed,
    };

    assert!(actor.core.admission_rejections.record(first, false));
    assert!(
        !actor.core.admission_rejections.record(second, false),
        "one bounded actor wake coalesces the burst"
    );
    actor.core.flush_pre_admission_rejections(true);

    let metrics = actor.core.state.metrics();
    assert_eq!(metrics.admission_rejected, 2);
    assert_eq!(metrics.dropped_staged_diagnostics, 1);
    assert_eq!(metrics.dropped_diagnostic_events, 0);
    assert_eq!(
        diagnostics.events(),
        vec![DiagnosticEvent::AdmissionRejected {
            target: crate::CameraId::CAMERA_1,
            lane: crate::observability::DiagnosticLane::Inquiry,
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
    let diagnostics = observe_diagnostics(&mut actor);
    let (replacement_sender, replacement_receiver) = flume::bounded(1);
    drop(replacement_sender);
    let original_receiver =
        std::mem::replace(&mut actor.core.receivers.admissions, replacement_receiver);
    drop(original_receiver);

    let error = match handle.try_submit(command()) {
        Ok(_) => panic!("a disconnected admission receiver must reject"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::OwnerTerminated));

    actor.core.flush_pre_admission_rejections(true);
    assert_eq!(actor.core.state.metrics().admission_rejected, 1);
    assert_eq!(
        diagnostics.events(),
        vec![DiagnosticEvent::AdmissionRejected {
            target: crate::CameraId::CAMERA_1,
            lane: crate::observability::DiagnosticLane::Command,
            error: crate::ErrorKind::IoClosed,
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
    gates.send_async(Ok(TransmissionMeta::Raw)).await.unwrap();
    frames
        .send_async(batch(vec![ack(ViscaSocket::S1)]))
        .await
        .unwrap();
    let cancel_handle = handle.clone();
    let cancel_task = tokio::spawn(async move {
        let observer = cancel_handle.cancel_test(&operation).await;
        (observer, operation)
    });
    let _cancel_write = started.recv_async().await.unwrap();
    gates.send_async(Ok(TransmissionMeta::Raw)).await.unwrap();
    let (cancellation, operation) = cancel_task.await.unwrap();
    let cancellation = cancellation.unwrap();
    frames
        .send_async(batch(vec![DecodedFrame {
            target: crate::target::TargetSlot::CAMERA_1,
            sequence: None,
            response: DecodedResponse::Error {
                socket: Some(ViscaSocket::S1),
                code: 0x04,
            },
        }]))
        .await
        .unwrap();
    // The cancellation concludes on the operation's terminal slot.
    let terminal = operation.terminal().await.unwrap();
    assert_eq!(
        cancellation_outcome(match &terminal {
            RuntimeOutcome::Command(outcome) => outcome,
            _ => panic!("command terminal"),
        })
        .unwrap(),
        CancellationOutcome::Cancelled
    );
    assert!(cancellation.try_recv().is_none());
    handle.shutdown().unwrap();
    assert_eq!(actor_task.await.unwrap().active, 0);
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
        gates.send_async(Ok(TransmissionMeta::Raw)).await.unwrap();
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
            RuntimeOutcome::Command(CommandOutcome::Applied)
        ));
        handle.shutdown().unwrap();
        let snapshot = task.await;
        assert_eq!(snapshot.state, SessionState::Shutdown);
        assert_eq!(snapshot.active, 0);
    });
}

/// Keep the actor alive between conclusion and Drop: post-terminal handle
/// calls must answer immediately, and sends that raced publication must not
/// revise metrics when the final drain answers them.
async fn terminal_handle_work_and_metrics_are_stable<R: Executor>(runtime: R) {
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBrc300>().unwrap();
    let (handle, mut actor) = AsyncOwnerActor::new(policy(1), runtime.clone()).unwrap();
    let harness = harness();
    let mut driver = harness.driver;
    let submitting = handle.submit_operation(prepared_zoom(&profile));
    let mut submitting = std::pin::pin!(submitting);
    assert!(future::poll_once(submitting.as_mut()).await.is_none());
    let admission = actor.core.receivers.admissions.try_recv().unwrap();
    harness.gates.try_send(Ok(TransmissionMeta::Raw)).unwrap();
    actor
        .handle_event(
            OwnerEvent::Admission(Ok(admission)),
            &mut driver,
            Executor::now(&runtime),
            false,
        )
        .await;
    let mut operation = submitting.await.unwrap();
    actor
        .handle_event(
            OwnerEvent::Receive {
                result: batch(vec![ack(ViscaSocket::S1), completion(ViscaSocket::S1)]),
                received_at: Executor::now(&runtime),
            },
            &mut driver,
            Executor::now(&runtime),
            false,
        )
        .await;
    handle.shutdown().unwrap();
    actor
        .handle_event(
            OwnerEvent::Shutdown,
            &mut driver,
            Executor::now(&runtime),
            false,
        )
        .await;
    actor.core.conclude();
    let published = handle.metrics().await.unwrap();
    assert_eq!(published.session, crate::SessionStatus::Shutdown);

    // A buffered successful terminal still wins over the session error (#777).
    assert_eq!(
        operation.cancel(None).await.unwrap(),
        CancellationOutcome::Completed
    );
    for _ in 0..8 {
        let (_observer, cell) = CancellationObserver::pair();
        let request = CancellationRequest {
            id: operation.observation.id(),
            observer: cell,
        };
        let deadline = handle.deadline_after(Duration::from_secs(1)).unwrap();
        assert!(matches!(
            future::poll_once(handle.request_cancellation(request, deadline)).await,
            Some(Err(Error::RuntimeShutdown))
        ));
        assert!(actor.core.receivers.cancellations.is_empty());
        let halt = crate::prepared::PreparedHalt {
            target: TargetSlot::CAMERA_1,
            axes: None,
            requests: [None, None, None],
            budget: Duration::from_secs(1),
        };
        assert!(matches!(
            future::poll_once(handle.halt(halt, Executor::now(&runtime))).await,
            Some(Err(Error::RuntimeShutdown))
        ));
        assert!(actor.core.receivers.cancellations.is_empty());
        assert!(matches!(
            future::poll_once(handle.subscribe_diagnostics(1)).await,
            Some(Err(Error::RuntimeShutdown))
        ));
        assert!(actor.core.receivers.control.is_empty());
        assert_eq!(handle.metrics().await.unwrap(), published);
        assert_eq!(handle.core.terminal_metrics().unwrap().unwrap(), published);
    }

    // Model one cancellation and control send that passed their lifecycle
    // check before the first publication but reached the queues after it.
    let (_observer, cell) = CancellationObserver::pair();
    let (reply, cancelled) = flume::bounded(1);
    handle
        .core
        .cancellations
        .try_send(
            super::super::super::boundary::CancellationBoundary::Cancel {
                request: CancellationRequest {
                    id: operation.observation.id(),
                    observer: cell,
                },
                reply,
            },
        )
        .unwrap();
    let (reply, controlled) = flume::bounded(1);
    handle
        .core
        .control
        .try_send(
            super::super::super::boundary::ControlBoundary::SubscribeDiagnostics {
                capacity: 1,
                reply,
            },
        )
        .unwrap();
    assert_eq!(handle.metrics().await.unwrap(), published);
    drop(actor);
    assert!(matches!(
        cancelled.try_recv().unwrap(),
        Err(Error::RuntimeShutdown)
    ));
    assert!(matches!(
        controlled.try_recv().unwrap(),
        Err(Error::RuntimeShutdown)
    ));
    for _ in 0..8 {
        assert_eq!(handle.metrics().await.unwrap(), published);
    }
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_terminal_handle_work_is_refused_and_metrics_stay_published_once() {
    terminal_handle_work_and_metrics_are_stable(TokioRuntime::from_current().unwrap()).await;
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_terminal_handle_work_is_refused_and_metrics_stay_published_once() {
    smol::block_on(terminal_handle_work_and_metrics_are_stable(
        SmolRuntime::new(),
    ));
}

#[cfg(feature = "test-utils")]
#[test]
fn manual_clock_terminal_handle_work_is_refused_and_metrics_stay_published_once() {
    let clock = crate::testing::testkit::ManualClock::new();
    let runtime = crate::testing::testkit::ManualClockExecutor::local(&clock);
    runtime.run_until(terminal_handle_work_and_metrics_are_stable(runtime.clone()));
}

#[cfg(feature = "test-utils")]
#[test]
fn manual_clock_terminal_metrics_stay_identical_during_concurrent_cancellation_and_shutdown() {
    use crate::testing::testkit::{ManualClock, ManualClockExecutor};

    let clock = ManualClock::new();
    let runtime = ManualClockExecutor::local(&clock);
    runtime.run_until(async {
        let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime.clone()).unwrap();
        let harness = harness();
        harness.gates.try_send(Ok(TransmissionMeta::Raw)).unwrap();
        harness.gates.try_send(Ok(TransmissionMeta::Raw)).unwrap();
        let client = async {
            let operation = handle.submit(command()).await.unwrap();
            harness
                .frames
                .send_async(batch(vec![ack(ViscaSocket::S1)]))
                .await
                .unwrap();
            let (started, ready) = flume::bounded(1);
            let cancelling = async {
                let mut snapshots = Vec::new();
                for index in 0..128 {
                    let (_observer, cell) = CancellationObserver::pair();
                    let request = CancellationRequest {
                        id: operation.id,
                        observer: cell,
                    };
                    let deadline = handle.deadline_after(Duration::from_secs(1)).unwrap();
                    assert!(matches!(
                        handle.request_cancellation(request, deadline).await,
                        Ok(()) | Err(Error::RuntimeShutdown)
                    ));
                    if index == 0 {
                        started.send_async(()).await.unwrap();
                    }
                    if let Some(snapshot) = handle.core.terminal_metrics() {
                        snapshots.push(snapshot.unwrap());
                    }
                    future::yield_now().await;
                }
                snapshots
            };
            let shutdown = async {
                ready.recv_async().await.unwrap();
                handle.shutdown().unwrap();
            };
            let (snapshots, ()) = future::zip(cancelling, shutdown).await;
            handle.wait_closed().await.unwrap();
            let published = handle.metrics().await.unwrap();
            assert_eq!(published.session, crate::SessionStatus::Shutdown);
            for snapshot in snapshots {
                assert_eq!(snapshot, published);
            }
            for _ in 0..8 {
                assert_eq!(handle.metrics().await.unwrap(), published);
            }
        };
        let ((), _) = future::zip(client, actor.run(harness.driver)).await;
    });
}
