use super::*;
/// A zero-length stream read is still the only close signal.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn zero_length_stream_read_still_closes_the_session() {
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
    let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime).unwrap();
    let actor_task = tokio::spawn(actor.run(adapter));

    let receipt = handle.submit(command()).await.unwrap();
    let _written = sent_rx.recv_async().await.unwrap();
    chunk_tx.send_async(Vec::new()).await.unwrap();

    assert!(matches!(
        receipt.terminal().await.unwrap(),
        RuntimeOutcome::Failed(Error::ConnectionClosed { .. })
    ));
    let snapshot = actor_task.await.unwrap();
    assert_eq!(snapshot.state, SessionState::Closed);
}
/// Issue #565: a transient receive failure (the classic case is a UDP
/// `recv` reporting ECONNREFUSED after an ICMP port-unreachable) retries
/// the in-flight command and leaves the session running.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn transient_receive_fault_retries_and_keeps_the_session_running() {
    let runtime = TokioRuntime::from_current().unwrap();
    // A receive fault while a raw command awaits its socket is
    // intentionally session-poisoning: replay could duplicate actuation.
    // Sony's envelope gives the retry an exact same-sequence identity, so
    // this fixture exercises the supported recovery path.
    let (handle, actor) = AsyncOwnerActor::new(sony_policy(1), runtime).unwrap();
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let frames = harness.frames.clone();
    let actor_task = tokio::spawn(actor.run(harness.driver));

    let receipt = handle.submit(retrying_command()).await.unwrap();
    assert_eq!(started.recv_async().await.unwrap(), receipt.id);
    gates
        .send_async(Ok(TransmissionMeta { sequence: Some(1) }))
        .await
        .unwrap();

    frames
        .send_async(Ok(AsyncReceive::Fault(Error::Io(Arc::new(
            std::io::Error::from(std::io::ErrorKind::ConnectionRefused),
        )))))
        .await
        .unwrap();

    // The very same request is written again rather than failed.
    assert_eq!(started.recv_async().await.unwrap(), receipt.id);
    gates
        .send_async(Ok(TransmissionMeta { sequence: Some(1) }))
        .await
        .unwrap();
    frames
        .send_async(batch(vec![sequenced(
            1,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        )]))
        .await
        .unwrap();
    frames
        .send_async(batch(vec![sequenced(
            1,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        )]))
        .await
        .unwrap();
    assert!(matches!(
        receipt.terminal().await.unwrap(),
        RuntimeOutcome::Applied
    ));
    handle.shutdown().await.unwrap();
    let snapshot = actor_task.await.unwrap();
    assert_eq!(snapshot.state, SessionState::Shutdown);
}
/// A receive failure that proves the connection is gone still ends the
/// session, and does so as a close rather than a byte-stream poison.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn fatal_receive_fault_closes_the_session() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(stream_policy(1), runtime).unwrap();
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
        .send_async(Ok(AsyncReceive::Fault(Error::Io(Arc::new(
            std::io::Error::from(std::io::ErrorKind::BrokenPipe),
        )))))
        .await
        .unwrap();

    assert!(matches!(
        receipt.terminal().await.unwrap(),
        RuntimeOutcome::Failed(Error::ConnectionClosed { .. })
    ));
    let snapshot = actor_task.await.unwrap();
    assert_eq!(snapshot.state, SessionState::Closed);
}
/// Issue #625. A transport that fails every read keeps the receive branch
/// of the left-biased race permanently ready. Before the fix that starved
/// shutdown, admissions, cancellations and control forever: `submit` never
/// returned, `shutdown` only queued a message nobody read, and the actor
/// kept reading hundreds of times a second with the session unkillable.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn a_transport_failing_every_read_still_serves_the_boundary() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(2), runtime).unwrap();
    let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let actor_task = tokio::spawn(actor.run(AlwaysFailingReceive {
        reads: Arc::clone(&reads),
    }));

    // Admission is polled even though the transport is always ready.
    // A raw command awaiting ACK would, by default, ride to its ACK
    // deadline and quarantine per-request (issue #671). Use an inquiry so
    // this test isolates source arbitration and boundary liveness without
    // creating an unconfirmed-command outcome.
    let receipt = tokio::time::timeout(Duration::from_secs(5), handle.submit(inquiry()))
        .await
        .expect("admission must not be starved by a failing transport")
        .unwrap();
    drop(receipt);
    // So is ordinary control.
    let metrics = tokio::time::timeout(Duration::from_secs(5), handle.metrics())
        .await
        .expect("control must not be starved by a failing transport")
        .unwrap();
    assert_eq!(
        metrics.session,
        crate::observability::SessionStatus::Running
    );

    // And so is shutdown: the session is killable.
    tokio::time::timeout(Duration::from_secs(5), handle.shutdown())
        .await
        .expect("shutdown must not be starved by a failing transport")
        .unwrap();
    let snapshot = tokio::time::timeout(Duration::from_secs(5), actor_task)
        .await
        .expect("the actor task must tear down within a bound")
        .unwrap();
    assert_eq!(snapshot.state, SessionState::Shutdown);
    assert!(
        reads.load(Ordering::Relaxed) < 500,
        "the escalating pause must stop the actor hot-looping on a failing read"
    );
}
/// The same guarantee on the other executor: the actor is executor-generic
/// and the fairness fix lives in its event race, not in tokio.
#[cfg(feature = "runtime-smol")]
#[test]
fn smol_transport_failing_every_read_still_serves_the_boundary() {
    smol::block_on(async {
        let runtime = SmolRuntime::new();
        let (handle, actor) = AsyncOwnerActor::new(policy(2), runtime).unwrap();
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let task = smol::spawn(actor.run(AlwaysFailingReceive {
            reads: Arc::clone(&reads),
        }));

        // Keep the fixture focused on boundary progress. A raw command
        // awaiting ACK would be poisoned by the first receive fault under
        // the no-replay rule; an inquiry has no ambiguous actuation.
        let receipt = handle.submit(inquiry()).await.unwrap();
        drop(receipt);
        assert_eq!(
            handle.snapshot().await.unwrap().state,
            SessionState::Running
        );
        handle.shutdown().await.unwrap();
        assert_eq!(task.await.state, SessionState::Shutdown);
    });
}
/// Issue #625. A transport that never recovers is not transient. The run
/// escalates its pause and then ends the session with the underlying cause,
/// rather than retrying against a dead adapter for the process lifetime.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn a_permanently_failing_transport_eventually_ends_the_session() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
    let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let snapshot = tokio::time::timeout(
        Duration::from_secs(30),
        actor.run(AlwaysFailingReceive {
            reads: Arc::clone(&reads),
        }),
    )
    .await
    .expect("a permanently failing transport must terminate the session");

    assert_eq!(snapshot.state, SessionState::Closed);
    assert!(
        reads.load(Ordering::Relaxed) >= usize::try_from(TRANSIENT_RECEIVE_FAULT_LIMIT).unwrap(),
        "the session must survive a genuinely transient burst first"
    );
    let error = handle.metrics().await.unwrap_err();
    let Error::ConnectionClosed {
        reason: Some(reason),
    } = error
    else {
        panic!("the terminal error must name the transport cause: {error:?}");
    };
    assert!(
        reason.contains("consecutive receive faults"),
        "the reason must say why the fault run stopped counting as transient: {reason}"
    );
}
/// A clean no-data receive is neither a transport fault nor proof that an
/// earlier transient fault recovered. Alternating the two inside the reset
/// window must therefore still reach the permanent-fault threshold.
#[cfg(feature = "runtime-tokio")]
#[tokio::test(start_paused = true)]
async fn alternating_fault_and_no_data_receives_still_end_the_session() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
    let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let faults = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let snapshot = tokio::time::timeout(
        Duration::from_secs(10),
        actor.run(AlternatingFaultNoData {
            reads: Arc::clone(&reads),
            faults: Arc::clone(&faults),
        }),
    )
    .await
    .expect("alternating no-data must not reset a transient fault run");

    assert_eq!(snapshot.state, SessionState::Closed);
    assert!(
        faults.load(Ordering::Relaxed) >= usize::try_from(TRANSIENT_RECEIVE_FAULT_LIMIT).unwrap(),
        "the full fault threshold must survive intervening no-data receives"
    );
    assert!(
        reads.load(Ordering::Relaxed)
            >= usize::try_from(TRANSIENT_RECEIVE_FAULT_LIMIT.saturating_mul(2) - 1).unwrap(),
        "the scripted driver must actually alternate faults with no-data reads"
    );
    let error = handle.shutdown().await.unwrap_err();
    assert!(matches!(error, Error::ConnectionClosed { .. }));
}
/// Issue #625. Genuinely transient faults still behave exactly as #620
/// specified: every command still awaiting its ACK is retransmitted, the
/// session survives, and a successful read clears the run so the next burst
/// starts from zero.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn a_burst_of_transient_faults_then_recovery_keeps_the_session() {
    let runtime = TokioRuntime::from_current().unwrap();
    // Same-sequence Sony retries are safe after a receive fault. A raw
    // command in this phase must poison the session and is covered by the
    // engine tests; this actor fixture must not weaken that rule.
    let (handle, actor) = AsyncOwnerActor::new(sony_policy(1), runtime).unwrap();
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let frames = harness.frames.clone();
    let actor_task = tokio::spawn(actor.run(harness.driver));

    let receipt = handle.submit(retrying_command()).await.unwrap();
    assert_eq!(started.recv_async().await.unwrap(), receipt.id);
    gates
        .send_async(Ok(TransmissionMeta { sequence: Some(1) }))
        .await
        .unwrap();

    // Two consecutive faults, each retransmitting the very same request.
    for _ in 0..2 {
        frames
            .send_async(Ok(AsyncReceive::Fault(Error::TransportError(
                "ICMP port unreachable".into(),
            ))))
            .await
            .unwrap();
        assert_eq!(started.recv_async().await.unwrap(), receipt.id);
        gates
            .send_async(Ok(TransmissionMeta { sequence: Some(1) }))
            .await
            .unwrap();
    }

    frames
        .send_async(batch(vec![sequenced(
            1,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        )]))
        .await
        .unwrap();
    frames
        .send_async(batch(vec![sequenced(
            1,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        )]))
        .await
        .unwrap();
    assert!(matches!(
        receipt.terminal().await.unwrap(),
        RuntimeOutcome::Applied
    ));

    // The successful read cleared the run, so more faults are still
    // transient and the session is still answering.
    for _ in 0..3 {
        frames
            .send_async(Ok(AsyncReceive::Fault(Error::TransportError(
                "ICMP port unreachable".into(),
            ))))
            .await
            .unwrap();
    }
    assert_eq!(
        handle.snapshot().await.unwrap().state,
        SessionState::Running
    );
    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
/// Issue #625/#719. An application idle timeout is no data, not a fault.
/// A transport with an internal idle timer — the shape the public trait
/// documents — must not retransmit anything, and must not starve the
/// boundary either. A raw I/O `TimedOut` is intentionally excluded: it can
/// be the OS reporting keepalive exhaustion and therefore ends the session.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn an_idle_read_timeout_is_not_a_receive_fault() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
    let harness = harness();
    let started = harness.started.clone();
    let gates = harness.gates.clone();
    let frames = harness.frames.clone();
    let writes = Arc::clone(&harness.writes);
    let actor_task = tokio::spawn(actor.run(harness.driver));

    let receipt = handle.submit(retrying_command()).await.unwrap();
    assert_eq!(started.recv_async().await.unwrap(), receipt.id);
    gates
        .send_async(Ok(TransmissionMeta { sequence: None }))
        .await
        .unwrap();

    for timeout in [
        Error::Timeout,
        Error::Io(Arc::new(std::io::Error::from(
            std::io::ErrorKind::WouldBlock,
        ))),
        Error::Io(Arc::new(std::io::Error::from(
            std::io::ErrorKind::Interrupted,
        ))),
    ] {
        frames
            .send_async(Ok(AsyncReceive::Fault(timeout)))
            .await
            .unwrap();
    }
    frames.send_async(Ok(AsyncReceive::NoData)).await.unwrap();

    // Control still answers, and nothing was retransmitted.
    assert_eq!(
        handle.snapshot().await.unwrap().state,
        SessionState::Running
    );
    assert_eq!(
        writes.lock().unwrap().len(),
        1,
        "an idle read timeout must not spend a request's retry budget"
    );

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
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
/// The escalation is bounded, monotonic, and long enough that a run only
/// ends the session after seconds of uninterrupted failure.
#[test]
fn the_transient_pause_escalates_and_stays_bounded() {
    assert_eq!(transient_receive_pause(1), TRANSIENT_RECEIVE_PAUSE);
    assert_eq!(transient_receive_pause(2), Duration::from_millis(20));
    assert_eq!(transient_receive_pause(3), Duration::from_millis(40));
    let mut previous = Duration::ZERO;
    let mut total = Duration::ZERO;
    for run in 1..TRANSIENT_RECEIVE_FAULT_LIMIT {
        let pause = transient_receive_pause(run);
        assert!(pause >= previous, "the pause must never shrink");
        assert!(pause <= MAXIMUM_TRANSIENT_RECEIVE_PAUSE);
        previous = pause;
        total = total.saturating_add(pause);
    }
    assert!(
        total >= TRANSIENT_RECEIVE_FAULT_SPAN,
        "a full fault run must span at least the documented minimum: {total:?}"
    );
}
/// A run only counts consecutive failures; a gap proves recovery, and a
/// successful read resets it outright.
#[test]
fn a_fault_run_only_counts_consecutive_failures() {
    let start = Instant::now();
    let mut run = TransientFaultRun::default();
    assert_eq!(run.record(start).0, 1);
    assert_eq!(run.record(start + Duration::from_millis(10)).0, 2);
    run.reset();
    assert_eq!(run.record(start + Duration::from_millis(20)).0, 1);
    // A gap longer than the reset window starts a fresh run.
    assert_eq!(
        run.record(start + Duration::from_millis(20) + TRANSIENT_RECEIVE_FAULT_RESET)
            .0,
        1
    );

    // The limit alone is not enough; the run must also be old enough.
    assert!(!TransientFaultRun::is_permanent(
        TRANSIENT_RECEIVE_FAULT_LIMIT,
        Duration::ZERO
    ));
    assert!(!TransientFaultRun::is_permanent(
        TRANSIENT_RECEIVE_FAULT_LIMIT - 1,
        TRANSIENT_RECEIVE_FAULT_SPAN
    ));
    assert!(TransientFaultRun::is_permanent(
        TRANSIENT_RECEIVE_FAULT_LIMIT,
        TRANSIENT_RECEIVE_FAULT_SPAN
    ));
}
/// Issue #625. The blocking owner clamps its transient pause to the
/// caller's deadline; the async owner clamps to the next scheduler wake, so
/// a fault can never delay a due deadline by the length of the pause.
#[test]
fn the_transient_pause_never_outlives_the_next_wake() {
    let now = Instant::now();
    let pause = Duration::from_millis(250);
    assert_eq!(clamp_receive_pause(pause, None, now), pause);
    assert_eq!(
        clamp_receive_pause(pause, Some(now + Duration::from_secs(1)), now),
        pause
    );
    assert_eq!(
        clamp_receive_pause(pause, Some(now + Duration::from_millis(3)), now),
        Duration::from_millis(3)
    );
    assert_eq!(
        clamp_receive_pause(pause, Some(now - Duration::from_millis(5)), now),
        Duration::ZERO,
        "an overdue deadline is serviced immediately"
    );
}
/// A discarded, consumed UDP datagram is successful transport activity,
/// even though its VISCA content is malformed. Alternating it with enough
/// transient read faults to otherwise cross the permanent-fault threshold
/// must leave the production adapter's owner Running.
#[cfg(feature = "runtime-tokio")]
#[tokio::test(start_paused = true)]
async fn consumed_truncated_datagrams_reset_the_async_fault_run() {
    let runtime = TokioRuntime::from_current().unwrap();
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
        .expect("the generic profile is valid");
    let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let faults = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let truncated = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let config = crate::transport::builder::TransportConfig {
        buffer_config: crate::transport::buffer::BufferConfig {
            recv_buffer_size: 3,
            ..crate::transport::buffer::BufferConfig::default()
        },
        ..crate::transport::builder::TransportConfig::default()
    };
    let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
        AlternatingFaultAndTruncatedDatagrams {
            config,
            reads,
            faults: Arc::clone(&faults),
            truncated: Arc::clone(&truncated),
        },
        &profile,
        CameraId::CAMERA_1,
    )
    .unwrap();
    assert_eq!(adapter.policy().protocol.transport, TransportKind::Datagram);
    let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime).unwrap();
    let actor_task = tokio::spawn(actor.run(adapter));
    let started_at = tokio::time::Instant::now();

    // Advancing in steps lets each transient-fault pause complete before
    // the next consumed oversized datagram, so the old accounting would
    // accumulate twelve faults over more than the one-second terminal span.
    for _ in 0..20 {
        tokio::time::advance(Duration::from_millis(100)).await;
        tokio::task::yield_now().await;
        if faults.load(Ordering::Relaxed) >= usize::try_from(TRANSIENT_RECEIVE_FAULT_LIMIT).unwrap()
            && truncated.load(Ordering::Relaxed)
                >= usize::try_from(TRANSIENT_RECEIVE_FAULT_LIMIT).unwrap()
        {
            break;
        }
    }
    assert!(
        tokio::time::Instant::now().duration_since(started_at) >= Duration::from_secs(1),
        "the regression must cross the permanent-fault time threshold"
    );
    assert!(
        faults.load(Ordering::Relaxed) >= usize::try_from(TRANSIENT_RECEIVE_FAULT_LIMIT).unwrap(),
        "the fixture must issue the whole transient-fault threshold"
    );
    assert!(
        truncated.load(Ordering::Relaxed)
            >= usize::try_from(TRANSIENT_RECEIVE_FAULT_LIMIT).unwrap(),
        "every intervening oversized datagram must be consumed and discarded"
    );
    assert_eq!(
        handle.snapshot().await.unwrap().state,
        SessionState::Running,
        "a consumed malformed datagram resets the fault run rather than closing the session"
    );

    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
/// Issue #637. One malformed datagram — the review's probe is `01 41 ff`,
/// a controller source byte that no camera ever sends — used to kill the
/// whole async session, while the blocking owner failed it per request and
/// kept pumping. A stray UDP datagram is not proof the session is dead.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn a_malformed_datagram_does_not_kill_the_async_session() {
    let runtime = TokioRuntime::from_current().unwrap();
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
        .expect("the generic profile is valid");
    let (datagram_tx, datagrams) = flume::bounded(8);
    let (sent, sent_rx) = flume::bounded(8);
    let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
        ScriptedDatagramTransport {
            config: crate::transport::builder::TransportConfig::default(),
            datagrams,
            sent,
        },
        &profile,
        CameraId::CAMERA_1,
    )
    .unwrap();
    assert_eq!(adapter.policy().protocol.transport, TransportKind::Datagram);
    let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime).unwrap();
    let actor_task = tokio::spawn(actor.run(adapter));

    let receipt = handle.submit(command()).await.unwrap();
    let _written = sent_rx.recv_async().await.unwrap();
    datagram_tx
        .send_async(vec![0x01, 0x41, 0xff])
        .await
        .unwrap();
    while !datagram_tx.is_empty() {
        tokio::task::yield_now().await;
    }

    assert_eq!(
        handle.snapshot().await.unwrap().state,
        SessionState::Running,
        "one undecodable datagram is not a session verdict"
    );

    // The next well-formed datagram still completes the in-flight command.
    datagram_tx
        .send_async(vec![0x90, 0x41, 0xff])
        .await
        .unwrap();
    datagram_tx
        .send_async(vec![0x90, 0x51, 0xff])
        .await
        .unwrap();
    assert!(matches!(
        receipt.terminal().await.unwrap(),
        RuntimeOutcome::Applied
    ));

    handle.shutdown().await.unwrap();
    let snapshot = actor_task.await.unwrap();
    assert_eq!(snapshot.state, SessionState::Shutdown);
    assert!(
        snapshot.diagnostics.iter().any(|event| matches!(
            event,
            DiagnosticEvent::Ignored(IgnoreReason::MalformedFrame)
        )),
        "the discarded datagram must still be observable"
    );
}
/// #672: a delimited-but-unclassifiable frame on a byte stream is a
/// malformed frame to discard, not a lost framing position. The framer kept
/// its place, so the stream stays Running, the frame is recorded as
/// `Ignored(MalformedFrame)`, and the in-flight command is settled by the
/// next well-formed reply — the log-and-continue tolerance 1.x had. A genuine
/// framing failure (buffer overflow / no boundary) still poisons and is
/// pinned separately.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn a_malformed_stream_frame_is_discarded_and_keeps_the_session() {
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
    // A padded ACK: delimited at its `FF`, but four bytes where an ACK is
    // exactly three, so it does not classify.
    chunk_tx
        .send_async(vec![0x90, 0x41, 0x00, 0xff])
        .await
        .unwrap();
    while !chunk_tx.is_empty() {
        tokio::task::yield_now().await;
    }
    assert_eq!(
        handle.snapshot().await.unwrap().state,
        SessionState::Running,
        "one malformed stream frame is not a session verdict"
    );

    // The next well-formed ACK and completion still settle the command.
    chunk_tx.send_async(vec![0x90, 0x41, 0xff]).await.unwrap();
    chunk_tx.send_async(vec![0x90, 0x51, 0xff]).await.unwrap();
    assert!(matches!(
        receipt.terminal().await.unwrap(),
        RuntimeOutcome::Applied
    ));

    handle.shutdown().await.unwrap();
    let snapshot = actor_task.await.unwrap();
    assert_eq!(snapshot.state, SessionState::Shutdown);
    assert!(
        snapshot.diagnostics.iter().any(|event| matches!(
            event,
            DiagnosticEvent::Ignored(IgnoreReason::MalformedFrame)
        )),
        "the discarded malformed stream frame must still be observable"
    );
}
/// #674: a single stream read that decodes more than the per-receive frame
/// limit (default 64) must not poison the session. The read stops at the
/// limit and the async owner drains the buffered remainder on the next turn
/// (protocol input first), so all frames are attributed and the command is
/// settled. Sixty-three harmless network-change notices sit ahead of the ACK
/// and completion, so the completion falls past the limit and is only
/// reachable through the drain path.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn a_stream_burst_over_the_frame_limit_keeps_the_session() {
    let runtime = TokioRuntime::from_current().unwrap();
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
        .expect("the generic profile is valid");
    let (chunk_tx, chunks) = flume::bounded(8);
    let (sent, sent_rx) = flume::bounded(8);
    // A large receive buffer so one read can carry a burst past the frame
    // limit, as a real 256-byte raw-IP session would.
    let mut config = crate::transport::builder::TransportConfig::default();
    config.buffer_config.recv_buffer_size = 1024;
    let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
        ChunkedStreamTransport {
            config,
            chunks,
            sent,
        },
        &profile,
        CameraId::CAMERA_1,
    )
    .unwrap();
    assert_eq!(adapter.policy().protocol.transport, TransportKind::Stream);
    let frame_limit = adapter.policy().limits.frames_per_receive;
    let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime).unwrap();
    let actor_task = tokio::spawn(actor.run(adapter));

    let receipt = handle.submit(command()).await.unwrap();
    let _written = sent_rx.recv_async().await.unwrap();
    // One read: (frame_limit - 1) network-change notices, then the ACK and
    // completion. The completion is the (frame_limit + 1)-th frame, so it is
    // only reached after the limit-saturated read is drained.
    let mut burst = Vec::new();
    for _ in 0..frame_limit.saturating_sub(1) {
        burst.extend_from_slice(&[0x90, 0x38, 0xff]);
    }
    burst.extend_from_slice(&[0x90, 0x41, 0xff]);
    burst.extend_from_slice(&[0x90, 0x51, 0xff]);
    chunk_tx.send_async(burst).await.unwrap();

    assert!(matches!(
        receipt.terminal().await.unwrap(),
        RuntimeOutcome::Applied
    ));
    assert_eq!(
        handle.snapshot().await.unwrap().state,
        SessionState::Running,
        "a large-but-valid burst is not a session verdict"
    );

    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
/// #681: a single-target IP session whose camera answers with its non-default
/// chain address (`0xA0` for VISCA address 2) still attributes the reply to
/// the sole outstanding command, rather than poisoning the stream.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn a_single_target_ip_chain_address_reply_settles_the_command() {
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
    let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime).unwrap();
    let actor_task = tokio::spawn(actor.run(adapter));

    let receipt = handle.submit(command()).await.unwrap();
    let _written = sent_rx.recv_async().await.unwrap();
    // The camera answers with chain address 2 for both frames.
    chunk_tx.send_async(vec![0xa0, 0x41, 0xff]).await.unwrap();
    chunk_tx.send_async(vec![0xa0, 0x51, 0xff]).await.unwrap();

    assert!(matches!(
        receipt.terminal().await.unwrap(),
        RuntimeOutcome::Applied
    ));

    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
/// Issue #637. A datagram send failure fails exactly one request and the
/// session keeps running, so the error the caller observes must not tell it
/// to open a replacement session. Before the fix a custom transport reached
/// that self-contradiction just by returning `ConnectionClosed` from `send`.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn a_datagram_send_failure_never_demands_a_new_session() {
    let runtime = TokioRuntime::from_current().unwrap();
    let profile = crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
        .expect("the generic profile is valid");
    let adapter = crate::runtime::owner::AsyncTransportAdapter::new(
        ClosedSendDatagramTransport {
            config: crate::transport::builder::TransportConfig::default(),
        },
        &profile,
        CameraId::CAMERA_1,
    )
    .unwrap();
    assert_eq!(adapter.policy().protocol.transport, TransportKind::Datagram);
    let (handle, actor) = AsyncOwnerActor::new(adapter.policy().clone(), runtime).unwrap();
    let actor_task = tokio::spawn(actor.run(adapter));

    let receipt = handle.submit(command()).await.unwrap();
    let RuntimeOutcome::Failed(error) = receipt.terminal().await.unwrap() else {
        panic!("a datagram send failure fails its own request");
    };
    assert!(
        !error.requires_new_session(),
        "a live session must never hand out a replacement-session verdict: {error:?}"
    );
    assert!(matches!(error, Error::TransportError(_)));
    assert_eq!(
        handle.snapshot().await.unwrap().state,
        SessionState::Running,
        "a datagram send failure is per request, not a session verdict"
    );

    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tokio_stalled_write_never_parks_close() {
    let mut owner_policy = policy(1);
    owner_policy.write_timeout = Duration::from_millis(50);
    let (handle, terminated, join) = run_isolated_tokio_actor(owner_policy, StallingWriteDriver);
    assert_stalled_write_never_parks_close(
        TokioRuntime::from_current().unwrap(),
        handle,
        terminated,
    )
    .await;
    join.join().unwrap();
}
#[cfg(feature = "runtime-smol")]
#[test]
fn smol_stalled_write_never_parks_close() {
    let mut owner_policy = policy(1);
    owner_policy.write_timeout = Duration::from_millis(50);
    let (handle, terminated, join) = run_isolated_smol_actor(owner_policy, StallingWriteDriver);
    smol::block_on(assert_stalled_write_never_parks_close(
        SmolRuntime::new(),
        handle,
        terminated,
    ));
    join.join().unwrap();
}
