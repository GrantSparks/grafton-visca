#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use crate::runtime::engine::AdmissionTicket;

use std::time::Duration;

use super::*;

#[cfg(any(feature = "blocking", feature = "test-utils"))]
pub(super) mod waiter_rejection;
use crate::CameraId;

/// The outcome of a scripted physical send; preparation errors use NotSent.
pub(super) fn write_outcome(result: Result<TransmissionMeta, Error>) -> WriteOutcome {
    match result {
        Ok(meta) => WriteOutcome::Sent(meta),
        Err(error) => WriteOutcome::Failed(error),
    }
}

/// Test history collected through the same bounded subscription production uses.
pub(super) struct DiagnosticRecorder {
    subscription: DiagnosticSubscription,
    observed: std::cell::RefCell<Vec<DiagnosticEvent>>,
}

impl DiagnosticRecorder {
    pub(super) fn new(state: &mut OwnerState) -> Self {
        let (_, ends) = boundary::OwnerHandleCore::new(state);
        let capacity = state
            .facts()
            .limits
            .diagnostic_events_per_subscription
            .get();
        Self {
            subscription: state
                .subscribe_diagnostics(capacity, ends.lifecycle)
                .unwrap(),
            observed: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub(super) fn events(&self) -> Vec<DiagnosticEvent> {
        let mut observed = self.observed.borrow_mut();
        observed.extend(std::iter::from_fn(|| self.subscription.try_recv()));
        observed.clone()
    }
}

fn policy_for_target_validation() -> ProtocolPolicy {
    ProtocolPolicy {
        capacity: std::num::NonZeroUsize::new(1).unwrap(),
        inquiry_cooldown: Duration::ZERO,
        raw_inquiry_release_hold: Duration::ZERO,
        ..ProtocolPolicy::test_default()
    }
}

/// Stage one admission with a fresh observer slot and admission reply, as a
/// handle's boundary message would carry them.
fn stage_admission(
    _state: &mut OwnerState,
    request: RuntimeRequest,
    permit: AdmissionPermit,
) -> (
    StagedAdmission,
    TerminalObserver<RuntimeOutcome>,
    flume::Receiver<Result<Admitted, Error>>,
) {
    let (observer, cell) = TerminalObserver::<RuntimeOutcome>::pair();
    let (reply, admission) = flume::bounded(1);
    let input = StagedAdmission {
        request,
        permit,
        observer: cell,
        reply,
    };
    (input, observer, admission)
}

/// Test-local compatibility fixture: one owned admission, with no owner ticket
/// allocator or map. Synthetic effects preserve the checked trace vocabulary.
struct StagedAdmission<T = RuntimeOutcome> {
    request: RuntimeRequest,
    permit: AdmissionPermit,
    observer: Arc<ObserverCell<T>>,
    reply: flume::Sender<Result<Admitted, Error>>,
}

impl<T: TerminalLane> StagedAdmission<T> {
    fn in_turn(self, state: &mut OwnerState, turn: &OwnerInputTurn) -> VecDeque<Effect> {
        let result = state.admit_in_turn(turn, self.request, self.permit, self.observer);
        let effect = match &result {
            Ok(admitted) => Effect::Admitted {
                ticket: AdmissionTicket(0),
                id: admitted.identity.request_id(),
            },
            Err(error) => Effect::AdmissionRejected {
                ticket: AdmissionTicket(0),
                error: error.clone(),
            },
        };
        let _ = self.reply.try_send(result);
        VecDeque::from([effect])
    }

    fn complete(self, state: &mut OwnerState, now: Instant) -> VecDeque<Effect> {
        let turn = state.begin_input_turn(now);
        let mut effects = self.in_turn(state, &turn);
        effects.extend(state.finish_input_turn(turn, EngineTurn::Complete));
        effects
    }
}

#[test]
fn an_individual_target_rejects_broadcast_without_panicking() {
    assert!(crate::IndividualCameraId::try_from(CameraId::BROADCAST).is_err());
}

#[test]
fn control_lane_uses_its_own_named_nonzero_bound() {
    let mut policy = OwnerPolicy::single_target(
        policy_for_target_validation(),
        TargetSlot::CAMERA_1,
        TargetPolicy::test_default(),
    )
    .unwrap();
    policy.limits.control_requests = std::num::NonZeroUsize::new(3).unwrap();
    policy.limits.staged_diagnostics = std::num::NonZeroUsize::new(7).unwrap();
    let state = OwnerState::new(policy.clone(), Instant::now()).unwrap();
    let (handle, _) = boundary::OwnerHandleCore::new(&state);
    assert_eq!(handle.control.capacity(), Some(3));
    let _: std::num::NonZeroUsize = policy.limits.control_requests;
}

/// Issue #631: the owner state used by the reconfiguration tests below.
///
/// The profile baseline is deliberately non-zero on both axes so a
/// re-derivation that quietly kept a previous override would be visible.
fn reconfigurable_owner_state_at(now: Instant) -> OwnerState {
    let mut protocol = policy_for_target_validation();
    protocol.command_spacing = Duration::from_millis(10);
    protocol.inquiry_spacing = Duration::from_millis(20);
    let policy =
        OwnerPolicy::single_target(protocol, TargetSlot::CAMERA_1, TargetPolicy::test_default())
            .expect("single-target owner policy");
    OwnerState::new(policy, now).expect("owner state")
}

fn command_sockets(state: &OwnerState) -> crate::CommandSocketCount {
    state.engine.operational_limits_for_test().2[TargetSlot::CAMERA_1]
        .expect("camera one is registered")
}

/// Retuning preserves the immutable construction baseline while replacing
/// the complete live proof value. Public #631 scenarios verify engine pacing.
#[test]
fn retuning_preserves_the_profile_baseline_and_updates_the_live_proof() {
    let mut state = reconfigurable_owner_state_at(Instant::now());
    let live = state.live_tuning();
    let tuning = crate::OperationalTuning::new()
        .with_command_spacing(Duration::from_millis(200))
        .with_inquiry_spacing(Duration::from_millis(300))
        .with_maximum_command_sockets(crate::types::CommandSocketCount::ONE);
    state
        .retune(
            crate::session_config::ValidatedTuning::for_test(tuning).expect("validated tuning"),
            Instant::now(),
        )
        .expect("accepted retune");
    assert_eq!(live.get().get(), tuning);
    let (command_spacing, inquiry_spacing, _) = state.engine.operational_limits_for_test();
    assert_eq!(command_spacing, Duration::from_millis(200));
    assert_eq!(inquiry_spacing, Duration::from_millis(300));
    assert_eq!(
        state.facts().baseline.command_spacing,
        Duration::from_millis(10)
    );
    assert_eq!(
        state.facts().baseline.inquiry_spacing,
        Duration::from_millis(20)
    );
    assert_eq!(
        command_sockets(&state),
        crate::CommandSocketCount::ONE,
        "socket capacity is read from the live engine"
    );
    state
        .retune(
            crate::session_config::ValidatedTuning::for_test(crate::OperationalTuning::new())
                .expect("default tuning"),
            Instant::now(),
        )
        .expect("clear overrides");
    assert_eq!(live.get().get(), crate::OperationalTuning::new());
    assert_eq!(
        state.facts().baseline.command_spacing,
        Duration::from_millis(10)
    );
    assert_eq!(
        state.facts().baseline.inquiry_spacing,
        Duration::from_millis(20)
    );
}

#[test]
fn retune_checks_the_selected_clock_before_replacing_live_tuning() {
    let mut state = reconfigurable_owner_state_at(Instant::now());
    let live = state.live_tuning();
    let previous = live.get();
    let latest = crate::timeout::saturating_deadline(Instant::now(), Duration::MAX);
    let selected = latest - Duration::from_millis(100);
    let update = crate::OperationalTuning::new().with_command_spacing(Duration::from_secs(1));
    let update =
        crate::session_config::ValidatedTuning::for_test(update).expect("profile-valid tuning");
    assert!(state.retune(update, selected).is_err());
    assert_eq!(live.get(), previous);
}

/// Invalid overrides cannot produce the proof consumed by owner retuning.
#[test]
fn a_rejected_retune_changes_neither_the_live_tuning_nor_the_policy() {
    let mut state = reconfigurable_owner_state_at(Instant::now());
    let live = state.live_tuning();
    let accepted = crate::OperationalTuning::new().with_command_spacing(Duration::from_millis(200));
    state
        .retune(
            crate::session_config::ValidatedTuning::for_test(accepted).expect("validated"),
            Instant::now(),
        )
        .expect("accepted");
    let error = crate::session_config::ValidatedTuning::for_test(
        crate::OperationalTuning::new().with_ack_timeout(Duration::ZERO),
    )
    .expect_err("invalid tuning cannot cross the owner boundary");
    assert!(matches!(error, Error::InvalidRequest { .. }));
    assert_eq!(live.get().get(), accepted);
    assert_eq!(
        state.facts().baseline.command_spacing,
        Duration::from_millis(10)
    );
    assert_eq!(command_sockets(&state), crate::CommandSocketCount::TWO);
}

/// Construction and retuning lower the same validated overrides from profile
/// facts, including clearing them after an earlier wider override.
#[test]
fn construction_and_retuning_have_identical_limits_for_every_profile_and_serial_registry() {
    let now = Instant::now();
    let config = crate::transport::ValidatedTransportConfig::default();
    let identity = crate::transport::TransportIdentity::Custom {
        semantics: crate::transport::SendSemantics::Stream,
        addressing: crate::transport::AddressingMode::Serial,
    };
    let check = |profiles: PerTarget<Option<&crate::ProfileSpec>>| {
        let registered = || profiles.iter().filter_map(|(_, profile)| *profile);
        let baseline = TuningBaseline::from_registry(&profiles);
        for tuning in [
            crate::OperationalTuning::new(),
            crate::OperationalTuning::new()
                .with_maximum_command_sockets(crate::types::CommandSocketCount::ONE),
            crate::OperationalTuning::new()
                .with_command_spacing(baseline.command_spacing + Duration::from_millis(17))
                .with_inquiry_spacing(baseline.inquiry_spacing + Duration::from_millis(23))
                .with_maximum_command_sockets(crate::types::CommandSocketCount::ONE),
        ] {
            let validated =
                crate::session_config::ValidatedTuning::validate(tuning, registered()).unwrap();
            let policy = adapter::owner_policy_for_registry_with_tuning(
                &profiles,
                &config,
                identity,
                validated,
                crate::DEFAULT_ADMISSION_CAPACITY,
                false,
            );
            let constructed = OwnerState::new(policy, now).unwrap();
            let initial = crate::session_config::ValidatedTuning::validate(
                crate::OperationalTuning::new(),
                registered(),
            )
            .unwrap();
            let policy = adapter::owner_policy_for_registry_with_tuning(
                &profiles,
                &config,
                identity,
                initial,
                crate::DEFAULT_ADMISSION_CAPACITY,
                false,
            );
            let mut retuned = OwnerState::new(policy, now).unwrap();
            retuned.retune(validated, now).unwrap();
            assert_eq!(
                constructed.engine.operational_limits_for_test(),
                retuned.engine.operational_limits_for_test()
            );
            retuned.retune(initial, now).unwrap();
            assert_eq!(
                retuned.engine.operational_limits_for_test(),
                baseline.apply(initial)
            );
        }
    };
    for id in crate::profiles::ProfileId::all() {
        let profile = id.spec().unwrap();
        check(PerTarget::from_fn(|target| {
            (target == TargetSlot::CAMERA_1).then_some(&profile)
        }));
    }
    let first = crate::profiles::ProfileId::GenericVisca.spec().unwrap();
    let second = crate::profiles::ProfileId::PtzOpticsG2.spec().unwrap();
    check(PerTarget::from_fn(|target| match target {
        TargetSlot::CAMERA_1 => Some(&first),
        TargetSlot::CAMERA_2 => Some(&second),
        _ => None,
    }));
}

/// The owner's cell is the slot's only strong owner (#777): dropping it
/// unresolved disconnects the observer, and a handle can tell that the owner
/// no longer holds the intent.
#[test]
fn dropping_an_unresolved_cell_disconnects_its_observer() {
    let (observer, cell) = TerminalObserver::<RuntimeOutcome>::pair();
    assert!(observer.cell().is_some());
    drop(cell);
    assert!(observer.cell().is_none());
    assert!(observer.receiver.is_disconnected());
    assert!(observer.try_recv().is_none());

    let (observer, cell) = TerminalObserver::<RuntimeOutcome>::pair();
    assert_eq!(
        cell.resolve(RuntimeOutcome::Command(CommandOutcome::Applied)),
        ObserverResolution::Delivered
    );
    drop(cell);
    assert!(
        matches!(
            observer.try_recv(),
            Some(RuntimeOutcome::Command(CommandOutcome::Applied))
        ),
        "a delivered value outlives the cell"
    );
}

#[test]
fn observer_resolution_distinguishes_receiver_loss_from_duplicate_delivery() {
    let (observer, cell) = TerminalObserver::<RuntimeOutcome>::pair();
    assert_eq!(
        cell.resolve(RuntimeOutcome::Command(CommandOutcome::Applied)),
        ObserverResolution::Delivered
    );
    drop(observer);
    assert_eq!(
        cell.resolve(RuntimeOutcome::Command(CommandOutcome::Applied)),
        ObserverResolution::AlreadyResolved,
        "dropping a receiver after delivery cannot turn a duplicate into loss"
    );

    let (observer, cell) = TerminalObserver::<RuntimeOutcome>::pair();
    drop(observer);
    assert_eq!(
        cell.resolve(RuntimeOutcome::Command(CommandOutcome::Applied)),
        ObserverResolution::ReceiverLost
    );
}

/// D26 (#778): the permit pool's two budgets.
mod control_reserve {
    use super::*;
    use crate::runtime::engine::{CommandReply, ControlPolicy, RetryPolicy, TimeoutPolicy};

    fn request(target: u8, reply_shape: CommandReply) -> RuntimeRequest {
        let context = RequestContext {
            requires_extended_pan_tilt_speed: false,
            motion: None,
            submission_order: 0,
            dispatch_deadline: None,
            target: TargetSlot::from(crate::IndividualCameraId::new(target).unwrap()),
            timeout: TimeoutPolicy {
                ack: crate::runtime::engine::NonZeroDuration::new(Duration::from_millis(10))
                    .unwrap(),
                completion: crate::runtime::engine::NonZeroDuration::new(Duration::from_millis(20))
                    .unwrap(),
                inquiry: crate::runtime::engine::NonZeroDuration::new(Duration::from_millis(20))
                    .unwrap(),
                cancellation: crate::runtime::engine::NonZeroDuration::new(Duration::from_millis(
                    10,
                ))
                .unwrap(),
                ambiguity: crate::runtime::engine::NonZeroDuration::new(Duration::from_millis(10))
                    .unwrap(),
            },
            retry: RetryPolicy::NEVER,
            control: ControlPolicy::default(),
        };
        RuntimeRequest::Command {
            reply_shape,
            context,
            wire: Arc::new(
                crate::runtime::engine::EncodedMessage::new(&[0x81, 0x01, 0x04, 0x00, 0xff])
                    .unwrap(),
            ),
            applied_state: None,
        }
    }

    /// One ordinary slot; cameras 1 and 2 reserve two and one control slots.
    fn pool() -> AdmissionPermitPool {
        let mut reserves = PerTarget::default();
        reserves[TargetSlot::CAMERA_1] = 2;
        reserves[TargetSlot::CAMERA_2] = 1;
        AdmissionPermitPool::new(1, reserves)
    }

    /// A waiter may resubmit inside its wake callback, before the owner returns
    /// from terminal delivery. Both admission budgets must already be free.
    #[test]
    fn terminal_wakeup_can_immediately_reacquire_ordinary_and_reserved_capacity() {
        use std::{
            future::Future,
            task::{Context, Poll, Wake, Waker},
        };

        struct Resubmit {
            pool: AdmissionPermitPool,
            request: RuntimeRequest,
            expected: AdmissionSlot,
            woke: AtomicBool,
        }
        impl Wake for Resubmit {
            fn wake(self: Arc<Self>) {
                self.wake_by_ref();
            }
            fn wake_by_ref(self: &Arc<Self>) {
                let permit = self
                    .pool
                    .try_acquire(&self.request)
                    .expect("terminal capacity is free inside the waiter's wake-up");
                assert_eq!(permit.slot(), self.expected);
                self.woke.store(true, Ordering::Release);
            }
        }

        for class in [CommandReply::AckThenCompletion, CommandReply::Stop] {
            for outcome in [
                CommandOutcome::Applied,
                CommandOutcome::Cancelled,
                CommandOutcome::Failed(Error::RuntimeShutdown),
            ] {
                let now = Instant::now();
                let policy = OwnerPolicy::single_target(
                    policy_for_target_validation(),
                    TargetSlot::CAMERA_1,
                    TargetPolicy {
                        control_reserve: 1,
                        ..TargetPolicy::test_default()
                    },
                )
                .unwrap();
                let mut state = OwnerState::new(policy, now).unwrap();
                let mut reserves = PerTarget::default();
                reserves[TargetSlot::CAMERA_1] = 1;
                let pool = AdmissionPermitPool::new(1, reserves);
                // Exhaust ordinary admission too, so an urgent resubmission
                // cannot hide an unreleased reserve by borrowing that budget.
                let _ordinary =
                    (class == CommandReply::Stop).then(|| pool.try_acquire_ordinary().unwrap());
                let request = request(1, class);
                let permit = pool.try_acquire(&request).unwrap();
                let expected = permit.slot();
                let (observer, cell) = TerminalObserver::<CommandOutcome>::pair();
                let turn = state.begin_input_turn(now);
                let admitted = state
                    .admit_in_turn(&turn, request.clone(), permit, cell)
                    .unwrap();
                let wake = Arc::new(Resubmit {
                    pool,
                    request,
                    expected,
                    woke: AtomicBool::new(false),
                });
                let waker = Waker::from(Arc::clone(&wake));
                let mut waiting = std::pin::pin!(observer.receiver.recv_async());
                assert!(waiting
                    .as_mut()
                    .poll(&mut Context::from_waker(&waker))
                    .is_pending());
                let _applied = state.apply_effect(
                    Effect::Terminal {
                        id: admitted.identity.request_id(),
                        outcome: RuntimeOutcome::Command(outcome),
                    },
                    now,
                );
                assert!(wake.woke.load(Ordering::Acquire));
                assert!(matches!(
                    waiting.as_mut().poll(&mut Context::from_waker(&waker)),
                    Poll::Ready(Ok(_))
                ));
            }
        }
    }

    #[test]
    fn urgent_requests_take_their_reserve_first_then_ordinary_slots() {
        let pool = pool();
        let urgent = request(1, CommandReply::Stop);
        let first = pool.try_acquire(&urgent).unwrap();
        let second = pool.try_acquire(&urgent).unwrap();
        assert_eq!(first.slot(), AdmissionSlot::ControlReserve);
        assert_eq!(second.slot(), AdmissionSlot::ControlReserve);
        let third = pool.try_acquire(&urgent).unwrap();
        assert_eq!(third.slot(), AdmissionSlot::Ordinary, "the reserve is held");
        assert!(matches!(
            pool.try_acquire(&urgent),
            Err(Error::ControlReserveExhausted { target, reserve: 2 })
                if target == TargetSlot::CAMERA_1.individual_camera()
        ));
        assert!(matches!(
            pool.try_acquire(&request(1, CommandReply::AckThenCompletion)),
            Err(Error::RuntimeQueueFull { capacity: 1 })
        ));
        // Camera 1 cannot use camera 2's reserve.
        let other = pool.try_acquire(&request(2, CommandReply::Stop)).unwrap();
        assert_eq!(other.slot(), AdmissionSlot::ControlReserve);
    }

    #[test]
    fn ordinary_requests_never_take_a_reserve_and_slots_return_to_their_budget() {
        let pool = pool();
        let ordinary = request(1, CommandReply::AckThenCompletion);
        let held = pool.try_acquire(&ordinary).unwrap();
        assert_eq!(held.slot(), AdmissionSlot::Ordinary);
        assert!(matches!(
            pool.try_acquire(&ordinary),
            Err(Error::RuntimeQueueFull { capacity: 1 })
        ));

        let urgent = request(1, CommandReply::Stop);
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
fn pan_tilt_limit_cache_preserves_typed_corner_updates_and_clears() {
    use crate::command::PanTiltLimitCorner;
    use crate::{KnownState, PanTiltLimitUpdate, StateEntry};
    let mut cache = TargetStateCache::default();
    let set = KnownState::PanTiltLimit(PanTiltLimitUpdate::set(
        PanTiltLimitCorner::DownLeft,
        crate::types::PanTiltPosition::from_degrees(12.0, -4.0).unwrap(),
    ));
    cache.apply(AppliedStateProjection::set(set));
    assert_eq!(cache.entry(StateKey::PanTiltLimit), StateEntry::Set(set));
    let clear = KnownState::PanTiltLimit(PanTiltLimitUpdate::clear(PanTiltLimitCorner::UpRight));
    cache.apply(AppliedStateProjection::set(clear));
    assert_eq!(cache.entry(StateKey::PanTiltLimit), StateEntry::Set(clear));
    cache.apply(AppliedStateProjection::invalidate(StateKey::PanTiltLimit));
    assert_eq!(cache.entry(StateKey::PanTiltLimit), StateEntry::Unknown);
}

#[test]
fn every_state_key_has_an_inline_slot_and_a_typed_accessor() {
    use crate::{KnownState, StateEntry};
    let registry = Arc::new(PerTarget::<Mutex<TargetStateCache>>::default());
    let cache = crate::StateCache::from_registry(Arc::clone(&registry), TargetSlot::CAMERA_1);
    let other = crate::StateCache::from_registry(Arc::clone(&registry), TargetSlot::CAMERA_2);
    let mut covered = Vec::new();
    macro_rules! check {
        ($variant:ident, $noun:ident, $getter:ident, $value:expr) => {{
            let value = $value;
            let known = KnownState::$variant(value);
            let key = known.key();
            assert_eq!(cache.value(key), StateEntry::Unknown);
            assert_eq!(cache.$noun().$getter(), None);
            registry[TargetSlot::CAMERA_1]
                .lock()
                .unwrap()
                .apply(AppliedStateProjection::set(known));
            assert_eq!(cache.value(key), StateEntry::Set(known));
            assert_eq!(cache.$noun().$getter(), Some(value));
            assert_eq!(other.value(key), StateEntry::Unknown);
            registry[TargetSlot::CAMERA_1]
                .lock()
                .unwrap()
                .apply(AppliedStateProjection::invalidate(key));
            assert_eq!(cache.value(key), StateEntry::Unknown);
            assert_eq!(cache.$noun().$getter(), None);
            registry[TargetSlot::CAMERA_1]
                .lock()
                .unwrap()
                .apply(AppliedStateProjection::set(known));
            covered.push(known);
        }};
    }
    check!(
        PanTiltLimit,
        pan_tilt,
        limit,
        crate::PanTiltLimitUpdate::set(
            crate::command::PanTiltLimitCorner::DownLeft,
            crate::types::PanTiltPosition::from_degrees(-123.0, 456.0).unwrap()
        )
    );
    check!(
        PresetsRecallSpeed,
        presets,
        recall_speed,
        crate::types::PresetSpeed::new(12).unwrap()
    );
    check!(FocusLock, focus, lock, true);
    check!(ExposureSpotlight, exposure, spotlight, true);
    check!(ExposureAutoSlowShutter, exposure, auto_slow_shutter, false);
    check!(
        NdFilterMode,
        nd_filter,
        mode,
        crate::command::NdFilterMode::Variable
    );
    check!(NdFilterAuto, nd_filter, auto, false);
    check!(ImageFreeze, image, freeze, true);
    check!(ZoomDigitalZoom, zoom, digital_zoom, false);
    check!(StreamingMulticast, streaming, multicast, true);
    check!(
        StreamingNdiQuality,
        streaming,
        ndi_quality,
        crate::types::NdiQuality::Medium
    );
    check!(
        TallyBrightness,
        tally,
        brightness,
        crate::types::TallyBrightness::Low
    );
    check!(
        PanTiltSpeedMode,
        pan_tilt,
        speed_mode,
        crate::command::PanTiltSpeedMode::Fine50
    );
    check!(TallyMode, tally, mode, crate::types::TallyMode::Flash);
    check!(
        MotionSyncSpeed,
        motion_sync,
        speed,
        crate::types::MotionSyncSpeed::NORMAL
    );
    check!(
        ExposureWideDynamicRange,
        exposure,
        wide_dynamic_range,
        crate::types::WideDynamicRangeLevel::new(4).unwrap()
    );
    check!(MotionSyncEnabled, motion_sync, enabled, true);
    assert_eq!(
        covered.iter().map(|value| value.key()).collect::<Vec<_>>(),
        StateKey::ALL
    );
    for known in covered {
        assert_eq!(cache.value(known.key()), StateEntry::Set(known));
    }
    for corner in [
        crate::command::PanTiltLimitCorner::DownLeft,
        crate::command::PanTiltLimitCorner::UpRight,
    ] {
        let update = crate::PanTiltLimitUpdate::clear(corner);
        registry[TargetSlot::CAMERA_1]
            .lock()
            .unwrap()
            .apply(AppliedStateProjection::set(KnownState::PanTiltLimit(
                update,
            )));
        assert_eq!(cache.pan_tilt().limit(), Some(update));
        assert!(cache.pan_tilt().limit().unwrap().is_cleared());
    }
    assert_eq!(
        size_of::<TargetStateCache>(),
        StateKey::ALL.len() * size_of::<StateEntry>()
    );
}

/// Issue #571: the counters a field debugging session reaches for first.
///
/// Every scenario here is scripted at the owner boundary rather than asserted
/// against a retry constant, so the counts stay meaningful as retry policy
/// changes underneath them.
mod metrics {
    mod fr7_speed;
    use super::DiagnosticRecorder;
    use std::{
        collections::VecDeque,
        sync::Arc,
        time::{Duration, Instant},
    };

    use super::super::*;
    use crate::{
        runtime::engine::{
            CancellationPolicy, CommandReply, ControlPolicy, DeadlineKind, DecodedFrame,
            DecodedResponse, EncodedMessage, EnvelopeKind, FrameSequence, RequestContext,
            RetryPolicy, TimeoutPolicy, TransmissionMeta,
        },
        Error, ViscaSocket,
    };

    const ACK: Duration = Duration::from_millis(10);
    const COMPLETION: Duration = Duration::from_millis(20);
    const INQUIRY: Duration = Duration::from_millis(15);

    fn retrying() -> RetryPolicy {
        RetryPolicy {
            max_retries: 3,
            initial_backoff: Duration::from_millis(5),
            maximum_backoff: Duration::from_millis(50),
            total_budget: Some(
                crate::runtime::engine::NonZeroDuration::new(Duration::from_secs(10)).unwrap(),
            ),
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
            capacity: std::num::NonZeroUsize::new(4).unwrap(),
            wire: crate::runtime::engine::WireProtocol::new(
                envelope,
                crate::transport::AddressingMode::Ip,
            ),
            inquiry_cooldown: Duration::ZERO,
            raw_inquiry_release_hold: Duration::from_millis(10),
            ..ProtocolPolicy::test_default()
        };
        OwnerPolicy::single_target(protocol, TargetSlot::CAMERA_1, TargetPolicy::test_default())
            .unwrap()
    }

    fn request_context(retry: RetryPolicy) -> RequestContext {
        RequestContext {
            requires_extended_pan_tilt_speed: false,
            motion: None,
            submission_order: 0,
            dispatch_deadline: None,
            target: TargetSlot::CAMERA_1,
            timeout: TimeoutPolicy {
                ack: crate::runtime::engine::NonZeroDuration::new(ACK).unwrap(),
                completion: crate::runtime::engine::NonZeroDuration::new(COMPLETION).unwrap(),
                inquiry: crate::runtime::engine::NonZeroDuration::new(INQUIRY).unwrap(),
                cancellation: crate::runtime::engine::NonZeroDuration::new(Duration::from_millis(
                    10,
                ))
                .unwrap(),
                ambiguity: crate::runtime::engine::NonZeroDuration::new(Duration::from_millis(10))
                    .unwrap(),
            },
            retry,
            control: ControlPolicy::default(),
        }
    }

    fn wire() -> Arc<EncodedMessage> {
        Arc::new(EncodedMessage::new(&[0x81, 0x01, 0x04, 0x00, 0xff]).unwrap())
    }

    fn command(retry: RetryPolicy) -> RuntimeRequest {
        RuntimeRequest::Command {
            reply_shape: CommandReply::AckThenCompletion,
            wire: wire(),
            context: request_context(retry),
            applied_state: None,
        }
    }

    fn inquiry(retry: RetryPolicy) -> RuntimeRequest {
        let mut context = request_context(retry);
        context.timeout.inquiry = crate::runtime::engine::NonZeroDuration::new(INQUIRY).unwrap();
        RuntimeRequest::Inquiry {
            payload_shape: crate::PayloadShape::Unknown,
            wire: wire(),
            context,
        }
    }

    fn frame(sequence: Option<u32>, response: DecodedResponse) -> Input {
        OwnerState::frame_input(DecodedFrame {
            target: TargetSlot::CAMERA_1,
            sequence: sequence.map(FrameSequence::from_reply_header),
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
            if let AppliedEffect::Transmit(staged) = state.apply_effect(effect, now) {
                let produced = state.finish_write(
                    &staged,
                    WriteOutcome::Sent(sequence.map_or(TransmissionMeta::Known, |sequence| {
                        TransmissionMeta::Allocated { sequence }
                    })),
                    now,
                );
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
    ) -> TerminalObserver<RuntimeOutcome> {
        let permit = state
            .permits()
            .try_acquire_ordinary()
            .expect("admission permit");
        let (input, observer, _admission) = super::stage_admission(state, request, permit);
        let effects = input.complete(state, now);
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

    fn saw_deadline(
        diagnostics: &DiagnosticRecorder,
        expected: DeadlineKind,
        retrying: bool,
    ) -> bool {
        diagnostics.events().iter().any(|event| {
            matches!(
                event,
                DiagnosticEvent::DeadlineExpired { deadline, will_retry, .. }
                    if *deadline == diagnostics::deadline(expected) && *will_retry == retrying
            )
        })
    }

    /// The complete public stream for one plain command, including every phase
    /// and transport event. This sequence also passes against the pre-#913
    /// producer/consumer projection; no event is filtered out of this assertion.
    #[test]
    fn plain_focus_command_emits_the_complete_public_event_sequence() {
        use crate::{
            observability::{
                DiagnosticEvent as Event, DiagnosticLane, DiagnosticOutcome, DiagnosticPhase,
                DiagnosticResponse,
            },
            prepared::{test_support::prepare_command, ClassSelection},
            profiles::GenericVisca,
            request::builtin::FocusModeCommand,
            IndividualCameraId, OperationalTuning, ProfileSpec, TargetRegistration,
        };

        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw), start).unwrap();
        let (_, ends) = boundary::OwnerHandleCore::new(&state);
        let subscription = crate::observability::DiagnosticSubscription::from_owner(
            state.subscribe_diagnostics(128, ends.lifecycle).unwrap(),
        );
        let target = IndividualCameraId::CAMERA_1;
        let profile = ProfileSpec::from_compile_time::<GenericVisca>().unwrap();
        let prepared = prepare_command(
            &FocusModeCommand::Manual,
            &TargetRegistration::new(target, profile),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .unwrap();
        let request = prepared.admit_with(|request, _| request.into_inner());
        let observer = sent(&mut state, request, None, start);
        apply(
            &mut state,
            frame(
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start + Duration::from_millis(1),
        );
        apply(
            &mut state,
            frame(
                None,
                DecodedResponse::Completion {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start + Duration::from_millis(2),
        );
        assert!(matches!(
            observer.try_recv(),
            Some(RuntimeOutcome::Command(CommandOutcome::Applied))
        ));

        let events: Vec<_> = std::iter::from_fn(|| subscription.try_recv()).collect();
        assert_eq!(events.len(), 8, "the full stream has exactly eight events");
        let Event::Admitted { id, .. } = events[0] else {
            panic!("the first event must be admission");
        };
        assert_eq!(
            events,
            [
                Event::Admitted {
                    id,
                    target,
                    lane: DiagnosticLane::Command
                },
                Event::Transition {
                    id,
                    target,
                    from: DiagnosticPhase::Ready,
                    to: DiagnosticPhase::Sending
                },
                Event::WriteFinished {
                    id,
                    target,
                    cancellation: false,
                    success: true
                },
                Event::Transition {
                    id,
                    target,
                    from: DiagnosticPhase::Sending,
                    to: DiagnosticPhase::AwaitingAck
                },
                Event::FrameReceived {
                    target,
                    response: DiagnosticResponse::Ack
                },
                Event::Transition {
                    id,
                    target,
                    from: DiagnosticPhase::AwaitingAck,
                    to: DiagnosticPhase::Executing
                },
                Event::FrameReceived {
                    target,
                    response: DiagnosticResponse::Completion
                },
                Event::Terminal {
                    id,
                    target,
                    outcome: DiagnosticOutcome::Applied
                },
            ]
        );
    }

    #[test]
    fn expired_ack_deadline_counts_a_timeout_and_the_retry_it_scheduled_for_sony() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Sony), Instant::now()).unwrap();
        let diagnostics = DiagnosticRecorder::new(&mut state);
        let _observer = sent(&mut state, command(retrying()), Some(1), start);
        assert_eq!(state.metrics().ack_timeouts, 0);
        advance(&mut state, start + ACK);
        let metrics = state.metrics();
        assert_eq!(metrics.ack_timeouts, 1);
        assert_eq!(metrics.retries_scheduled, 1);
        assert_eq!(metrics.completion_timeouts, 0);
        assert_eq!(metrics.inquiry_timeouts, 0);
        assert!(saw_deadline(&diagnostics, DeadlineKind::Ack, true));
    }

    #[test]
    fn expired_ack_deadline_without_a_retry_reports_the_decision_directly() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw), Instant::now()).unwrap();
        let diagnostics = DiagnosticRecorder::new(&mut state);
        let _observer = sent(&mut state, command(RetryPolicy::NEVER), None, start);
        advance(&mut state, start + ACK);
        let metrics = state.metrics();
        assert_eq!(metrics.ack_timeouts, 1);
        assert_eq!(metrics.retries_scheduled, 0);
        assert!(saw_deadline(&diagnostics, DeadlineKind::Ack, false));
    }

    #[test]
    fn expired_completion_deadline_counts_separately_from_the_ack_deadline_for_sony() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Sony), Instant::now()).unwrap();
        let diagnostics = DiagnosticRecorder::new(&mut state);
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
        assert!(saw_deadline(&diagnostics, DeadlineKind::Completion, false));
    }

    #[test]
    fn expired_inquiry_deadline_counts_as_an_inquiry_timeout() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw), Instant::now()).unwrap();
        let diagnostics = DiagnosticRecorder::new(&mut state);
        let _observer = sent(&mut state, inquiry(retrying()), None, start);
        advance(&mut state, start + INQUIRY);
        let metrics = state.metrics();
        assert_eq!(metrics.inquiry_timeouts, 1);
        assert_eq!(metrics.ack_timeouts, 0);
        assert_eq!(metrics.completion_timeouts, 0);
        assert!(saw_deadline(&diagnostics, DeadlineKind::InquiryReply, true));
    }

    #[test]
    fn camera_backpressure_codes_count_as_busy_errors() {
        for code in [0x03_u8, 0x05, 0x41] {
            let start = Instant::now();
            let mut state =
                OwnerState::new(owner_policy(EnvelopeKind::Raw), Instant::now()).unwrap();
            let _observer = sent(&mut state, command(retrying()), None, start);
            apply(
                &mut state,
                frame(
                    None,
                    DecodedResponse::Error {
                        socket: None,
                        code: code.into(),
                    },
                ),
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
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw), Instant::now()).unwrap();
        let _observer = sent(&mut state, command(retrying()), None, start);
        apply(
            &mut state,
            frame(
                None,
                DecodedResponse::Error {
                    socket: None,
                    code: 0x02.into(),
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
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw), Instant::now()).unwrap();
        let _observer = sent(&mut state, command(retrying()), None, start);
        apply(
            &mut state,
            frame(
                None,
                DecodedResponse::Error {
                    socket: Some(ViscaSocket::S1),
                    code: 0x04.into(),
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
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Sony), Instant::now()).unwrap();
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
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Sony), Instant::now()).unwrap();
        let diagnostics = DiagnosticRecorder::new(&mut state);

        apply(
            &mut state,
            frame(None, DecodedResponse::SonyControl { code: 0x0F01 }),
            start,
        );

        assert_eq!(state.metrics().received_frames, 1);
        assert_eq!(state.metrics().protocol_errors, 0);
        assert_eq!(state.metrics().terminal, 0);
        assert!(diagnostics.events().iter().any(|event| matches!(
            event,
            DiagnosticEvent::FrameReceived {
                target: crate::IndividualCameraId::CAMERA_1,
                response: crate::observability::DiagnosticResponse::SonyControl { code: 0x0F01 },
            }
        )));
        assert!(diagnostics.events().iter().any(|event| matches!(
            event,
            DiagnosticEvent::Ignored {
                reason: crate::observability::DiagnosticIgnoreReason::UnmatchedFrame
            }
        )));
    }

    /// A transient receive fault (#565) retries a sequenced Sony command while
    /// it awaits its ACK. Raw commands poison the session because they have no
    /// sequence key that can make replay safe.
    #[test]
    fn a_transient_receive_fault_retry_counts_as_a_retry_for_sony() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Sony), Instant::now()).unwrap();
        let _observer = sent(&mut state, command(retrying()), Some(1), start);
        apply(
            &mut state,
            Input::ReceiveFault {
                error: Error::Io {
                    source: Arc::new(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
                },
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
        assert_eq!(size_of::<OwnerMetrics>(), 24 * size_of::<u64>());
    }
    fn admit_observation(
        state: &mut OwnerState,
        mut request: RuntimeRequest,
        now: Instant,
    ) -> (OperationObservation, Arc<ObserverCell<CommandOutcome>>) {
        let motion = request.context().motion.unwrap_or(MotionEffect {
            axes: crate::AffectedAxes::ZOOM,
        });
        request.context_mut().motion = Some(motion);
        let permit = state.permits.try_acquire(&request).unwrap();
        let (observer, cell) = TerminalObserver::<CommandOutcome>::pair();
        let turn = state.begin_input_turn(now);
        let admitted =
            state.admit_operation_in_turn(&turn, request, motion, permit, Arc::clone(&cell));
        let _ = state.finish_input_turn(turn, EngineTurn::InputOnly);
        let admitted = admitted.unwrap();
        let core = ReceiptCore::admitted(admitted.admitted, observer, ACK).unwrap();
        (OperationObservation::new(core, admitted.motion, ACK), cell)
    }

    fn observation_fixture() -> (OperationObservation, Arc<ObserverCell<CommandOutcome>>) {
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw), Instant::now()).unwrap();
        admit_observation(&mut state, command(retrying()), Instant::now())
    }

    #[test]
    fn halt_supersession_uses_its_delivery_time_after_an_earlier_turn() {
        use crate::AffectedAxes;
        let earlier = Instant::now();
        let deadline = earlier + Duration::from_millis(1);
        let delivered = earlier + Duration::from_millis(2);
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw), earlier).unwrap();
        let mut request = command(retrying());
        request.context_mut().motion = Some(MotionEffect {
            axes: AffectedAxes::PAN_TILT,
        });
        request.context_mut().submission_order = 1;
        let (mut observation, _) = admit_observation(&mut state, request, earlier);
        let halt_deadline = delivered + ACK;
        let boundary::AdmissionClaim::Claimed { remaining } =
            boundary::AdmissionValidity::until(halt_deadline).claim_for_admission(delivered)
        else {
            panic!("the halt has a positive admission budget");
        };
        let prepared = crate::prepared::PreparedHalt {
            target: TargetSlot::CAMERA_1,
            axes: Some(AffectedAxes::PAN_TILT),
            requests: [None, None, None],
            budget: ACK,
        };
        let (turn, effects) = state.begin_halt(&prepared, 2, delivered);
        for effect in effects {
            assert!(matches!(
                state.apply_effect(effect, delivered),
                AppliedEffect::None
            ));
        }
        let (receipt, effects) = state.accept_halt(turn, prepared, halt_deadline, 2, remaining);
        for effect in effects {
            assert!(matches!(
                state.apply_effect(effect, delivered),
                AppliedEffect::None
            ));
        }
        assert!(receipt.slots.iter().all(Option::is_none));
        observation.poll();
        let terminal = observation
            .terminal
            .clone()
            .expect("halt superseded queued motion");
        assert_eq!(terminal.at, delivered);
        assert!(matches!(
            terminal.value,
            CommandOutcome::Failed(Error::MotionSuperseded { .. })
        ));
        assert!(matches!(
            observation.core.conclude(terminal, deadline),
            Err(Error::ObservationTimeout { .. })
        ));
    }

    /// #1021: the owner completes a cancel outside its input turn. Its
    /// recursive drain must write the matured retry only after that receipt.
    #[test]
    fn zero_spacing_cancellation_drains_before_a_matured_retry() {
        let start = Instant::now();
        let mut policy = owner_policy(EnvelopeKind::Raw);
        policy.protocol.transport = TransportKind::Stream;
        let mut state = OwnerState::new(policy, start).unwrap();
        let first_observer = sent(&mut state, command(RetryPolicy::NEVER), None, start);
        let executing = *state.active.keys().next().unwrap();
        apply(
            &mut state,
            frame(
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start,
        );
        let retry_observer = sent(&mut state, command(retrying()), None, start);
        let retry = *state.active.keys().find(|id| **id != executing).unwrap();
        let rejected = state.input(
            frame(
                None,
                DecodedResponse::Error {
                    socket: None,
                    code: 0x03.into(),
                },
            ),
            start,
        );
        let ready_at = rejected
            .iter()
            .find_map(|effect| match effect {
                Effect::RetryScheduled { id, ready_at, .. } if *id == retry => Some(*ready_at),
                _ => None,
            })
            .expect("buffer-full retry");
        drain(&mut state, rejected, None, start);
        let (_observation, observer) = CancellationObserver::pair();
        let (mut effects, answer) = state.begin_cancellation(
            CancellationRequest {
                id: state.engine.command_id(executing),
                observer,
            },
            ready_at,
        );
        answer.unwrap();
        assert_eq!(
            effects
                .iter()
                .filter(|effect| matches!(effect, Effect::Transmit { .. }))
                .count(),
            1
        );
        let mut writes = Vec::new();
        while let Some(effect) = effects.pop_front() {
            if let AppliedEffect::Transmit(staged) = state.apply_effect(effect, ready_at) {
                writes.push(staged.request);
                let produced = state.finish_write(
                    &staged,
                    WriteOutcome::Sent(TransmissionMeta::Known),
                    ready_at,
                );
                prepend_effects(&mut effects, produced);
            }
        }
        assert_eq!(
            writes,
            [executing, retry],
            "ledger answers follow the actual write order"
        );
        apply(
            &mut state,
            frame(
                None,
                DecodedResponse::Error {
                    socket: Some(ViscaSocket::S1),
                    code: 0x04.into(),
                },
            ),
            ready_at,
        );
        apply(
            &mut state,
            frame(
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S2),
                },
            ),
            ready_at,
        );
        apply(
            &mut state,
            frame(
                None,
                DecodedResponse::Completion {
                    socket: Some(ViscaSocket::S2),
                },
            ),
            ready_at,
        );
        assert!(matches!(
            first_observer.try_recv(),
            Some(RuntimeOutcome::Command(CommandOutcome::Cancelled))
        ));
        assert!(matches!(
            retry_observer.try_recv(),
            Some(RuntimeOutcome::Command(CommandOutcome::Applied))
        ));
        assert!(matches!(state.state(), SessionState::Running));
    }

    /// #986: the owner's intent exists only after the engine accepts it,
    /// before the returned diagnostic and lifecycle effects are driven.
    #[test]
    fn cancellation_intent_is_installed_only_on_synchronous_acceptance() {
        for cancellation in [
            CancellationPolicy::Supported,
            CancellationPolicy::Unsupported,
        ] {
            let now = Instant::now();
            let mut policy = owner_policy(EnvelopeKind::Raw);
            policy.targets[TargetSlot::CAMERA_1]
                .as_mut()
                .unwrap()
                .cancellation = cancellation;
            let mut state = OwnerState::new(policy, now).unwrap();
            let request = command(RetryPolicy::NEVER);
            let _terminal = sent(&mut state, request, None, now);
            let id = *state.active.keys().next().unwrap();
            let (_observation, observer) = CancellationObserver::pair();
            let retained = Arc::clone(&observer);
            let (effects, answer) = state.begin_cancellation(
                CancellationRequest {
                    id: state.engine.command_id(id),
                    observer,
                },
                now + Duration::from_millis(1),
            );
            match cancellation {
                CancellationPolicy::Supported => {
                    answer.unwrap();
                    assert!(Arc::ptr_eq(
                        state.active[&id].cancellation.as_ref().unwrap(),
                        &retained,
                    ));
                    assert!(effects.iter().any(|effect| matches!(effect, Effect::CancellationRecorded { id: recorded } if *recorded == id)));
                }
                CancellationPolicy::Unsupported => {
                    assert!(matches!(answer, Err(Error::CancellationNotSupported)));
                    assert!(state.active[&id].cancellation.is_none());
                }
            }
        }
    }

    #[test]
    fn cancellation_failure_uses_the_explicit_delivery_instant() {
        let earlier = Instant::now();
        let deadline = earlier + Duration::from_millis(1);
        let delivered = earlier + Duration::from_millis(2);
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw), earlier).unwrap();
        let (mut observation, _) = admit_observation(&mut state, command(retrying()), earlier);
        let cancellation = observation.cancellation_request();
        state
            .active
            .get_mut(&observation.id())
            .unwrap()
            .cancellation = Some(cancellation.observer);
        let _applied = state.apply_effect(
            Effect::CancellationObservation {
                id: observation.id(),
                observation: CancellationObservation::AttemptFailed(Error::CancellationUnconfirmed),
            },
            delivered,
        );
        assert!(observation.cancellation(deadline).is_none());
        assert_eq!(
            observation.cancellation_failure.as_ref().unwrap().at,
            delivered
        );
        assert!(matches!(
            observation.cancellation(delivered),
            Some(Err(Error::CancellationUnconfirmed))
        ));
    }

    #[test]
    fn cancellation_conclusion_waits_for_the_terminal_error() {
        use crate::{Certainty, FailureStage, Recovery};

        let now = Instant::now();
        for (diagnostic, error) in [
            (
                Error::CommandNotExecutable.kind(),
                Error::command_failed_after_ack(Error::CommandNotExecutable),
            ),
            (
                ErrorKind::Timeout,
                Error::timeout(FailureStage::Terminal, Certainty::Unconfirmed),
            ),
        ] {
            let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw), now).unwrap();
            let (mut observation, _) = admit_observation(&mut state, command(retrying()), now);
            let cancellation = observation.cancellation_request();
            let id = observation.id();
            state.active.get_mut(&id).unwrap().cancellation = Some(cancellation.observer);
            let decision = [
                Effect::CancellationObservation {
                    id,
                    observation: CancellationObservation::Concluded(
                        CancellationConclusion::Failed(diagnostic),
                    ),
                },
                Effect::Terminal {
                    id,
                    outcome: RuntimeOutcome::Command(CommandOutcome::Failed(error)),
                },
            ];
            let [conclusion, terminal] = decision;
            let _ = state.apply_effect(conclusion, now);
            assert!(observation.cancellation(now).is_none());
            let _ = state.apply_effect(terminal, now);
            let error = observation.cancellation(now).unwrap().unwrap_err();
            match error {
                Error::CommandFailedAfterAck { ref source } => {
                    assert_eq!(diagnostic, Error::CommandNotExecutable.kind());
                    assert!(matches!(**source, Error::CommandNotExecutable));
                }
                Error::Timeout { .. } => assert_eq!(diagnostic, ErrorKind::Timeout),
                other => panic!("expected the operation's terminal error, got {other:?}"),
            }
            assert_eq!(error.recovery(), Recovery::Reconcile);
            let context = error.failure_context().unwrap();
            assert_eq!(context.stage, FailureStage::Terminal);
            assert_eq!(context.certainty, Certainty::Unconfirmed);
        }
    }

    #[test]
    fn operation_application_and_cancellation_share_each_terminal_verdict() {
        for outcome in [
            CommandOutcome::Written,
            CommandOutcome::Applied,
            CommandOutcome::Cancelled,
            CommandOutcome::Failed(Error::SyntaxError),
        ] {
            let (mut observation, cell) = observation_fixture();
            let at = Instant::now();
            assert_eq!(
                cell.resolve_at(outcome.clone(), at),
                ObserverResolution::Delivered
            );
            match (
                outcome,
                observation.applied(at),
                observation.cancellation(at),
            ) {
                (
                    CommandOutcome::Written | CommandOutcome::Applied,
                    Some(Ok(())),
                    Some(Ok(CancellationOutcome::Completed)),
                )
                | (
                    CommandOutcome::Cancelled,
                    Some(Err(Error::CommandCancelled)),
                    Some(Ok(CancellationOutcome::Cancelled)),
                )
                | (
                    CommandOutcome::Failed(Error::SyntaxError),
                    Some(Err(Error::SyntaxError)),
                    Some(Err(Error::SyntaxError)),
                ) => {}
                other => panic!("application and cancellation disagreed on {other:?}"),
            }
        }
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn admission_supersedes_even_if_cancelled_before_write_but_rejection_does_not() {
        use crate::AffectedAxes;
        let now = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw), Instant::now()).unwrap();
        let mut request = command(retrying());
        request.context_mut().motion = Some(MotionEffect {
            axes: AffectedAxes::PAN_TILT,
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
            handle.enqueue_admission::<CommandOutcome>(request.clone(), None),
            Err(Error::RuntimeQueueFull { .. })
        ));
        assert!(original.check_settlement().is_ok());
        drop(held);
        request.context_mut().submission_order = 2;
        let permit = state.permits.try_acquire(&request).unwrap();
        let (completion, observer) = TerminalObserver::<CommandOutcome>::pair();
        let motion = request.context().motion.unwrap();
        let admission_turn = state.begin_input_turn(now);
        let admitted = state
            .admit_operation_in_turn(&admission_turn, request, motion, permit, observer)
            .unwrap();
        assert!(
            matches!(
                original.check_settlement(),
                Err(Error::SettlementSuperseded { .. })
            ),
            "engine admission invalidates attribution before the owner runs due work or dispatches"
        );
        let _ = state.finish_input_turn(admission_turn, EngineTurn::InputOnly);
        let later = OperationObservation::new(
            ReceiptCore::admitted(admitted.admitted, completion, ACK).unwrap(),
            admitted.motion,
            ACK,
        );
        let turn = state.begin_input_turn(now);
        for effect in state.input_in_turn(
            &turn,
            Input::Cancel {
                id: later.command_id(),
            },
        ) {
            let _ = state.apply_effect(effect, now);
        }
        let _ = state.finish_input_turn(turn, EngineTurn::InputOnly);
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
            Some(CommandOutcome::Cancelled)
        ));
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn halt_owner_acceptance_orders_concurrent_ingress_and_settlement_generation() {
        use crate::AffectedAxes;
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw), Instant::now()).unwrap();
        let (handle, ends) = boundary::OwnerHandleCore::new(&state);
        let stamp = state
            .motion
            .establish(TargetSlot::CAMERA_1, AffectedAxes::PAN_TILT, 0);
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let producer = handle.clone();
        let ready = Arc::clone(&barrier);
        let request = command(retrying());
        let joined = std::thread::spawn(move || {
            ready.wait();
            producer
                .enqueue_admission::<CommandOutcome>(request, None)
                .unwrap()
        });
        barrier.wait();
        let cutoff = ends
            .lifecycle
            .fence_order(
                &state.motion,
                TargetSlot::CAMERA_1,
                Some(AffectedAxes::PAN_TILT),
            )
            .unwrap();
        let _held = joined.join().unwrap();
        let concurrent = ends.receivers.admissions.recv().unwrap();
        let order = concurrent.request().context().submission_order;
        assert_ne!(
            order, cutoff,
            "concurrent enqueue has exactly one side of acceptance"
        );
        assert!(
            stamp.check(crate::OperationId::from_raw(1)).is_err(),
            "generation publishes at acceptance"
        );
        let _later = handle
            .enqueue_admission::<CommandOutcome>(command(retrying()), None)
            .unwrap();
        assert!(
            ends.receivers
                .admissions
                .recv()
                .unwrap()
                .request()
                .context()
                .submission_order
                > cutoff
        );
        // The engine uses the actual acceptance cutoff, whichever side the
        // concurrent enqueue took; an earlier halt enqueue is not the fence.
        let mut motion = command(retrying());
        motion.context_mut().motion = Some(MotionEffect {
            axes: AffectedAxes::PAN_TILT,
        });
        motion.context_mut().submission_order = order;
        let _ = state.engine.halt_in_turn(
            &state.engine.begin_input_turn(Instant::now()),
            TargetSlot::CAMERA_1,
            AffectedAxes::PAN_TILT,
            cutoff,
        );
        let effects = state.engine.handle_turn(
            Input::Admit {
                ticket: crate::runtime::engine::AdmissionTicket(99),
                request: motion,
                slot: AdmissionSlot::Ordinary,
            },
            Instant::now(),
            EngineTurn::InputOnly,
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
            cell.resolve_at(CommandOutcome::Applied, delivered);
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
        cancel
            .observer
            .resolve_at(Error::CancellationNotSupported, deadline);
        terminal.resolve_at(CommandOutcome::Applied, deadline + Duration::from_nanos(1));
        assert!(matches!(
            observation.cancellation(deadline),
            Some(Err(Error::CancellationNotSupported))
        ));
        assert!(matches!(
            observation.cancellation(deadline + Duration::from_nanos(1)),
            Some(Ok(CancellationOutcome::Completed))
        ));
        let (mut observation, _) = observation_fixture();
        observation.cancellation_request().observer.resolve_at(
            Error::CancellationNotSupported,
            deadline + Duration::from_nanos(1),
        );
        assert!(observation.cancellation(deadline).is_none());
        assert!(matches!(
            observation.cancellation(deadline + Duration::from_nanos(1)),
            Some(Err(Error::CancellationNotSupported))
        ));
    }

    /// A cancellation's failure slot can wake its caller before the engine's
    /// matching terminal effect is delivered. Both slots must expose the same
    /// canonical verdict, independently of that delivery order (#857).
    #[test]
    fn canonical_cancellation_failure_survives_receipt_delivery_order() {
        fn assert_canonical(verdict: Option<Result<CancellationOutcome, Error>>) {
            let Some(Err(error)) = verdict else {
                panic!("the cancellation must have a recorded failure");
            };
            assert!(
                matches!(error, Error::CancellationUnconfirmed),
                "expected the canonical ambiguity verdict, got {error:?}"
            );
            assert!(!matches!(
                error.recovery(),
                crate::Recovery::Resubmit { .. }
            ));
            assert!(!matches!(
                error.recovery(),
                crate::Recovery::Resubmit { .. }
            ));
            assert_eq!(
                error.failure_context(),
                Some(crate::FailureContext::new(
                    crate::FailureStage::CancellationAttempt,
                    crate::Certainty::Unconfirmed
                ))
            );
        }

        let delivered = Instant::now();
        let deadline = delivered + Duration::from_secs(1);
        let (mut observation, terminal) = observation_fixture();
        let cancellation = observation.cancellation_request();
        cancellation.observer.resolve_at(
            Error::cancellation_observation_failed(Error::CancellationUnconfirmed),
            delivered,
        );

        // Force the formerly failing schedule: the cancellation answer is
        // observable and cached while the terminal slot is still unresolved.
        assert!(observation.applied(deadline).is_none());
        assert_canonical(observation.cancellation(deadline));
        assert_canonical(observation.cancellation(deadline));

        terminal.resolve_at(
            CommandOutcome::Failed(Error::CancellationUnconfirmed),
            delivered + Duration::from_nanos(1),
        );
        assert_canonical(observation.cancellation(deadline));
        assert!(matches!(
            observation.applied(deadline),
            Some(Err(Error::CancellationUnconfirmed))
        ));
    }

    #[test]
    fn settlement_attribution_is_axis_local_cached_and_profile_completion_is_exact() {
        use crate::{AffectedAxes, Settlement};
        let registry = Arc::new(MotionRegistry::default());
        let (mut observation, _) = observation_fixture();
        observation.motion = registry.establish(TargetSlot::CAMERA_1, AffectedAxes::PAN_TILT, 1);
        registry.establish(TargetSlot::CAMERA_1, AffectedAxes::ZOOM, 2);
        registry.establish(TargetSlot::CAMERA_2, AffectedAxes::PAN_TILT, 3);
        assert!(observation.check_settlement().is_ok());
        let evidence = Settlement::observed_stable(
            AffectedAxes::PAN_TILT,
            ACK,
            crate::camera::MovementTolerance::default(),
        );
        assert_eq!(
            observation
                .commit_position_observation(Ok(evidence))
                .unwrap(),
            evidence
        );
        registry.establish(TargetSlot::CAMERA_1, AffectedAxes::PAN_TILT, 4);
        assert_eq!(
            observation.settled().unwrap().unwrap(),
            evidence,
            "established evidence persists"
        );
        let (mut superseded, _) = observation_fixture();
        superseded.motion = registry.establish(TargetSlot::CAMERA_1, AffectedAxes::PAN_TILT, 5);
        registry.establish(TargetSlot::CAMERA_1, AffectedAxes::PAN_TILT, 6);
        assert!(matches!(
            superseded.commit_position_observation(Ok(evidence)),
            Err(Error::SettlementSuperseded { .. })
        ));
        let (mut exact, _) = observation_fixture();
        exact.motion = registry.establish(TargetSlot::CAMERA_1, AffectedAxes::PAN_TILT, 7);
        registry.establish(TargetSlot::CAMERA_1, AffectedAxes::PAN_TILT, 8);
        let exact_evidence = Settlement::profile_completion(AffectedAxes::PAN_TILT);
        assert_eq!(exact.commit_completion(exact_evidence), exact_evidence);
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
        assert!(!matches!(
            error.recovery(),
            crate::Recovery::Resubmit { .. }
        ));
        assert!(
            matches!(error, Error::SettlementObservationFailed { operation, source } if operation == crate::OperationId::from_nonzero(observation.id().as_nonzero()) && source.failure_context() == Some(crate::FailureContext::new(crate::FailureStage::Terminal, crate::Certainty::FailedConclusively)))
        );
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn halt_reserve_exhaustion_counts_each_axis_and_releases_no_held_permit() {
        let now = Instant::now();
        let mut policy = owner_policy(EnvelopeKind::Raw);
        policy.targets[TargetSlot::CAMERA_1]
            .as_mut()
            .unwrap()
            .control_reserve = 2;
        let mut state = OwnerState::new(policy, Instant::now()).unwrap();
        let held: Vec<_> = (0..4)
            .map(|_| state.permits.try_acquire_ordinary().unwrap())
            .collect();
        let mut stop = command(retrying());
        if let RuntimeRequest::Command { reply_shape, .. } = &mut stop {
            *reply_shape = CommandReply::Stop;
        }
        let reserved: Vec<_> = (0..2)
            .map(|_| state.permits.try_acquire(&stop).unwrap())
            .collect();
        assert!(reserved
            .iter()
            .all(|permit| permit.slot() == AdmissionSlot::ControlReserve));
        let stop_request = stop.clone();
        let prepared = crate::prepared::PreparedHalt {
            target: TargetSlot::CAMERA_1,
            axes: Some(crate::AffectedAxes::MOVEMENT),
            requests: [
                Some(Ok(stop.clone().into())),
                Some(Ok(stop.clone().into())),
                Some(Ok(stop.into())),
            ],
            budget: ACK,
        };
        let (turn, effects) = state.begin_halt(&prepared, 1, now);
        for effect in effects {
            assert!(matches!(
                state.apply_effect(effect, now),
                AppliedEffect::None
            ));
        }
        let (receipt, effects) = state.accept_halt(
            turn,
            prepared,
            now + ACK,
            1,
            crate::runtime::engine::NonZeroDuration::new(ACK).unwrap(),
        );
        for effect in effects {
            assert!(matches!(
                state.apply_effect(effect, now),
                AppliedEffect::None
            ));
        }
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
            state.permits.try_acquire(&stop_request).unwrap().slot(),
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
        Error::Io {
            source: Arc::new(std::io::Error::from(ErrorKind::WouldBlock)),
        },
        Error::Io {
            source: Arc::new(std::io::Error::from(ErrorKind::Interrupted)),
        },
        Error::io_timeout().with_context("idle poll"),
    ] {
        assert!(
            matches!(
                OwnerReceive::read_error(idle.clone()),
                OwnerReceive::NoProgress(NoProgressReason::EagerIdle)
            ),
            "an idle read must not be classified as a fault: {idle}"
        );
    }
    for fault in [
        Error::Io {
            source: Arc::new(std::io::Error::from(ErrorKind::TimedOut)),
        },
        Error::transport_error(std::io::Error::other("ICMP port unreachable")),
        Error::Io {
            source: Arc::new(std::io::Error::from(ErrorKind::ConnectionRefused)),
        },
        Error::ConnectionClosed { source: None },
    ] {
        assert!(
            !matches!(
                OwnerReceive::read_error(fault.clone()),
                OwnerReceive::NoProgress(_)
            ),
            "a real read failure must stay a fault: {fault}"
        );
    }

    let keepalive_timeout = Error::Io {
        source: Arc::new(std::io::Error::from(ErrorKind::TimedOut)),
    };
    assert!(
        matches!(
            OwnerReceive::read_error(keepalive_timeout),
            OwnerReceive::Fatal(_)
        ),
        "an OS TCP timeout is terminal instead of an endless idle read"
    );

    let invalid_input = Error::Io {
        source: Arc::new(std::io::Error::from(ErrorKind::InvalidInput)),
    };
    assert!(
        matches!(
            OwnerReceive::read_error(invalid_input),
            OwnerReceive::Fatal(_)
        ),
        "a transport contract/configuration error closes immediately"
    );
}

/// Issue #729: normalizing a custom transport's already-typed close must not
/// duplicate the public variant prefix.
#[test]
fn transport_close_normalization_keeps_one_connection_closed_prefix() {
    let original = Arc::new(Error::from(std::io::Error::from(
        std::io::ErrorKind::ConnectionReset,
    )));
    let direct = Error::connection_closed(Some(original.clone()));
    for depth in 0..5 {
        let mut contextual = direct.clone();
        for _ in 0..depth {
            contextual = contextual.with_context("control socket receive");
        }
        let expected = contextual.to_string();
        let mut state = reconfigurable_owner_state_at(Instant::now());
        state.input(
            Input::Shutdown(ShutdownReason::TransportClosed {
                cause: Some(Arc::new(contextual)),
            }),
            Instant::now(),
        );
        let normalized = state.boundary_error().expect("engine terminal verdict");
        assert_eq!(normalized.to_string(), expected);
        assert_eq!(
            normalized.to_string().matches("Connection closed").count(),
            1
        );
        let mut cursor: &(dyn std::error::Error + 'static) = &normalized;
        let mut closes = 0;
        loop {
            if let Some(Error::ConnectionClosed {
                source: Some(source),
            }) = cursor
                .downcast_ref::<Error>()
                .or_else(|| cursor.downcast_ref::<Arc<Error>>().map(Arc::as_ref))
                .or_else(|| cursor.downcast_ref::<Box<Error>>().map(Box::as_ref))
            {
                closes += 1;
                assert!(Arc::ptr_eq(source, &original));
            }
            let Some(source) = cursor.source() else { break };
            cursor = source;
        }
        assert_eq!(closes, 1);
    }
}

/// Boundary observations read the engine's verdict even while its terminal
/// effect is still waiting to be applied by the owner.
#[test]
fn engine_terminal_cause_is_authoritative_before_effect_delivery() {
    let mut state = reconfigurable_owner_state_at(Instant::now());
    let now = Instant::now();
    let effects = state.engine.handle(
        Input::Shutdown(ShutdownReason::FramingFailure {
            cause: Arc::new(Error::from(std::io::Error::other(
                "retained stream position lost",
            ))),
        }),
        now,
    );
    let expected = state.engine.terminal_error().unwrap().clone();
    assert_eq!(
        state.boundary_error().unwrap().to_string(),
        expected.to_string()
    );
    for effect in effects {
        let _applied = state.apply_effect(effect, now);
    }
    state.input(Input::Shutdown(ShutdownReason::Explicit), now);
    assert_eq!(
        state.boundary_error().unwrap().to_string(),
        expected.to_string()
    );
}

/// Issue #637: a datagram send failure fails one request while the session
/// keeps running, so the value the caller sees must never claim a replacement
/// session is required.
#[test]
fn a_datagram_send_failure_is_normalized_to_a_per_request_error() {
    let normalized = normalize_datagram_send_error(Error::ConnectionClosed {
        source: Some(Arc::new(Error::from(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "socket closed",
        )))),
    });
    assert!(matches!(normalized, Error::TransportError { .. }));
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
        normalize_datagram_send_error(Error::transport_error(std::io::Error::other(
            "serial encode failed"
        ))),
        Error::TransportError { .. }
    ));
}

/// Issue #634: every normative issue-542 lifecycle fixture replays through the
/// production owner and engine.
///
/// The replay below is deliberately *not* a model of the runtime. It drives the
/// real [`OwnerState`] — which owns the real protocol engine, the real
/// admission permit pool, the real terminal observers and the real
/// target-local state cache — and renders what production actually produced
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
        collections::{BTreeMap, BTreeSet, VecDeque},
        sync::Arc,
        time::{Duration, Instant},
    };

    use super::super::*;
    use crate::{
        protocol::response::{decode_basic, BasicKind},
        runtime::engine::{
            CancellationPolicy, CommandReply, ControlPolicy, EncodedMessage, RequestContext,
            RetryPolicy, TimeoutPolicy, TransportKind,
        },
        wire::WireValue,
        Error,
    };

    const OBSERVER_LATE_DELIVERY: &str =
        include_str!("../../../testdata/issue_542/lifecycle/observer_late_delivery.trace");
    const DEADLINE_CLASSES: &str =
        include_str!("../../../testdata/issue_542/lifecycle/deadline_classes.trace");
    const CAPACITY_AND_FAILURES: &str =
        include_str!("../../../testdata/issue_542/lifecycle/capacity_and_failures.trace");
    const PTZOPTICS_CANCEL: &str =
        include_str!("../../../testdata/issue_542/lifecycle/ptzoptics_cancel.trace");
    const BLOCKING_OUT_OF_ORDER: &str =
        include_str!("../../../testdata/issue_542/lifecycle/blocking_out_of_order.trace");

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
        observer: Option<TerminalObserver<RuntimeOutcome>>,
        cell: Arc<ObserverCell<RuntimeOutcome>>,
        terminal_source: Option<&'static str>,
    }

    struct Pending {
        label: String,
        kind: ObserverKind,
        observer: TerminalObserver<RuntimeOutcome>,
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
            Phase::Ready => "ready".to_owned(),
            Phase::Sending { .. } => "sending".to_owned(),
            Phase::AwaitingAck { .. } => "awaiting-ack".to_owned(),
            Phase::AwaitingCompletion { .. } => "awaiting-completion".to_owned(),
            Phase::Executing { socket, .. } => {
                format!("executing(socket={})", socket.to_wire())
            }
            Phase::ExecutingDisplaced { .. } => "executing-displaced".to_owned(),
            Phase::AwaitingReply { .. } => "awaiting-reply".to_owned(),
            Phase::Backoff { .. } => "backoff".to_owned(),
            Phase::AwaitingCancellationResolution { socket, .. } => {
                format!("awaiting-cancellation(socket={})", socket.to_wire())
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
            Error::InquiryFailed { source } => format!("InquiryFailed({})", error_label(source)),
            Error::MessageLengthError => "MessageLengthError".to_owned(),
            Error::CommandBufferFull => "CommandBufferFull".to_owned(),
            Error::CommandCancelled => "CommandCancelled".to_owned(),
            Error::NoSocket => "NoSocket".to_owned(),
            Error::CommandNotExecutable => "CommandNotExecutable".to_owned(),
            Error::CancellationNotSupported => "CancellationNotSupported".to_owned(),
            Error::RuntimeShutdown => "RuntimeShutdown".to_owned(),
            Error::StreamPoisoned { .. } => "StreamPoisoned".to_owned(),
            Error::RuntimeQueueFull { .. } => "Capacity".to_owned(),
            Error::TransportError { source: reason, .. } => match reason.as_ref() {
                Error::Io { source } => source.to_string(),
                other => error_label(other),
            },
            other => format!("{other:?}"),
        }
    }

    fn outcome_text(outcome: &RuntimeOutcome) -> String {
        match outcome {
            RuntimeOutcome::Command(CommandOutcome::Written) => "Written".to_owned(),
            RuntimeOutcome::Command(CommandOutcome::Applied) => "Applied".to_owned(),
            RuntimeOutcome::Command(CommandOutcome::Cancelled) => "Cancelled".to_owned(),
            RuntimeOutcome::Command(CommandOutcome::Failed(error))
            | RuntimeOutcome::Inquiry(InquiryOutcome::Failed(error)) => {
                format!("Error::{}", error_label(error))
            }
            RuntimeOutcome::Inquiry(InquiryOutcome::Reply { payload }) => {
                format!("Reply(payload={payload:?})")
            }
        }
    }

    fn write_result(label: &str) -> Result<TransmissionMeta, Error> {
        match label {
            "ok" => Ok(TransmissionMeta::Known),
            "Timeout" => Err(Error::io_timeout()),
            other => Err(Error::transport_error(std::io::Error::other(
                other.to_owned(),
            ))),
        }
    }

    /// Recovers the fixture label of the write failure that poisoned a stream
    /// session from the owner's own boundary error.
    fn poison_cause(state: &OwnerState) -> String {
        match state.boundary_error() {
            Some(Error::StreamPoisoned { source }) => ["WriteFailed", "WriteTimeout", "Timeout"]
                .into_iter()
                .find(|label| {
                    write_result(label)
                        .err()
                        .is_some_and(|error| error.to_string() == source.to_string())
                })
                .map_or_else(|| source.to_string(), ToOwned::to_owned),
            other => panic!("a poisoned session without a stream-poison boundary error: {other:?}"),
        }
    }

    fn owner_policy(
        transport: TransportKind,
        capacity: usize,
        cancellation: CancellationPolicy,
    ) -> OwnerPolicy {
        let protocol = ProtocolPolicy {
            capacity: std::num::NonZeroUsize::new(capacity).unwrap(),
            transport,
            inquiry_cooldown: Duration::ZERO,
            raw_inquiry_release_hold: Duration::from_millis(10),
            ..ProtocolPolicy::test_default()
        };
        let mut targets = PerTarget::default();
        for (_, slot) in targets.iter_mut().take(3) {
            *slot = Some(TargetPolicy {
                cancellation,
                ..TargetPolicy::test_default()
            });
        }
        let baseline = TuningBaseline::for_test(&protocol, &targets);
        OwnerPolicy::with_targets(
            protocol,
            targets,
            crate::session_config::ValidatedTuning::for_test(crate::OperationalTuning::new())
                .unwrap(),
            baseline,
        )
    }

    fn command(target: TargetSlot, wire: &str, ack: Duration) -> RuntimeRequest {
        RuntimeRequest::Command {
            reply_shape: CommandReply::AckThenCompletion,
            wire: Arc::new(EncodedMessage::new(&hex(wire)).unwrap()),
            context: RequestContext {
                requires_extended_pan_tilt_speed: false,
                motion: None,
                submission_order: 0,
                dispatch_deadline: None,
                target,
                timeout: TimeoutPolicy {
                    ack: crate::runtime::engine::NonZeroDuration::new(ack).unwrap(),
                    completion: crate::runtime::engine::NonZeroDuration::new(UNREACHABLE).unwrap(),
                    inquiry: crate::runtime::engine::NonZeroDuration::new(UNREACHABLE).unwrap(),
                    cancellation: crate::runtime::engine::NonZeroDuration::new(UNREACHABLE)
                        .unwrap(),
                    ambiguity: crate::runtime::engine::NonZeroDuration::new(UNREACHABLE).unwrap(),
                },
                retry: RetryPolicy::NEVER,
                control: ControlPolicy::default(),
            },
            // Fixtures observe this exact write-only effect through StateCache.
            applied_state: Some(AppliedStateProjection::set(
                crate::KnownState::ExposureSpotlight(true),
            )),
        }
    }

    fn inquiry(target: TargetSlot, wire: &str, reply: Duration) -> RuntimeRequest {
        RuntimeRequest::Inquiry {
            payload_shape: crate::PayloadShape::Unknown,
            wire: Arc::new(EncodedMessage::new(&hex(wire)).unwrap()),
            context: RequestContext {
                requires_extended_pan_tilt_speed: false,
                motion: None,
                submission_order: 0,
                dispatch_deadline: None,
                target,
                timeout: TimeoutPolicy {
                    ack: crate::runtime::engine::NonZeroDuration::new(UNREACHABLE).unwrap(),
                    completion: crate::runtime::engine::NonZeroDuration::new(UNREACHABLE).unwrap(),
                    inquiry: crate::runtime::engine::NonZeroDuration::new(reply).unwrap(),
                    cancellation: crate::runtime::engine::NonZeroDuration::new(UNREACHABLE)
                        .unwrap(),
                    ambiguity: crate::runtime::engine::NonZeroDuration::new(UNREACHABLE).unwrap(),
                },
                retry: RetryPolicy::NEVER,
                control: ControlPolicy::default(),
            },
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
        cache_observers: Vec<(String, TargetSlot)>,
        diagnostics: Option<super::DiagnosticRecorder>,
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
                cache_observers: Vec::new(),
                diagnostics: None,
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
                "observe-state" => self.observe_state(input, &mut out),
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
            self.cache_observers.clear();
            self.pending = None;
            self.next_transmission = 0;
            self.cancel_transmissions = 0;
            let mut state = OwnerState::new(
                owner_policy(transport, capacity, cancellation),
                Instant::now(),
            )
            .unwrap();
            self.diagnostics = Some(super::DiagnosticRecorder::new(&mut state));
            out.push(format!(
                "{} effect session state={} transport={} capacity={} cancel={}",
                self.at,
                session_name(state.state()),
                transport_name(state.facts().transport),
                state.permits().capacity(),
                cancellation_name(cancellation),
            ));
            self.state = Some(state);
        }

        fn observe_state(&mut self, input: &[&str], out: &mut Vec<String>) {
            let name = input[3].to_owned();
            let target = TargetSlot::from(
                crate::IndividualCameraId::new(required(input, "target").parse().unwrap()).unwrap(),
            );
            self.cache_observers.push((name.clone(), target));
            out.push(format!(
                "{} effect cache-observer-added observer={name} target={}",
                self.at,
                target.camera().value()
            ));
        }

        fn admit(&mut self, input: &[&str], out: &mut Vec<String>) {
            let label = input[3].to_owned();
            let target = TargetSlot::from(
                crate::IndividualCameraId::new(required(input, "target").parse().unwrap()).unwrap(),
            );
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
                "command" => command(target, wire, ack),
                "inquiry" => inquiry(target, wire, ack),
                other => panic!("unknown fixture request kind {other}"),
            };
            let now = self.now;
            let (staged, observer, admission) =
                super::stage_admission(self.state_mut(), request, permit);
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
            let mut effects = staged.in_turn(self.state_mut(), &turn);
            effects.extend(
                self.state_mut()
                    .finish_input_turn(turn, EngineTurn::InputOnly),
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
            let (response, wire_socket) = match (kind, decoded.kind) {
                ("ack", BasicKind::Ack { socket }) => (DecodedResponse::Ack { socket }, socket),
                ("complete", BasicKind::Completion { socket }) => {
                    (DecodedResponse::Completion { socket }, socket)
                }
                ("error", BasicKind::Error { socket, code }) => {
                    (DecodedResponse::Error { socket, code }, socket)
                }
                other => panic!("fixture frame {other:?} disagrees with its own bytes"),
            };
            if let Some(socket) = field(input, "socket") {
                assert_eq!(
                    wire_socket.map(|socket| socket.to_wire()),
                    socket.parse::<u8>().ok(),
                    "fixture socket disagrees with its own bytes"
                );
            }
            if let Some(code) = field(input, "code") {
                assert!(
                    matches!(decoded.kind, BasicKind::Error { code: actual, .. }
                        if actual == crate::protocol::response::CameraErrorCode::from(u8::from_str_radix(code, 16).unwrap())),
                    "fixture error code disagrees with its own bytes"
                );
            }
            // Every lifecycle fixture is a raw session, so correlation is by
            // target and socket rather than by envelope sequence.
            let target =
                TargetSlot::from(crate::IndividualCameraId::try_from(decoded.source).unwrap());
            let sequence: Option<FrameSequence> = None;
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
            let socket = wire_socket
                .or_else(|| owned.get(&routed).copied())
                .expect("a routed raw frame owns a socket");
            out.push(format!(
                "{} effect frame-routed id={} target={} via={} socket={} bytes={}",
                self.at,
                routed.get(),
                target.camera().value(),
                if sequence.is_some() {
                    "sequence"
                } else {
                    "target-socket"
                },
                socket.to_wire(),
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
            let command_id = self.state().engine.command_id(id);
            let effects = self
                .state_mut()
                .input_in_turn(&turn, Input::Cancel { id: command_id });
            let ignored = effects.iter().all(|effect| {
                matches!(
                    effect,
                    Effect::CancellationObservation {
                        observation: CancellationObservation::AttemptFailed(_),
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
                        let _applied = self.apply_effect(effect);
                        out.push(format!(
                            "{} effect state id={} from={} to={}",
                            self.at,
                            id.get(),
                            phase_name(from.phase()),
                            phase_name(to.phase())
                        ));
                    }
                    Effect::Transmit { .. } => {
                        self.record_transmit(effect, &mut queue, out);
                    }
                    Effect::DeadlineExpired { id, .. } => {
                        deadlines.insert(id);
                        let _applied = self.apply_effect(effect);
                    }
                    Effect::CancellationRecorded { id } => {
                        let _applied = self.apply_effect(effect);
                        let queued = matches!(
                            self.cancel_phase,
                            Some(Phase::Ready | Phase::Backoff { .. })
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
                            CancellationObservation::Concluded(
                                CancellationConclusion::Cancelled,
                            ) => ("Cancelled".to_owned(), "local"),
                            CancellationObservation::Concluded(
                                CancellationConclusion::Completed,
                            ) => ("Completed".to_owned(), "protocol"),
                            CancellationObservation::Concluded(CancellationConclusion::Failed(
                                kind,
                            )) => (format!("ErrorKind::{kind:?}"), "engine"),
                            CancellationObservation::AttemptFailed(error) => (
                                format!("Error::{}", error_label(error)),
                                if matches!(error, Error::CancellationNotSupported) {
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
                        let _applied = self.apply_effect(effect);
                    }
                    Effect::AppliedState { effect: projection } => {
                        let _applied =
                            self.apply_effect(Effect::AppliedState { effect: projection });
                        let entry = self.state().target_cache[projection.target]
                            .lock()
                            .unwrap()
                            .entry(projection.projection.key());
                        let known = !matches!(entry, crate::StateEntry::Unknown);
                        assert_eq!(known, projection.projection.is_known());
                        for (name, target) in &self.cache_observers {
                            if *target == projection.target {
                                applied.push((
                                    projection.request,
                                    format!(
                                        "{} outcome applied-state observer={name} target={} id={} state={}",
                                        self.at,
                                        target.camera().value(),
                                        projection.request.get(),
                                        if known { "applied" } else { "invalidated" }
                                    ),
                                ));
                            }
                        }
                    }
                    Effect::Terminal { id, ref outcome } => {
                        let source = if deadlines.contains(&id) {
                            "scheduler"
                        } else if boundary == "protocol"
                            && matches!(outcome, RuntimeOutcome::Command(CommandOutcome::Failed(_)))
                        {
                            "camera"
                        } else {
                            boundary
                        };
                        let text = outcome_text(outcome);
                        let permits_before = self.state().permits().available();
                        let _applied = self.apply_effect(effect.clone());
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
                    Effect::SessionEnded(ref end) => {
                        let from = SessionState::Running;
                        let to = end.status.state();
                        let _applied = self.apply_effect(effect);
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

        fn apply_effect(&mut self, effect: Effect) -> AppliedEffect {
            let delivered_at = self.now;
            self.state_mut().apply_effect(effect, delivered_at)
        }

        fn record_admitted(&mut self, id: RequestId, effect: Effect, out: &mut Vec<String>) {
            let pending = self.pending.take().expect("a staged admission");
            let _applied = self.apply_effect(effect);
            let target = self
                .diagnostics
                .as_ref()
                .unwrap()
                .events()
                .iter()
                .filter_map(|event| match event {
                    DiagnosticEvent::Admitted {
                        id: recorded,
                        target,
                        ..
                    } if *recorded == diagnostics::id(id) => Some(*target),
                    _ => None,
                })
                .next_back()
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
                target.value(),
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
            let AppliedEffect::AdmissionRejected(error) = self.apply_effect(effect) else {
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
                    target.camera().value(),
                    hex_text(wire.as_bytes())
                )),
                Transmission::Cancel { target, socket, .. } => {
                    self.cancel_transmissions += 1;
                    out.push(format!(
                        "{} effect transmit tx={transmission} id={} kind=cancel target={} socket={}",
                        self.at,
                        request.get(),
                        target.camera().value(),
                        socket.to_wire()
                    ));
                }
            }
            let AppliedEffect::Transmit(staged) = self.apply_effect(effect) else {
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
                super::write_outcome(write_result(&label)),
                now,
                EngineTurn::InputOnly,
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
            Effect::AdmissionRejected { .. } | Effect::SessionEnded(_) | Effect::Ignored(_) => None,
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
            .any(|line| line.contains("Error::CancellationNotSupported source=profile")));
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

#[test]
fn fatal_receive_io_table_drives_raw_and_wrapped_verdicts() {
    for &kind in crate::transport::FATAL_RECEIVE_IO_KINDS {
        for error in [
            Error::Io {
                source: Arc::new(std::io::Error::from(kind)),
            },
            Error::Io {
                source: Arc::new(std::io::Error::from(kind)),
            }
            .with_context("read camera"),
            Error::transport_error(Error::Io {
                source: Arc::new(std::io::Error::from(kind)),
            }),
        ] {
            let original = error.to_string();
            let OwnerReceive::Fatal(ReceiveFatal::TransportClosed { cause: Some(cause) }) =
                OwnerReceive::read_error(error)
            else {
                panic!("{kind:?} must close the read side");
            };
            assert_eq!(
                cause.to_string(),
                original,
                "read classification retains its exact cause"
            );
        }
    }
    for error in [
        Error::Io {
            source: Arc::new(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
        },
        Error::transport_error(Error::Io {
            source: Arc::new(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
        }),
    ] {
        assert!(matches!(
            OwnerReceive::read_error(error),
            OwnerReceive::Transient(_)
        ));
    }
}

#[test]
fn consumed_input_verdict_preserves_stream_cause_and_discards_one_datagram() {
    let error = Error::ResponseTooLarge { max_size: 24 }.with_context("consumed reply");
    let OwnerReceive::Fatal(ReceiveFatal::FramingFailure { cause, .. }) =
        OwnerReceive::consumed_error(error.clone(), TransportKind::Stream)
    else {
        panic!("consumed stream bytes lost their framing boundary");
    };
    assert_eq!(cause.to_string(), error.to_string());
    assert!(matches!(
        OwnerReceive::consumed_error(error, TransportKind::Datagram),
        OwnerReceive::NoProgress(NoProgressReason::MalformedDatagram)
    ));
}

#[test]
fn effect_storage_retains_warm_capacity_and_prepends_recursive_results_in_order() {
    let mut buffers = OwnerBuffers::new(OwnerLimits::default());
    buffers.effect_scratch.extend([
        Effect::Ignored(IgnoreReason::UnknownRequest),
        Effect::Ignored(IgnoreReason::MalformedFrame),
    ]);
    let mut effects = buffers.take_effects();
    buffers.effect_scratch.extend([
        Effect::Ignored(IgnoreReason::StaleTransmission),
        Effect::Ignored(IgnoreReason::ReplyShapeMismatch),
    ]);
    buffers.prepend_effects(&mut effects);
    let reasons: Vec<_> = effects
        .iter()
        .map(|effect| match effect {
            Effect::Ignored(reason) => *reason,
            _ => panic!("only ignored effects in this queue fixture"),
        })
        .collect();
    assert_eq!(
        reasons,
        [
            IgnoreReason::StaleTransmission,
            IgnoreReason::ReplyShapeMismatch,
            IgnoreReason::UnknownRequest,
            IgnoreReason::MalformedFrame,
        ]
    );
    let queue_capacity = effects.capacity();
    let scratch_capacity = buffers.effect_scratch.capacity();
    effects.clear();
    buffers.recycle_effects(effects);
    assert_eq!(buffers.take_effects().capacity(), queue_capacity);
    assert_eq!(buffers.effect_scratch.capacity(), scratch_capacity);
    assert!(buffers.effect_scratch.is_empty());
}

#[test]
fn every_camera_code_preserves_the_owner_metric_bucket() {
    for code in 0..=u8::MAX {
        assert_eq!(
            crate::protocol::response::CameraErrorCode::from(code).is_busy(),
            matches!(code, 3 | 5 | 0x41)
        );
        assert_eq!(
            crate::protocol::response::CameraErrorCode::from(code).confirms_cancellation(),
            code == 4
        );
    }
}

#[test]
fn frame_batch_refuses_the_first_frame_beyond_its_logical_bound() {
    let mut buffers = OwnerBuffers::new(OwnerLimits {
        frames_per_receive: std::num::NonZeroUsize::new(3).unwrap(),
        ..OwnerLimits::default()
    });
    let mut frames = buffers.take_frames();
    let frame = DecodedFrame {
        target: TargetSlot::CAMERA_1,
        sequence: None,
        response: DecodedResponse::NetworkChange,
    };
    for _ in 0..3 {
        frames.try_push(frame.clone()).unwrap();
    }
    assert!(frames.try_push(frame).is_err());
    assert_eq!(frames.len(), 3);
    buffers.recycle_frames(frames);
    let recycled = buffers.take_frames();
    assert!(recycled.is_empty());
    assert_eq!(recycled.limit, 3);
}
