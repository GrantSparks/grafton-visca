use super::*;
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn async_receive_batch_precedes_a_completion_deadline_at_equality() {
    let runtime = ManualRuntime::new(Instant::now());
    let (handle, actor) = AsyncOwnerActor::new(policy(2), runtime.clone()).unwrap();
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let frames = harness.frames.clone();
    let actor_task = tokio::spawn(actor.run(harness.driver));

    let first = handle.submit(command()).await.unwrap();
    assert_eq!(started.recv_async().await.unwrap(), first.id);
    gates
        .send_async(Ok(TransmissionMeta { sequence: None }))
        .await
        .unwrap();
    frames
        .send_async(batch(vec![ack(ViscaSocket::S1)]))
        .await
        .unwrap();
    assert_eq!(handle.snapshot().await.unwrap().active, 1);
    let completion_deadline = Executor::now(&runtime)
        .checked_add(Duration::from_secs(5))
        .expect("the fixed completion deadline must be representable");

    let second = handle.submit(command()).await.unwrap();
    assert_eq!(started.recv_async().await.unwrap(), second.id);
    gates
        .send_async(Ok(TransmissionMeta { sequence: None }))
        .await
        .unwrap();
    assert_eq!(handle.snapshot().await.unwrap().active, 2);

    // Deliver ACK S2 and completion S1 at S1's exact completion deadline.
    // The ordered receive batch must settle S1 before due work runs; a
    // strictly late correlated completion is rejected by the engine.
    runtime.advance(Duration::from_secs(5));
    assert_eq!(Executor::now(&runtime), completion_deadline);
    frames
        .send_async(batch(vec![
            ack(ViscaSocket::S2),
            completion(ViscaSocket::S1),
        ]))
        .await
        .unwrap();
    assert_eq!(handle.snapshot().await.unwrap().active, 1);
    let first_outcome = first.terminal().await.unwrap();
    assert!(
        matches!(first_outcome, RuntimeOutcome::Applied),
        "completion from the receive batch must win: {first_outcome:?}"
    );

    drop(second);
    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
/// A receive sampled even one nanosecond after its correlated completion
/// deadline is ordinary late input: the frame is ignored, the due
/// transition reports the command's unconfirmed outcome immediately while
/// a keyed hold keeps the raw socket unavailable. This keeps the
/// actor-level overdue half of the strict boundary contract alongside the
/// equality test above (#731/#723).
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn async_receive_batch_rejects_an_overdue_completion() {
    let runtime = ManualRuntime::new(Instant::now());
    let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime.clone()).unwrap();
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let frames = harness.frames.clone();
    let actor_task = tokio::spawn(actor.run(harness.driver));

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
    assert_eq!(handle.snapshot().await.unwrap().active, 1);

    runtime.advance(Duration::from_secs(5) + Duration::from_nanos(1));
    frames
        .send_async(batch(vec![completion(ViscaSocket::S1)]))
        .await
        .unwrap();
    let snapshot = handle.snapshot().await.unwrap();
    assert_eq!(
        snapshot.active, 0,
        "a correlation hold is not a live request"
    );
    assert!(snapshot.diagnostics.iter().any(|event| matches!(
        event,
        DiagnosticEvent::Ignored(IgnoreReason::UnmatchedFrame)
    )));
    let outcome = terminal_within_test_deadline(
        &receipt,
        "the overdue completion decides the command immediately",
    )
    .await;
    assert!(matches!(
        outcome,
        RuntimeOutcome::Failed(Error::UnsequencedCommandUnconfirmed)
    ));

    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
/// Datagram input has no retained framer, but the pre-H idle/readiness
/// race is the same actor-level source-order bug as the stream path.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_datagram_release_probe_precedes_stale_frame_after_idle_sleep() {
    assert_raw_release_probe_precedes_stale_frame_after_idle_sleep(policy(1)).await;
}
/// The stream path must use the identical exact-release probe before its
/// own framing-specific retained-prefix rules are considered.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_stream_release_probe_precedes_stale_frame_after_idle_sleep() {
    assert_raw_release_probe_precedes_stale_frame_after_idle_sleep(stream_policy(1)).await;
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_datagram_pre_h_idle_read_resumed_at_h_requires_fresh_probe() {
    assert_pre_h_idle_read_resumed_at_h_requires_fresh_probe(policy(1)).await;
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_stream_pre_h_idle_read_resumed_at_h_requires_fresh_probe() {
    assert_pre_h_idle_read_resumed_at_h_requires_fresh_probe(stream_policy(1)).await;
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_datagram_release_idle_fault_fences_once() {
    assert_raw_release_idle_fault_fences_once(policy(1)).await;
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_stream_release_idle_fault_fences_once() {
    assert_raw_release_idle_fault_fences_once(stream_policy(1)).await;
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_datagram_parked_cross_target_write_returns_to_raw_coordinator() {
    assert_parked_cross_target_write_returns_to_raw_coordinator(two_target_raw_policy(
        TransportKind::Datagram,
    ))
    .await;
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_stream_parked_cross_target_write_returns_to_raw_coordinator() {
    assert_parked_cross_target_write_returns_to_raw_coordinator(two_target_raw_policy(
        TransportKind::Stream,
    ))
    .await;
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_datagram_release_fault_then_stale_frame_stays_input_first() {
    assert_raw_release_fault_then_stale_frame_stays_input_first(policy(1)).await;
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_stream_release_fault_then_stale_frame_stays_input_first() {
    assert_raw_release_fault_then_stale_frame_stays_input_first(stream_policy(1)).await;
}
/// Retained stream input is bounded by verified decoder progress, not an
/// arbitrary number of actor turns. More than 64 notifications may extend
/// one incomplete prefix before H; the release must discard that prefix
/// exactly once and leave the successor usable.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_stream_more_than_sixty_four_retained_turns_release_by_progress() {
    let initial = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(initial);
    let mut policy = stream_policy(1);
    // Keep the receive-first run longer than this regression so its old
    // count-based cap would have been the only forced boundary.
    const RETAINED_TURNS: usize = 65;
    policy.limits.frames_per_receive = RETAINED_TURNS + 1;
    let (handle, actor) = AsyncOwnerActor::new(policy, runtime.clone()).unwrap();
    let mut harness = boundary_stream_harness(false);
    let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
    let successor = establish_raw_inquiry_tombstone(&handle, &harness).await;
    while harness.reads_observed.try_recv().is_ok() {}

    for _ in 0..RETAINED_TURNS {
        harness.reads.try_send(ScriptedRawRead::Prefix).unwrap();
    }
    for _ in 0..RETAINED_TURNS {
        harness.reads_observed.recv_async().await.unwrap();
    }

    runtime.advance(Duration::from_secs(1));
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), harness.writes.recv_async())
            .await
            .expect("verified progress releases the retained raw prefix")
            .unwrap(),
        successor.id
    );
    assert_eq!(
        harness.discards.load(Ordering::Relaxed),
        1,
        "the accumulated prefix is discarded once, independent of turn count"
    );

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
/// A tail ready at the exact raw tombstone expiry is decoded and made
/// inert before the due pass releases the successor. This is the original
/// split-tail correlation regression.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_stream_split_tail_at_tombstone_boundary_precedes_release() {
    let now = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(now);
    let (handle, actor) = AsyncOwnerActor::new(stream_policy(1), runtime.clone()).unwrap();
    let mut harness = boundary_stream_harness(false);
    let reads = harness.reads.clone();
    let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
    let successor = establish_raw_inquiry_tombstone(&handle, &harness).await;

    reads.send_async(ScriptedRawRead::Prefix).await.unwrap();
    while !harness.buffered.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }
    runtime.advance(Duration::from_secs(1));
    reads
        .send_async(ScriptedRawRead::Tail(raw_inquiry_reply(0xa1)))
        .await
        .unwrap();

    assert_eq!(harness.writes.recv_async().await.unwrap(), successor.id);
    reads
        .send_async(ScriptedRawRead::Complete(raw_inquiry_reply(0xb2)))
        .await
        .unwrap();
    assert!(matches!(
        successor.terminal().await.unwrap(),
        RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
    ));
    assert_eq!(harness.discards.load(Ordering::Relaxed), 0);

    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
/// Retained stream input survives a transient-fault turn. Even when both
/// fault and tail are queued at the exact boundary, the completing tail
/// keeps receive precedence until it has been made inert.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_stream_prefix_fault_tail_keeps_boundary_input_precedence() {
    let now = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(now);
    let (handle, actor) = AsyncOwnerActor::new(stream_policy(1), runtime.clone()).unwrap();
    let mut harness = boundary_stream_harness(false);
    let reads = harness.reads.clone();
    let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
    let successor = establish_raw_inquiry_tombstone(&handle, &harness).await;

    reads.send_async(ScriptedRawRead::Prefix).await.unwrap();
    while !harness.buffered.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }
    runtime.advance(Duration::from_secs(1));
    reads
        .send_async(ScriptedRawRead::Fault(Error::Io(Arc::new(
            std::io::Error::from(std::io::ErrorKind::ConnectionRefused),
        ))))
        .await
        .unwrap();
    reads
        .send_async(ScriptedRawRead::Tail(raw_inquiry_reply(0xa1)))
        .await
        .unwrap();

    assert_eq!(harness.writes.recv_async().await.unwrap(), successor.id);
    reads
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
/// The idle/no-data classification also preserves a positively identified
/// prefix instead of yielding the due raw boundary ahead of its ready tail.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_stream_prefix_idle_tail_keeps_boundary_input_precedence() {
    let now = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(now);
    let (handle, actor) = AsyncOwnerActor::new(stream_policy(1), runtime.clone()).unwrap();
    let mut harness = boundary_stream_harness(false);
    let reads = harness.reads.clone();
    let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
    let successor = establish_raw_inquiry_tombstone(&handle, &harness).await;

    reads.send_async(ScriptedRawRead::Prefix).await.unwrap();
    while !harness.buffered.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }
    runtime.advance(Duration::from_secs(1));
    reads.send_async(ScriptedRawRead::NoData).await.unwrap();
    reads
        .send_async(ScriptedRawRead::Tail(raw_inquiry_reply(0xa1)))
        .await
        .unwrap();

    assert_eq!(harness.writes.recv_async().await.unwrap(), successor.id);
    reads
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
/// If no completing tail is ready at release, the old prefix is discarded
/// before the successor writes. A later tail therefore cannot become the
/// successor's response.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_stream_orphan_prefix_is_reset_before_successor_dispatch() {
    let now = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(now);
    let (handle, actor) = AsyncOwnerActor::new(stream_policy(1), runtime.clone()).unwrap();
    let mut harness = boundary_stream_harness(false);
    let reads = harness.reads.clone();
    let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
    let successor = establish_raw_inquiry_tombstone(&handle, &harness).await;

    reads.send_async(ScriptedRawRead::Prefix).await.unwrap();
    while !harness.buffered.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }
    runtime.advance(Duration::from_secs(1));
    let _ = handle.snapshot().await.unwrap();
    assert_eq!(harness.writes.recv_async().await.unwrap(), successor.id);
    assert_eq!(
        harness.discards.load(Ordering::Relaxed),
        1,
        "the orphaned fragment is discarded exactly once"
    );

    reads
        .send_async(ScriptedRawRead::Tail(raw_inquiry_reply(0xa1)))
        .await
        .unwrap();
    reads
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
/// A decoder that claims to discard but retains the old prefix makes safe
/// correlation release impossible. Fail closed instead of dispatching the
/// successor behind unverifiable framing state.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_stream_nonclearing_decoder_poisons_at_release() {
    let now = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(now);
    let (handle, actor) = AsyncOwnerActor::new(stream_policy(1), runtime.clone()).unwrap();
    let mut harness = boundary_stream_harness(true);
    let reads = harness.reads.clone();
    let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
    let successor = establish_raw_inquiry_tombstone(&handle, &harness).await;

    reads.send_async(ScriptedRawRead::Prefix).await.unwrap();
    while !harness.buffered.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }
    runtime.advance(Duration::from_secs(1));
    let _ = handle.snapshot().await.unwrap();

    assert!(matches!(
        successor.terminal().await.unwrap(),
        RuntimeOutcome::Failed(Error::StreamPoisoned { .. })
    ));
    assert_eq!(
        harness.discards.load(Ordering::Relaxed),
        1,
        "a non-clearing decoder fails on the first unverifiable discard"
    );
    assert_eq!(actor_task.await.unwrap().state, SessionState::Poisoned);
}
/// An ambiguous retained prefix consumes elapsed grace, not a fixed number
/// of immediately-ready actor turns. At expiry it is discarded and the
/// successor remains usable (#713).
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_stream_ambiguous_prefix_expires_without_poisoning_successor() {
    let now = Instant::now();
    let (runtime, sleeps) = ManualRuntime::with_polling_sleeps_and_sleep_barrier(now);
    let (handle, actor) = AsyncOwnerActor::new(stream_policy(1), runtime.clone()).unwrap();
    let mut harness = boundary_stream_harness(false);
    // Only an identity-weak prefix is unresolved at an inquiry release.
    // The default named-terminal fixture is intentionally discarded by
    // policy, which is correct for its other stale-prefix tests but cannot
    // exercise this fail-closed deferral cap.
    harness.driver.as_mut().unwrap().prefix_kind =
        crate::protocol::framer::RawIncompletePrefix::SourceOnly;
    let reads = harness.reads.clone();
    let actor_task = tokio::spawn(actor.run(harness.driver.take().unwrap()));
    let successor = establish_raw_inquiry_tombstone(&handle, &harness).await;

    reads.send_async(ScriptedRawRead::Prefix).await.unwrap();
    while !harness.buffered.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }
    // Move to H and wake the old timer. The engine now establishes one
    // 100 ms grace deadline for the retained source-only prefix.
    runtime.advance(Duration::from_secs(1));
    loop {
        if sleeps.recv_async().await.unwrap() == Duration::from_millis(100) {
            break;
        }
    }
    runtime.advance(Duration::from_millis(100));
    let successor_id = tokio::time::timeout(Duration::from_secs(1), harness.writes.recv_async())
        .await
        .expect("the successor writes after the orphan grace")
        .unwrap();
    assert_eq!(successor_id, successor.id);
    assert_eq!(harness.discards.load(Ordering::Relaxed), 1);

    reads
        .try_send(ScriptedRawRead::Complete(raw_inquiry_reply(0xb2)))
        .unwrap();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(1), successor.terminal())
            .await
            .expect("the successor remains usable after orphan discard")
            .unwrap(),
        RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
    ));
    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
/// A completed tail can perform the due release from `finish_input_turn`,
/// without another `Wake` arm. Its former ambiguous-prefix deferrals must
/// not be charged to a later, independent raw hold.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_stream_completed_tail_resets_next_hold_grace_budget() {
    let initial = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(initial);
    let (handle, mut actor) = AsyncOwnerActor::new(stream_policy(1), runtime.clone()).unwrap();
    let mut harness = boundary_stream_harness(false);
    let mut driver = harness.driver.take().unwrap();
    // Make retained bytes deliberately unkeyed so the engine assigns a
    // grace deadline rather than discarding immediately.
    driver.prefix_kind = crate::protocol::framer::RawIncompletePrefix::SourceOnly;

    // A times out after its successful write, which installs the genuine
    // late-reply hold retained by #712. B remains queued behind it.
    let (a_completion, a_admitted) = handle.enqueue_admission(timed_out_inquiry(), None).unwrap();
    let a_boundary = actor.admissions.try_recv().unwrap();
    actor
        .handle_admission(a_boundary, &mut driver, &runtime, Executor::now(&runtime))
        .await;
    let a = a_admitted.recv_async().await.unwrap().unwrap();
    assert_eq!(harness.writes.recv_async().await.unwrap(), a);
    assert!(matches!(
        a_completion.recv_async().await.unwrap(),
        ReceiptObservation::Terminal(RuntimeOutcome::Failed(Error::Timeout))
    ));

    let (b_completion, b_admitted) = handle.enqueue_admission(timed_out_inquiry(), None).unwrap();
    let b_boundary = actor.admissions.try_recv().unwrap();
    actor
        .handle_admission(b_boundary, &mut driver, &runtime, Executor::now(&runtime))
        .await;
    let _b = b_admitted.recv_async().await.unwrap().unwrap();
    assert!(harness.writes.try_recv().is_err());

    driver.buffered.store(true, Ordering::Release);
    runtime.advance(Duration::from_secs(1));
    assert_eq!(
        actor
            .handle_event(
                ActorEvent::Wake,
                &mut driver,
                &runtime,
                Executor::now(&runtime),
                false,
            )
            .await,
        TurnOutcome::ContinueBuffered
    );
    assert!(actor.raw_release.await_until().is_some());

    // The completing tail is real decoded input. Because no fragment is
    // retained, this turn runs the due release and dispatches B directly.
    driver.buffered.store(false, Ordering::Release);
    let now = Executor::now(&runtime);
    assert_eq!(
        actor
            .handle_event(
                ActorEvent::Receive {
                    result: batch(vec![raw_inquiry_reply(0xa1)]),
                    received_at: now,
                },
                &mut driver,
                &runtime,
                Executor::now(&runtime),
                false,
            )
            .await,
        TurnOutcome::Continue
    );
    assert!(actor.raw_release.await_until().is_none());
    assert_eq!(harness.writes.recv_async().await.unwrap(), _b);

    // B's zero-length response deadline creates its own timeout hold as
    // soon as that dispatch succeeds. C waits behind the new hold and must
    // receive a fresh grace budget rather than inheriting A's deadline.
    assert!(matches!(
        b_completion.recv_async().await.unwrap(),
        ReceiptObservation::Terminal(RuntimeOutcome::Failed(Error::Timeout))
    ));

    let (_c_completion, c_admitted) = handle.enqueue_admission(inquiry(), None).unwrap();
    let c_boundary = actor.admissions.try_recv().unwrap();
    actor
        .handle_admission(c_boundary, &mut driver, &runtime, Executor::now(&runtime))
        .await;
    let c = c_admitted.recv_async().await.unwrap().unwrap();
    assert!(harness.writes.try_recv().is_err());

    driver.buffered.store(true, Ordering::Release);
    runtime.advance(Duration::from_secs(1));
    assert_eq!(
        actor
            .handle_event(
                ActorEvent::Wake,
                &mut driver,
                &runtime,
                Executor::now(&runtime),
                false,
            )
            .await,
        TurnOutcome::ContinueBuffered,
        "each independent hold receives its own grace deadline"
    );

    driver.buffered.store(false, Ordering::Release);
    let now = Executor::now(&runtime);
    assert_eq!(
        actor
            .handle_event(
                ActorEvent::Receive {
                    result: batch(vec![raw_inquiry_reply(0xb2)]),
                    received_at: now,
                },
                &mut driver,
                &runtime,
                Executor::now(&runtime),
                false,
            )
            .await,
        TurnOutcome::Continue
    );
    assert!(actor.raw_release.await_until().is_none());
    assert_eq!(harness.writes.recv_async().await.unwrap(), c);
}
/// Production adapter/framer coverage for the original literal-byte hole:
/// a raw reply split at the exact tombstone boundary remains attributed to
/// the old interval, and only B's own later reply settles B.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn production_raw_stream_literal_split_tail_precedes_tombstone_release() {
    let now = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(now);
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
        .expect("generic raw profile");
    let (chunk_tx, chunks) = flume::bounded(16);
    let (sent, sent_rx) = flume::bounded(16);
    let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
        ChunkedStreamTransport {
            config: crate::transport::builder::TransportConfig::default(),
            chunks,
            sent,
        },
        &profile,
        CameraId::CAMERA_1,
    )
    .unwrap();
    let mut actor_policy = adapter.policy().clone();
    actor_policy.limits.frames_per_receive = 1;
    actor_policy.protocol.raw_inquiry_release_hold = Duration::from_secs(1);
    let (handle, actor) = AsyncOwnerActor::new(actor_policy, runtime.clone()).unwrap();
    let actor_task = tokio::spawn(actor.run(adapter));

    let predecessor = handle.submit(timed_out_inquiry()).await.unwrap();
    let _ = sent_rx.recv_async().await.unwrap();
    assert!(matches!(
        terminal_within_test_deadline(
            &predecessor,
            "the production split-tail predecessor terminalizes",
        )
        .await,
        RuntimeOutcome::Failed(Error::Timeout)
    ));

    let successor = handle.submit(inquiry()).await.unwrap();
    assert!(sent_rx.try_recv().is_err());
    chunk_tx.send_async(vec![0x90, 0x50]).await.unwrap();
    let _ = handle.snapshot().await.unwrap();
    runtime.advance(Duration::from_secs(1));
    chunk_tx.send_async(vec![0xa1, 0xff]).await.unwrap();

    let _ = sent_rx.recv_async().await.unwrap();
    let boundary_snapshot = handle.snapshot().await.unwrap();
    assert!(boundary_snapshot.diagnostics.iter().any(|event| matches!(
        event,
        DiagnosticEvent::Ignored(IgnoreReason::UnmatchedFrame)
    )));
    chunk_tx
        .send_async(vec![0x90, 0x50, 0xb2, 0xff])
        .await
        .unwrap();
    assert!(matches!(
        terminal_within_test_deadline(
            &successor,
            "the production split-tail successor receives its own reply",
        )
        .await,
        RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
    ));

    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
/// A byte-stream release gives an ownerless prefix one real grace interval,
/// then discards it without poisoning. This runs the production adapter and
/// framer for source-only, ACK, and socketless-completion prefixes (#713).
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn production_raw_ambiguous_prefixes_expire_by_time_without_poison() {
    for prefix in [vec![0x90], vec![0x90, 0x41], vec![0x90, 0x50]] {
        let now = Instant::now();
        let (runtime, sleeps) = ManualRuntime::with_polling_sleeps_and_sleep_barrier(now);
        let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("generic raw profile");
        let (chunk_tx, chunks) = flume::bounded(16);
        let (sent, sent_rx) = flume::bounded(16);
        let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
            ChunkedStreamTransport {
                config: crate::transport::builder::TransportConfig::default(),
                chunks,
                sent,
            },
            &profile,
            CameraId::CAMERA_1,
        )
        .unwrap();
        let mut actor_policy = adapter.policy().clone();
        actor_policy.limits.frames_per_receive = 1;
        actor_policy.protocol.raw_inquiry_release_hold = Duration::from_secs(1);
        let (handle, actor) = AsyncOwnerActor::new(actor_policy, runtime.clone()).unwrap();
        let actor_task = tokio::spawn(actor.run(adapter));

        let predecessor = handle.submit(timed_out_inquiry()).await.unwrap();
        let _ = sent_rx.recv_async().await.unwrap();
        assert!(matches!(
            terminal_within_test_deadline(
                &predecessor,
                "the ambiguous-prefix predecessor terminalizes",
            )
            .await,
            RuntimeOutcome::Failed(Error::Timeout)
        ));
        let successor = handle.submit(inquiry()).await.unwrap();
        assert!(sent_rx.try_recv().is_err());

        chunk_tx.send_async(prefix.clone()).await.unwrap();
        let _ = handle.snapshot().await.unwrap();
        runtime.advance(Duration::from_secs(1));
        // Wait until the engine's grace has actually been mapped onto an
        // executor sleep, then advance virtual time through that deadline.
        loop {
            if sleeps.recv_async().await.unwrap() == Duration::from_millis(100) {
                break;
            }
        }
        runtime.advance(Duration::from_millis(100));
        let _sent_successor = tokio::time::timeout(Duration::from_secs(1), sent_rx.recv_async())
            .await
            .expect("the successor writes after grace expiry")
            .unwrap();

        let snapshot = handle.snapshot().await.unwrap();
        assert_eq!(snapshot.state, SessionState::Running);
        assert!(snapshot.diagnostics.iter().any(|event| matches!(
            event,
            DiagnosticEvent::Ignored(IgnoreReason::MalformedFrame)
        )));
        chunk_tx
            .send_async(vec![0x90, 0x50, 0xb2, 0xff])
            .await
            .unwrap();
        assert!(matches!(
            terminal_within_test_deadline(
                &successor,
                "the successor remains usable after orphan discard",
            )
            .await,
            RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
        ));
        handle.shutdown().await.unwrap();
        assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
    }
}
/// A source-only fragment cannot prove which same-target socket it belongs
/// to. At Y/S2's ambiguity expiry it must therefore keep input first; when
/// its exact S1 tail arrives at equality, X settles before Z is allowed to
/// write. This is the target-mask deletion bug that motivated the typed
/// release set: `[90]` followed by `[51 FF]` must never be erased merely
/// because another socket on camera A releases.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn production_raw_s1_split_tail_precedes_same_target_s2_release() {
    let now = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(now);
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
        .expect("generic raw profile");
    let (chunk_tx, chunks) = flume::bounded(16);
    let (sent, sent_rx) = flume::bounded(16);
    let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
        ChunkedStreamTransport {
            config: crate::transport::builder::TransportConfig::default(),
            chunks,
            sent,
        },
        &profile,
        CameraId::CAMERA_1,
    )
    .unwrap();
    let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime.clone()).unwrap();
    let actor_task = tokio::spawn(actor.run(adapter));
    let (x, _y, _z) = establish_production_two_socket_boundary(&handle, &chunk_tx, &sent_rx).await;

    enter_production_s2_quarantine(&handle, &runtime).await;
    chunk_tx.send_async(vec![0x90]).await.unwrap();
    let _ = handle.snapshot().await.unwrap();
    runtime.advance(Duration::from_secs(1));
    // Receive is deliberately ready at the same manual instant as the due
    // release. The actor's fixed ordering must decode this tail before
    // `advance` can free Y/S2 and write Z.
    chunk_tx.send_async(vec![0x51, 0xff]).await.unwrap();

    assert!(matches!(
        terminal_within_test_deadline(
            &x,
            "the literal S1 tail completes X at the release boundary",
        )
        .await,
        RuntimeOutcome::Applied
    ));
    assert_eq!(
        sent_rx.recv_async().await.unwrap(),
        vec![0x81, 0x01, 0x04, 0x33, 0xff],
        "Z may write only after X's equal-boundary completion is correlated",
    );

    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
/// An already-buffered named S1 prefix is exact evidence for live X, not
/// stale evidence for Y/S2. The due pass may release Y while preserving
/// `[90 51]`; its trailing terminator still completes X through the real
/// `ProtocolFramer` after Z has been dispatched.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn production_raw_live_s1_prefix_survives_same_target_s2_release() {
    let now = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(now);
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
        .expect("generic raw profile");
    let (chunk_tx, chunks) = flume::bounded(16);
    let (sent, sent_rx) = flume::bounded(16);
    let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
        ChunkedStreamTransport {
            config: crate::transport::builder::TransportConfig::default(),
            chunks,
            sent,
        },
        &profile,
        CameraId::CAMERA_1,
    )
    .unwrap();
    let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime.clone()).unwrap();
    let actor_task = tokio::spawn(actor.run(adapter));
    let (x, _y, _z) = establish_production_two_socket_boundary(&handle, &chunk_tx, &sent_rx).await;

    enter_production_s2_quarantine(&handle, &runtime).await;
    chunk_tx.send_async(vec![0x90, 0x51]).await.unwrap();
    let _ = handle.snapshot().await.unwrap();
    runtime.advance(Duration::from_secs(1));
    let _ = handle.snapshot().await.unwrap();

    assert_eq!(
        sent_rx.recv_async().await.unwrap(),
        vec![0x81, 0x01, 0x04, 0x33, 0xff],
        "Y/S2 may release without deleting live X/S1 evidence",
    );
    chunk_tx.send_async(vec![0xff]).await.unwrap();
    assert!(matches!(
        terminal_within_test_deadline(&x, "the retained live S1 prefix completes X").await,
        RuntimeOutcome::Applied
    ));

    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
/// Conversely, a named prefix for the releasing S2 is stale. It is
/// discarded one raw fragment before Z writes; a later terminator must not
/// turn the old fragment into a completion for either X or Z.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn production_raw_stale_s2_prefix_is_discarded_before_successor_write() {
    let now = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(now);
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
        .expect("generic raw profile");
    let (chunk_tx, chunks) = flume::bounded(16);
    let (sent, sent_rx) = flume::bounded(16);
    let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
        ChunkedStreamTransport {
            config: crate::transport::builder::TransportConfig::default(),
            chunks,
            sent,
        },
        &profile,
        CameraId::CAMERA_1,
    )
    .unwrap();
    let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime.clone()).unwrap();
    let actor_task = tokio::spawn(actor.run(adapter));
    let (x, _y, _z) = establish_production_two_socket_boundary(&handle, &chunk_tx, &sent_rx).await;

    enter_production_s2_quarantine(&handle, &runtime).await;
    chunk_tx.send_async(vec![0x90, 0x52]).await.unwrap();
    let _ = handle.snapshot().await.unwrap();
    runtime.advance(Duration::from_secs(1));
    let _ = handle.snapshot().await.unwrap();
    assert_eq!(
        sent_rx.recv_async().await.unwrap(),
        vec![0x81, 0x01, 0x04, 0x33, 0xff],
        "the exact stale S2 fragment is removed before successor dispatch",
    );

    // If the stale prefix survived, this byte would finish `[90 52 FF]`.
    // On its own it is malformed and ignored, so only X's explicit S1
    // completion below may settle X.
    chunk_tx.send_async(vec![0xff]).await.unwrap();
    let _ = handle.snapshot().await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(20), x.terminal())
            .await
            .is_err(),
        "the stale S2 tail must not complete live X/S1",
    );
    chunk_tx.send_async(vec![0x90, 0x51, 0xff]).await.unwrap();
    assert!(matches!(
        terminal_within_test_deadline(&x, "X completes only after its explicit S1 response",).await,
        RuntimeOutcome::Applied
    ));

    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
/// Raw serial holds are target-local. While camera C has a retained partial
/// reply, camera A's independently eligible successor may already be sent;
/// C's later literal tail must still resolve C without being confused with A.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn production_raw_serial_a_release_preserves_c_partial_reply() {
    let now = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(now);
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
        .expect("generic raw profile");
    let (chunk_tx, chunks) = flume::bounded(16);
    let (sent, sent_rx) = flume::bounded(16);
    let config = crate::transport::builder::TransportConfig {
        addressing: crate::transport::builder::AddressingMode::Serial,
        ..crate::transport::builder::TransportConfig::default()
    };
    let adapter = crate::runtime::owner::AsyncTransportAdapter::new_with_targets(
        ChunkedStreamTransport {
            config,
            chunks,
            sent,
        },
        &[
            (CameraId::CAMERA_1, &profile),
            (CameraId::CAMERA_3, &profile),
        ],
        crate::OperationalTuning::new(),
        std::num::NonZeroUsize::new(4).unwrap(),
        false,
    )
    .unwrap();
    let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime.clone()).unwrap();
    let actor_task = tokio::spawn(actor.run(adapter));

    let predecessor = handle
        .submit(inquiry_for(CameraId::CAMERA_1))
        .await
        .unwrap();
    let _ = sent_rx.recv_async().await.unwrap();
    chunk_tx
        .send_async(vec![0x90, 0x50, 0xa1, 0xff])
        .await
        .unwrap();
    let _ = terminal_within_test_deadline(
        &predecessor,
        "the serial camera-A predecessor receives its reply",
    )
    .await;

    let camera_c = handle
        .submit(inquiry_for(CameraId::CAMERA_3))
        .await
        .unwrap();
    let _ = sent_rx.recv_async().await.unwrap();
    let successor = handle
        .submit(inquiry_for(CameraId::CAMERA_1))
        .await
        .unwrap();
    // A's inquiry lane is independent from C's live inquiry, so the successor
    // is written before C's retained prefix is installed.
    let _ = sent_rx.recv_async().await.unwrap();

    chunk_tx.send_async(vec![0xb0, 0x50]).await.unwrap();
    let _ = handle.snapshot().await.unwrap();
    runtime.advance(Duration::from_secs(1));
    // The control allowance wakes the actor at the new manual instant; the
    // following due turn sees C's target and leaves its prefix untouched.
    let _ = handle.snapshot().await.unwrap();
    chunk_tx.send_async(vec![0xc3, 0xff]).await.unwrap();
    assert!(matches!(
        terminal_within_test_deadline(
            &camera_c,
            "camera C completes from its preserved partial reply",
        )
        .await,
        RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xc3]
    ));

    chunk_tx
        .send_async(vec![0x90, 0x50, 0xb2, 0xff])
        .await
        .unwrap();
    assert!(matches!(
        terminal_within_test_deadline(
            &successor,
            "camera A's queued successor receives its own reply",
        )
        .await,
        RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
    ));

    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
/// A frame-limit batch can leave a complete camera-C reply ahead of an
/// incomplete camera-A prefix. The async owner must decode C without due
/// work, then keep the ambiguous socketless A prefix input-first until its
/// tail is delimited for A's already-sent, target-local successor. This covers
/// hidden suffix ordering, the fail-closed socketless rule, and raw serial
/// framing through the production adapter and `ProtocolFramer`.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn production_raw_serial_frame_limit_drains_c_before_discarding_a_prefix() {
    let now = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(now);
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
        .expect("generic raw profile");
    let (chunk_tx, chunks) = flume::bounded(16);
    let (sent, sent_rx) = flume::bounded(16);
    let config = crate::transport::builder::TransportConfig {
        addressing: crate::transport::builder::AddressingMode::Serial,
        buffer_config: crate::transport::buffer::BufferConfig {
            recv_buffer_size: 512,
            ..crate::transport::buffer::BufferConfig::default()
        },
        ..crate::transport::builder::TransportConfig::default()
    };
    let adapter = crate::runtime::owner::AsyncTransportAdapter::new_with_targets(
        ChunkedStreamTransport {
            config,
            chunks,
            sent,
        },
        &[
            (CameraId::CAMERA_1, &profile),
            (CameraId::CAMERA_3, &profile),
        ],
        crate::OperationalTuning::new(),
        std::num::NonZeroUsize::new(4).unwrap(),
        false,
    )
    .unwrap();
    let frame_limit = adapter.policy().limits.frames_per_receive;
    let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime.clone()).unwrap();
    let actor_task = tokio::spawn(actor.run(adapter));

    let predecessor = handle
        .submit(inquiry_for(CameraId::CAMERA_1))
        .await
        .unwrap();
    let _ = sent_rx.recv_async().await.unwrap();
    chunk_tx
        .send_async(vec![0x90, 0x50, 0xa1, 0xff])
        .await
        .unwrap();
    assert!(matches!(
        terminal_within_test_deadline(
            &predecessor,
            "the frame-limit predecessor receives its reply",
        )
        .await,
        RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xa1]
    ));

    let camera_c = handle
        .submit(inquiry_for(CameraId::CAMERA_3))
        .await
        .unwrap();
    let _ = sent_rx.recv_async().await.unwrap();
    let successor = handle
        .submit(inquiry_for(CameraId::CAMERA_1))
        .await
        .unwrap();
    // A's per-target inquiry lane is independent from C's, so this write is
    // legal before the retained C/A input batch arrives.
    let _ = sent_rx.recv_async().await.unwrap();

    runtime.advance(Duration::from_secs(1));
    let mut burst = Vec::new();
    for _ in 0..frame_limit {
        burst.extend_from_slice(&[0xb0, 0x38, 0xff]);
    }
    // C's complete reply is the first retained item after the frame-limit
    // batch. A's socketless prefix sits behind it and must remain input-first
    // until its tail completes the already-sent A successor's reply.
    burst.extend_from_slice(&[0xb0, 0x50, 0xc3, 0xff]);
    burst.extend_from_slice(&[0x90, 0x50]);
    chunk_tx.send_async(burst).await.unwrap();
    // Queue the tail before yielding the actor: after it drains the
    // frame-limited batch and C's retained complete reply, receive-first
    // must consume this tail rather than spinning the already-due,
    // socketless prefix to the fail-closed cap.
    chunk_tx.send_async(vec![0xa1, 0xff]).await.unwrap();

    assert!(matches!(
        terminal_within_test_deadline(
            &camera_c,
            "camera C completes after the frame-limit batch",
        )
        .await,
        RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xc3]
    ));
    assert!(matches!(
        terminal_within_test_deadline(
            &successor,
            "camera A's successor completes from its retained input",
        )
        .await,
        RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xa1]
    ));

    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
#[cfg(feature = "runtime-tokio")]
async fn assert_production_raw_invalid_prefix_is_ignored(
    prefix: &[u8],
    complete: bool,
    addressing: crate::transport::builder::AddressingMode,
    case: &str,
) {
    let now = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(now);
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
        .expect("generic raw profile");
    let (chunk_tx, chunks) = flume::bounded(16);
    let (sent, sent_rx) = flume::bounded(16);
    let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
        ChunkedStreamTransport {
            config: crate::transport::builder::TransportConfig {
                addressing,
                ..crate::transport::builder::TransportConfig::default()
            },
            chunks,
            sent,
        },
        &profile,
        CameraId::CAMERA_1,
    )
    .unwrap();
    let mut actor_policy = adapter.policy().clone();
    actor_policy.limits.frames_per_receive = 1;
    actor_policy.protocol.raw_inquiry_release_hold = Duration::from_secs(1);
    let (handle, actor) = AsyncOwnerActor::new(actor_policy, runtime.clone()).unwrap();
    let actor_task = tokio::spawn(actor.run(adapter));

    let predecessor = handle.submit(timed_out_inquiry()).await.unwrap();
    let _ = sent_rx.recv_async().await.unwrap();
    assert!(matches!(
        terminal_within_test_deadline(&predecessor, "the invalid-prefix predecessor terminalizes",)
            .await,
        RuntimeOutcome::Failed(Error::Timeout)
    ));
    let successor = handle.submit(inquiry()).await.unwrap();
    assert!(sent_rx.try_recv().is_err());

    runtime.advance(Duration::from_secs(1));
    // One valid frame fills the batch. The malformed twin then remains in the
    // production framer exactly when the raw release is due: complete input
    // follows #672's ordinary decode path; incomplete input follows #745's
    // explicit malformed-prefix discard path.
    let mut chunk = vec![0x90, 0x38, 0xff];
    chunk.extend_from_slice(prefix);
    if complete {
        chunk.push(0xff);
    }
    chunk_tx.send_async(chunk).await.unwrap();

    let _ = sent_rx.recv_async().await.unwrap();
    assert_eq!(
        handle.snapshot().await.unwrap().state,
        SessionState::Running,
        "{case}: malformed raw input must not poison the async owner"
    );
    chunk_tx
        .send_async(vec![0x90, 0x50, 0xb2, 0xff])
        .await
        .unwrap();
    assert!(matches!(
        terminal_within_test_deadline(
            &successor,
            "{case}: successor survives the retained malformed input",
        )
        .await,
        RuntimeOutcome::Reply { payload, .. } if payload.as_slice() == [0xb2]
    ));

    handle.shutdown().await.unwrap();
    let snapshot = actor_task.await.unwrap();
    assert_eq!(snapshot.state, SessionState::Shutdown);
    assert_eq!(
        snapshot
            .diagnostics
            .iter()
            .filter(|event| matches!(
                event,
                DiagnosticEvent::Ignored(IgnoreReason::MalformedFrame)
            ))
            .count(),
        1,
        "{case}: exactly one malformed-frame diagnostic is required"
    );
}

/// Complete and incomplete malformed raw input must stay paired: both owners
/// discard it at a due release and retain a runnable session. The first two
/// cases are the literal #745 regression pair; the final two pin a passed-
/// through address-set broadcast and one noise byte.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn production_raw_stream_complete_and_incomplete_invalid_prefixes_are_ignored() {
    for (prefix, complete, addressing, case) in [
        (
            &[0x80, 0x50, 0xdd][..],
            true,
            crate::transport::builder::AddressingMode::Ip,
            "complete 80 50 dd ff",
        ),
        (
            &[0x80, 0x50, 0xdd][..],
            false,
            crate::transport::builder::AddressingMode::Ip,
            "incomplete 80 50 dd",
        ),
        (
            &[0x88, 0x30, 0x02][..],
            false,
            crate::transport::builder::AddressingMode::Serial,
            "incomplete address-set 88 30 02",
        ),
        (
            &[0x00][..],
            false,
            crate::transport::builder::AddressingMode::Ip,
            "incomplete stray noise",
        ),
    ] {
        assert_production_raw_invalid_prefix_is_ignored(prefix, complete, addressing, case).await;
    }
}
/// Regression test for a reply split across two stream reads (#560). A read
/// that only advances a partial frame decodes to an empty batch, which must
/// not be mistaken for a transport close.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn stream_reply_split_across_two_reads_keeps_the_session_running() {
    let runtime = TokioRuntime::from_current().unwrap();
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
        .expect("the generic profile is valid");
    let (chunk_tx, chunks) = flume::bounded(8);
    let (sent, sent_rx) = flume::bounded(8);
    let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
        ChunkedStreamTransport {
            config: crate::transport::builder::TransportConfig::default(),
            chunks,
            sent,
        },
        &profile,
        CameraId::CAMERA_1,
    )
    .unwrap();
    assert_eq!(adapter.policy().protocol.transport, TransportKind::Stream);
    let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime).unwrap();
    let actor_task = tokio::spawn(actor.run(adapter));

    let receipt = handle.submit(command()).await.unwrap();
    let _written = sent_rx.recv_async().await.unwrap();

    // Half of the ack. The framer buffers it and decodes nothing.
    chunk_tx.send_async(vec![0x90, 0x41]).await.unwrap();
    while !chunk_tx.is_empty() {
        tokio::task::yield_now().await;
    }
    // A snapshot only answers while the actor is still pumping; under the
    // #560 bug the actor has already terminated by this point.
    assert_eq!(
        handle.snapshot().await.unwrap().state,
        SessionState::Running,
        "a partial frame must not close the session"
    );

    // The rest of the ack, then the completion split the same way.
    chunk_tx.send_async(vec![0xff]).await.unwrap();
    chunk_tx.send_async(vec![0x90, 0x51]).await.unwrap();
    chunk_tx.send_async(vec![0xff]).await.unwrap();

    assert!(matches!(
        receipt.terminal().await.unwrap(),
        RuntimeOutcome::Applied
    ));
    assert_eq!(handle.snapshot().await.unwrap().active, 0);
    handle.shutdown().await.unwrap();
    let snapshot = actor_task.await.unwrap();
    assert_eq!(snapshot.state, SessionState::Shutdown);
}
