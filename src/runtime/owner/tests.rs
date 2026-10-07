#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::time::Duration;

use super::*;
use crate::{runtime::engine::CancellationPolicy, CameraId};

fn policy_for_target_validation() -> ProtocolPolicy {
    ProtocolPolicy {
        capacity: 1,
        inquiry_capacity: 1,
        inquiry_cooldown: Duration::ZERO,
        raw_inquiry_release_hold: Duration::ZERO,
        ..ProtocolPolicy::test_default()
    }
}

fn target_policy_for_validation() -> TargetPolicy {
    TargetPolicy {
        command_sockets: 1,
        ..TargetPolicy::test_default()
    }
}

/// Stage one admission with a fresh observer slot and admission reply, as a
/// handle's boundary message would carry them.
fn stage_admission(
    state: &mut OwnerState,
    request: RuntimeRequest,
    permit: AdmissionPermit,
) -> (
    Input,
    TerminalObserver,
    flume::Receiver<Result<Admitted, Error>>,
) {
    let (observer, cell) = TerminalObserver::pair();
    let (reply, admission) = flume::bounded(1);
    let input = state.stage_admission_with(request, permit, cell, reply);
    (input, observer, admission)
}

#[test]
fn single_target_rejects_broadcast_without_panicking() {
    let result = OwnerPolicy::single_target(
        policy_for_target_validation(),
        CameraId::BROADCAST,
        target_policy_for_validation(),
    );
    assert!(result.is_err());
}

#[test]
fn with_targets_rejects_broadcast_slot_without_panicking() {
    let mut targets = [None; 9];
    targets[8] = Some(target_policy_for_validation());
    let result = OwnerPolicy::with_targets(policy_for_target_validation(), targets);
    assert!(result.is_err());
}

/// Issue #631: the owner state used by the reconfiguration tests below.
///
/// The profile baseline is deliberately non-zero on both axes so a
/// re-derivation that quietly kept a previous override would be visible.
fn reconfigurable_owner_state() -> OwnerState {
    let mut protocol = policy_for_target_validation();
    protocol.command_spacing = Duration::from_millis(10);
    protocol.inquiry_spacing = Duration::from_millis(20);
    let policy =
        OwnerPolicy::single_target(protocol, CameraId::CAMERA_1, TargetPolicy::test_default())
            .expect("single-target owner policy");
    OwnerState::new(policy).expect("owner state")
}

fn command_sockets(state: &OwnerState) -> u8 {
    state.policy().targets[usize::from(CameraId::CAMERA_1.id())]
        .expect("camera one is registered")
        .command_sockets
}

/// Issue #631: pacing and socket capacity are re-derived from the *profile*
/// baseline, so an override can be relaxed again instead of becoming the new
/// floor.
#[test]
fn retuning_re_derives_pacing_from_the_profile_baseline() {
    let mut state = reconfigurable_owner_state();
    assert_eq!(
        state.policy().protocol.command_spacing,
        Duration::from_millis(10)
    );
    assert_eq!(
        state.policy().protocol.inquiry_spacing,
        Duration::from_millis(20)
    );
    assert_eq!(command_sockets(&state), 2);

    state
        .retune(
            crate::OperationalTuning::new()
                .command_spacing(Duration::from_millis(200))
                .inquiry_spacing(Duration::from_millis(300))
                .maximum_command_sockets(1),
        )
        .expect("widening pacing and lowering socket capacity is accepted");
    assert_eq!(
        state.policy().protocol.command_spacing,
        Duration::from_millis(200)
    );
    assert_eq!(
        state.policy().protocol.inquiry_spacing,
        Duration::from_millis(300)
    );
    assert_eq!(command_sockets(&state), 1);

    // Clearing every override must return to the profile facts. Deriving the
    // new value from the *tuned* one instead would leave 200 ms installed
    // forever.
    state
        .retune(crate::OperationalTuning::new())
        .expect("clearing the overrides is accepted");
    assert_eq!(
        state.policy().protocol.command_spacing,
        Duration::from_millis(10)
    );
    assert_eq!(
        state.policy().protocol.inquiry_spacing,
        Duration::from_millis(20)
    );
    assert_eq!(command_sockets(&state), 2);
}

/// Issue #631: the live tuning cell handed to request-preparing handles follows
/// the owner, and a rejected update leaves both the cell and the policy alone.
#[test]
fn a_rejected_retune_changes_neither_the_live_tuning_nor_the_policy() {
    let mut state = reconfigurable_owner_state();
    let live = state.live_tuning();
    assert_eq!(live.get(), crate::OperationalTuning::new());

    let accepted = crate::OperationalTuning::new().command_spacing(Duration::from_millis(50));
    state.retune(accepted).expect("accepted");
    assert_eq!(live.get(), accepted, "the shared cell follows the owner");

    // The facades reject this before it reaches the owner; the engine's own
    // socket bound is the second line of defence, and it must fail cleanly.
    let error = state
        .retune(crate::OperationalTuning::new().maximum_command_sockets(3))
        .expect_err("three command sockets is not a VISCA capacity");
    assert!(matches!(error, Error::InvalidRequest(_)), "got {error:?}");
    assert_eq!(
        live.get(),
        accepted,
        "a rejected update leaves the live tuning untouched"
    );
    assert_eq!(
        state.policy().protocol.command_spacing,
        Duration::from_millis(50),
        "a rejected update leaves the owner policy untouched"
    );
    assert_eq!(command_sockets(&state), 2);
}

/// The owner's cell is the slot's only strong owner (#777): dropping it
/// unresolved disconnects the observer, and a handle can tell that the owner
/// no longer holds the intent.
#[test]
fn dropping_an_unresolved_cell_disconnects_its_observer() {
    let (observer, cell) = TerminalObserver::pair();
    assert!(observer.cell().is_some());
    drop(cell);
    assert!(observer.cell().is_none());
    assert!(observer.receiver.is_disconnected());
    assert!(observer.try_recv().is_none());

    let (observer, cell) = TerminalObserver::pair();
    assert_eq!(
        cell.resolve(RuntimeOutcome::Applied),
        ObserverResolution::Delivered
    );
    drop(cell);
    assert!(
        matches!(observer.try_recv(), Some(RuntimeOutcome::Applied)),
        "a delivered value outlives the cell"
    );
}

#[test]
fn observer_resolution_distinguishes_receiver_loss_from_duplicate_delivery() {
    let (observer, cell) = TerminalObserver::pair();
    assert_eq!(
        cell.resolve(RuntimeOutcome::Applied),
        ObserverResolution::Delivered
    );
    drop(observer);
    assert_eq!(
        cell.resolve(RuntimeOutcome::Applied),
        ObserverResolution::AlreadyResolved,
        "dropping a receiver after delivery cannot turn a duplicate into loss"
    );

    let (observer, cell) = TerminalObserver::pair();
    drop(observer);
    assert_eq!(
        cell.resolve(RuntimeOutcome::Applied),
        ObserverResolution::ReceiverLost
    );
}

/// D26 (#778): the permit pool's two budgets.
mod control_reserve {
    use super::*;
    use crate::runtime::engine::{ControlPolicy, ReplyShape, RetryPolicy, TimeoutPolicy};

    fn context(target: u8, class: ControlClass) -> RequestContext {
        RequestContext {
            motion: None,
            submission_order: 0,
            dispatch_deadline: None,
            target: CameraId::new(target).unwrap(),
            timeout: TimeoutPolicy {
                ack: Duration::from_millis(10),
                completion: Duration::from_millis(20),
                inquiry: Duration::from_millis(20),
                cancellation: Duration::from_millis(10),
                ambiguity: Duration::from_millis(10),
            },
            retry: RetryPolicy::NEVER,
            control: ControlPolicy {
                class,
                ..ControlPolicy::default()
            },
            cancellation: CancellationPolicy::Supported,
            reply_shape: ReplyShape::AckThenCompletion,
        }
    }

    /// One ordinary slot; cameras 1 and 2 reserve two and one control slots.
    fn pool() -> AdmissionPermitPool {
        let mut reserves = [0; 9];
        reserves[1] = 2;
        reserves[2] = 1;
        AdmissionPermitPool::new(1, reserves)
    }

    #[test]
    fn urgent_requests_take_their_reserve_first_then_ordinary_slots() {
        let pool = pool();
        let urgent = context(1, ControlClass::Urgent);
        let first = pool.try_acquire(&urgent).unwrap();
        let second = pool.try_acquire(&urgent).unwrap();
        assert_eq!(first.slot(), AdmissionSlot::ControlReserve);
        assert_eq!(second.slot(), AdmissionSlot::ControlReserve);
        let third = pool.try_acquire(&urgent).unwrap();
        assert_eq!(third.slot(), AdmissionSlot::Ordinary, "the reserve is held");
        assert!(matches!(
            pool.try_acquire(&urgent),
            Err(Error::ControlReserveExhausted { target, reserve: 2 })
                if target == CameraId::CAMERA_1
        ));
        assert!(matches!(
            pool.try_acquire(&context(1, ControlClass::User)),
            Err(Error::RuntimeQueueFull { capacity: 1 })
        ));
        // Camera 1 cannot use camera 2's reserve.
        let other = pool.try_acquire(&context(2, ControlClass::Urgent)).unwrap();
        assert_eq!(other.slot(), AdmissionSlot::ControlReserve);
    }

    #[test]
    fn ordinary_requests_never_take_a_reserve_and_slots_return_to_their_budget() {
        let pool = pool();
        let ordinary = context(1, ControlClass::Normal);
        let held = pool.try_acquire(&ordinary).unwrap();
        assert_eq!(held.slot(), AdmissionSlot::Ordinary);
        assert!(matches!(
            pool.try_acquire(&ordinary),
            Err(Error::RuntimeQueueFull { capacity: 1 })
        ));

        let urgent = context(1, ControlClass::Urgent);
        let reserved = pool.try_acquire(&urgent).unwrap();
        drop(reserved);
        assert_eq!(
            pool.available(),
            0,
            "a reserved slot never frees an ordinary one"
        );
        assert_eq!(
            pool.try_acquire(&urgent).unwrap().slot(),
            AdmissionSlot::ControlReserve,
            "the released reserved slot returned to camera 1's reserve"
        );
        drop(held);
        assert_eq!(pool.available(), 1);
    }
}

#[test]
fn pan_tilt_limit_cache_rejects_unknown_corner_discriminators() {
    let mut cache = TargetStateCache::default();
    let known = AppliedStateProjection::set(StateKey::PanTiltLimits, &[0, 12, -4])
        .expect("valid down-left limit projection");
    let malformed_set = AppliedStateProjection::set(StateKey::PanTiltLimits, &[3, 9, 8])
        .expect("bounded malformed projection");
    let malformed_clear = AppliedStateProjection::clear_with_values(StateKey::PanTiltLimits, &[3])
        .expect("bounded malformed projection");

    cache.apply(known, 1);
    cache.apply(malformed_set, 1);
    assert_eq!(
        cache.get(StateKey::PanTiltLimits),
        Some(known),
        "a malformed set must not replace the known corner"
    );

    cache.apply(malformed_clear, 1);
    assert_eq!(
        cache.get(StateKey::PanTiltLimits),
        Some(known),
        "a malformed clear must not replace the known corner"
    );

    // A limit clear is local to one corner like a limit set; one that names
    // no corner is malformed, not a whole-key clear.
    cache.apply(AppliedStateProjection::clear(StateKey::PanTiltLimits), 1);
    assert_eq!(
        cache.get(StateKey::PanTiltLimits),
        Some(known),
        "a clear that names no corner must not replace the known corner"
    );
}

/// Issue #571: the counters a field debugging session reaches for first.
///
/// Every scenario here is scripted at the owner boundary rather than asserted
/// against a retry constant, so the counts stay meaningful as retry policy
/// changes underneath them.
mod metrics {
    use std::{
        collections::VecDeque,
        sync::Arc,
        time::{Duration, Instant},
    };

    use super::super::*;
    use crate::{
        runtime::engine::{
            CancellationPolicy, ControlPolicy, DeadlineKind, DecodedFrame, DecodedResponse,
            EncodedMessage, EnvelopeKind, EnvelopeSequence, InquiryRoute, ReplyShape,
            RequestContext, RetryPolicy, SequenceWidth, TimeoutPolicy, TransmissionMeta,
        },
        CameraId, Error, ViscaSocket,
    };

    const ACK: Duration = Duration::from_millis(10);
    const COMPLETION: Duration = Duration::from_millis(20);
    const INQUIRY: Duration = Duration::from_millis(15);

    fn retrying() -> RetryPolicy {
        RetryPolicy {
            max_retries: 3,
            initial_backoff: Duration::from_millis(5),
            maximum_backoff: Duration::from_millis(50),
            total_budget: Duration::from_secs(10),
            ack_timeout: true,
            completion_timeout: true,
            inquiry_timeout: true,
            buffer_full: true,
            movement_not_executable: true,
            builtin_inquiry_syntax: true,
        }
    }

    fn owner_policy(envelope: EnvelopeKind) -> OwnerPolicy {
        let protocol = ProtocolPolicy {
            capacity: 4,
            envelope,
            inquiry_capacity: 2,
            inquiry_cooldown: Duration::ZERO,
            raw_inquiry_release_hold: Duration::from_millis(10),
            ..ProtocolPolicy::test_default()
        };
        OwnerPolicy::single_target(protocol, CameraId::CAMERA_1, TargetPolicy::test_default())
            .unwrap()
    }

    fn request_context(retry: RetryPolicy) -> RequestContext {
        RequestContext {
            motion: None,
            submission_order: 0,
            dispatch_deadline: None,
            target: CameraId::CAMERA_1,
            timeout: TimeoutPolicy {
                ack: ACK,
                completion: COMPLETION,
                inquiry: INQUIRY,
                cancellation: Duration::from_millis(10),
                ambiguity: Duration::from_millis(10),
            },
            retry,
            control: ControlPolicy::default(),
            cancellation: CancellationPolicy::Supported,
            reply_shape: ReplyShape::AckThenCompletion,
        }
    }

    fn wire() -> Arc<EncodedMessage> {
        Arc::new(EncodedMessage::new(&[0x81, 0x01, 0x04, 0x00, 0xff]).unwrap())
    }

    fn command(retry: RetryPolicy) -> RuntimeRequest {
        RuntimeRequest::Command {
            wire: wire(),
            context: request_context(retry),
            applied_state: None,
        }
    }

    fn inquiry(retry: RetryPolicy) -> RuntimeRequest {
        let mut context = request_context(retry);
        context.timeout.inquiry = INQUIRY;
        RuntimeRequest::Inquiry {
            wire: wire(),
            context,
            route: InquiryRoute(1),
        }
    }

    fn frame(sequence: Option<u32>, response: DecodedResponse) -> Input {
        OwnerState::frame_input(DecodedFrame {
            target: CameraId::CAMERA_1,
            sequence: sequence.map(|value| EnvelopeSequence {
                value,
                width: SequenceWidth::Full32,
            }),
            response,
        })
    }

    /// Applies one effect batch to quiescence, confirming every staged write.
    fn drain(
        state: &mut OwnerState,
        mut effects: VecDeque<Effect>,
        sequence: Option<u32>,
        now: Instant,
    ) {
        while let Some(effect) = effects.pop_front() {
            if let AppliedEffect::Transmit(staged) = state.apply_effect(effect) {
                let produced = state.finish_write(&staged, Ok(TransmissionMeta { sequence }), now);
                prepend_effects(&mut effects, produced);
            }
        }
    }

    /// Admits one request and leaves it waiting on the camera.
    fn sent(
        state: &mut OwnerState,
        request: RuntimeRequest,
        sequence: Option<u32>,
        now: Instant,
    ) -> TerminalObserver {
        let permit = state
            .permits()
            .try_acquire_ordinary()
            .expect("admission permit");
        let (input, observer, _admission) = super::stage_admission(state, request, permit);
        let effects = state.input(input, now);
        drain(state, effects, sequence, now);
        observer
    }

    fn apply(state: &mut OwnerState, input: Input, now: Instant) {
        let effects = state.input(input, now);
        drain(state, effects, None, now);
    }

    fn advance(state: &mut OwnerState, now: Instant) {
        let effects = state.advance(now);
        drain(state, effects, None, now);
    }

    fn saw_deadline(state: &OwnerState, expected: DeadlineKind, retrying: bool) -> bool {
        state.diagnostics().any(|event| {
            matches!(
                event,
                DiagnosticEvent::DeadlineExpired { deadline, will_retry, .. }
                    if *deadline == expected && *will_retry == retrying
            )
        })
    }

    #[test]
    fn expired_ack_deadline_counts_a_timeout_and_the_retry_it_scheduled_for_sony() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Sony)).unwrap();
        let _observer = sent(&mut state, command(retrying()), Some(1), start);
        assert_eq!(state.metrics().ack_timeouts, 0);
        advance(&mut state, start + ACK);
        let metrics = state.metrics();
        assert_eq!(metrics.ack_timeouts, 1);
        assert_eq!(metrics.retries_scheduled, 1);
        assert_eq!(metrics.completion_timeouts, 0);
        assert_eq!(metrics.inquiry_timeouts, 0);
        assert!(saw_deadline(&state, DeadlineKind::Ack, true));
    }

    #[test]
    fn expired_ack_deadline_without_a_retry_reports_the_decision_directly() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw)).unwrap();
        let _observer = sent(&mut state, command(RetryPolicy::NEVER), None, start);
        advance(&mut state, start + ACK);
        let metrics = state.metrics();
        assert_eq!(metrics.ack_timeouts, 1);
        assert_eq!(metrics.retries_scheduled, 0);
        assert!(saw_deadline(&state, DeadlineKind::Ack, false));
    }

    #[test]
    fn expired_completion_deadline_counts_separately_from_the_ack_deadline_for_sony() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Sony)).unwrap();
        let _observer = sent(&mut state, command(retrying()), Some(1), start);
        apply(
            &mut state,
            frame(
                Some(1),
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start,
        );
        advance(&mut state, start + COMPLETION);
        let metrics = state.metrics();
        assert_eq!(metrics.completion_timeouts, 1);
        assert_eq!(metrics.ack_timeouts, 0);
        // #795: an acknowledged Sony command is never rewritten, so its
        // completion deadline is reported without a retry.
        assert_eq!(metrics.retries_scheduled, 0);
        assert!(saw_deadline(&state, DeadlineKind::Completion, false));
    }

    #[test]
    fn expired_inquiry_deadline_counts_as_an_inquiry_timeout() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw)).unwrap();
        let _observer = sent(&mut state, inquiry(retrying()), None, start);
        advance(&mut state, start + INQUIRY);
        let metrics = state.metrics();
        assert_eq!(metrics.inquiry_timeouts, 1);
        assert_eq!(metrics.ack_timeouts, 0);
        assert_eq!(metrics.completion_timeouts, 0);
        assert!(saw_deadline(&state, DeadlineKind::InquiryReply, true));
    }

    #[test]
    fn camera_backpressure_codes_count_as_busy_errors() {
        for code in [0x03_u8, 0x05, 0x41] {
            let start = Instant::now();
            let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw)).unwrap();
            let _observer = sent(&mut state, command(retrying()), None, start);
            apply(
                &mut state,
                frame(None, DecodedResponse::Error { socket: None, code }),
                start,
            );
            let metrics = state.metrics();
            assert_eq!(metrics.busy_errors, 1, "code {code:#04x}");
            assert_eq!(metrics.protocol_errors, 0, "code {code:#04x}");
            assert_eq!(metrics.retries_scheduled, 1, "code {code:#04x}");
        }
    }

    #[test]
    fn other_error_codes_count_as_protocol_errors() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw)).unwrap();
        let _observer = sent(&mut state, command(retrying()), None, start);
        apply(
            &mut state,
            frame(
                None,
                DecodedResponse::Error {
                    socket: None,
                    code: 0x02,
                },
            ),
            start,
        );
        let metrics = state.metrics();
        assert_eq!(metrics.protocol_errors, 1);
        assert_eq!(metrics.busy_errors, 0);
        assert_eq!(metrics.terminal, 1);
    }

    /// `0x04` answers a cancel; counting it would make every successful
    /// cancellation look like a camera fault.
    #[test]
    fn the_cancellation_reply_code_is_neither_busy_nor_protocol() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw)).unwrap();
        let _observer = sent(&mut state, command(retrying()), None, start);
        apply(
            &mut state,
            frame(
                None,
                DecodedResponse::Error {
                    socket: Some(ViscaSocket::S1),
                    code: 0x04,
                },
            ),
            start,
        );
        let metrics = state.metrics();
        assert_eq!(metrics.busy_errors, 0);
        assert_eq!(metrics.protocol_errors, 0);
    }

    #[test]
    fn a_sequenced_reply_matching_no_request_is_counted_as_ignored() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Sony)).unwrap();
        let _observer = sent(&mut state, command(retrying()), Some(1), start);
        apply(
            &mut state,
            frame(
                Some(9_999),
                DecodedResponse::Completion {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start,
        );
        assert_eq!(state.metrics().ignored_unmatched_sequenced_replies, 1);
        assert_eq!(state.metrics().terminal, 0);
    }

    #[test]
    fn sony_control_reply_is_observable_without_affecting_request_state() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Sony)).unwrap();

        apply(
            &mut state,
            frame(None, DecodedResponse::SonyControl { code: 0x0F01 }),
            start,
        );

        assert_eq!(state.metrics().received_frames, 1);
        assert_eq!(state.metrics().protocol_errors, 0);
        assert_eq!(state.metrics().terminal, 0);
        assert!(state.diagnostics().any(|event| matches!(
            event,
            DiagnosticEvent::FrameReceived {
                target: CameraId::CAMERA_1,
                sequence: None,
                response: ResponseDiagnostic::SonyControl { code: 0x0F01 },
            }
        )));
        assert!(state.diagnostics().any(|event| matches!(
            event,
            DiagnosticEvent::Ignored(IgnoreReason::UnmatchedFrame)
        )));
    }

    /// A transient receive fault (#565) retries a sequenced Sony command while
    /// it awaits its ACK. Raw commands poison the session because they have no
    /// sequence key that can make replay safe.
    #[test]
    fn a_transient_receive_fault_retry_counts_as_a_retry_for_sony() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Sony)).unwrap();
        let _observer = sent(&mut state, command(retrying()), Some(1), start);
        apply(
            &mut state,
            Input::ReceiveFault {
                error: Error::Io(Arc::new(std::io::Error::from(
                    std::io::ErrorKind::ConnectionRefused,
                ))),
            },
            start,
        );
        let metrics = state.metrics();
        assert_eq!(metrics.retries_scheduled, 1);
        assert_eq!(metrics.ack_timeouts, 0);
    }

    /// The counters are scalars and the diagnostic vocabulary stays `Copy`, so
    /// an increment is a register add and observing a snapshot never allocates.
    #[test]
    fn owner_counters_stay_scalar_and_copyable() {
        const fn assert_copy<T: Copy>() {}
        assert_copy::<OwnerMetrics>();
        assert_copy::<DiagnosticEvent>();
        assert_eq!(size_of::<OwnerMetrics>(), 23 * size_of::<u64>());
    }
    fn admit_observation(
        state: &mut OwnerState,
        request: RuntimeRequest,
        now: Instant,
    ) -> (OperationObservation, Arc<ObserverCell<RuntimeOutcome>>) {
        let permit = state.permits.try_acquire(request.context()).unwrap();
        let (observer, cell) = TerminalObserver::pair();
        let (reply, admitted) = flume::bounded(1);
        let input = state.stage_admission_with(request, permit, Arc::clone(&cell), reply);
        let turn = state.begin_input_turn(now);
        for effect in state.input_in_turn(&turn, input) {
            let _ = state.apply_effect(effect);
        }
        let _ = state.finish_input_turn(turn, EngineTurn::INPUT_ONLY);
        let core = ReceiptCore::admitted(
            admitted.recv().unwrap().unwrap(),
            CameraId::CAMERA_1,
            observer,
            ACK,
        );
        (OperationObservation::new(core, ACK), cell)
    }

    fn observation_fixture() -> (OperationObservation, Arc<ObserverCell<RuntimeOutcome>>) {
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw)).unwrap();
        admit_observation(&mut state, command(retrying()), Instant::now())
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn admission_supersedes_even_if_cancelled_before_write_but_rejection_does_not() {
        use crate::AffectedAxes;
        let now = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw)).unwrap();
        let mut request = command(retrying());
        request.context_mut().motion = Some(crate::runtime::engine::MotionEffect {
            axes: AffectedAxes::PAN_TILT,
            stop: false,
        });
        request.context_mut().submission_order = 1;
        let (mut original, _) = admit_observation(&mut state, request.clone(), now);
        // A same-target, same-axis request rejected before admission cannot
        // steal attribution from the operation occupying the owner.
        let held: Vec<_> = (0..3)
            .map(|_| state.permits.try_acquire_ordinary().unwrap())
            .collect();
        let (handle, _ends) = boundary::OwnerHandleCore::new(&state);
        assert!(matches!(
            handle.enqueue_admission(request.clone(), None),
            Err(Error::RuntimeQueueFull { .. })
        ));
        assert!(original.check_settlement().is_ok());
        drop(held);
        request.context_mut().submission_order = 2;
        let permit = state.permits.try_acquire(request.context()).unwrap();
        let (input, completion, reply) = super::stage_admission(&mut state, request, permit);
        let admission_turn = state.begin_input_turn(now);
        let effects = state.input_in_turn(&admission_turn, input);
        assert!(
            matches!(
                original.check_settlement(),
                Err(Error::SettlementSuperseded { .. })
            ),
            "engine admission invalidates attribution before its Admitted effect drains"
        );
        for effect in effects {
            let _ = state.apply_effect(effect);
        }
        let _ = state.finish_input_turn(admission_turn, EngineTurn::INPUT_ONLY);
        let later = OperationObservation::new(
            ReceiptCore::admitted(
                reply.recv().unwrap().unwrap(),
                CameraId::CAMERA_1,
                completion,
                ACK,
            ),
            ACK,
        );
        let turn = state.begin_input_turn(now);
        for effect in state.input_in_turn(&turn, Input::Cancel { id: later.id() }) {
            let _ = state.apply_effect(effect);
        }
        let _ = state.finish_input_turn(turn, EngineTurn::INPUT_ONLY);
        assert!(matches!(
            original.check_settlement(),
            Err(Error::SettlementSuperseded { .. })
        ));
        assert!(matches!(
            later
                .core
                .completion
                .try_observed()
                .map(|event| event.value),
            Some(RuntimeOutcome::Cancelled)
        ));
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn halt_owner_acceptance_orders_concurrent_ingress_and_settlement_generation() {
        use crate::AffectedAxes;
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw)).unwrap();
        let (handle, ends) = boundary::OwnerHandleCore::new(&state);
        let stamp = state
            .motion
            .establish(CameraId::CAMERA_1, AffectedAxes::PAN_TILT, 0);
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let producer = handle.clone();
        let ready = Arc::clone(&barrier);
        let request = command(retrying());
        let joined = std::thread::spawn(move || {
            ready.wait();
            producer.enqueue_admission(request, None).unwrap()
        });
        barrier.wait();
        let cutoff = ends
            .lifecycle
            .fence_order(
                &state.motion,
                CameraId::CAMERA_1,
                Some(AffectedAxes::PAN_TILT),
            )
            .unwrap();
        let _held = joined.join().unwrap();
        let concurrent = ends.receivers.admissions.recv().unwrap();
        let order = concurrent.request.context().submission_order;
        assert_ne!(
            order, cutoff,
            "concurrent enqueue has exactly one side of acceptance"
        );
        assert!(
            stamp.check(crate::OperationId::from_raw(1)).is_err(),
            "generation publishes at acceptance"
        );
        let _later = handle.enqueue_admission(command(retrying()), None).unwrap();
        assert!(
            ends.receivers
                .admissions
                .recv()
                .unwrap()
                .request
                .context()
                .submission_order
                > cutoff
        );
        // The engine uses the actual acceptance cutoff, whichever side the
        // concurrent enqueue took; an earlier halt enqueue is not the fence.
        let mut motion = command(retrying());
        motion.context_mut().motion = Some(crate::runtime::engine::MotionEffect {
            axes: AffectedAxes::PAN_TILT,
            stop: false,
        });
        motion.context_mut().submission_order = order;
        let _ = state
            .engine
            .halt(CameraId::CAMERA_1, AffectedAxes::PAN_TILT, cutoff);
        let effects = state.engine.handle_turn(
            Input::Admit {
                ticket: AdmissionTicket(99),
                request: motion,
                slot: AdmissionSlot::Ordinary,
            },
            Instant::now(),
            EngineTurn::INPUT_ONLY,
        );
        assert_eq!(
            effects.iter().any(|effect| matches!(
                effect,
                Effect::AdmissionRejected {
                    error: Error::MotionSuperseded { .. },
                    ..
                }
            )),
            order < cutoff
        );
    }

    #[test]
    fn operation_delivery_before_equal_and_after_deadline_is_cached_honestly() {
        let deadline = Instant::now() + Duration::from_secs(1);
        for delivered in [
            deadline - Duration::from_nanos(1),
            deadline,
            deadline + Duration::from_nanos(1),
        ] {
            let (mut observation, cell) = observation_fixture();
            cell.resolve_at(RuntimeOutcome::Applied, delivered);
            assert_eq!(
                observation.applied(deadline).is_some(),
                delivered <= deadline
            );
            assert!(
                matches!(
                    observation.applied(deadline + Duration::from_secs(1)),
                    Some(Ok(()))
                ),
                "late result stays reusable"
            );
        }
    }

    #[test]
    fn timely_cancel_failure_wins_over_late_terminal_then_reobservation_gets_terminal() {
        let deadline = Instant::now() + Duration::from_secs(1);
        let (mut observation, terminal) = observation_fixture();
        let cancel = observation.cancellation_request();
        cancel.observer.resolve_at(Error::NotSupported, deadline);
        terminal.resolve_at(RuntimeOutcome::Applied, deadline + Duration::from_nanos(1));
        assert!(matches!(
            observation.cancellation(deadline),
            Some(Err(Error::NotSupported))
        ));
        assert!(matches!(
            observation.cancellation(deadline + Duration::from_nanos(1)),
            Some(Ok(CancellationOutcome::Completed))
        ));
        let (mut observation, _) = observation_fixture();
        observation
            .cancellation_request()
            .observer
            .resolve_at(Error::NotSupported, deadline + Duration::from_nanos(1));
        assert!(observation.cancellation(deadline).is_none());
        assert!(matches!(
            observation.cancellation(deadline + Duration::from_nanos(1)),
            Some(Err(Error::NotSupported))
        ));
    }

    #[test]
    fn settlement_attribution_is_axis_local_cached_and_profile_completion_is_exact() {
        use crate::{AffectedAxes, Settlement};
        let registry = Arc::new(MotionRegistry::default());
        let (mut observation, _) = observation_fixture();
        observation.core.motion =
            Some(registry.establish(CameraId::CAMERA_1, AffectedAxes::PAN_TILT, 1));
        registry.establish(CameraId::CAMERA_1, AffectedAxes::ZOOM, 2);
        registry.establish(CameraId::new(2).unwrap(), AffectedAxes::PAN_TILT, 3);
        assert!(observation.check_settlement().is_ok());
        let evidence = Settlement::observed_stable(
            AffectedAxes::PAN_TILT,
            ACK,
            crate::camera::MovementTolerance::default(),
        );
        assert_eq!(
            observation.commit_settlement(Ok(evidence), true).unwrap(),
            evidence
        );
        registry.establish(CameraId::CAMERA_1, AffectedAxes::PAN_TILT, 4);
        assert_eq!(
            observation.settled().unwrap().unwrap(),
            evidence,
            "established evidence persists"
        );
        let (mut superseded, _) = observation_fixture();
        superseded.core.motion =
            Some(registry.establish(CameraId::CAMERA_1, AffectedAxes::PAN_TILT, 5));
        registry.establish(CameraId::CAMERA_1, AffectedAxes::PAN_TILT, 6);
        assert!(matches!(
            superseded.commit_settlement(Ok(evidence), true),
            Err(Error::SettlementSuperseded { .. })
        ));
        let (mut exact, _) = observation_fixture();
        exact.core.motion = Some(registry.establish(CameraId::CAMERA_1, AffectedAxes::PAN_TILT, 7));
        registry.establish(CameraId::CAMERA_1, AffectedAxes::PAN_TILT, 8);
        let exact_evidence = Settlement::profile_completion(AffectedAxes::PAN_TILT);
        assert_eq!(
            exact.commit_settlement(Ok(exact_evidence), false).unwrap(),
            exact_evidence
        );
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn failed_settlement_poll_retains_cause_without_granting_original_move_replay() {
        let (observation, _) = observation_fixture();
        let error = settlement_error(
            Error::timeout(
                crate::FailureStage::Terminal,
                crate::Certainty::FailedConclusively,
            ),
            observation.id(),
        );
        assert_eq!(
            error.failure_context(),
            Some(crate::FailureContext::new(
                crate::FailureStage::Observation,
                crate::Certainty::Unconfirmed
            ))
        );
        assert!(!error.is_retryable());
        assert!(
            matches!(error, Error::SettlementObservationFailed { operation, source } if operation.get() == observation.id().get() && source.failure_context() == Some(crate::FailureContext::new(crate::FailureStage::Terminal, crate::Certainty::FailedConclusively)))
        );
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn halt_reserve_exhaustion_counts_each_axis_and_releases_no_held_permit() {
        let now = Instant::now();
        let mut policy = owner_policy(EnvelopeKind::Raw);
        policy.targets[1].as_mut().unwrap().control_reserve = 2;
        let mut state = OwnerState::new(policy).unwrap();
        let held: Vec<_> = (0..4)
            .map(|_| state.permits.try_acquire_ordinary().unwrap())
            .collect();
        let mut stop = command(retrying());
        stop.context_mut().control.class = ControlClass::Urgent;
        let reserved: Vec<_> = (0..2)
            .map(|_| state.permits.try_acquire(stop.context()).unwrap())
            .collect();
        assert!(reserved
            .iter()
            .all(|permit| permit.slot() == AdmissionSlot::ControlReserve));
        let stop_context = *stop.context();
        let receipt = state.accept_halt(
            crate::prepared::PreparedHalt {
                target: CameraId::CAMERA_1,
                axes: Some(crate::AffectedAxes::MOVEMENT),
                requests: [
                    Some(Ok(stop.clone())),
                    Some(Ok(stop.clone())),
                    Some(Ok(stop)),
                ],
                budget: ACK,
            },
            now + ACK,
            1,
            now,
        );
        for slot in receipt.slots {
            assert!(matches!(
                slot,
                Some(Err(Error::ControlReserveExhausted { .. }))
            ));
        }
        assert_eq!(state.metrics.admission_rejected, 3);
        assert_eq!(state.metrics.control_reserve_rejected, 3);
        assert_eq!(state.permits.available(), 0);
        drop(reserved);
        assert_eq!(
            state.permits.try_acquire(&stop_context).unwrap().slot(),
            AdmissionSlot::ControlReserve
        );
        assert_eq!(state.permits.available(), 0);
        drop(held);
        assert_eq!(state.permits.available(), 4);
    }
}

/// Issue #637: the receive-error contract both owners now share. A read error
/// that only reports "no bytes arrived" is an idle read, not a fault.
#[test]
fn an_idle_read_error_reports_no_data_rather_than_a_fault() {
    use std::io::ErrorKind;
    use std::sync::Arc;

    for idle in [
        Error::io_timeout(),
        Error::Io(Arc::new(std::io::Error::from(ErrorKind::WouldBlock))),
        Error::Io(Arc::new(std::io::Error::from(ErrorKind::Interrupted))),
        Error::io_timeout().with_context("idle poll"),
    ] {
        assert!(
            receive_reported_no_data(&idle),
            "an idle read must not be classified as a fault: {idle}"
        );
    }
    for fault in [
        Error::Io(Arc::new(std::io::Error::from(ErrorKind::TimedOut))),
        Error::TransportError("ICMP port unreachable".into()),
        Error::Io(Arc::new(std::io::Error::from(ErrorKind::ConnectionRefused))),
        Error::ConnectionClosed { reason: None },
    ] {
        assert!(
            !receive_reported_no_data(&fault),
            "a real read failure must stay a fault: {fault}"
        );
    }

    let keepalive_timeout = Error::Io(Arc::new(std::io::Error::from(ErrorKind::TimedOut)));
    assert!(
        !receive_fault_is_transient(&keepalive_timeout),
        "an OS TCP timeout is terminal instead of an endless idle read"
    );

    let invalid_input = Error::Io(Arc::new(std::io::Error::from(ErrorKind::InvalidInput)));
    assert!(
        !receive_fault_is_transient(&invalid_input),
        "a transport contract/configuration error closes immediately"
    );
}

/// Issue #729: normalizing a custom transport's already-typed close must not
/// duplicate the public variant prefix.
#[test]
fn transport_close_normalization_keeps_one_connection_closed_prefix() {
    let direct = Error::ConnectionClosed {
        reason: Some("peer closed connection".into()),
    };
    let reason = transport_close_reason(&direct).expect("typed close reason");
    assert_eq!(&*reason, "peer closed connection");

    let contextual = direct.with_context("control socket receive");
    let reason = transport_close_reason(&contextual).expect("contextual close reason");
    assert_eq!(&*reason, "control socket receive: peer closed connection");

    let normalized = boundary_error_for_input(&Input::Shutdown(ShutdownReason::TransportClosed {
        reason: Some(reason),
    }))
    .expect("shutdown boundary");
    assert_eq!(
        normalized.to_string(),
        "Connection closed: control socket receive: peer closed connection"
    );
}

/// Issue #637: a datagram send failure fails one request while the session
/// keeps running, so the value the caller sees must never claim a replacement
/// session is required.
#[test]
fn a_datagram_send_failure_is_normalized_to_a_per_request_error() {
    let normalized = normalize_datagram_send_error(Error::ConnectionClosed {
        reason: Some("socket closed".into()),
    });
    assert!(matches!(normalized, Error::TransportError(_)));
    assert!(!normalized.requires_new_session());
    assert!(
        normalized.to_string().contains("socket closed"),
        "the original cause must survive normalization: {normalized}"
    );

    // Errors that are already per-request are passed through untouched.
    assert!(matches!(
        normalize_datagram_send_error(Error::io_timeout()),
        Error::Timeout { .. }
    ));
    assert!(matches!(
        normalize_datagram_send_error(Error::TransportError("serial encode failed".into())),
        Error::TransportError(_)
    ));
}

/// Issue #634: every normative issue-542 lifecycle fixture replays through the
/// production owner and engine.
///
/// The replay below is deliberately *not* a model of the runtime. It drives the
/// real [`OwnerState`] — which owns the real protocol engine, the real
/// admission permit pool, the real terminal observers and the real
/// applied-state subscribers — and renders what production actually produced
/// back into the fixture's own record language. Every fixture record is then
/// compared against that rendering, so an engine or owner behavior change fails
/// the fixture instead of a hand-written simulator.
///
/// Only [`Effect::DeadlineExpired`] has no fixture vocabulary: the trace format
/// names the *boundary* that resolved a request rather than the deadline
/// classification, so the effect is consumed to prove `source=scheduler` and
/// emits no record of its own.
mod lifecycle_trace {
    use std::{
        borrow::Cow,
        collections::{BTreeMap, BTreeSet, VecDeque},
        sync::Arc,
        time::{Duration, Instant},
    };

    use super::super::*;
    use crate::{
        protocol::response::{decode_basic, BasicKind},
        runtime::engine::{
            CancellationPolicy, ControlPolicy, EncodedMessage, InquiryRoute, ReplyShape,
            RequestContext, RetryPolicy, TimeoutPolicy, TransportKind,
        },
        CameraId, Error,
    };

    const OBSERVER_LATE_DELIVERY: &str =
        include_str!("../../../tests/fixtures/issue_542/lifecycle/observer_late_delivery.trace");
    const DEADLINE_CLASSES: &str =
        include_str!("../../../tests/fixtures/issue_542/lifecycle/deadline_classes.trace");
    const CAPACITY_AND_FAILURES: &str =
        include_str!("../../../tests/fixtures/issue_542/lifecycle/capacity_and_failures.trace");
    const PTZOPTICS_CANCEL: &str =
        include_str!("../../../tests/fixtures/issue_542/lifecycle/ptzoptics_cancel.trace");
    const BLOCKING_OUT_OF_ORDER: &str =
        include_str!("../../../tests/fixtures/issue_542/lifecycle/blocking_out_of_order.trace");

    /// No fixture request may reach a deadline it did not ask for.
    const UNREACHABLE: Duration = Duration::from_secs(3_600);

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum ObserverKind {
        Blocking,
        Async,
    }

    struct ObserverSlot {
        kind: ObserverKind,
        /// Dropping the observer *is* the detach operation.
        observer: Option<TerminalObserver>,
        cell: Arc<ObserverCell<RuntimeOutcome>>,
        terminal_source: Option<&'static str>,
    }

    struct Pending {
        label: String,
        kind: ObserverKind,
        observer: TerminalObserver,
        admission: flume::Receiver<Result<Admitted, Error>>,
        permits_before: usize,
        active_before: usize,
    }

    fn lines(fixture: &str) -> Vec<&str> {
        fixture
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect()
    }

    fn tokens(line: &str) -> Vec<&str> {
        line.split_ascii_whitespace().collect()
    }

    fn field<'a>(tokens: &[&'a str], name: &str) -> Option<&'a str> {
        tokens.iter().find_map(|token| {
            token
                .split_once('=')
                .filter(|(key, _)| *key == name)
                .map(|(_, value)| value)
        })
    }

    fn required<'a>(tokens: &[&'a str], name: &str) -> &'a str {
        field(tokens, name).unwrap_or_else(|| panic!("fixture field {name} in {tokens:?}"))
    }

    fn hex(text: &str) -> Vec<u8> {
        assert!(text.len().is_multiple_of(2), "fixture byte string {text}");
        (0..text.len() / 2)
            .map(|index| u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).unwrap())
            .collect()
    }

    fn hex_text(bytes: &[u8]) -> String {
        bytes.iter().fold(String::new(), |mut text, byte| {
            use std::fmt::Write as _;
            let _ = write!(text, "{byte:02x}");
            text
        })
    }

    fn phase_name(phase: Phase) -> String {
        match phase {
            Phase::Ready { .. } => "ready".to_owned(),
            Phase::Sending { .. } => "sending".to_owned(),
            Phase::AwaitingAck { .. } => "awaiting-ack".to_owned(),
            Phase::AwaitingCompletion { .. } => "awaiting-completion".to_owned(),
            Phase::Executing { socket, .. } => {
                format!("executing(socket={})", socket.as_socket_number())
            }
            Phase::AwaitingReply { .. } => "awaiting-reply".to_owned(),
            Phase::Backoff { .. } => "backoff".to_owned(),
            Phase::AwaitingCancellationResolution { socket, .. } => {
                format!(
                    "awaiting-cancellation(socket={})",
                    socket.as_socket_number()
                )
            }
        }
    }

    const fn session_name(state: SessionState) -> &'static str {
        match state {
            SessionState::Running => "running",
            SessionState::Closed => "closed",
            SessionState::Shutdown => "shutdown",
            SessionState::Poisoned => "poisoned",
        }
    }

    const fn transport_name(transport: TransportKind) -> &'static str {
        match transport {
            TransportKind::Datagram => "datagram",
            TransportKind::Stream => "stream",
        }
    }

    const fn cancellation_name(policy: CancellationPolicy) -> &'static str {
        match policy {
            CancellationPolicy::Supported => "supported",
            CancellationPolicy::Unsupported => "unsupported",
        }
    }

    /// Maps one engine error back onto the fixture's terminal vocabulary.
    ///
    /// A transport failure round-trips through its injected error, so an engine
    /// that substitutes or collapses errors renders a different label.
    fn error_label(error: &Error) -> String {
        match error {
            Error::Timeout { .. } => "Timeout".to_owned(),
            Error::SyntaxError => "SyntaxError".to_owned(),
            // #795: an error after the request's ACK keeps the camera's exact
            // error as its source.
            Error::CommandFailedAfterAck { source, .. } => {
                format!("CommandFailedAfterAck({})", error_label(source))
            }
            Error::MessageLengthError => "MessageLengthError".to_owned(),
            Error::CommandBufferFull => "CommandBufferFull".to_owned(),
            Error::CommandCanceled => "CommandCanceled".to_owned(),
            Error::NoSocket => "NoSocket".to_owned(),
            Error::CommandNotExecutable => "CommandNotExecutable".to_owned(),
            Error::NotSupported => "NotSupported".to_owned(),
            Error::RuntimeShutdown => "RuntimeShutdown".to_owned(),
            Error::StreamPoisoned { .. } => "StreamPoisoned".to_owned(),
            Error::RuntimeQueueFull { .. } => "Capacity".to_owned(),
            Error::TransportError(reason) => reason.to_string(),
            other => format!("{other:?}"),
        }
    }

    fn outcome_text(outcome: &RuntimeOutcome) -> String {
        match outcome {
            RuntimeOutcome::Written => "Written".to_owned(),
            RuntimeOutcome::Applied => "Applied".to_owned(),
            RuntimeOutcome::Cancelled => "Cancelled".to_owned(),
            RuntimeOutcome::Failed(error) => format!("Error::{}", error_label(error)),
            RuntimeOutcome::Reply { route, payload } => {
                format!("Reply(route={route:?},payload={payload:?})")
            }
        }
    }

    fn write_result(label: &str) -> Result<TransmissionMeta, Error> {
        match label {
            "ok" => Ok(TransmissionMeta { sequence: None }),
            "Timeout" => Err(Error::io_timeout()),
            other => Err(Error::TransportError(Cow::Owned(other.to_owned()))),
        }
    }

    /// Recovers the fixture label of the write failure that poisoned a stream
    /// session from the owner's own boundary error.
    fn poison_cause(state: &OwnerState) -> String {
        match state.boundary_error() {
            Some(Error::StreamPoisoned { reason }) => ["WriteFailed", "WriteTimeout", "Timeout"]
                .into_iter()
                .find(|label| {
                    write_result(label)
                        .err()
                        .is_some_and(|error| error.to_string() == reason.as_ref())
                })
                .map_or_else(|| reason.to_string(), ToOwned::to_owned),
            other => panic!("a poisoned session without a stream-poison boundary error: {other:?}"),
        }
    }

    fn owner_policy(
        transport: TransportKind,
        capacity: usize,
        cancellation: CancellationPolicy,
    ) -> OwnerPolicy {
        let protocol = ProtocolPolicy {
            capacity,
            transport,
            inquiry_capacity: capacity,
            inquiry_cooldown: Duration::ZERO,
            raw_inquiry_release_hold: Duration::from_millis(10),
            ..ProtocolPolicy::test_default()
        };
        let mut targets = [None; 9];
        for slot in targets.iter_mut().take(4).skip(1) {
            *slot = Some(TargetPolicy {
                cancellation,
                ..TargetPolicy::test_default()
            });
        }
        OwnerPolicy::with_targets(protocol, targets).unwrap()
    }

    fn command(
        target: CameraId,
        wire: &str,
        cancellation: CancellationPolicy,
        ack: Duration,
    ) -> RuntimeRequest {
        RuntimeRequest::Command {
            wire: Arc::new(EncodedMessage::new(&hex(wire)).unwrap()),
            context: RequestContext {
                motion: None,
                submission_order: 0,
                dispatch_deadline: None,
                target,
                timeout: TimeoutPolicy {
                    ack,
                    completion: UNREACHABLE,
                    inquiry: UNREACHABLE,
                    cancellation: UNREACHABLE,
                    ambiguity: UNREACHABLE,
                },
                retry: RetryPolicy::NEVER,
                control: ControlPolicy::default(),
                cancellation,
                reply_shape: ReplyShape::AckThenCompletion,
            },
            // Applied-state delivery is one of the normative observations, and
            // an unsubscribed fixture simply produces no subscriber record.
            applied_state: Some(AppliedStateProjection::set(StateKey::Spotlight, &[1]).unwrap()),
        }
    }

    fn inquiry(
        target: CameraId,
        wire: &str,
        cancellation: CancellationPolicy,
        reply: Duration,
    ) -> RuntimeRequest {
        RuntimeRequest::Inquiry {
            wire: Arc::new(EncodedMessage::new(&hex(wire)).unwrap()),
            context: RequestContext {
                motion: None,
                submission_order: 0,
                dispatch_deadline: None,
                target,
                timeout: TimeoutPolicy {
                    ack: UNREACHABLE,
                    completion: UNREACHABLE,
                    inquiry: reply,
                    cancellation: UNREACHABLE,
                    ambiguity: UNREACHABLE,
                },
                retry: RetryPolicy::NEVER,
                control: ControlPolicy::default(),
                cancellation,
                reply_shape: ReplyShape::AckThenCompletion,
            },
            route: InquiryRoute::UNKNOWN,
        }
    }

    struct Replay {
        state: Option<OwnerState>,
        cancellation: CancellationPolicy,
        base: Instant,
        at: u64,
        now: Instant,
        scheduler_dispatch: BTreeMap<String, u64>,
        labels: BTreeMap<String, RequestId>,
        observers: BTreeMap<RequestId, ObserverSlot>,
        subscribers: Vec<(String, AppliedStateSubscription)>,
        pending: Option<Pending>,
        write_label: String,
        cancel_phase: Option<Phase>,
        next_transmission: u64,
        cancel_transmissions: usize,
    }

    impl Replay {
        fn new(scheduler_dispatch: BTreeMap<String, u64>) -> Self {
            let base = Instant::now();
            Self {
                state: None,
                cancellation: CancellationPolicy::Supported,
                base,
                at: 0,
                now: base,
                scheduler_dispatch,
                labels: BTreeMap::new(),
                observers: BTreeMap::new(),
                subscribers: Vec::new(),
                pending: None,
                write_label: "ok".to_owned(),
                cancel_phase: None,
                next_transmission: 0,
                cancel_transmissions: 0,
            }
        }

        fn state(&self) -> &OwnerState {
            self.state.as_ref().expect("fixture session")
        }

        fn state_mut(&mut self) -> &mut OwnerState {
            self.state.as_mut().expect("fixture session")
        }

        fn id(&self, label: &str) -> RequestId {
            *self
                .labels
                .get(label)
                .unwrap_or_else(|| panic!("unknown fixture request label {label}"))
        }

        fn step(&mut self, input: &[&str]) -> Vec<String> {
            self.at = input[0].parse().expect("fixture timestamp");
            self.now = self.base + Duration::from_millis(self.at);
            let mut out = Vec::new();
            match input[2] {
                "session" => self.session(input, &mut out),
                "subscribe-applied" => self.subscribe_applied(input, &mut out),
                "admit" => self.admit(input, &mut out),
                "blocking-submit" => self.blocking_submit(input, &mut out),
                "dispatch" => self.dispatch(self.id(input[3]), required(input, "result"), &mut out),
                "frame" => self.frame(input, &mut out),
                "observer-timeout" => self.observer_deadline(input, true, &mut out),
                "detach" => self.observer_deadline(input, false, &mut out),
                "wake" => self.wake(&mut out),
                "cancel" => self.cancel(input, &mut out),
                "shutdown" => self.shutdown(&mut out),
                "blocking-wait" => self.blocking_wait(input, &mut out),
                "inspect-transmissions" => self.inspect_transmissions(input, &mut out),
                other => panic!("unknown lifecycle fixture input {other}"),
            }
            out
        }

        fn session(&mut self, input: &[&str], out: &mut Vec<String>) {
            let transport = match required(input, "transport") {
                "datagram" => TransportKind::Datagram,
                "stream" => TransportKind::Stream,
                other => panic!("unknown fixture transport {other}"),
            };
            let capacity = required(input, "capacity")
                .parse()
                .expect("fixture admission capacity");
            let cancellation = match required(input, "cancel") {
                "supported" => CancellationPolicy::Supported,
                "unsupported" => CancellationPolicy::Unsupported,
                other => panic!("unknown fixture cancellation policy {other}"),
            };
            self.cancellation = cancellation;
            self.labels.clear();
            self.observers.clear();
            self.subscribers.clear();
            self.pending = None;
            self.next_transmission = 0;
            self.cancel_transmissions = 0;
            let state = OwnerState::new(owner_policy(transport, capacity, cancellation)).unwrap();
            let registered = state.policy().targets[1].expect("camera one is registered");
            out.push(format!(
                "{} effect session state={} transport={} capacity={} cancel={}",
                self.at,
                session_name(state.state()),
                transport_name(state.policy().protocol.transport),
                state.permits().capacity(),
                cancellation_name(registered.cancellation),
            ));
            self.state = Some(state);
        }

        fn subscribe_applied(&mut self, input: &[&str], out: &mut Vec<String>) {
            let name = input[3].to_owned();
            let target = CameraId::new(required(input, "target").parse().unwrap()).unwrap();
            let subscription = self
                .state_mut()
                .subscribe_applied(Some(target), 8)
                .expect("applied-state subscription");
            self.subscribers.push((name.clone(), subscription));
            out.push(format!(
                "{} effect subscriber-added observer={name} target={}",
                self.at,
                target.id()
            ));
        }

        fn admit(&mut self, input: &[&str], out: &mut Vec<String>) {
            let label = input[3].to_owned();
            let target = CameraId::new(required(input, "target").parse().unwrap()).unwrap();
            let wire = required(input, "wire");
            let kind = match required(input, "observer") {
                "async" => ObserverKind::Async,
                "blocking" => ObserverKind::Blocking,
                other => panic!("unknown fixture observer kind {other}"),
            };
            // A scheduler deadline in the fixture is an absolute trace time; the
            // engine computes it from the instant the request is actually sent.
            let ack = field(input, "scheduler").map_or(UNREACHABLE, |value| {
                let deadline: u64 = value.parse().expect("fixture scheduler deadline");
                let sent_at = self.scheduler_dispatch[&label];
                Duration::from_millis(deadline - sent_at)
            });

            let permits_before = self.state().permits().available();
            let active_before = self.state().active_len();
            let Ok(permit) = self.state().permits().try_acquire_ordinary() else {
                out.push(format!(
                    "{} outcome admission ticket={label} error=Capacity capacity={} engine-entry=none observer=none transmission=none",
                    self.at,
                    self.state().permits().capacity()
                ));
                assert_eq!(
                    self.state().active_len(),
                    active_before,
                    "a rejected admission never creates an engine entry"
                );
                return;
            };

            let request = match field(input, "kind").unwrap_or("command") {
                "command" => command(target, wire, self.cancellation, ack),
                "inquiry" => inquiry(target, wire, self.cancellation, ack),
                other => panic!("unknown fixture request kind {other}"),
            };
            let now = self.now;
            let (staged, observer, admission) =
                super::stage_admission(self.state_mut(), request, permit);
            let Input::Admit {
                ticket,
                request,
                slot,
            } = staged
            else {
                panic!("staged admission did not produce an admit input")
            };
            self.pending = Some(Pending {
                label,
                kind,
                observer,
                admission,
                permits_before,
                active_before,
            });
            // Admission alone: the fixture's own `dispatch` step decides when
            // the scheduler writes.
            let turn = self.state().begin_input_turn(now);
            let mut effects = self.state_mut().input_in_turn(
                &turn,
                Input::Admit {
                    ticket,
                    request,
                    slot,
                },
            );
            effects.extend(
                self.state_mut()
                    .finish_input_turn(turn, EngineTurn::INPUT_ONLY),
            );
            self.drain(effects, "admission", out);
            assert!(
                self.pending.is_none(),
                "admission produced neither an entry nor a rejection"
            );
        }

        /// A blocking submission is admission followed, in the same owner
        /// turn, by the scheduler's ordinary dispatch of the admitted request.
        fn blocking_submit(&mut self, input: &[&str], out: &mut Vec<String>) {
            self.admit(input, out);
            let id = self.id(input[3]);
            self.dispatch(id, "ok", out);
        }

        fn dispatch(&mut self, id: RequestId, result: &str, out: &mut Vec<String>) {
            self.write_label = result.to_owned();
            let now = self.now;
            let effects = self.state_mut().advance(now);
            assert!(
                effects.iter().any(
                    |effect| matches!(effect, Effect::Transmit { request, .. } if *request == id)
                ),
                "fixture dispatch is not the scheduler winner"
            );
            self.drain(effects, "transport", out);
        }

        fn frame(&mut self, input: &[&str], out: &mut Vec<String>) {
            let kind = input[3];
            let bytes = hex(required(input, "bytes"));
            let decoded = decode_basic(&bytes).expect("fixture frame bytes decode");
            let response = match (kind, decoded.kind) {
                ("ack", BasicKind::Ack) => DecodedResponse::Ack {
                    socket: decoded.socket,
                },
                ("complete", BasicKind::Completion) => DecodedResponse::Completion {
                    socket: decoded.socket,
                },
                ("error", BasicKind::Error(code)) => DecodedResponse::Error {
                    socket: decoded.socket,
                    code,
                },
                other => panic!("fixture frame {other:?} disagrees with its own bytes"),
            };
            if let Some(socket) = field(input, "socket") {
                assert_eq!(
                    decoded.socket.map(|socket| socket.as_socket_number()),
                    socket.parse::<u8>().ok(),
                    "fixture socket disagrees with its own bytes"
                );
            }
            if let Some(code) = field(input, "code") {
                assert_eq!(
                    decoded.kind,
                    BasicKind::Error(u8::from_str_radix(code, 16).unwrap()),
                    "fixture error code disagrees with its own bytes"
                );
            }
            // Every lifecycle fixture is a raw session, so correlation is by
            // target and socket rather than by envelope sequence.
            let target = decoded.source;
            let sequence: Option<EnvelopeSequence> = None;
            let now = self.now;
            let pre_socket = |state: &OwnerState, id: RequestId| match state.request_state(id) {
                Some((Phase::Executing { socket, .. }, _)) => Some(socket),
                _ => None,
            };
            let owned = self
                .labels
                .values()
                .copied()
                .filter_map(|id| pre_socket(self.state(), id).map(|socket| (id, socket)))
                .collect::<BTreeMap<_, _>>();

            let turn = self.state().begin_input_turn(now);
            let effects = self.state_mut().input_in_turn(
                &turn,
                Input::Frame(DecodedFrame {
                    target,
                    sequence,
                    response,
                }),
            );
            let routed = effects
                .iter()
                .find_map(effect_request)
                .expect("a fixture frame is always routed to one request");
            let socket = decoded
                .socket
                .or_else(|| owned.get(&routed).copied())
                .expect("a routed raw frame owns a socket");
            out.push(format!(
                "{} effect frame-routed id={} target={} via={} socket={} bytes={}",
                self.at,
                routed.get(),
                target.id(),
                if sequence.is_some() {
                    "sequence"
                } else {
                    "target-socket"
                },
                socket.as_socket_number(),
                hex_text(&bytes)
            ));
            self.drain(effects, "protocol", out);
        }

        fn observer_deadline(&mut self, input: &[&str], timed_out: bool, out: &mut Vec<String>) {
            let id = self.id(input[3]);
            let slot = self
                .observers
                .get_mut(&id)
                .expect("fixture observer is registered");
            let observer = slot.observer.as_ref().expect("an attached observer");
            assert!(
                observer.try_recv().is_none(),
                "an observer that gives up cannot already hold a terminal outcome"
            );
            slot.observer = None;
            assert!(
                !slot.cell.is_attached(),
                "dropping the observer detaches it"
            );
            if timed_out {
                out.push(format!(
                    "{} outcome operation id={} Error::{} source=observer",
                    self.at,
                    id.get(),
                    error_label(&Error::io_timeout())
                ));
            }
            let (phase, _) = self
                .state()
                .request_state(id)
                .expect("observer detach never removes engine routing");
            out.push(format!(
                "{} effect observer-detached id={} protocol-state={} routing=retained",
                self.at,
                id.get(),
                phase_name(phase)
            ));
        }

        fn wake(&mut self, out: &mut Vec<String>) {
            let now = self.now;
            let effects = self.state_mut().advance(now);
            self.drain(effects, "scheduler", out);
        }

        fn cancel(&mut self, input: &[&str], out: &mut Vec<String>) {
            let id = self.id(input[3]);
            let (phase, _) = self
                .state()
                .request_state(id)
                .expect("cancellation target is active");
            self.cancel_phase = Some(phase);
            let cancels_before = self.cancel_transmissions;
            let now = self.now;
            let turn = self.state().begin_input_turn(now);
            let effects = self.state_mut().input_in_turn(&turn, Input::Cancel { id });
            let ignored = effects.iter().all(|effect| {
                matches!(
                    effect,
                    Effect::CancellationObservation {
                        observation: CancellationObservation::Failed(_),
                        ..
                    }
                )
            });
            self.drain(effects, "local", out);
            if ignored {
                let (phase, cancellation) = self
                    .state()
                    .request_state(id)
                    .expect("an ignored cancellation leaves the request able to complete");
                assert_eq!(
                    cancellation,
                    CancelState::None,
                    "an ignored cancellation records no cancellation substate"
                );
                assert_eq!(
                    self.cancel_transmissions, cancels_before,
                    "an ignored cancellation emits no cancel frame"
                );
                out.push(format!(
                    "{} effect cancellation-ignored id={} protocol-state={} cancel-frame=none",
                    self.at,
                    id.get(),
                    phase_name(phase)
                ));
            }
        }

        fn shutdown(&mut self, out: &mut Vec<String>) {
            let now = self.now;
            let turn = self.state().begin_input_turn(now);
            let effects = self
                .state_mut()
                .input_in_turn(&turn, Input::Shutdown(ShutdownReason::Explicit));
            self.drain(effects, "shutdown", out);
        }

        fn blocking_wait(&mut self, input: &[&str], out: &mut Vec<String>) {
            let id = self.id(input[3]);
            let slot = self
                .observers
                .get_mut(&id)
                .expect("fixture observer is registered");
            let observer = slot.observer.take().expect("a retained blocking observer");
            let outcome = observer
                .try_recv()
                .expect("a blocking wait consumes its retained terminal outcome");
            out.push(format!(
                "{} outcome blocking-wait id={} {} source={}",
                self.at,
                id.get(),
                outcome_text(&outcome),
                slot.terminal_source.expect("recorded terminal source")
            ));
        }

        fn inspect_transmissions(&mut self, input: &[&str], out: &mut Vec<String>) {
            assert_eq!(required(input, "kind"), "cancel");
            out.push(format!(
                "{} outcome transmissions kind=cancel count={}",
                self.at, self.cancel_transmissions
            ));
        }

        fn drain(
            &mut self,
            effects: VecDeque<Effect>,
            boundary: &'static str,
            out: &mut Vec<String>,
        ) {
            let mut queue = effects;
            let mut deferred: Vec<(RequestId, String)> = Vec::new();
            let mut applied: Vec<(RequestId, String)> = Vec::new();
            let mut deadlines: BTreeSet<RequestId> = BTreeSet::new();
            while let Some(effect) = queue.pop_front() {
                match effect {
                    Effect::Admitted { id, .. } => {
                        self.record_admitted(id, effect, out);
                    }
                    Effect::AdmissionRejected { .. } => {
                        self.record_admission_rejected(effect, out);
                    }
                    Effect::Transition { id, from, to, .. } => {
                        self.state_mut().apply_effect(effect);
                        out.push(format!(
                            "{} effect state id={} from={} to={}",
                            self.at,
                            id.get(),
                            phase_name(from),
                            phase_name(to)
                        ));
                    }
                    Effect::Transmit { .. } => {
                        self.record_transmit(effect, &mut queue, out);
                    }
                    Effect::DeadlineExpired { id, .. } => {
                        deadlines.insert(id);
                        self.state_mut().apply_effect(effect);
                    }
                    Effect::CancellationRecorded { id } => {
                        self.state_mut().apply_effect(effect);
                        let queued = matches!(
                            self.cancel_phase,
                            Some(Phase::Ready { .. } | Phase::Backoff { .. })
                        );
                        out.push(format!(
                            "{} effect cancellation-recorded id={} disposition={} cancel-frame=none",
                            self.at,
                            id.get(),
                            if queued { "queued-local" } else { "sent" }
                        ));
                    }
                    Effect::CancellationObservation {
                        id,
                        ref observation,
                    } => {
                        let (text, source) = match observation {
                            CancellationObservation::Recorded => ("Recorded".to_owned(), "local"),
                            CancellationObservation::Cancelled => ("Cancelled".to_owned(), "local"),
                            CancellationObservation::Completed => {
                                ("Completed".to_owned(), "protocol")
                            }
                            CancellationObservation::Failed(error) => (
                                format!("Error::{}", error_label(error)),
                                if matches!(error, Error::NotSupported) {
                                    "profile"
                                } else {
                                    "engine"
                                },
                            ),
                        };
                        deferred.push((
                            id,
                            format!(
                                "{} outcome cancellation id={} {text} source={source}",
                                self.at,
                                id.get()
                            ),
                        ));
                        self.state_mut().apply_effect(effect);
                    }
                    Effect::AppliedState { effect: projection } => {
                        self.state_mut()
                            .apply_effect(Effect::AppliedState { effect: projection });
                        for (name, subscription) in &self.subscribers {
                            if let Some(AppliedStateEvent(delivered)) = subscription.try_recv() {
                                applied.push((
                                    projection.request,
                                    format!(
                                        "{} outcome applied-state observer={name} target={} id={} state={}",
                                        self.at,
                                        delivered.target.id(),
                                        delivered.request.get(),
                                        if delivered.projection.is_known() {
                                            "applied"
                                        } else {
                                            "invalidated"
                                        }
                                    ),
                                ));
                            }
                        }
                    }
                    Effect::Terminal { id, ref outcome } => {
                        let source = if deadlines.contains(&id) {
                            "scheduler"
                        } else if boundary == "protocol"
                            && matches!(outcome, RuntimeOutcome::Failed(_))
                        {
                            "camera"
                        } else {
                            boundary
                        };
                        let text = outcome_text(outcome);
                        let permits_before = self.state().permits().available();
                        self.state_mut().apply_effect(effect.clone());
                        let removed = self.state().request_state(id).is_none();
                        let released = self.state().permits().available() == permits_before + 1;
                        out.push(format!(
                            "{} effect terminal id={} outcome={text} source={source} state={} permit={}",
                            self.at,
                            id.get(),
                            if removed { "removed" } else { "retained" },
                            if released { "released" } else { "held" }
                        ));
                        self.record_observer(id, source, out);
                        applied.retain(|(request, record)| {
                            let mine = *request == id;
                            if mine {
                                out.push(record.clone());
                            }
                            !mine
                        });
                        deferred.retain(|(request, record)| {
                            let mine = *request == id;
                            if mine {
                                out.push(record.clone());
                            }
                            !mine
                        });
                    }
                    Effect::SessionChanged { from, to } => {
                        self.state_mut().apply_effect(effect);
                        let detail = match to {
                            SessionState::Poisoned => {
                                format!("cause={}", poison_cause(self.state()))
                            }
                            SessionState::Shutdown => "reason=explicit".to_owned(),
                            other => panic!("no fixture vocabulary for session state {other:?}"),
                        };
                        out.push(format!(
                            "{} effect session from={} to={} {detail}",
                            self.at,
                            session_name(from),
                            session_name(to)
                        ));
                    }
                    Effect::RetryScheduled { .. } | Effect::Ignored(_) => {
                        panic!("no fixture vocabulary for {effect:?}")
                    }
                }
            }
            for (_, record) in applied {
                out.push(record);
            }
            for (_, record) in deferred {
                out.push(record);
            }
        }

        fn record_admitted(&mut self, id: RequestId, effect: Effect, out: &mut Vec<String>) {
            let pending = self.pending.take().expect("a staged admission");
            self.state_mut().apply_effect(effect);
            let target = self
                .state()
                .diagnostics()
                .filter_map(|event| match event {
                    DiagnosticEvent::Admitted {
                        id: recorded,
                        target,
                        ..
                    } if *recorded == id => Some(*target),
                    _ => None,
                })
                .last()
                .expect("owner records every admission");
            let (phase, _) = self
                .state()
                .request_state(id)
                .expect("an admitted request owns an engine entry");
            let acquired = self.state().permits().available() == pending.permits_before - 1;
            out.push(format!(
                "{} effect admitted ticket={} id={} target={} permit={} state={} observer={}",
                self.at,
                pending.label,
                id.get(),
                target.id(),
                if acquired { "acquired" } else { "unclaimed" },
                phase_name(phase),
                match pending.kind {
                    ObserverKind::Async => "async",
                    ObserverKind::Blocking => "blocking",
                }
            ));
            let admitted = pending
                .admission
                .try_recv()
                .expect("admission reply")
                .expect("admission reply carries the engine identity");
            out.push(format!(
                "{} outcome admission ticket={} id={}",
                self.at,
                pending.label,
                admitted.get()
            ));
            self.labels.insert(pending.label, id);
            self.observers.insert(
                id,
                ObserverSlot {
                    kind: pending.kind,
                    cell: pending
                        .observer
                        .cell()
                        .expect("the owner holds the admitted cell"),
                    observer: Some(pending.observer),
                    terminal_source: None,
                },
            );
        }

        fn record_admission_rejected(&mut self, effect: Effect, out: &mut Vec<String>) {
            let pending = self.pending.take().expect("a staged admission");
            let AppliedEffect::AdmissionRejected(error) = self.state_mut().apply_effect(effect)
            else {
                panic!("a rejected admission did not report its error")
            };
            let capacity = match &error {
                Error::RuntimeQueueFull { capacity } => format!(" capacity={capacity}"),
                _ => String::new(),
            };
            assert_eq!(
                self.state().active_len(),
                pending.active_before,
                "a rejected admission never creates an engine entry"
            );
            assert_eq!(
                self.state().permits().available(),
                pending.permits_before,
                "a rejected admission releases its permit"
            );
            assert!(
                matches!(pending.admission.try_recv(), Ok(Err(_)) | Err(_)),
                "a rejected admission never replies with an engine identity"
            );
            out.push(format!(
                "{} outcome admission ticket={} error={}{capacity} engine-entry=none observer=none transmission=none",
                self.at,
                pending.label,
                error_label(&error)
            ));
        }

        fn record_transmit(
            &mut self,
            effect: Effect,
            queue: &mut VecDeque<Effect>,
            out: &mut Vec<String>,
        ) {
            let Effect::Transmit {
                request, ref kind, ..
            } = effect
            else {
                unreachable!()
            };
            self.next_transmission += 1;
            let transmission = self.next_transmission;
            match kind {
                Transmission::Request { target, wire, .. } => out.push(format!(
                    "{} effect transmit tx={transmission} id={} kind=request target={} wire={}",
                    self.at,
                    request.get(),
                    target.id(),
                    hex_text(wire.as_bytes())
                )),
                Transmission::Cancel { target, socket, .. } => {
                    self.cancel_transmissions += 1;
                    out.push(format!(
                        "{} effect transmit tx={transmission} id={} kind=cancel target={} socket={}",
                        self.at,
                        request.get(),
                        target.id(),
                        socket.as_socket_number()
                    ));
                }
            }
            let AppliedEffect::Transmit(staged) = self.state_mut().apply_effect(effect) else {
                panic!("a transmit effect did not stage a write")
            };
            let label = self.write_label.clone();
            out.push(format!(
                "{} driver-input transmission-finished tx={transmission} result={label}",
                self.at
            ));
            let now = self.now;
            let produced = self.state_mut().finish_write_turn(
                &staged,
                write_result(&label),
                now,
                EngineTurn::INPUT_ONLY,
            );
            prepend_effects(queue, produced);
        }

        fn record_observer(&mut self, id: RequestId, source: &'static str, out: &mut Vec<String>) {
            let at = self.at;
            let Some(slot) = self.observers.get_mut(&id) else {
                return;
            };
            slot.terminal_source = Some(source);
            let Some(observer) = slot.observer.as_ref() else {
                assert!(
                    !slot.cell.is_attached() && !slot.cell.resolved.load(Ordering::Acquire),
                    "a detached observer is never resolved"
                );
                out.push(format!(
                    "{at} effect delivery-discarded id={} observer=detached",
                    id.get()
                ));
                return;
            };
            match slot.kind {
                ObserverKind::Async => {
                    let outcome = observer
                        .try_recv()
                        .expect("an attached observer receives its terminal outcome");
                    out.push(format!(
                        "{at} outcome operation id={} {} source={source}",
                        id.get(),
                        outcome_text(&outcome)
                    ));
                }
                ObserverKind::Blocking => {
                    assert!(
                        !observer.receiver.is_empty(),
                        "a blocking observer retains its terminal outcome until it waits"
                    );
                    out.push(format!(
                        "{at} effect outcome-retained id={} observer=blocking",
                        id.get()
                    ));
                }
            }
        }
    }

    fn effect_request(effect: &Effect) -> Option<RequestId> {
        match effect {
            Effect::Transition { id, .. }
            | Effect::Terminal { id, .. }
            | Effect::CancellationRecorded { id }
            | Effect::CancellationObservation { id, .. }
            | Effect::DeadlineExpired { id, .. }
            | Effect::Admitted { id, .. }
            | Effect::RetryScheduled { id, .. } => Some(*id),
            Effect::AppliedState { effect } => Some(effect.request),
            Effect::Transmit { request, .. } => Some(*request),
            Effect::AdmissionRejected { .. }
            | Effect::SessionChanged { .. }
            | Effect::Ignored(_) => None,
        }
    }

    /// Replays one fixture through the production owner and asserts that every
    /// `effect`, `outcome` and `driver-input` record it names is exactly what
    /// production produced.
    fn replay(fixture: &str) -> Vec<String> {
        let records = lines(fixture);
        let mut scheduler_dispatch = BTreeMap::new();
        for record in &records {
            let parsed = tokens(record);
            if parsed[1] == "input" && matches!(parsed[2], "dispatch" | "blocking-submit") {
                scheduler_dispatch.insert(parsed[3].to_owned(), parsed[0].parse().unwrap());
            }
        }

        let mut replay = Replay::new(scheduler_dispatch);
        let mut produced = Vec::new();
        let mut index = 0;
        while index < records.len() {
            let line = records[index];
            let parsed = tokens(line);
            assert_eq!(
                parsed[1], "input",
                "every fixture step begins with an input record: {line}"
            );
            index += 1;
            let mut expected = Vec::new();
            while index < records.len() {
                if tokens(records[index])[1] == "input" {
                    break;
                }
                expected.push(records[index].to_owned());
                index += 1;
            }
            let actual = replay.step(&parsed);
            assert_eq!(actual, expected, "production output diverged from {line}");
            produced.push(line.to_owned());
            produced.extend(actual);
        }
        assert_eq!(
            produced,
            records
                .iter()
                .map(|line| (*line).to_owned())
                .collect::<Vec<_>>(),
            "the fixture and the production replay must be the same record stream"
        );
        produced
    }

    #[test]
    fn observer_timeout_and_detach_preserve_late_routing_and_target_delivery() {
        let records = replay(OBSERVER_LATE_DELIVERY);
        assert!(records.iter().any(
            |line| line.contains("observer-detached id=1") && line.contains("routing=retained")
        ));
        assert!(records
            .iter()
            .any(|line| line.contains("applied-state observer=target-one")));
    }

    #[test]
    fn observer_scheduler_and_transport_deadlines_stay_distinct() {
        let records = replay(DEADLINE_CLASSES);
        for source in ["observer", "scheduler", "transport"] {
            assert!(
                records
                    .iter()
                    .any(|line| line.contains(&format!("source={source}"))),
                "the production replay must expose the {source} deadline boundary"
            );
        }
    }

    #[test]
    fn capacity_datagram_stream_poison_and_shutdown_boundaries_stay_distinct() {
        replay(CAPACITY_AND_FAILURES);
    }

    #[test]
    fn ptzoptics_g2_queued_cancel_is_local_but_sent_cancel_is_unsupported() {
        let records = replay(PTZOPTICS_CANCEL);
        assert!(records.iter().any(|line| line
            .contains("cancellation-recorded id=1 disposition=queued-local cancel-frame=none")));
        assert!(records
            .iter()
            .any(|line| line.contains("Error::NotSupported source=profile")));
        assert!(records
            .iter()
            .any(|line| line.ends_with("transmissions kind=cancel count=0")));
    }

    #[test]
    fn blocking_observers_retain_exact_out_of_order_outcomes() {
        let records = replay(BLOCKING_OUT_OF_ORDER);
        let retained = records
            .iter()
            .position(|line| line.contains("outcome-retained id=1 observer=blocking"))
            .expect("the first request's outcome is retained");
        let waited = records
            .iter()
            .position(|line| line.contains("outcome blocking-wait id=1 Applied"))
            .expect("the first request's wait");
        assert!(
            retained < waited,
            "an out-of-order outcome is retained until its own wait reads it"
        );
    }
}
