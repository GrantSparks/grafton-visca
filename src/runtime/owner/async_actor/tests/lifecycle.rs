use super::*;
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn ready_transport_close_precedes_shutdown_and_queued_admission() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
    let harness = harness();
    let frames = harness.frames.clone();
    let admission = handle.try_submit(command()).unwrap();
    handle.shutdown().await.unwrap();
    frames.send_async(Ok(AsyncReceive::Closed)).await.unwrap();
    let snapshot = actor.run(harness.driver).await;
    assert_eq!(snapshot.state, SessionState::Closed);
    assert!(matches!(
        admission.await.unwrap_err(),
        Error::ConnectionClosed { .. }
    ));
    let frame_or_close = snapshot
        .diagnostics
        .iter()
        .position(|event| matches!(event, DiagnosticEvent::SessionChanged { .. }))
        .unwrap();
    assert_eq!(frame_or_close, 0, "close is the first applied ready source");
}
/// The engine's terminal verdict is published while `handle_event` drives
/// its `SessionChanged` effect, not only in `run`'s epilogue. Keeping this
/// seam direct makes the race deterministic: the actor has returned `Stop`
/// but still owns a live shutdown receiver, exactly where a pre-fix caller
/// could enqueue and incorrectly receive `Ok(())`.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn terminal_stop_is_published_before_shutdown_can_enter_the_live_lane() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, mut actor) = AsyncOwnerActor::new(policy(1), runtime.clone()).unwrap();
    let mut driver = harness().driver;

    let outcome = actor
        .handle_event(
            ActorEvent::Receive {
                result: Ok(AsyncReceive::Closed),
                received_at: Executor::now(&runtime),
            },
            &mut driver,
            &runtime,
            Executor::now(&runtime),
            false,
        )
        .await;
    assert_eq!(outcome, TurnOutcome::Stop);

    let error = handle.shutdown().await.unwrap_err();
    assert!(matches!(error, Error::ConnectionClosed { .. }));
    assert!(
        actor.shutdown.is_empty(),
        "a terminal session must not retain a shutdown signal it can never poll"
    );
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn ready_frame_is_observed_before_explicit_shutdown() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
    let harness = harness();
    let frames = harness.frames.clone();
    frames
        .send_async(batch(vec![DecodedFrame {
            target: CameraId::CAMERA_1,
            sequence: None,
            response: DecodedResponse::Unknown,
        }]))
        .await
        .unwrap();
    handle.shutdown().await.unwrap();
    let snapshot = actor.run(harness.driver).await;
    let frame = snapshot
        .diagnostics
        .iter()
        .position(|event| matches!(event, DiagnosticEvent::FrameReceived { .. }))
        .unwrap();
    let shutdown = snapshot
        .diagnostics
        .iter()
        .position(|event| matches!(event, DiagnosticEvent::SessionChanged { .. }))
        .unwrap();
    assert!(frame < shutdown);
    assert_eq!(snapshot.state, SessionState::Shutdown);
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn shutdown_drain_reuses_buffered_operation_receiver_for_queued_cancel() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let frames = harness.frames.clone();
    let writes = Arc::clone(&harness.writes);
    let actor_task = tokio::spawn(actor.run(harness.driver));

    let operation = handle.submit(command()).await.unwrap();
    let _ = started.recv_async().await.unwrap();
    let cancel_handle = handle.clone();
    let cancel_task = tokio::spawn(async move { cancel_handle.cancel_test(operation).await });
    while handle.cancellations.is_empty() {
        tokio::task::yield_now().await;
    }
    frames
        .send_async(batch(vec![
            ack(ViscaSocket::S1),
            completion(ViscaSocket::S1),
        ]))
        .await
        .unwrap();
    handle.shutdown().await.unwrap();
    gates
        .send_async(Ok(TransmissionMeta { sequence: None }))
        .await
        .unwrap();

    let cancellation = cancel_task.await.unwrap().unwrap();
    assert!(matches!(
        cancellation.recv_test().await.unwrap(),
        CancellationObservation::Completed
    ));
    assert_eq!(
        writes
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, _, cancellation)| *cancellation)
            .count(),
        0,
        "queued cancellation is never written before shutdown"
    );
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn stream_poison_resolves_active_and_drains_unstaged_boundary() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(stream_policy(2), runtime).unwrap();
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let actor_task = tokio::spawn(actor.run(harness.driver));

    let first = handle.submit(command()).await.unwrap();
    assert_eq!(started.recv_async().await.unwrap(), first.id);
    let queued_handle = handle.clone();
    let queued = tokio::spawn(async move { queued_handle.submit(command()).await });
    while handle.permits.available() != 0 {
        tokio::task::yield_now().await;
    }
    // The permit is acquired before the bounded boundary send. One more
    // yield lets that infallible next step enqueue before poison is released.
    tokio::task::yield_now().await;
    gates.send_async(Err(Error::Timeout)).await.unwrap();

    assert!(matches!(
        first.terminal().await.unwrap(),
        RuntimeOutcome::Failed(Error::StreamPoisoned { .. })
    ));
    assert!(matches!(
        queued.await.unwrap().unwrap_err(),
        Error::StreamPoisoned { .. }
    ));
    let snapshot = actor_task.await.unwrap();
    assert_eq!(snapshot.state, SessionState::Poisoned);
    assert_eq!(snapshot.active, 0);
    assert_eq!(
        snapshot
            .metrics
            .dropped_boundary_work
            .saturating_add(snapshot.metrics.admission_rejected),
        1,
        "queued work is either drained before staging or rejected by the poisoned engine"
    );
}
/// A task that disappears before `run` can publish its terminal result is
/// not an orderly runtime shutdown. Every handle-facing wait must fail
/// closed instead of manufacturing `RuntimeShutdown` or success.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn actor_disconnect_without_terminal_result_fails_closed() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
    let actor_task = tokio::spawn(actor.run(PanickingReceiveDriver));

    let error = tokio::time::timeout(Duration::from_secs(1), handle.wait_closed())
        .await
        .expect("liveness must observe the panicking actor")
        .unwrap_err();
    assert!(matches!(
        error,
        Error::InvalidState(message)
            if message.contains("without publishing a terminal result")
    ));
    assert!(actor_task.await.is_err(), "the test driver must panic");

    let error = handle.shutdown().await.unwrap_err();
    assert!(matches!(
        error,
        Error::InvalidState(message)
            if message.contains("without publishing a terminal result")
    ));
}
/// A boundary admission queued before an actor panic is answered by the
/// same fail-closed terminal result observed by later handle calls, and
/// its permit is returned. This is the bounded ownership guarantee
/// (#542 §4; architecture §Operational invariants) without relying on a
/// scheduler sleep to arrange the panic/admission order.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn queued_admission_actor_disconnect_fails_closed_and_releases_capacity() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();

    let queued = handle.try_submit(inquiry()).unwrap();
    assert_eq!(handle.permits.available(), 0);

    // The receive-first actor turn panics before it can consume the
    // already-buffered admission. Awaiting the task makes Drop's final
    // boundary drain a deterministic happens-before edge for the checks
    // below; no timing or polling sleep is involved.
    let actor_task = tokio::spawn(actor.run(PanickingReceiveDriver));
    assert!(actor_task.await.is_err(), "the test driver must panic");

    let queued_error = queued.await.unwrap_err();
    assert!(matches!(
        &queued_error,
        Error::InvalidState(message)
            if message.contains("without publishing a terminal result")
    ));
    let published = handle.shutdown().await.unwrap_err();
    assert_eq!(queued_error.to_string(), published.to_string());
    assert_eq!(
        handle.permits.available(),
        handle.permits.capacity(),
        "the drained admission must return its permit"
    );
}
/// An admitted request whose actor disappeared must fail closed promptly,
/// rather than wait for its long protocol deadline or report an orderly
/// `RuntimeShutdown`.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn active_receipt_actor_disconnect_fails_closed() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
    let (panic_signal, panic_signal_rx) = flume::bounded(1);
    let actor_task = tokio::spawn(actor.run(PanickingAfterAdmissionDriver {
        panic_signal: panic_signal_rx,
    }));

    let receipt = tokio::time::timeout(Duration::from_secs(1), handle.submit(inquiry()))
        .await
        .expect("admission must complete before the actor panic")
        .unwrap();
    assert_eq!(handle.snapshot().await.unwrap().active, 1);
    panic_signal.send_async(()).await.unwrap();

    let error = tokio::time::timeout(
        Duration::from_secs(1),
        wait_core_for(receipt, handle.receipt_control(), Duration::from_secs(5)),
    )
    .await
    .expect("receipt wait must observe the actor disappearance")
    .unwrap_err();
    assert!(matches!(
        error,
        Error::InvalidState(message)
            if message.contains("without publishing a terminal result")
    ));
    assert!(actor_task.await.is_err(), "the test driver must panic");
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn active_cancellation_actor_disconnect_fails_closed() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
    let (panic_signal, panic_signal_rx) = flume::bounded(1);
    let actor_task = tokio::spawn(actor.run(PanickingAfterAdmissionDriver {
        panic_signal: panic_signal_rx,
    }));

    let receipt = tokio::time::timeout(Duration::from_secs(1), handle.submit(command()))
        .await
        .expect("admission must complete before the actor panic")
        .unwrap();
    assert_eq!(handle.snapshot().await.unwrap().active, 1);
    let cancellation = handle.cancel_test(receipt).await.unwrap();
    panic_signal.send_async(()).await.unwrap();

    let error = tokio::time::timeout(
        Duration::from_secs(1),
        cancellation.outcome(handle.receipt_control(), Duration::from_secs(5)),
    )
    .await
    .expect("cancellation wait must observe the actor disappearance")
    .unwrap_err();
    assert!(matches!(
        error,
        Error::InvalidState(message)
            if message.contains("without publishing a terminal result")
    ));
    assert!(actor_task.await.is_err(), "the test driver must panic");
}
/// Issue #626. `run` drains the boundary lanes once and then drops its
/// receivers. A message that lands in between used to be stranded forever:
/// this handle's own sender keeps flume's queue alive, and with it the
/// reply sender inside the stranded message, so the caller's wait never
/// disconnected. The losing caller is typically the one that just watched
/// the session die and immediately asked a follow-up question.
///
/// The window is only reachable when the caller runs on another thread, so
/// this drives a multi-threaded runtime and repeats enough to hit it.
///
/// The driver is deliberately not `harness()`'s. That one parks in `write`
/// until the test releases a gate, which is exactly what several ordering
/// tests need and exactly wrong here: a command admitted before the close
/// is read would park the actor mid-transmission, and every later question
/// would then hang on an actor that is stalled rather than racing its own
/// teardown. That is a property of the fake transport, not of the boundary,
/// and it is not what this probe is for.
#[cfg(feature = "runtime-tokio")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_boundary_request_racing_teardown_never_hangs() {
    for iteration in 0..64 {
        let runtime = TokioRuntime::from_current().unwrap();
        let (handle, actor) = AsyncOwnerActor::new(policy(8), runtime).unwrap();
        let (frames, receives) = flume::bounded(1);
        let actor_task = tokio::spawn(actor.run(UngatedDriver { receives }));

        let asking = handle.clone();
        let questions = tokio::spawn(async move {
            // Keep asking until the session answers with its terminal
            // error. Under the bug one of these calls parks forever.
            loop {
                if asking.metrics().await.is_err() {
                    break;
                }
                if asking.snapshot().await.is_err() {
                    break;
                }
                match asking.submit(command()).await {
                    Ok(receipt) => drop(receipt),
                    // Nothing acknowledges these commands, so the lane
                    // fills up and stays full. Capacity is a "ask again"
                    // answer, not a terminal one: the probe is only over
                    // when the session itself answers.
                    Err(Error::RuntimeQueueFull { .. }) => {}
                    Err(_) => break,
                }
            }
        });

        // Let the questioner get in flight first, so the close lands while
        // boundary work is actually moving. The exact interleaving is left
        // to the scheduler; over this many iterations both orders occur.
        tokio::task::yield_now().await;
        frames.send_async(Ok(AsyncReceive::Closed)).await.unwrap();

        // The bug this guards is an unbounded park, so the bound only has
        // to be longer than a healthy teardown ever takes. It is generous
        // because CI runners are small, not because the answer is slow.
        tokio::time::timeout(Duration::from_secs(30), questions)
            .await
            .unwrap_or_else(|_| {
                panic!("iteration {iteration}: a boundary request outlived the actor")
            })
            .unwrap();
        let snapshot = tokio::time::timeout(Duration::from_secs(30), actor_task)
            .await
            .unwrap_or_else(|_| panic!("iteration {iteration}: the actor never finished"))
            .unwrap();
        assert_eq!(snapshot.state, SessionState::Closed);
    }
}
/// The drain still answers what it can see: a cancellation queued before
/// teardown keeps its buffered terminal observation rather than being
/// replaced by the session error.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn teardown_answers_queued_work_before_the_liveness_lane_fires() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
    let harness = harness();
    let frames = harness.frames.clone();
    let control_handle = handle.clone();
    let queued = tokio::spawn(async move { control_handle.metrics().await });
    while handle.control.is_empty() {
        tokio::task::yield_now().await;
    }
    frames.send_async(Ok(AsyncReceive::Closed)).await.unwrap();
    let snapshot = actor.run(harness.driver).await;
    assert_eq!(snapshot.state, SessionState::Closed);
    assert!(matches!(
        queued.await.unwrap().unwrap_err(),
        Error::ConnectionClosed { .. }
    ));
}
