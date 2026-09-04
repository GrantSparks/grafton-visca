use super::*;
#[test]
fn simultaneous_source_readiness_follows_the_explicit_phase() {
    let receive = || std::future::ready("receive");
    let boundary = || std::future::ready("boundary");

    assert_eq!(
        future::block_on(select_source(
            SourcePhase::ReceiveFirst,
            receive(),
            boundary(),
        )),
        "receive",
    );
    assert_eq!(
        future::block_on(select_source(
            SourcePhase::BoundariesFirst,
            receive(),
            boundary(),
        )),
        "boundary",
    );
    assert_eq!(
        TurnOutcome::Continue.next_source_phase(),
        Some(SourcePhase::ReceiveFirst),
    );
    assert_eq!(
        TurnOutcome::ContinueBuffered.next_source_phase(),
        Some(SourcePhase::ReceiveFirst),
    );
    assert_eq!(
        TurnOutcome::YieldBoundaries.next_source_phase(),
        Some(SourcePhase::BoundariesFirst),
    );
}

/// A due raw release used to bypass the ordinary boundary future entirely.
/// This byte flood keeps the stream framer buffered and never produces a
/// complete frame, so a regression can only admit the Urgent lane if the raw
/// selector charges the normal fairness ceiling and then polls admission.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_release_flood_admits_and_writes_urgent_within_the_fairness_bound() {
    const HOLD: Duration = Duration::from_secs(1);
    const GRACE: Duration = Duration::from_millis(100);
    const FAIRNESS_CEILING: usize = 4;
    // The actor cooperatively yields every four receives; an executor may run
    // several such quanta before the test task advances virtual grace. Keep a
    // finite ceiling for the regression without depending on that executor
    // scheduling detail.
    const POST_H_POLL_BOUND: u64 = 128;

    let initial = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(initial);
    let mut owner_policy = stream_policy(1);
    owner_policy.limits.frames_per_receive = FAIRNESS_CEILING;
    owner_policy.protocol.raw_inquiry_release_hold = HOLD;
    owner_policy.protocol.raw_release_grace = GRACE;
    let reads = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let buffered = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let (write_tx, writes) = flume::bounded(8);
    let mut driver = RawBufferedFloodDriver {
        reads: Arc::clone(&reads),
        buffered,
        writes: write_tx,
    };
    let (handle, mut actor) = AsyncOwnerActor::new(owner_policy, runtime.clone()).unwrap();

    // Install a real inquiry release hold without first running the eager
    // flood. The zero-length response deadline terminalizes A immediately;
    // H remains one second away.
    let (predecessor_completion, predecessor_admitted) =
        handle.enqueue_admission(timed_out_inquiry(), None).unwrap();
    let predecessor_boundary = actor.admissions.try_recv().unwrap();
    actor
        .handle_admission(
            predecessor_boundary,
            &mut driver,
            &runtime,
            Executor::now(&runtime),
        )
        .await;
    let predecessor = predecessor_admitted.recv_async().await.unwrap().unwrap();
    assert_eq!(writes.recv_async().await.unwrap(), predecessor);
    assert!(matches!(
        predecessor_completion.recv_async().await.unwrap(),
        ReceiptObservation::Terminal(RuntimeOutcome::Failed(Error::Timeout))
    ));

    // Establish the due retained-prefix gate without starting the flood yet.
    // The actor loop below must then select every byte-flood receive and its
    // eventual urgent admission through the raw-release path, rather than
    // inheriting an ordinary pre-H selection.
    runtime.advance(HOLD);
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

    let (urgent_completion, urgent_admitted) =
        handle.enqueue_admission(urgent_command(), None).unwrap();
    assert!(!handle.admissions.is_empty());
    let actor_task = tokio::spawn(actor.run(driver));
    // The admission is deliberately queued before the flood starts. It cannot
    // be consumed until the raw selector spends a forced fairness turn; once
    // removed it remains deferred behind the release proof.
    while !handle.admissions.is_empty() {
        tokio::task::yield_now().await;
    }
    let reads_before_release = reads.load(Ordering::Relaxed);
    assert!(
        reads_before_release >= FAIRNESS_CEILING as u64,
        "the raw byte flood must reach the normal fairness ceiling before admission"
    );

    // At H + grace the timer and the perpetually-ready receive are both
    // runnable. The release timer must win its boundary-first phase, after
    // admission has been staged through that same ordered lane.
    runtime.advance(GRACE);
    let urgent = tokio::time::timeout(Duration::from_secs(1), urgent_admitted.recv_async())
        .await
        .expect("raw flood must not starve urgent admission")
        .unwrap()
        .unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), writes.recv_async())
            .await
            .expect("urgent request writes after the bounded raw release")
            .unwrap(),
        urgent
    );
    assert!(
        urgent_completion.try_recv().is_none(),
        "the urgent write remains live; only admission and pacing are asserted here"
    );
    let post_h_polls = reads.load(Ordering::Relaxed) - reads_before_release;
    assert!(
        post_h_polls <= POST_H_POLL_BOUND,
        "raw release flood took {post_h_polls} polls after Urgent queued (bound {POST_H_POLL_BOUND})"
    );

    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}

/// The raw coordinator may add a proof and defer the selected boundary, but it
/// must not change the ordinary receive-first fairness cadence.  Poll both
/// branches directly so executor scheduling cannot hide a different phase
/// sequence: four flood reads yield, the next poll takes admission, and four
/// more reads reach the following cooperative yield.
#[cfg(feature = "runtime-tokio")]
async fn raw_release_flood_boundary_cadence(latched_raw_release: bool) -> (u64, u64) {
    const HOLD: Duration = Duration::from_secs(1);
    const FAIRNESS_CEILING: usize = 4;

    let runtime = ManualRuntime::with_polling_sleeps(Instant::now());
    let mut owner_policy = stream_policy(1);
    owner_policy.limits.frames_per_receive = FAIRNESS_CEILING;
    owner_policy.protocol.raw_inquiry_release_hold = HOLD;
    let reads = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let buffered = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let (write_tx, writes) = flume::bounded(8);
    let mut driver = RawBufferedFloodDriver {
        reads: Arc::clone(&reads),
        buffered,
        writes: write_tx,
    };
    let (handle, mut actor) = AsyncOwnerActor::new(owner_policy, runtime.clone()).unwrap();

    if latched_raw_release {
        let (completion, admitted) = handle.enqueue_admission(timed_out_inquiry(), None).unwrap();
        let predecessor = actor.admissions.try_recv().unwrap();
        actor
            .handle_admission(predecessor, &mut driver, &runtime, Executor::now(&runtime))
            .await;
        let predecessor = admitted.recv_async().await.unwrap().unwrap();
        assert_eq!(writes.recv_async().await.unwrap(), predecessor);
        assert!(matches!(
            completion.recv_async().await.unwrap(),
            ReceiptObservation::Terminal(RuntimeOutcome::Failed(Error::Timeout))
        ));

        runtime.advance(HOLD);
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
    }

    let (_completion, _admitted) = handle.enqueue_admission(urgent_command(), None).unwrap();
    let mut run = Box::pin(actor.run(driver));
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);

    assert!(std::future::Future::poll(run.as_mut(), &mut context).is_pending());
    let first_yield_reads = reads.load(Ordering::Relaxed);
    assert!(
        !handle.admissions.is_empty(),
        "the forced boundary begins on the next poll"
    );

    assert!(std::future::Future::poll(run.as_mut(), &mut context).is_pending());
    let second_yield_reads = reads.load(Ordering::Relaxed);
    assert!(
        handle.admissions.is_empty(),
        "the same boundary poll must consume the queued urgent admission"
    );
    drop(run);
    (first_yield_reads, second_yield_reads)
}

/// The due raw-release selector has the same forced-boundary cadence as the
/// normal selector. This differential guards against future raw-only selector
/// branches silently omitting admission or fairness (#746).
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn raw_release_flood_matches_normal_boundary_cadence() {
    let normal = raw_release_flood_boundary_cadence(false).await;
    let raw = raw_release_flood_boundary_cadence(true).await;
    assert_eq!(normal, (4, 8), "normal selector phase sequence");
    assert_eq!(raw, normal, "raw release must use the same phase cadence");
}

/// A due engine wake may allow one ordinary control observation, but a
/// chained backlog of real public controls cannot keep it from advancing
/// engine time. The first metrics call intentionally observes the pending
/// inquiry; the second must observe its reply deadline having fired.
///
/// Before the tail fairness bound, every metrics call won
/// `future::or(control, wake)`, so all four observed `active == 1` and an
/// unbounded caller chain could postpone the deadline forever.
#[cfg(feature = "runtime-tokio")]
#[tokio::test(start_paused = true)]
async fn due_wake_is_not_starved_by_chained_public_controls() {
    const CONTROL_CHAIN: usize = 4;

    let runtime = TokioRuntime::from_current().unwrap();
    let mut owner_policy = policy(1);
    // The actor's bounded control lane is intentionally full before it
    // starts, making this a deterministic chain rather than a scheduler
    // race between the caller and the ready timer.
    owner_policy.limits.applied_subscribers = CONTROL_CHAIN;
    let (handle, mut actor) = AsyncOwnerActor::new(owner_policy, runtime.clone()).unwrap();
    let (_frames, receives) = flume::bounded(1);
    let mut driver = UngatedDriver { receives };

    // Install one sent inquiry without running the event loop yet. Its
    // reply deadline is therefore the next authoritative engine wake.
    let (completion, admitted) = handle.enqueue_admission(inquiry(), None).unwrap();
    let admission = actor
        .admissions
        .try_recv()
        .expect("the staged inquiry must be waiting for the actor");
    actor
        .handle_admission(admission, &mut driver, &runtime, Executor::now(&runtime))
        .await;
    assert!(admitted.recv_async().await.unwrap().is_ok());
    assert_eq!(actor.state.active_len(), 1);
    let deadline = actor
        .state
        .next_wake()
        .expect("the sent inquiry must own a reply deadline");
    assert_eq!(
        deadline.saturating_duration_since(Executor::now(&runtime)),
        Duration::from_secs(5)
    );

    // Send each request through the public control API and wait until it
    // has joined the actor's FIFO lane before starting its successor. No
    // actor is polling yet, so the resulting sequence is deterministic.
    let mut controls = Vec::with_capacity(CONTROL_CHAIN);
    for expected_queued in 1..=CONTROL_CHAIN {
        let control_handle = handle.clone();
        controls.push(tokio::spawn(async move { control_handle.metrics().await }));
        while handle.control.len() < expected_queued {
            tokio::task::yield_now().await;
        }
    }
    assert!(handle.control.is_full());

    // Make the protocol deadline due before selection begins. Tokio's
    // paused clock keeps this exact and avoids a wall-clock liveness race.
    tokio::time::advance(Duration::from_secs(5)).await;
    assert_eq!(
        deadline.saturating_duration_since(Executor::now(&runtime)),
        Duration::ZERO,
        "the chained controls race an already-due protocol deadline"
    );
    let actor_task = tokio::spawn(actor.run(driver));

    let first = controls.remove(0).await.unwrap().unwrap();
    let second = controls.remove(0).await.unwrap().unwrap();
    assert_eq!(
        first.active, 1,
        "one control may observe the pre-wake state"
    );
    assert_eq!(
        second.active, 0,
        "the due Wake must advance the engine before a second queued control",
    );
    assert!(matches!(
        completion.recv_async().await.unwrap(),
        ReceiptObservation::Terminal(RuntimeOutcome::Failed(Error::Timeout))
    ));

    for control in controls {
        control.await.unwrap().unwrap();
    }
    handle.shutdown().await.unwrap();
    assert_eq!(actor_task.await.unwrap().state, SessionState::Shutdown);
}
/// A timer can mature after the actor has constructed its tail selection.
/// The first control then wins the old left-biased race, but it must still
/// spend the due wake's one allowance before the loop is rebuilt.
///
/// This manually polls the same actor future before and after advancing a
/// test clock, so no Tokio task scheduling order can accidentally let Wake
/// run before the two controls are queued.
#[cfg(feature = "runtime-tokio")]
#[tokio::test(start_paused = true)]
async fn parked_future_wake_charges_the_first_ready_control() {
    let runtime = ManualRuntime::with_polling_sleeps(Instant::now());
    let mut owner_policy = policy(1);
    owner_policy.limits.applied_subscribers = 2;
    // Keep the receive timeout after the inquiry deadline so the parked
    // receive arm cannot become the source that wakes this test.
    owner_policy.read_timeout = Duration::from_secs(10);
    let (handle, mut actor) = AsyncOwnerActor::new(owner_policy, runtime.clone()).unwrap();
    let (_frames, receives) = flume::bounded(1);
    let mut driver = UngatedDriver { receives };

    let (completion, admitted) = handle.enqueue_admission(inquiry(), None).unwrap();
    let admission = actor
        .admissions
        .try_recv()
        .expect("the staged inquiry must be waiting for the actor");
    actor
        .handle_admission(admission, &mut driver, &runtime, Executor::now(&runtime))
        .await;
    assert!(admitted.recv_async().await.unwrap().is_ok());
    let deadline = actor
        .state
        .next_wake()
        .expect("the sent inquiry must own a reply deadline");

    let mut run = Box::pin(actor.run(driver));
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    assert!(std::future::Future::poll(run.as_mut(), &mut context).is_pending());

    // The original tail was built with a positive delay and is now parked.
    // Advance the logical clock without polling the actor, then enqueue two
    // controls before its next poll. Both the timer and first control are
    // ready when that already-built left-biased tail resumes.
    runtime.advance(Duration::from_secs(5));
    assert_eq!(
        deadline.saturating_duration_since(Executor::now(&runtime)),
        Duration::ZERO,
    );
    let (first_reply, first_result) = flume::bounded(1);
    let (second_reply, second_result) = flume::bounded(1);
    handle
        .control
        .try_send(ControlBoundary::Metrics(first_reply))
        .unwrap();
    handle
        .control
        .try_send(ControlBoundary::Metrics(second_reply))
        .unwrap();

    assert!(std::future::Future::poll(run.as_mut(), &mut context).is_pending());
    let first = first_result.try_recv().unwrap().unwrap();
    let second = second_result.try_recv().unwrap().unwrap();
    assert_eq!(first.active, 1, "the first control owns the allowance");
    assert_eq!(
        second.active, 0,
        "the matured wake must run before a second queued control",
    );
    assert!(matches!(
        completion.recv_async().await.unwrap(),
        ReceiptObservation::Terminal(RuntimeOutcome::Failed(Error::Timeout))
    ));
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tokio_babbling_peer_never_starves_boundaries() {
    let (handle, terminated, join) = run_isolated_tokio_actor(policy(1), BabblingDriver);
    assert_babble_never_starves_boundaries(
        TokioRuntime::from_current().unwrap(),
        handle,
        terminated,
    )
    .await;
    join.join().unwrap();
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tokio_buffered_babbling_peer_never_starves_boundaries() {
    let (handle, terminated, join) = run_isolated_tokio_actor(policy(1), BufferedBabblingDriver);
    assert_babble_never_starves_boundaries(
        TokioRuntime::from_current().unwrap(),
        handle,
        terminated,
    )
    .await;
    join.join().unwrap();
}
#[cfg(feature = "runtime-tokio")]
#[test]
fn tokio_current_thread_babbling_peer_yields_to_caller_control_and_timer() {
    tokio_current_thread_ready_receive_yields_to_boundaries(false, false, "a babbling peer");
}
/// Delimited malformed stream frames reach the actor as an empty frame
/// batch after the adapter records and discards them. Unlike `NoData`, that
/// path has no pacing sleep, so this current-thread probe requires its own
/// cooperative handoff. It queues every boundary class plus a timer after
/// the first malformed batch; a pre-fix actor never gives those tasks a
/// chance to enqueue on the same executor.
#[cfg(feature = "runtime-tokio")]
#[test]
fn tokio_current_thread_malformed_stream_batches_yield_to_all_boundaries() {
    tokio_current_thread_ready_receive_yields_to_boundaries(
        true,
        true,
        "discarded malformed stream frames",
    );
}
/// The same no-progress handoff on a single-thread, runtime-neutral
/// executor. `AsyncOwnerActor` uses no Tokio scheduling primitive here:
/// a discarded malformed batch must let independently spawned admission,
/// control, timer, and shutdown work run under smol as well.
#[cfg(feature = "runtime-smol")]
#[test]
fn smol_current_thread_malformed_stream_batches_yield_to_boundaries() {
    const WATCHDOG: Duration = Duration::from_secs(2);

    let reads = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (finished, result) = flume::bounded(1);
    let worker_reads = Arc::clone(&reads);
    let worker_stop = Arc::clone(&stop);
    let worker = std::thread::spawn(move || {
        let local = async_executor::LocalExecutor::new();
        let outcome: Result<(), String> = future::block_on(local.run(async {
            let (handle, actor) = AsyncOwnerActor::new(policy(1), SmolRuntime::new())
                .map_err(|error| error.to_string())?;
            let actor_task = local.spawn(actor.run(CountingBabblingDriver {
                reads: Arc::clone(&worker_reads),
                stop: Arc::clone(&worker_stop),
                empty_batches: true,
            }));

            while worker_reads.load(Ordering::Acquire) == 0 {
                future::yield_now().await;
            }

            let caller_handle = handle.clone();
            let caller = local.spawn(async move {
                caller_handle
                    .submit(command())
                    .await
                    .map_err(|error| error.to_string())
            });
            let control_handle = handle.clone();
            let control = local.spawn(async move {
                control_handle
                    .snapshot()
                    .await
                    .map_err(|error| error.to_string())
            });
            let timer = local.spawn(async {
                smol::Timer::after(Duration::from_millis(1)).await;
            });

            drop(caller.await?);
            let snapshot = control.await?;
            timer.await;
            if snapshot.state != SessionState::Running {
                return Err(format!(
                    "control observed an unexpected owner state: {:?}",
                    snapshot.state
                ));
            }

            handle.shutdown().await.map_err(|error| error.to_string())?;
            let terminal = actor_task.await;
            if terminal.state != SessionState::Shutdown {
                return Err(format!(
                    "actor ended in an unexpected state: {:?}",
                    terminal.state
                ));
            }
            Ok(())
        }));
        let _ = finished.send(outcome);
    });

    match result.recv_timeout(WATCHDOG) {
        Ok(Ok(())) => worker.join().unwrap(),
        Ok(Err(error)) => {
            worker.join().unwrap();
            panic!("malformed-frame smol liveness scenario failed: {error}");
        }
        Err(flume::RecvTimeoutError::Timeout) => {
            stop.store(true, Ordering::Release);
            if result.recv_timeout(WATCHDOG).is_ok() {
                worker.join().unwrap();
            } else {
                drop(worker);
            }
            panic!(
                "discarded malformed stream frames monopolized smol's current-thread executor before boundary or timer work could run"
            );
        }
        Err(flume::RecvTimeoutError::Disconnected) => {
            worker.join().unwrap();
            panic!("malformed-frame smol liveness worker exited without a result");
        }
    }
}
#[cfg(feature = "runtime-tokio")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tokio_nodata_receive_never_hot_spins() {
    let reads = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let (handle, terminated, join) = run_isolated_tokio_actor(
        policy(1),
        NoDataDriver {
            reads: Arc::clone(&reads),
        },
    );
    assert_nodata_never_hot_spins(
        TokioRuntime::from_current().unwrap(),
        handle,
        terminated,
        reads,
    )
    .await;
    join.join().unwrap();
}
#[cfg(feature = "runtime-smol")]
#[test]
fn smol_babbling_peer_never_starves_boundaries() {
    let (handle, terminated, join) = run_isolated_smol_actor(policy(1), BabblingDriver);
    smol::block_on(assert_babble_never_starves_boundaries(
        SmolRuntime::new(),
        handle,
        terminated,
    ));
    join.join().unwrap();
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_buffered_babbling_peer_never_starves_boundaries() {
    let (handle, terminated, join) = run_isolated_smol_actor(policy(1), BufferedBabblingDriver);
    smol::block_on(assert_babble_never_starves_boundaries(
        SmolRuntime::new(),
        handle,
        terminated,
    ));
    join.join().unwrap();
}
#[cfg(feature = "runtime-smol")]
#[test]
fn smol_nodata_receive_never_hot_spins() {
    let reads = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let (handle, terminated, join) = run_isolated_smol_actor(
        policy(1),
        NoDataDriver {
            reads: Arc::clone(&reads),
        },
    );
    smol::block_on(assert_nodata_never_hot_spins(
        SmolRuntime::new(),
        handle,
        terminated,
        reads,
    ));
    join.join().unwrap();
}
