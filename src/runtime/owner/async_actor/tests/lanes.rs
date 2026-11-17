use super::*;

fn selection(boundaries_eligible: bool) -> Selection {
    Selection {
        phase: crate::runtime::owner::turn::SourcePhase::BoundariesFirst,
        receive: ReceiveArm::Poll,
        timer: TimerArm::Engine {
            at: None,
            due: false,
        },
        boundaries_eligible,
        timer_precedes_control: false,
    }
}

#[test]
fn registered_ineligible_admission_keeps_control_live_and_next_turn_ordered() {
    let state = OwnerState::new(policy(1), Instant::now()).unwrap();
    let (handle, ends) = OwnerHandleCore::new(&state);
    let mut lanes = ActorLanes::new(&ends.receivers);

    // Register every idle stream hook in a parked selection first. A later
    // plan suppresses admission without dropping its existing hook.
    {
        let mut parked = pin!(select_actor_event(
            selection(true),
            future::pending(),
            &mut lanes,
            future::pending(),
        ));
        assert!(future::block_on(future::poll_once(parked.as_mut())).is_none());
    }

    let (_completion, _admitted) = handle
        .enqueue_admission::<RuntimeOutcome>(command(), None)
        .unwrap();
    let (reply, _answer) = flume::bounded(1);
    handle
        .control
        .try_send(ControlBoundary::Metrics(reply))
        .unwrap();
    let event = future::block_on(select_actor_event(
        selection(false),
        future::pending(),
        &mut lanes,
        future::pending(),
    ));
    assert!(matches!(event, OwnerEvent::Control(_)));
    assert_eq!(ends.receivers.admissions.len(), 1);
    assert!(ends.receivers.control.is_empty());

    // Once the retained boundary has cleared, admission again precedes a
    // simultaneously ready control and receive in the coordinator's order.
    let (reply, _answer) = flume::bounded(1);
    handle
        .control
        .try_send(ControlBoundary::Metrics(reply))
        .unwrap();
    let event = future::block_on(select_actor_event(
        selection(true),
        async { OwnerEvent::Wake },
        &mut lanes,
        future::pending(),
    ));
    assert!(matches!(event, OwnerEvent::Admission(Ok(_))));
    assert!(ends.receivers.admissions.is_empty());
    assert_eq!(ends.receivers.control.len(), 1);
}

#[test]
fn disconnected_optional_lanes_stay_pending_and_admission_reports_disconnect() {
    let (shutdown_tx, shutdown) = flume::bounded(1);
    let (cancellation_tx, cancellations) = flume::bounded(1);
    let (admission_tx, admissions) = flume::bounded(1);
    let (control_tx, control) = flume::bounded(1);
    let receivers = BoundaryReceivers {
        shutdown,
        cancellations,
        admissions,
        control,
    };
    let mut lanes = ActorLanes::new(&receivers);
    drop((shutdown_tx, cancellation_tx, control_tx));
    {
        let mut parked = pin!(select_actor_event(
            selection(true),
            future::pending(),
            &mut lanes,
            future::pending(),
        ));
        assert!(future::block_on(future::poll_once(parked.as_mut())).is_none());
    }
    drop(admission_tx);
    let event = future::block_on(select_actor_event(
        selection(true),
        future::pending(),
        &mut lanes,
        future::pending(),
    ));
    assert!(matches!(
        event,
        OwnerEvent::Admission(Err(flume::RecvError::Disconnected))
    ));
}

// Local fake: a permanently pending device leaves only the actual boundary
// streams able to wake the owner when its final handle is dropped.
struct PendingDriver;

impl RetainedStreamInput for PendingDriver {}

impl AsyncOwnerDriver for PendingDriver {
    async fn write(&mut self, _write: WireWrite<'_>) -> WriteOutcome {
        WriteOutcome::Sent(crate::runtime::owner::TransmissionMeta::Raw)
    }

    async fn receive(
        &mut self,
        _buffers: &mut crate::runtime::owner::OwnerBuffers,
        _frame_limit: usize,
    ) -> OwnerReceive {
        future::pending().await
    }
}

fn assert_final_handle_disconnect_ends_a_parked_actor(runtime: impl Executor) {
    let (handle, actor) = AsyncOwnerActor::new(policy(1), runtime).unwrap();
    let mut run = pin!(actor.run(PendingDriver));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(run.as_mut().poll(&mut context).is_pending());
    drop(handle);
    let Poll::Ready(snapshot) = run.as_mut().poll(&mut context) else {
        panic!("disconnected admission must end the parked actor");
    };
    assert_eq!(snapshot.state, SessionState::Shutdown);
}

#[cfg(feature = "runtime-tokio")]
#[test]
fn tokio_final_handle_disconnect_ends_a_parked_actor() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    assert_final_handle_disconnect_ends_a_parked_actor(TokioRuntime::from_handle(
        runtime.handle().clone(),
    ));
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_final_handle_disconnect_ends_a_parked_actor() {
    assert_final_handle_disconnect_ends_a_parked_actor(SmolRuntime::new());
}

#[cfg(feature = "test-utils")]
#[test]
fn manual_clock_final_handle_disconnect_ends_a_parked_actor() {
    use crate::testing::testkit::{ManualClock, ManualClockExecutor};
    let clock = ManualClock::new();
    let _participant = clock.enter();
    assert_final_handle_disconnect_ends_a_parked_actor(ManualClockExecutor::local(&clock));
}
