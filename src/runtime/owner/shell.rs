//! Sans-IO owner shell core (D24, #780).
//!
//! [`OwnerShellCore`] holds everything an owner shell needs between I/O
//! points: the engine owner state, the owner's ends of the boundary lanes,
//! the receive fault and idle runs, the lifecycle publication and the
//! [`OwnerCoordinator`]. It runs every turn body as plain code and stops at
//! the turn's I/O points, so a shell supplies only I/O, exactly as D25 already
//! required of source arbitration:
//!
//! - selecting an event: the shell polls the sources a [`TurnPlan`] names in
//!   [`Selection::order`], then hands the event to [`OwnerShellCore::classify`];
//! - running a turn: [`OwnerShellCore::handle`] returns a [`TurnStep`]. A
//!   [`TurnStep::Drive`] asks the shell to write each transmit that
//!   [`OwnerShellCore::next_transmit`] stages and report it back through
//!   [`OwnerShellCore::finish_transmit`] (or its in-turn variant), then to
//!   [`OwnerShellCore::resume`] the turn; a [`TurnStep::Pause`] asks it to
//!   wait for the interval [`OwnerShellCore::receive_pause`] computes.
//!
//! Nothing here touches a transport, an executor or the clock: every instant
//! is sampled by the shell and passed in.

use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::Error;

use super::boundary::{
    missing_terminal_error, AdmissionBoundary, AdmissionClaim, AdmissionRejectionIngress,
    BoundaryReceivers, CancellationBoundary, ControlBoundary, OwnerEnds, OwnerLifecycle,
    OwnerSnapshot, PreAdmissionRejection, RetainedBoundary,
};
use super::turn::{Disposition, OwnerCoordinator, ReceiveClass, Selected, TurnOutcome, TurnPlan};
use super::{
    clamp_receive_pause, prepend_effects, transient_receive_pause, AppliedEffect, DecodedFrame,
    IdleReceiveRun, Input, OwnerInputTurn, OwnerReceive, OwnerState, RawReleaseResolution,
    RequestLane, RetainedStreamInput, SessionState, ShutdownReason, StagedWrite, TransientFaultRun,
    TransmissionMeta,
};
use crate::runtime::engine::{
    Effect, EngineTurn, IgnoreReason, RawPrefixEvidence, RequestId, TransportKind,
};

/// One event an owner shell selected.
///
/// Admission events retain the inline request owned by the boundary channel;
/// boxing it here would add a heap allocation to every accepted admission.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(super) enum OwnerEvent {
    Admission(Result<AdmissionBoundary, flume::RecvError>),
    Cancellation(CancellationBoundary),
    Control(ControlBoundary),
    Shutdown,
    Receive {
        result: Result<OwnerReceive, Error>,
        received_at: Instant,
    },
    Wake,
}

/// An event the coordinator let through to its engine turn.
#[derive(Debug)]
pub(super) struct ClassifiedEvent {
    pub(super) event: OwnerEvent,
    /// What was selected, for [`OwnerShellCore::finish`].
    pub(super) selected: Selected,
    /// Set when a raw release probe hit a transient fault, so this turn must
    /// not run due work.
    pub(super) suppress_due: bool,
}

/// What a shell does next to carry one owner turn forward.
#[derive(Debug)]
pub(super) enum TurnStep {
    /// Drive `effects` to completion, writing each staged transmit in order,
    /// then [`resume`](OwnerShellCore::resume) the turn with `then`.
    Drive {
        effects: VecDeque<Effect>,
        then: Resume,
    },
    /// Wait for the paced interval [`OwnerShellCore::receive_pause`] computes
    /// for `pause`, then end the turn with `outcome`.
    Pause {
        pause: ReceivePause,
        outcome: TurnOutcome,
    },
    /// The turn is over.
    Done(TurnOutcome),
}

/// Why a receive turn paces the next read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReceivePause {
    /// A receive carried no data (#675).
    Idle,
    /// The `run`-th consecutive transient receive fault.
    Fault { run: u32 },
}

/// The remainder of a turn once its effects have been driven.
#[derive(Debug)]
pub(super) struct Resume(Continuation);

#[derive(Debug)]
enum Continuation {
    Done(TurnOutcome),
    Pause(ReceivePause, TurnOutcome),
    /// Answer a cancellation once its input has been driven, so the answer
    /// reflects any conclusion the drive reached.
    ConcludeCancellation {
        id: RequestId,
        reply: flume::Sender<Result<(), Error>>,
    },
    /// Feed the next frame of an input turn.
    NextFrame(FrameTurn),
    /// Discard a receive whose consumed bytes did not decode.
    Discard {
        error: Error,
        received_at: Instant,
    },
}

/// The frames of one receive, fed through a single engine input turn.
#[derive(Debug)]
struct FrameTurn {
    turn: OwnerInputTurn,
    frames: std::vec::IntoIter<DecodedFrame>,
    received_at: Instant,
}

impl Resume {
    const fn done(outcome: TurnOutcome) -> Self {
        Self(Continuation::Done(outcome))
    }

    /// The input turn the effects being driven belong to. A transmit driven
    /// inside an input turn finishes in that turn, and needs no clock sample.
    pub(super) const fn input_turn(&self) -> Option<&OwnerInputTurn> {
        match &self.0 {
            Continuation::NextFrame(frames) => Some(&frames.turn),
            _ => None,
        }
    }
}

/// The executor-free state and turn logic of one serialized owner. See the
/// module docs.
#[derive(Debug)]
pub(super) struct OwnerShellCore {
    pub(super) state: OwnerState,
    pub(super) receivers: BoundaryReceivers,
    pub(super) admission_rejections: Arc<AdmissionRejectionIngress>,
    /// Preallocated owner scratch keeps ingress draining bounded without
    /// allocating on a rejected submission path.
    admission_rejection_scratch: VecDeque<PreAdmissionRejection>,
    /// Monotonic total already merged into `OwnerState` metrics.
    observed_pre_admission_rejections: u64,
    observed_control_reserve_rejections: u64,
    /// Dropped with the core, after the shell has explicitly dropped its
    /// driver/transport, disconnecting every handle's `actor_alive` receiver.
    /// Nothing is ever sent on it (#626).
    alive: flume::Sender<()>,
    /// Shared with the handles so terminal publication and a concurrent
    /// shutdown acceptance have one lifecycle linearization point.
    lifecycle: Arc<OwnerLifecycle>,
    faults: TransientFaultRun,
    /// Consecutive receives that carried no data (an idle read timeout, or a
    /// driver that reports "no data" immediately). Escalates a cooperative
    /// pause so an immediately-returning idle read cannot hot-spin the owner,
    /// without recording a transport fault or spending any retry budget (#675).
    idle_receives: IdleReceiveRun,
    /// Executor-free source arbitration (#776): source phase and fairness, the
    /// control allowance, the receive-first raw-release proof (#723), and the
    /// retained-boundary slot (#775). A field rather than a loop local so
    /// `Drop` can answer a retained boundary.
    pub(super) coordinator: OwnerCoordinator<RetainedBoundary>,
}

impl OwnerShellCore {
    pub(super) fn new(state: OwnerState, ends: OwnerEnds) -> Self {
        let rejection_capacity = state.policy().limits.diagnostics;
        let mut coordinator = OwnerCoordinator::default();
        // #542's deterministic source order keeps valid protocol input first:
        // an already-buffered ACK/completion settles state before concurrent
        // control observes it. A receive that makes no protocol progress
        // yields the next selection to the boundary channels. That makes an
        // always-failing or idle transport unable to starve shutdown,
        // cancellation, admission, control, or a due timer (#625), without the
        // previous arbitrary receive-history counter.
        //
        // The *succeeding* arm needs its own bound (#675): a peer that returns
        // a valid frame on every read keeps winning the receive-first
        // selection forever and would starve those same boundary sources. A
        // fairness ceiling forces one boundary-first turn after this many
        // consecutive receive-first wins, restoring #625's acceptance
        // criterion — the boundary channels are always eventually polled —
        // even against an unbounded flood of valid frames. It is tied to the
        // receive batch limit because a burst that large is adversarial rather
        // than a real camera's reply stream, so the settle-first ordering
        // above still holds for real traffic.
        coordinator.set_fairness_ceiling(state.policy().limits.frames_per_receive);
        Self {
            state,
            receivers: ends.receivers,
            admission_rejections: ends.admission_rejections,
            admission_rejection_scratch: VecDeque::with_capacity(rejection_capacity),
            observed_pre_admission_rejections: 0,
            observed_control_reserve_rejections: 0,
            alive: ends.alive,
            lifecycle: ends.lifecycle,
            faults: TransientFaultRun::default(),
            idle_receives: IdleReceiveRun::default(),
            coordinator,
        }
    }

    pub(super) fn is_running(&self) -> bool {
        self.state.state() == SessionState::Running
    }

    /// Start one turn. Returns `true` when this turn is a forced
    /// boundary-first turn, before which the shell hands off to other callers.
    pub(super) fn begin_turn(&mut self) -> bool {
        self.coordinator.begin_turn()
    }

    /// Plan one turn at `now`, the instant the shell sampled for it.
    pub(super) fn plan(
        &mut self,
        now: Instant,
        driver: &mut impl RetainedStreamInput,
    ) -> TurnPlan<RetainedBoundary> {
        self.coordinator.plan(
            now,
            self.state.next_wake(),
            self.state.raw_correlation_releases_due(now),
            || {
                matches!(
                    driver.buffered_raw_prefix_evidence(),
                    Ok(Some(RawPrefixEvidence::Complete))
                )
            },
        )
    }

    /// The event for a boundary [`TurnPlan::Redeliver`] hands back.
    ///
    /// The retained boundary predates the just-cleared raw gate. Preserve
    /// shutdown's established priority without re-entering any channel ahead
    /// of it. A signal arriving after this nonblocking check races exactly as
    /// it did with an ordinary already-selected boundary; the final drain
    /// answers the retained payload if shutdown wins.
    pub(super) fn redeliver(&mut self, retained: RetainedBoundary) -> OwnerEvent {
        match self.receivers.shutdown.try_recv() {
            Ok(()) => {
                self.coordinator.restore(retained);
                OwnerEvent::Shutdown
            }
            Err(flume::TryRecvError::Empty | flume::TryRecvError::Disconnected) => match retained {
                RetainedBoundary::Admission(admission) => OwnerEvent::Admission(Ok(admission)),
                RetainedBoundary::Cancellation(cancellation) => {
                    OwnerEvent::Cancellation(cancellation)
                }
            },
        }
    }

    /// Report a selected event to the coordinator at `selected_at`, the one
    /// instant carried through its engine turn. Returns `None` when the
    /// event was discarded, retained or answered and the shell must plan
    /// again.
    ///
    /// If `selected_at` is still before a hold deadline, the turn cannot cross
    /// it merely because a later clock sample happens a few instructions
    /// later; the next plan then enters the raw-release proof.
    pub(super) fn classify(
        &mut self,
        event: OwnerEvent,
        selected_at: Instant,
    ) -> Option<ClassifiedEvent> {
        let selected = match &event {
            OwnerEvent::Wake => Selected::Timer,
            OwnerEvent::Admission(Ok(_)) | OwnerEvent::Cancellation(_) => Selected::Boundary,
            OwnerEvent::Receive {
                result,
                received_at,
            } => Selected::Receive {
                due_at_receive: self.state.raw_correlation_releases_due(*received_at),
                // A normalized idle fault is semantically identical to
                // `NoData`: it consumed no bytes. A genuine transient fault
                // cannot prove there is no ready stale frame behind it.
                class: match result {
                    Ok(OwnerReceive::NoData) => ReceiveClass::NoInput,
                    Ok(OwnerReceive::Fault(error)) if super::receive_reported_no_data(error) => {
                        ReceiveClass::NoInput
                    }
                    Ok(OwnerReceive::Fault(_)) => ReceiveClass::TransientFault,
                    _ => ReceiveClass::Other,
                },
            },
            _ => Selected::Other,
        };
        let disposition = self.coordinator.classify(
            selected,
            self.state.raw_correlation_releases_due(selected_at),
        );
        let (event, suppress_due) = match disposition {
            Disposition::Restart => return None,
            Disposition::Retain => match self.retain_selected_boundary(event, selected_at) {
                None => {
                    self.coordinator.restart_after_retain();
                    return None;
                }
                // Unreachable while selection keeps boundaries ineligible
                // behind an occupied slot; the boundary is applied rather
                // than overwriting the retained one.
                Some(event) => (event, false),
            },
            Disposition::Apply { suppress_due } => (event, suppress_due),
        };
        Some(ClassifiedEvent {
            event,
            selected,
            suppress_due,
        })
    }

    /// The planned wake a selected control event consumed, re-sampled at
    /// `observed_at`.
    ///
    /// The selection describes the instant before it began. If the timer
    /// matured while both tail sources were parked, the control source can
    /// still win the selection. Re-sample the engine before the control
    /// handler mutates it and charge that control to the exact planned wake.
    pub(super) fn control_consumed_wake(&self, observed_at: Instant) -> Option<Instant> {
        self.coordinator
            .control_consumed_wake(self.state.next_wake(), observed_at)
    }

    /// Account for a finished turn. Returns `false` once the session is over.
    pub(super) fn finish(
        &mut self,
        selected: Selected,
        control_consumed_wake: Option<Instant>,
        outcome: TurnOutcome,
    ) -> bool {
        self.coordinator
            .finish(selected, control_consumed_wake, outcome)
    }

    /// Run the engine turn of `event`, selected at `selected_at`, up to its
    /// first I/O point.
    pub(super) fn handle(
        &mut self,
        event: OwnerEvent,
        driver: &mut impl RetainedStreamInput,
        selected_at: Instant,
        suppress_due_for_raw_probe_fault: bool,
    ) -> TurnStep {
        match event {
            OwnerEvent::Shutdown | OwnerEvent::Admission(Err(_)) => {
                self.terminate(ShutdownReason::Explicit, selected_at)
            }
            OwnerEvent::Cancellation(CancellationBoundary { request, reply }) => {
                let id = request.id;
                match self.state.begin_cancellation(request) {
                    Some(input) => TurnStep::Drive {
                        effects: self.state.input(input, selected_at),
                        then: Resume(Continuation::ConcludeCancellation { id, reply }),
                    },
                    None => self.conclude_cancellation(id, &reply),
                }
            }
            OwnerEvent::Control(control) => {
                self.handle_control(control);
                TurnStep::Done(TurnOutcome::Continue)
            }
            OwnerEvent::Admission(Ok(admission)) => self.admit(admission, selected_at),
            OwnerEvent::Wake => self.wake(driver, selected_at),
            OwnerEvent::Receive {
                result: Ok(OwnerReceive::Closed),
                received_at,
            } => self.terminate(
                ShutdownReason::TransportClosed { reason: None },
                received_at,
            ),
            OwnerEvent::Receive {
                result: Ok(OwnerReceive::NoData),
                received_at,
            } => {
                // An expired idle read timeout is not a fault: nothing was
                // consumed, nothing failed, and no request's retry budget is
                // touched. It made no protocol progress, so let a queued
                // boundary run before another idle read (#625). A driver that
                // returns NoData immediately would otherwise spin the owner, so
                // pace the idle-read rate (#675).
                self.idle_receive(driver, received_at)
            }
            OwnerEvent::Receive {
                result: Ok(OwnerReceive::Frames(frames)),
                received_at,
            } => self.frames(frames, driver, received_at),
            OwnerEvent::Receive {
                result: Ok(OwnerReceive::Fault(error)),
                received_at,
            } => self.receive_fault(error, driver, received_at, suppress_due_for_raw_probe_fault),
            OwnerEvent::Receive {
                result: Err(error),
                received_at,
            } => self.discard_undecodable_receive(&error, received_at),
        }
    }

    /// Continue a turn whose effects the shell has driven.
    pub(super) fn resume(
        &mut self,
        then: Resume,
        driver: &mut impl RetainedStreamInput,
    ) -> TurnStep {
        match then.0 {
            Continuation::Done(outcome) => TurnStep::Done(outcome),
            Continuation::Pause(pause, outcome) => TurnStep::Pause { pause, outcome },
            Continuation::ConcludeCancellation { id, reply } => {
                self.conclude_cancellation(id, &reply)
            }
            Continuation::NextFrame(frames) => self.next_frame(frames, driver),
            Continuation::Discard { error, received_at } => {
                self.discard_undecodable_receive(&error, received_at)
            }
        }
    }

    /// The interval to pause the receive source for `pause`, sampled at
    /// `now`. A zero interval means no pause.
    ///
    /// Both pauses escalate with their run and are clamped to the next
    /// scheduler deadline. An idle pause records no fault, does not clear an
    /// existing fault run, and spends no retry budget: only a successful read
    /// or the five-second fault gap proves transient failures stopped
    /// accumulating. For an idle pause, a retained-prefix grace supersedes an
    /// already-expired raw hold: using that stale hold as a zero-duration
    /// clamp would make an immediately-idle custom transport spin before the
    /// real grace timer is selectable.
    pub(super) fn receive_pause(&mut self, pause: ReceivePause, now: Instant) -> Duration {
        match pause {
            ReceivePause::Idle => {
                let deadline = self
                    .coordinator
                    .release_mut()
                    .await_until()
                    .filter(|deadline| *deadline > now)
                    .or_else(|| self.state.next_wake());
                clamp_receive_pause(self.idle_receives.record(), deadline, now)
            }
            ReceivePause::Fault { run } => {
                clamp_receive_pause(transient_receive_pause(run), self.state.next_wake(), now)
            }
        }
    }

    /// Apply `effects` up to the next staged transmit, publishing a terminal
    /// session verdict as it is applied. Returns `None` once `effects` is
    /// exhausted.
    pub(super) fn next_transmit(&mut self, effects: &mut VecDeque<Effect>) -> Option<StagedWrite> {
        while let Some(effect) = effects.pop_front() {
            let terminal_transition = matches!(
                &effect,
                Effect::SessionChanged { to, .. } if *to != SessionState::Running
            );
            let applied = self.state.apply_effect(effect);
            if terminal_transition {
                let error = self
                    .state
                    .boundary_error()
                    .unwrap_or_else(missing_terminal_error);
                self.lifecycle.publish_terminal(error);
            }
            if let AppliedEffect::Transmit(staged) = applied {
                return Some(staged);
            }
        }
        None
    }

    /// Report a transmit driven outside an input turn, which finished at
    /// `finished_at`. Its exact completion is processed before the next
    /// effect or any channel input.
    pub(super) fn finish_transmit(
        &mut self,
        staged: &StagedWrite,
        result: Result<TransmissionMeta, Error>,
        finished_at: Instant,
        effects: &mut VecDeque<Effect>,
    ) {
        // A write can complete while the shell is waiting on the transport
        // and cross a raw correlation deadline. Its transmission-result input
        // is real ordered protocol input, but it must not run the due/dispatch
        // tail before the coordinator has taken its mandatory receive probe.
        // A transmit inside an input turn already has this property by using
        // the active input turn; this is the equivalent seam for a standalone
        // effect chain.
        let produced = if self
            .state
            .raw_correlation_releases_due(finished_at)
            .is_empty()
        {
            self.state.finish_write(staged, result, finished_at)
        } else {
            self.state
                .finish_write_turn(staged, result, finished_at, EngineTurn::INPUT_ONLY)
        };
        prepend_effects(effects, produced);
    }

    /// Report a transmit driven inside `turn`.
    pub(super) fn finish_transmit_in_turn(
        &mut self,
        turn: &OwnerInputTurn,
        staged: &StagedWrite,
        result: Result<TransmissionMeta, Error>,
        effects: &mut VecDeque<Effect>,
    ) {
        let produced = self.state.finish_write_in_turn(turn, staged, result);
        prepend_effects(effects, produced);
    }

    /// End the session: publish its terminal result, answer every boundary
    /// still queued or retained, and return the final snapshot. The shell then
    /// drops its driver before this core, so the liveness lane disconnects
    /// only after transport teardown.
    pub(super) fn conclude(&mut self) -> OwnerSnapshot {
        let boundary_error = self
            .state
            .boundary_error()
            .unwrap_or(Error::RuntimeShutdown);
        self.lifecycle.publish_terminal(boundary_error.clone());
        self.flush_pre_admission_rejections(true);
        if let Some(retained) = self.coordinator.take_retained() {
            self.answer_retained_boundary(retained, boundary_error.clone());
        }
        self.drain_boundaries(boundary_error);
        self.snapshot()
    }

    fn terminate(&mut self, reason: ShutdownReason, observed_at: Instant) -> TurnStep {
        if self.state.state() == SessionState::Running {
            TurnStep::Drive {
                effects: self.state.input(Input::Shutdown(reason), observed_at),
                then: Resume::done(TurnOutcome::Stop),
            }
        } else {
            TurnStep::Done(TurnOutcome::Stop)
        }
    }

    fn wake(&mut self, driver: &mut impl RetainedStreamInput, now: Instant) -> TurnStep {
        // A due raw-correlation release is the last point at which input
        // retained by a byte-stream framer can still belong to the old
        // request. Receive-first selection gives a simultaneously ready
        // completing tail precedence. If the tail is not ready, remove the
        // orphaned prefix before the due pass can dispatch a successor;
        // otherwise that successor could consume the old frame after its tail
        // arrives.
        let releases = self
            .coordinator
            .release_mut()
            .observe(self.state.raw_correlation_releases_due(now))
            .unwrap_or_default();
        if !releases.is_empty() {
            // The selected receive proof belongs only to the exact set that
            // was latched before this Wake. A second hold can become due while
            // the selector is parked; do not let this Wake classify or advance
            // that grown scope. The coordinator drops the old fence/grace and
            // sends the replacement through its own receive-first turn.
            if self
                .coordinator
                .release_mut()
                .replace_if_changed(self.state.raw_correlation_releases_due(now))
            {
                return TurnStep::Done(TurnOutcome::Continue);
            }
            match self.state.resolve_retained_raw_input(driver, now) {
                Ok(RawReleaseResolution::Advance) => {}
                Ok(RawReleaseResolution::AwaitInputUntil(deadline)) => {
                    self.coordinator
                        .release_mut()
                        .wait_for_input_until(deadline);
                    return TurnStep::Done(TurnOutcome::ContinueBuffered);
                }
                Err(error) => {
                    return self.terminate(
                        ShutdownReason::FramingFailure {
                            reason: error.to_string().into_boxed_str(),
                        },
                        now,
                    );
                }
            }
        }
        self.coordinator.release_mut().complete();
        TurnStep::Drive {
            effects: self.state.advance(now),
            then: Resume::done(TurnOutcome::Continue),
        }
    }

    /// A receive that carried no data made no protocol progress, so yield the
    /// next selection to the boundary sources after pacing the idle-read rate:
    /// a transport that returns "no data" immediately (rather than after its
    /// read timeout) would otherwise spin the owner at hundreds of thousands
    /// of reads a second (#675).
    fn idle_receive(
        &mut self,
        driver: &mut impl RetainedStreamInput,
        received_at: Instant,
    ) -> TurnStep {
        match driver.has_buffered_stream_input() {
            Ok(buffered) => TurnStep::Pause {
                pause: ReceivePause::Idle,
                outcome: Self::receive_outcome(buffered),
            },
            Err(error) => self.discard_undecodable_receive(&error, received_at),
        }
    }

    fn frames(
        &mut self,
        frames: Vec<DecodedFrame>,
        driver: &mut impl RetainedStreamInput,
        received_at: Instant,
    ) -> TurnStep {
        self.faults.reset();
        // Real bytes decoded: the transport is not idle, so restart the
        // no-data escalation (#675).
        self.idle_receives.reset();
        // #672: a stream tolerates a delimited frame that did not classify by
        // discarding it and staying Running, exactly as a datagram already
        // does and as 1.x did (log-and-continue). Record one Ignored per
        // discarded frame so the discard stays observable; the session is
        // never poisoned for it.
        for _ in 0..self.state.buffers().take_discarded_malformed() {
            let _ = self
                .state
                .apply_effect(Effect::Ignored(IgnoreReason::MalformedFrame));
        }
        if let Err(error) = self.state.validate_frame_batch(&frames) {
            return self.discard_undecodable_receive(&error, received_at);
        }
        if frames.is_empty() {
            // The read carried bytes that did not finish a frame, or only
            // frames that were discarded above. The framer holds any partial
            // frame; keep pumping so the rest of it can arrive in a later read.
            return match driver.has_buffered_stream_input() {
                Ok(true) => TurnStep::Done(TurnOutcome::ContinueBuffered),
                Ok(false) => TurnStep::Done(TurnOutcome::YieldBoundaries),
                Err(error) => self.discard_undecodable_receive(&error, received_at),
            };
        }
        let frames = FrameTurn {
            turn: self.state.begin_input_turn(received_at),
            frames: frames.into_iter(),
            received_at,
        };
        self.next_frame(frames, driver)
    }

    fn next_frame(
        &mut self,
        mut frames: FrameTurn,
        driver: &mut impl RetainedStreamInput,
    ) -> TurnStep {
        if let Some(frame) = frames.frames.next() {
            return TurnStep::Drive {
                effects: self.state.input_in_turn(&frames.turn, Input::Frame(frame)),
                then: Resume(Continuation::NextFrame(frames)),
            };
        }
        let FrameTurn {
            turn, received_at, ..
        } = frames;
        let buffered = match driver.has_buffered_stream_input() {
            Ok(buffered) => buffered,
            Err(error) => {
                return TurnStep::Drive {
                    effects: self.state.finish_input_turn(turn, EngineTurn::INPUT_ONLY),
                    then: Resume(Continuation::Discard { error, received_at }),
                };
            }
        };
        // A frame-limit batch can leave a complete frame followed by an
        // incomplete stale prefix in the production framer. Do not run due
        // work until each complete retained frame has taken its ordered input
        // turn and any final orphan is classified at the raw release boundary.
        if buffered {
            return TurnStep::Drive {
                effects: self.state.finish_input_turn(turn, EngineTurn::INPUT_ONLY),
                then: Resume::done(TurnOutcome::ContinueBuffered),
            };
        }
        let effects = self.state.finish_input_turn(turn, EngineTurn::COMPLETE);
        // A completed input turn drained the retained fragment before due work
        // ran. If that input settled the old raw correlation at an exact
        // boundary, `finish_input_turn` released it without another Wake turn,
        // so its next independent hold must start with a fresh grace budget.
        // Do not reset for an empty/partial receive: its unresolved prefix
        // still owns the current deadline. Driving the effects never reads
        // the release proof, so completing it first is equivalent.
        self.coordinator.release_mut().complete();
        TurnStep::Drive {
            effects,
            then: Resume::done(TurnOutcome::Continue),
        }
    }

    fn receive_fault(
        &mut self,
        error: Error,
        driver: &mut impl RetainedStreamInput,
        received_at: Instant,
        suppress_due_for_raw_probe_fault: bool,
    ) -> TurnStep {
        if super::receive_reported_no_data(&error) {
            // A driver that reports an idle timeout as a fault still means "no
            // bytes arrived". Normalizing here as well as at the adapter keeps
            // every driver on one contract (#637), and paces the idle-read
            // rate so it cannot hot-spin (#675).
            return self.idle_receive(driver, received_at);
        }
        if !super::receive_fault_is_transient(&error) {
            return self.terminate(
                ShutdownReason::TransportClosed {
                    reason: super::transport_close_reason(&error),
                },
                received_at,
            );
        }
        let (length, span) = self.faults.record(received_at);
        if TransientFaultRun::is_permanent(length, span) {
            // A read that has failed this many times in a row, over this long,
            // is a broken transport rather than a transient fault. End the
            // session with the cause rather than retrying against it forever
            // (#625).
            return self.terminate(
                ShutdownReason::TransportClosed {
                    reason: Some(
                        format!("{length} consecutive receive faults: {error}").into_boxed_str(),
                    ),
                },
                received_at,
            );
        }
        // The engine safely retries sequenced Sony work with its same
        // sequence; a raw command awaiting ACK is left to its own ACK deadline
        // (issue #671; the strict opt-in poisons instead) rather than being
        // replayed. The pause mirrors 1.x's guard against hot-looping on an
        // immediately failing transport; it grows with the run and is clamped
        // to the next scheduler deadline.
        let buffered = match driver.has_buffered_stream_input() {
            Ok(buffered) => buffered,
            Err(framing_error) => {
                return self.discard_undecodable_receive(&framing_error, received_at);
            }
        };
        let effects = if buffered || suppress_due_for_raw_probe_fault {
            let turn = self.state.begin_input_turn(received_at);
            let mut effects = self
                .state
                .input_in_turn(&turn, Input::ReceiveFault { error });
            effects.extend(self.state.finish_input_turn(turn, EngineTurn::INPUT_ONLY));
            effects
        } else {
            self.state.input(Input::ReceiveFault { error }, received_at)
        };
        TurnStep::Drive {
            effects,
            then: Resume(Continuation::Pause(
                ReceivePause::Fault { run: length },
                Self::receive_outcome(buffered),
            )),
        }
    }

    /// Keep protocol input first while the framer retains input; otherwise
    /// give the boundaries the next selection.
    const fn receive_outcome(buffered_stream_input: bool) -> TurnOutcome {
        if buffered_stream_input {
            TurnOutcome::ContinueBuffered
        } else {
            TurnOutcome::YieldBoundaries
        }
    }

    /// Handle bytes that were consumed but did not decode.
    ///
    /// On a byte stream the position is now unknowable, so the session is
    /// poisoned. On a datagram transport it is one bad datagram: nothing else
    /// was consumed, the next datagram frames independently, and the blocking
    /// owner has always failed this per request and kept pumping (#637).
    fn discard_undecodable_receive(&mut self, error: &Error, received_at: Instant) -> TurnStep {
        if self.state.policy().protocol.transport != TransportKind::Stream {
            // The adapter reached this path only after consuming one complete
            // datagram (including an oversized datagram whose copied prefix was
            // rejected). That successful read proves the transport is live even
            // though its payload is unusable, so it clears both consecutive
            // receive-fault accounting and the idle no-data pacing run before
            // the discard becomes observable. A stream takes the terminal path
            // below and deliberately retains its existing framing semantics.
            self.faults.reset();
            self.idle_receives.reset();
            let _ = self
                .state
                .apply_effect(Effect::Ignored(IgnoreReason::MalformedFrame));
            return TurnStep::Done(TurnOutcome::YieldBoundaries);
        }
        self.terminate(
            ShutdownReason::FramingFailure {
                reason: error.to_string().into_boxed_str(),
            },
            received_at,
        )
    }

    /// Retain a boundary the coordinator classified as [`Disposition::Retain`],
    /// or answer it now. Returns `None` once the boundary is retained or
    /// answered. An admission whose claim expired is answered by the claim; a
    /// cancellation whose terminal result is already buffered is answered with
    /// it. Returns the event back only if the slot was unexpectedly occupied.
    fn retain_selected_boundary(
        &mut self,
        event: OwnerEvent,
        selected_at: Instant,
    ) -> Option<OwnerEvent> {
        let boundary = match event {
            OwnerEvent::Admission(Ok(mut admission)) => {
                if !self.claim_admission_boundary(&mut admission, selected_at) {
                    return None;
                }
                RetainedBoundary::Admission(admission)
            }
            OwnerEvent::Cancellation(cancellation) => {
                // An operation that already concluded has delivered its
                // terminal outcome, which answers the cancellation.
                if !self.state.is_active(cancellation.request.id) {
                    let _ = cancellation.reply.try_send(Ok(()));
                    return None;
                }
                RetainedBoundary::Cancellation(cancellation)
            }
            event => return Some(event),
        };
        match self.coordinator.retain(boundary) {
            Ok(()) => None,
            Err(RetainedBoundary::Admission(admission)) => {
                Some(OwnerEvent::Admission(Ok(admission)))
            }
            Err(RetainedBoundary::Cancellation(cancellation)) => {
                Some(OwnerEvent::Cancellation(cancellation))
            }
        }
    }

    /// Claims the one-way pre-admission deadline race before this boundary is
    /// either staged immediately or held behind an exact raw-release gate.
    /// Returning `false` means this method already answered the caller.
    fn claim_admission_boundary(
        &mut self,
        admission: &mut AdmissionBoundary,
        now: Instant,
    ) -> bool {
        if admission.validity_claimed {
            return true;
        }
        let target = admission.request.context().target;
        let lane = RequestLane::of(&admission.request);
        let Some(validity) = admission.validity.as_ref() else {
            return true;
        };
        match validity.claim_for_admission(now) {
            AdmissionClaim::Claimed => {
                admission.validity_claimed = true;
                true
            }
            AdmissionClaim::ExpiredHere => {
                let error = Error::admission_timeout();
                self.state.record_admission_rejection(target, lane, &error);
                let _ = admission.reply.try_send(Err(error));
                false
            }
            AdmissionClaim::ExpiredElsewhere => {
                let _ = admission.reply.try_send(Err(Error::admission_timeout()));
                false
            }
        }
    }

    fn admit(&mut self, mut admission: AdmissionBoundary, accepted_at: Instant) -> TurnStep {
        // A deadline that wins before this exact boundary is admitted is not
        // observer detachment: no engine entry exists yet. Drop the boundary
        // (and therefore its permit and observer) before staging any engine
        // input, so a stale admission can never become a later write. The
        // winner records the rejection exactly once: caller-expiry already
        // entered the handle-side ingress, while owner-expiry records directly
        // into the serialized owner state.
        if !self.claim_admission_boundary(&mut admission, accepted_at) {
            return TurnStep::Done(TurnOutcome::Continue);
        }
        let input = self.state.stage_admission_with(
            admission.request,
            admission.permit,
            admission.observer,
            admission.reply,
        );
        TurnStep::Drive {
            effects: self.state.input(input, accepted_at),
            then: Resume::done(TurnOutcome::Continue),
        }
    }

    /// Answer a cancellation. A refusal leaves the original request
    /// scheduled; the caller keeps its handle and the operation's terminal
    /// slot (#612, #777).
    fn conclude_cancellation(
        &mut self,
        id: RequestId,
        reply: &flume::Sender<Result<(), Error>>,
    ) -> TurnStep {
        let _ = reply.try_send(self.state.conclude_cancellation(id));
        TurnStep::Done(TurnOutcome::Continue)
    }

    /// Merges handle-side, pre-boundary rejection telemetry into owner-held
    /// metrics and diagnostic delivery.
    ///
    /// A control request flushes opportunistically as well as an explicit
    /// ingress wake. That makes a `metrics()` or diagnostics subscription
    /// issued immediately after a fail-fast rejection observe that rejection
    /// without relying on scheduler timing.
    pub(super) fn flush_pre_admission_rejections(&mut self, consumed_wake: bool) {
        let dropped_diagnostics = self
            .admission_rejections
            .drain_into(&mut self.admission_rejection_scratch, consumed_wake);
        // Load after the ingress lock is released. A reporter that raced this
        // drain either contributed an event to this batch or observed the
        // cleared wake marker and reserved the next owner wake, so its scalar
        // count cannot be stranded behind an already-consumed notification.
        let total = self.admission_rejections.total();
        let control_reserve = self.admission_rejections.control_reserve_total();
        let new_rejections = total.saturating_sub(self.observed_pre_admission_rejections);
        let new_control_reserve =
            control_reserve.saturating_sub(self.observed_control_reserve_rejections);
        if new_rejections != 0 || new_control_reserve != 0 {
            self.state
                .record_pre_admission_rejections(new_rejections, new_control_reserve);
            self.observed_pre_admission_rejections = total;
            self.observed_control_reserve_rejections = control_reserve;
        }
        if dropped_diagnostics != 0 {
            self.state.record_dropped_diagnostics(dropped_diagnostics);
        }
        while let Some(rejection) = self.admission_rejection_scratch.pop_front() {
            self.state.record_admission_rejection_diagnostic(
                rejection.target,
                rejection.lane,
                rejection.error,
            );
        }
    }

    fn handle_control(&mut self, control: ControlBoundary) {
        self.flush_pre_admission_rejections(true);
        match control {
            ControlBoundary::FlushAdmissionRejections => {}
            #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
            ControlBoundary::Snapshot(reply) => {
                let _ = reply.try_send(self.snapshot());
            }
            ControlBoundary::Metrics(reply) => {
                let _ = reply.try_send(Ok(self.state.metrics_snapshot()));
            }
            ControlBoundary::SubscribeDiagnostics { capacity, reply } => {
                let result = self.state.subscribe_diagnostics(capacity);
                let _ = reply.try_send(result);
            }
            ControlBoundary::Reconfigure {
                validated_tuning,
                reply,
            } => {
                let result = match self.state.boundary_error() {
                    Some(error) => Err(error),
                    None => (*validated_tuning).and_then(|tuning| self.state.retune(tuning)),
                };
                let _ = reply.try_send(result);
            }
        }
    }

    /// Answer a boundary which the raw-release coordinator had already removed
    /// from its channel when the session became terminal.  The ordinary queue
    /// drain cannot see this payload, so retaining its exact cancellation
    /// observation semantics here closes the same lifecycle edge.
    fn answer_retained_boundary(&mut self, retained: RetainedBoundary, error: Error) {
        match retained {
            RetainedBoundary::Admission(admission) => {
                let _ = admission.reply.try_send(Err(error));
            }
            // The caller still holds its handle; a terminal outcome already
            // delivered to it takes precedence over this error (#777).
            RetainedBoundary::Cancellation(cancellation) => {
                let _ = cancellation.reply.try_send(Err(error));
            }
        }
        self.state.fail_unstaged_boundary(1);
    }

    /// Answer every queued boundary message with the session's terminal error.
    ///
    /// The lanes are drained repeatedly until one whole pass finds all three
    /// empty, because a caller can enqueue on a lane that was already visited
    /// while a later one is still being drained (#626). The residual window
    /// between the last pass and the owner dropping its receivers is closed by
    /// the liveness lane, not here.
    fn drain_boundaries(&mut self, error: Error) {
        // A reporter may have coalesced behind the owner while it was handling
        // its terminal transition. Merge all compact facts the owner can still
        // deliver before answering/dropping ordinary boundary work.
        self.flush_pre_admission_rejections(true);
        let mut dropped = 0usize;
        loop {
            let before = dropped;
            while let Ok(admission) = self.receivers.admissions.try_recv() {
                let _ = admission.reply.try_send(Err(error.clone()));
                drop(admission);
                dropped = dropped.saturating_add(1);
            }
            while let Ok(cancel) = self.receivers.cancellations.try_recv() {
                // The caller keeps its handle, and a terminal outcome already
                // delivered to it takes precedence over this error (#777).
                let _ = cancel.reply.try_send(Err(error.clone()));
                dropped = dropped.saturating_add(1);
            }
            while let Ok(control) = self.receivers.control.try_recv() {
                match control {
                    ControlBoundary::FlushAdmissionRejections => {
                        self.flush_pre_admission_rejections(true);
                        continue;
                    }
                    #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
                    ControlBoundary::Snapshot(reply) => {
                        let _ = reply.try_send(self.snapshot());
                    }
                    ControlBoundary::Metrics(reply) => {
                        let _ = reply.try_send(Err(error.clone()));
                    }
                    ControlBoundary::SubscribeDiagnostics { reply, .. } => {
                        let _ = reply.try_send(Err(error.clone()));
                    }
                    ControlBoundary::Reconfigure { reply, .. } => {
                        let _ = reply.try_send(Err(error.clone()));
                    }
                }
                dropped = dropped.saturating_add(1);
            }
            self.flush_pre_admission_rejections(true);
            if dropped == before {
                break;
            }
        }
        self.state.fail_unstaged_boundary(dropped);
    }

    #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
    fn snapshot(&self) -> OwnerSnapshot {
        OwnerSnapshot {
            #[cfg(feature = "runtime-tokio")]
            metrics: self.state.metrics(),
            #[cfg(feature = "runtime-tokio")]
            diagnostics: self.state.diagnostics().copied().collect(),
            state: self.state.state(),
            active: self.state.active_len(),
        }
    }

    #[cfg(not(all(test, any(feature = "runtime-tokio", feature = "runtime-smol"))))]
    const fn snapshot(&self) -> OwnerSnapshot {
        OwnerSnapshot {}
    }
}

impl Drop for OwnerShellCore {
    fn drop(&mut self) {
        // A shell can unwind before it reaches `conclude`, so fail closed
        // before draining.
        let error = self.lifecycle.fail_closed();

        // The explicit normal-path drain may race with a sender that was
        // already admitted. A final drain closes that residual window before
        // the sender fields are released. Keep the liveness sender borrowed
        // through this drain so its disconnect remains the final teardown
        // barrier for waiters.
        let _alive_during_drain = &self.alive;
        // A boundary retained behind a raw release when the shell unwound is
        // no longer in any channel; answer it before the queues.
        if let Some(retained) = self.coordinator.take_retained() {
            self.answer_retained_boundary(retained, error.clone());
        }
        self.drain_boundaries(error);
    }
}

/// Turn bodies driven directly, with explicit instants and no executor: the
/// shell's only contribution is running the steps the core returns.
#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::unwrap_used)]

    use super::super::boundary::OwnerHandleCore;
    use super::super::OwnerPolicy;
    use super::*;
    use crate::runtime::engine::{CancellationPolicy, EnvelopeKind, ProtocolPolicy, TargetPolicy};
    use crate::CameraId;

    /// A datagram driver: it never retains stream input.
    struct Datagrams;

    impl RetainedStreamInput for Datagrams {}

    fn owner() -> (OwnerHandleCore, OwnerShellCore) {
        let policy = OwnerPolicy::single_target(
            ProtocolPolicy {
                capacity: 1,
                envelope: EnvelopeKind::Raw,
                transport: TransportKind::Datagram,
                inquiry_capacity: 1,
                command_spacing: Duration::ZERO,
                inquiry_spacing: Duration::ZERO,
                inquiry_cooldown: Duration::ZERO,
                raw_inquiry_release_hold: Duration::from_secs(1),
                raw_release_grace: Duration::from_millis(100),
                strict_unconfirmed_poison: false,
            },
            CameraId::CAMERA_1,
            TargetPolicy {
                command_sockets: 2,
                cancellation: CancellationPolicy::Supported,
                control_reserve: 0,
            },
        )
        .unwrap();
        let state = OwnerState::new(policy).unwrap();
        let (handle, ends) = OwnerHandleCore::new(&state);
        (handle, OwnerShellCore::new(state, ends))
    }

    fn receive(result: Result<OwnerReceive, Error>, received_at: Instant) -> OwnerEvent {
        OwnerEvent::Receive {
            result,
            received_at,
        }
    }

    /// Runs one turn to its end, as a shell would, with writes that never
    /// happen: none of these turns stage a transmit.
    fn run_turn(core: &mut OwnerShellCore, mut step: TurnStep) -> TurnStep {
        while let TurnStep::Drive { mut effects, then } = step {
            assert!(core.next_transmit(&mut effects).is_none());
            step = core.resume(then, &mut Datagrams);
        }
        step
    }

    #[test]
    fn explicit_shutdown_drives_the_terminal_transition_and_publishes_it() {
        let (handle, mut core) = owner();
        let step = core.handle(OwnerEvent::Shutdown, &mut Datagrams, Instant::now(), false);
        assert!(matches!(step, TurnStep::Drive { .. }));
        assert!(matches!(
            run_turn(&mut core, step),
            TurnStep::Done(TurnOutcome::Stop)
        ));
        assert!(!core.is_running());
        assert!(
            matches!(handle.shutdown(), Err(Error::RuntimeShutdown)),
            "the terminal result was published while its effect was applied"
        );
    }

    #[test]
    fn a_closed_transport_ends_the_session_with_its_cause() {
        let (handle, mut core) = owner();
        let step = core.handle(
            receive(Ok(OwnerReceive::Closed), Instant::now()),
            &mut Datagrams,
            Instant::now(),
            false,
        );
        assert!(matches!(
            run_turn(&mut core, step),
            TurnStep::Done(TurnOutcome::Stop)
        ));
        assert!(matches!(
            handle.shutdown(),
            Err(Error::ConnectionClosed { .. })
        ));
    }

    #[test]
    fn idle_receives_pace_without_a_fault_until_bytes_arrive() {
        let (_handle, mut core) = owner();
        let now = Instant::now();
        for expected in [Duration::from_millis(10), Duration::from_millis(20)] {
            let step = core.handle(
                receive(Ok(OwnerReceive::NoData), now),
                &mut Datagrams,
                now,
                false,
            );
            let TurnStep::Pause { pause, outcome } = step else {
                panic!("an idle receive pauses the receive source: {step:?}");
            };
            assert_eq!(pause, ReceivePause::Idle);
            assert_eq!(outcome, TurnOutcome::YieldBoundaries);
            assert_eq!(core.receive_pause(pause, now), expected);
        }

        // A byte-bearing receive, even one that completed no frame, restarts
        // the idle run.
        let step = core.handle(
            receive(Ok(OwnerReceive::Frames(Vec::new())), now),
            &mut Datagrams,
            now,
            false,
        );
        assert!(matches!(step, TurnStep::Done(TurnOutcome::YieldBoundaries)));
        assert_eq!(
            core.receive_pause(ReceivePause::Idle, now),
            Duration::from_millis(10)
        );
        assert!(core.is_running());
    }

    #[test]
    fn a_transient_fault_is_driven_before_its_pause_grows_with_the_run() {
        let (_handle, mut core) = owner();
        let start = Instant::now();
        for (offset, run, expected) in [
            (Duration::ZERO, 1, Duration::from_millis(10)),
            (Duration::from_millis(1), 2, Duration::from_millis(20)),
        ] {
            let at = start + offset;
            let fault = Error::TransportError("transient receive fault".into());
            let step = core.handle(
                receive(Ok(OwnerReceive::Fault(fault)), at),
                &mut Datagrams,
                at,
                false,
            );
            assert!(matches!(step, TurnStep::Drive { .. }));
            let TurnStep::Pause { pause, outcome } = run_turn(&mut core, step) else {
                panic!("a transient fault pauses after its input is driven");
            };
            assert_eq!(pause, ReceivePause::Fault { run });
            assert_eq!(outcome, TurnOutcome::YieldBoundaries);
            assert_eq!(core.receive_pause(pause, at), expected);
        }
        assert!(core.is_running());
    }

    #[test]
    fn an_undecodable_datagram_is_discarded_and_the_session_survives() {
        let (_handle, mut core) = owner();
        let now = Instant::now();
        let step = core.handle(
            receive(Err(Error::TransportError("bad datagram".into())), now),
            &mut Datagrams,
            now,
            false,
        );
        assert!(matches!(step, TurnStep::Done(TurnOutcome::YieldBoundaries)));
        assert!(core.is_running());
    }
}
