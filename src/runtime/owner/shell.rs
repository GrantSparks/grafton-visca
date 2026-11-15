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
//!   [`OwnerShellCore::finish_transmit`], then to
//!   [`OwnerShellCore::resume`] the turn; a [`TurnStep::Pace`] asks it to
//!   sample the clock for [`OwnerShellCore::pace_receive`], which defers the
//!   next receive while every other source stays selectable.
//!
//! Nothing here touches a transport, an executor or the clock: every instant
//! is sampled by the shell and passed in.

#[cfg(test)]
use super::TransmissionMeta;
use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};

use super::WriteOutcome;
use crate::Error;

use super::boundary::{
    missing_terminal_error, AdmissionBoundary, AdmissionBoundaryClaim, AdmissionClaim,
    AdmissionRejectionIngress, BoundaryReceivers, CancellationBoundary, ControlBoundary, OwnerEnds,
    OwnerLifecycle, OwnerSnapshot, PreAdmissionRejection, RetainedBoundary,
};
use super::turn::{Disposition, OwnerCoordinator, ReceiveClass, Selected, TurnOutcome, TurnPlan};
use super::{
    clamp_receive_pause, prepend_effects, transient_receive_pause, AppliedEffect, IdleReceiveRun,
    Input, OwnerInputTurn, OwnerReceive, OwnerState, RawReleaseResolution, RetainedStreamInput,
    SessionState, ShutdownReason, StagedWrite, TransientFaultRun,
};
use crate::runtime::engine::{Effect, EngineTurn, IgnoreReason};

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
        result: OwnerReceive,
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
    /// Pace the next receive by the interval [`OwnerShellCore::pace_receive`]
    /// computes for `pause` at an instant the shell samples once the turn's
    /// effects are driven, then end the turn with `outcome`.
    Pace {
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
    /// Answer a cancellation once its input has been driven, so the answer
    /// reflects any conclusion the drive reached.
    ConcludeCancellation {
        answer: Result<(), Error>,
        reply: flume::Sender<Result<(), Error>>,
    },
    /// Feed the next frame of an input turn.
    NextFrame(FrameTurn),
}

/// The frames of one receive, fed through a single engine input turn.
#[derive(Debug)]
struct FrameTurn {
    turn: OwnerInputTurn,
    frames: super::FrameBatch,
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
    /// Consecutive eager receives that carried no data. Escalates a cooperative
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
        let rejection_capacity = state.policy().limits.staged_diagnostics;
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
            self.state.next_wake(now),
            self.state.raw_correlation_releases_due(now),
            || {
                matches!(
                    driver.retained().map(|input| input.evidence),
                    Some(super::RetainedEvidence::Complete)
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
                    OwnerReceive::NoProgress(
                        super::NoProgressReason::EagerIdle | super::NoProgressReason::IdleTimeout,
                    ) => ReceiveClass::NoInput,
                    OwnerReceive::Transient(_) => ReceiveClass::TransientFault,
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
            .control_consumed_wake(self.state.next_wake(observed_at), observed_at)
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
            OwnerEvent::Cancellation(CancellationBoundary::Halt(halt)) => {
                match halt.validity.claim_for_admission(selected_at) {
                    AdmissionClaim::Claimed => {
                        match self.lifecycle.fence_order(
                            &self.state.motion,
                            halt.prepared.target,
                            halt.prepared.axes,
                        ) {
                            Ok(cutoff) => {
                                let receipt = self.state.accept_halt(
                                    halt.prepared,
                                    halt.validity.deadline(),
                                    cutoff,
                                    selected_at,
                                );
                                let _ = halt.reply.try_send(Ok(receipt));
                                TurnStep::Drive {
                                    effects: self.state.advance(selected_at),
                                    then: Resume::done(TurnOutcome::Continue),
                                }
                            }
                            Err(error) => {
                                let _ = halt.reply.try_send(Err(error));
                                TurnStep::Done(TurnOutcome::Continue)
                            }
                        }
                    }
                    AdmissionClaim::ExpiredHere | AdmissionClaim::ExpiredElsewhere => {
                        let _ = halt.reply.try_send(Err(Error::admission_timeout()));
                        TurnStep::Done(TurnOutcome::Continue)
                    }
                }
            }
            OwnerEvent::Cancellation(CancellationBoundary::Cancel { request, reply }) => {
                let (effects, answer) = self.state.begin_cancellation(request, selected_at);
                TurnStep::Drive {
                    effects,
                    then: Resume(Continuation::ConcludeCancellation { answer, reply }),
                }
            }
            OwnerEvent::Control(control) => {
                self.handle_control(control, selected_at);
                TurnStep::Done(TurnOutcome::Continue)
            }
            OwnerEvent::Admission(Ok(admission)) => self.admit(admission, selected_at),
            OwnerEvent::Wake => self.wake(driver, selected_at),
            OwnerEvent::Receive {
                result,
                received_at,
            } => match result {
                OwnerReceive::Fatal(fatal) => self.receive_fatal(fatal, received_at),
                OwnerReceive::NoProgress(super::NoProgressReason::EagerIdle) => {
                    self.idle_receive(driver, true)
                }
                OwnerReceive::NoProgress(super::NoProgressReason::IdleTimeout) => {
                    self.idle_receive(driver, false)
                }
                OwnerReceive::NoProgress(super::NoProgressReason::MalformedDatagram) => {
                    self.malformed_datagram()
                }
                OwnerReceive::Frames(frames) => self.frames(frames, driver, received_at),
                OwnerReceive::Transient(error) => {
                    self.receive_fault(error, driver, received_at, suppress_due_for_raw_probe_fault)
                }
            },
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
            Continuation::ConcludeCancellation { answer, reply } => {
                let _ = reply.try_send(answer);
                TurnStep::Done(TurnOutcome::Continue)
            }
            Continuation::NextFrame(frames) => self.next_frame(frames, driver),
        }
    }

    /// Defer the next receive by the paced interval for `pause`, sampled at
    /// `now`. The coordinator keeps every other source selectable meanwhile,
    /// so the pause never delays a boundary or a timer.
    pub(super) fn pace_receive(&mut self, pause: ReceivePause, now: Instant) {
        let pause = self.receive_pause(pause, now);
        if let Some(until) = now.checked_add(pause).filter(|_| !pause.is_zero()) {
            self.coordinator.pace_receive(until);
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
    fn receive_pause(&mut self, pause: ReceivePause, now: Instant) -> Duration {
        match pause {
            ReceivePause::Idle => {
                let deadline = self
                    .coordinator
                    .release_mut()
                    .await_until()
                    .filter(|deadline| *deadline > now)
                    .or_else(|| self.state.next_wake(now));
                clamp_receive_pause(self.idle_receives.record(), deadline, now)
            }
            ReceivePause::Fault { run } => {
                clamp_receive_pause(transient_receive_pause(run), self.state.next_wake(now), now)
            }
        }
    }

    /// Apply `effects` up to the next staged transmit, publishing a terminal
    /// session verdict as it is applied. Returns `None` once `effects` is
    /// exhausted. A staged transmit ends any receive pause: its reply must
    /// be read as soon as it arrives.
    pub(super) fn next_transmit(
        &mut self,
        effects: &mut VecDeque<Effect>,
        observed_at: Instant,
    ) -> Option<StagedWrite> {
        self.state.observed_at = observed_at;
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
                if let Some(expired) = self.state.engine.expire_unwritten_halt(
                    staged.request,
                    staged.transmission,
                    observed_at,
                ) {
                    prepend_effects(effects, expired.into());
                    continue;
                }
                return Some(staged);
            }
        }
        None
    }

    /// Report a transmit the shell just wrote. Inside an input `turn` it
    /// finishes in that turn; otherwise it finishes at `finished_at`, the
    /// instant the write returned. Its exact completion is processed before
    /// the next effect or any channel input.
    pub(super) fn finish_transmit(
        &mut self,
        turn: Option<&OwnerInputTurn>,
        staged: &StagedWrite,
        result: WriteOutcome,
        finished_at: Instant,
        effects: &mut VecDeque<Effect>,
    ) {
        if matches!(result, WriteOutcome::Sent(_)) {
            self.idle_receives.reset();
            self.coordinator.end_receive_pause();
        }
        let Some(turn) = turn else {
            return self.finish_transmit_outside_turn(staged, result, finished_at, effects);
        };
        let produced = self.state.finish_write_in_turn(turn, staged, result);
        prepend_effects(effects, produced);
    }

    fn finish_transmit_outside_turn(
        &mut self,
        staged: &StagedWrite,
        result: WriteOutcome,
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
                .finish_write_turn(staged, result, finished_at, EngineTurn::InputOnly)
        };
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
        self.publish_metrics();
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
                            cause: Arc::new(error),
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

    /// Eager no-data reads yield to boundaries after pacing the idle-read
    /// rate (#675). A read that waited its full timeout already yielded while
    /// blocked; keep receive first so its next reply can beat an equal timer.
    fn idle_receive(&mut self, driver: &mut impl RetainedStreamInput, eager: bool) -> TurnStep {
        let buffered = driver.retained().is_some();
        if eager {
            TurnStep::Pace {
                pause: ReceivePause::Idle,
                outcome: Self::receive_outcome(buffered),
            }
        } else if buffered {
            TurnStep::Done(TurnOutcome::ContinueBuffered)
        } else {
            TurnStep::Done(TurnOutcome::Continue)
        }
    }

    fn frames(
        &mut self,
        frames: super::FrameBatch,
        driver: &mut impl RetainedStreamInput,
        received_at: Instant,
    ) -> TurnStep {
        self.faults.reset();
        // Real bytes decoded: the transport is not idle, so restart the
        // no-data escalation (#675).
        self.idle_receives.reset();
        // #672: a stream tolerates a delimited frame that did not classify by
        // discarding it and staying Running, exactly as a datagram already
        // does: an invalid frame is counted and discarded, and the session
        // continues. Record one Ignored per
        // discarded frame so the discard stays observable; the session is
        // never poisoned for it.
        for _ in 0..self.state.buffers().take_discarded_malformed() {
            let _ = self
                .state
                .apply_effect(Effect::Ignored(IgnoreReason::MalformedFrame));
        }
        if let Err(error) = self.state.validate_frame_batch(&frames) {
            self.state.buffers().recycle_frames(frames);
            return self.discard_undecodable_receive(&error, received_at);
        }
        if frames.is_empty() {
            self.state.buffers().recycle_frames(frames);
            // The read carried bytes that did not finish a frame, or only
            // frames that were discarded above. The framer holds any partial
            // frame; keep pumping so the rest of it can arrive in a later read.
            return TurnStep::Done(if driver.retained().is_some() {
                TurnOutcome::ContinueBuffered
            } else {
                TurnOutcome::YieldBoundaries
            });
        }
        let frames = FrameTurn {
            turn: self.state.begin_input_turn(received_at),
            frames,
        };
        self.next_frame(frames, driver)
    }

    fn next_frame(
        &mut self,
        mut frames: FrameTurn,
        driver: &mut impl RetainedStreamInput,
    ) -> TurnStep {
        if let Some(frame) = frames.frames.pop_front() {
            return TurnStep::Drive {
                effects: self.state.input_in_turn(&frames.turn, Input::Frame(frame)),
                then: Resume(Continuation::NextFrame(frames)),
            };
        }
        let FrameTurn { turn, frames } = frames;
        self.state.buffers().recycle_frames(frames);
        let buffered = driver.retained().is_some();
        // A frame-limit batch can leave a complete frame followed by an
        // incomplete stale prefix in the production framer. Do not run due
        // work until each complete retained frame has taken its ordered input
        // turn and any final orphan is classified at the raw release boundary.
        if buffered {
            return TurnStep::Drive {
                effects: self.state.finish_input_turn(turn, EngineTurn::InputOnly),
                then: Resume::done(TurnOutcome::ContinueBuffered),
            };
        }
        let effects = self.state.finish_input_turn(turn, EngineTurn::Complete);
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
        let (length, span) = self.faults.record(received_at);
        if TransientFaultRun::is_permanent(length, span) {
            // A read that has failed this many times in a row, over this long,
            // is a broken transport rather than a transient fault. End the
            // session with the cause rather than retrying against it forever
            // (#625).
            return self.terminate(
                ShutdownReason::TransportClosed {
                    cause: Some(Arc::new(
                        error.with_context(format!("{length} consecutive receive faults")),
                    )),
                },
                received_at,
            );
        }
        // The engine safely retries sequenced Sony work with its same
        // sequence; a raw command awaiting ACK is left to its own ACK deadline
        // (issue #671; the strict opt-in poisons instead) rather than being
        // replayed. The pause guards against hot-looping on an immediately
        // failing transport; it grows with the run and is clamped
        // to the next scheduler deadline.
        let buffered = driver.retained().is_some();
        let effects = if buffered || suppress_due_for_raw_probe_fault {
            let turn = self.state.begin_input_turn(received_at);
            let mut effects = self
                .state
                .input_in_turn(&turn, Input::ReceiveFault { error });
            effects.extend(self.state.finish_input_turn(turn, EngineTurn::InputOnly));
            effects
        } else {
            self.state.input(Input::ReceiveFault { error }, received_at)
        };
        // Arm before the effects are driven: a staged write must end this
        // pause rather than have its invited reply delayed by a later re-arm.
        self.pace_receive(ReceivePause::Fault { run: length }, received_at);
        TurnStep::Drive {
            effects,
            then: Resume::done(Self::receive_outcome(buffered)),
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

    fn receive_fatal(&mut self, fatal: super::ReceiveFatal, received_at: Instant) -> TurnStep {
        let reason = match fatal {
            super::ReceiveFatal::TransportClosed { cause } => ShutdownReason::TransportClosed {
                cause: cause.map(Arc::new),
            },
            super::ReceiveFatal::FramingFailure { cause } => ShutdownReason::FramingFailure {
                cause: Arc::new(cause),
            },
        };
        self.terminate(reason, received_at)
    }

    fn malformed_datagram(&mut self) -> TurnStep {
        self.faults.reset();
        self.idle_receives.reset();
        let _ = self
            .state
            .apply_effect(Effect::Ignored(IgnoreReason::MalformedFrame));
        TurnStep::Done(TurnOutcome::YieldBoundaries)
    }

    /// Invalid internal batches obey the same consumed-input verdict as the adapter.
    fn discard_undecodable_receive(&mut self, error: &Error, received_at: Instant) -> TurnStep {
        match self.state.policy().protocol.transport {
            crate::runtime::engine::TransportKind::Stream => self.receive_fatal(
                super::ReceiveFatal::FramingFailure {
                    cause: error.clone(),
                },
                received_at,
            ),
            crate::runtime::engine::TransportKind::Datagram => self.malformed_datagram(),
        }
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
                if let CancellationBoundary::Cancel { request, reply } = &cancellation {
                    if !self.state.is_active(request.id) {
                        let _ = reply.try_send(Ok(()));
                        return None;
                    }
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
        let validity = match &admission.claim {
            AdmissionBoundaryClaim::Unbounded | AdmissionBoundaryClaim::Claimed => return true,
            AdmissionBoundaryClaim::Pending(validity) => validity,
        };
        let target = admission.request.context().target;
        let lane = admission.request.lane();
        match validity.claim_for_admission(now) {
            AdmissionClaim::Claimed => {
                admission.claim = AdmissionBoundaryClaim::Claimed;
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
        let turn = self.state.begin_input_turn(accepted_at);
        let verdict = self.state.admit_in_turn(
            &turn,
            admission.request,
            admission.permit,
            admission.observer,
        );
        let _ = admission.reply.try_send(verdict);
        TurnStep::Drive {
            effects: self.state.finish_input_turn(turn, EngineTurn::Complete),
            then: Resume::done(TurnOutcome::Continue),
        }
    }

    /// Merges handle-side, pre-boundary rejection telemetry into owner-held
    /// metrics and diagnostic delivery.
    ///
    /// A control request flushes opportunistically as well as an explicit
    /// ingress wake. That makes a `metrics()` or diagnostics subscription
    /// issued immediately after a fail-fast rejection observe that rejection
    /// without relying on scheduler timing.
    pub(super) fn flush_pre_admission_rejections(&mut self, consumed_wake: bool) {
        let dropped_staged_diagnostics = self
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
        if dropped_staged_diagnostics != 0 {
            self.state
                .record_dropped_staged_diagnostics(dropped_staged_diagnostics);
        }
        while let Some(rejection) = self.admission_rejection_scratch.pop_front() {
            self.state.record_admission_rejection_diagnostic(
                rejection.target,
                rejection.lane,
                rejection.error,
            );
        }
    }

    fn handle_control(&mut self, control: ControlBoundary, now: Instant) {
        self.flush_pre_admission_rejections(true);
        match control {
            ControlBoundary::FlushAdmissionRejections => {}
            #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
            ControlBoundary::Snapshot(reply) => {
                let _ = reply.try_send(self.snapshot());
            }
            ControlBoundary::Metrics(reply) => {
                let pending =
                    self.receivers
                        .admissions
                        .len()
                        .saturating_add(usize::from(matches!(
                            self.coordinator.retained(),
                            Some(RetainedBoundary::Admission(_))
                        )));
                let _ = reply.try_send(Ok(self.state.metrics_snapshot(pending)));
            }
            ControlBoundary::SubscribeDiagnostics { capacity, reply } => {
                let result = self
                    .state
                    .subscribe_diagnostics(capacity, Arc::clone(&self.lifecycle));
                let _ = reply.try_send(result);
            }
            ControlBoundary::Reconfigure {
                validated_tuning,
                reply,
            } => {
                let result = match self.state.boundary_error() {
                    Some(error) => Err(error),
                    None => (*validated_tuning).and_then(|tuning| self.state.retune(tuning, now)),
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
                cancellation.fail(error);
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
                cancel.fail(error.clone());
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
                        // Releasing the reply wakes its handle, which waits
                        // on liveness and reads the completed lifecycle
                        // snapshot. Retaining no replies keeps this drain
                        // bounded even under concurrent control traffic.
                        drop(reply);
                        continue;
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

    /// Publish only after counting every drained boundary and diagnostic.
    /// Queued metrics wait at the teardown barrier and never count as dropped
    /// work; no per-observation state survives the drain.
    fn publish_metrics(&self) {
        let _ = self
            .lifecycle
            .publish_metrics(self.state.metrics_snapshot(0));
    }

    #[cfg(all(test, any(feature = "runtime-tokio", feature = "runtime-smol")))]
    fn snapshot(&self) -> OwnerSnapshot {
        OwnerSnapshot {
            #[cfg(feature = "runtime-tokio")]
            metrics: self.state.metrics(),
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
        self.publish_metrics();
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
    use crate::target::TargetSlot;

    /// A datagram driver: it never retains stream input.
    struct Datagrams;

    impl RetainedStreamInput for Datagrams {}

    fn owner() -> (OwnerHandleCore, OwnerShellCore) {
        owner_with_capacity(1)
    }

    fn owner_with_capacity(capacity: usize) -> (OwnerHandleCore, OwnerShellCore) {
        let policy = OwnerPolicy::single_target(
            ProtocolPolicy {
                capacity,
                wire: crate::runtime::engine::WireProtocol::new(
                    EnvelopeKind::Raw,
                    crate::transport::AddressingMode::Ip,
                ),
                transport: crate::runtime::engine::TransportKind::Datagram,
                inquiry_capacity: 1,
                command_spacing: Duration::ZERO,
                inquiry_spacing: Duration::ZERO,
                inquiry_cooldown: Duration::ZERO,
                raw_inquiry_release_hold: Duration::from_secs(1),
                raw_release_grace: Duration::from_millis(100),
                strict_unconfirmed_poison: false,
            },
            TargetSlot::CAMERA_1,
            TargetPolicy {
                command_sockets: 2,
                cancellation: CancellationPolicy::Supported,
                control_reserve: 0,
            },
        )
        .unwrap();
        let state = OwnerState::new(policy, Instant::now()).unwrap();
        let (handle, ends) = OwnerHandleCore::new(&state);
        (handle, OwnerShellCore::new(state, ends))
    }

    fn receive(result: OwnerReceive, received_at: Instant) -> OwnerEvent {
        OwnerEvent::Receive {
            result,
            received_at,
        }
    }

    /// Runs one turn to its end, as a shell would, with writes that never
    /// happen: none of these turns stage a transmit.
    fn run_turn(core: &mut OwnerShellCore, mut step: TurnStep) -> TurnStep {
        while let TurnStep::Drive { mut effects, then } = step {
            assert!(core.next_transmit(&mut effects, Instant::now()).is_none());
            step = core.resume(then, &mut Datagrams);
        }
        step
    }

    fn declared_request(stop: bool) -> crate::runtime::engine::RuntimeRequest {
        use crate::runtime::engine::*;
        RuntimeRequest::Command {
            reply_shape: ReplyShape::AckThenCompletion,
            wire: Arc::new(EncodedMessage::new(&[0x81, 0x01, 0x04, 0x07, 0x00, 0xff]).unwrap()),
            context: RequestContext {
                requires_extended_pan_tilt_speed: false,
                motion: Some(MotionEffect {
                    axes: crate::AffectedAxes::ZOOM,
                    stop,
                }),
                submission_order: 0,
                dispatch_deadline: None,
                target: TargetSlot::CAMERA_1,
                timeout: TimeoutPolicy {
                    ack: NonZeroDuration::new(Duration::from_secs(1)).unwrap(),
                    completion: NonZeroDuration::new(Duration::from_secs(1)).unwrap(),
                    inquiry: NonZeroDuration::new(Duration::from_secs(1)).unwrap(),
                    cancellation: NonZeroDuration::new(Duration::from_secs(1)).unwrap(),
                    ambiguity: NonZeroDuration::new(Duration::from_secs(1)).unwrap(),
                },
                retry: RetryPolicy::NEVER,
                control: ControlPolicy {
                    class: if stop {
                        ControlClass::Urgent
                    } else {
                        ControlClass::Normal
                    },
                    ..ControlPolicy::default()
                },
                cancellation: CancellationPolicy::Supported,
            },
            applied_state: None,
        }
    }

    #[test]
    fn concluded_metrics_include_drained_work_and_answer_queued_observations() {
        let (handle, mut core) = owner();
        let (_observer, admission) = handle
            .enqueue_admission::<crate::runtime::engine::CommandOutcome>(
                declared_request(false),
                None,
            )
            .unwrap();
        let (reply, metrics) = flume::bounded(1);
        handle
            .control
            .send(ControlBoundary::Metrics(reply))
            .unwrap();
        handle.shutdown().unwrap();
        let step = core.handle(OwnerEvent::Shutdown, &mut Datagrams, Instant::now(), false);
        run_turn(&mut core, step);
        assert!(
            handle.terminal_metrics().is_none(),
            "the normal drain is not finished yet"
        );
        core.conclude();
        assert!(
            metrics.recv().is_err(),
            "the handle resumes at the teardown barrier"
        );
        let snapshot = handle.terminal_metrics().unwrap().unwrap();
        assert_eq!(snapshot.session, crate::SessionStatus::Shutdown);
        assert_eq!(snapshot.active, 0);
        assert_eq!(snapshot.pending, 0);
        assert_eq!(
            snapshot.dropped_boundary_work, 1,
            "only the admission is dropped"
        );
        assert!(matches!(
            admission.recv().unwrap(),
            Err(Error::RuntimeShutdown)
        ));
        assert_eq!(handle.terminal_metrics().unwrap().unwrap(), snapshot);
        drop(core);
        for _ in 0..8 {
            assert_eq!(handle.terminal_metrics().unwrap().unwrap(), snapshot);
        }
    }

    #[test]
    fn repeated_terminal_control_drains_retain_no_metrics_replies() {
        let (handle, mut core) = owner();
        let step = core.handle(OwnerEvent::Shutdown, &mut Datagrams, Instant::now(), false);
        run_turn(&mut core, step);
        let capacity = handle.control.capacity().unwrap();
        let mut acting = 0;
        for _ in 0..64 {
            let mut metrics = Vec::new();
            let mut subscriptions = Vec::new();
            for index in 0..capacity {
                if index % 2 == 0 {
                    let (reply, receiver) = flume::bounded(1);
                    handle
                        .control
                        .send(ControlBoundary::Metrics(reply))
                        .unwrap();
                    metrics.push(receiver);
                } else {
                    let (reply, receiver) = flume::bounded(1);
                    handle
                        .control
                        .send(ControlBoundary::SubscribeDiagnostics { capacity: 1, reply })
                        .unwrap();
                    subscriptions.push(receiver);
                    acting += 1;
                }
            }
            core.drain_boundaries(Error::RuntimeShutdown);
            // Every reply is released in this pass rather than accumulating
            // across passes until conclusion. Handles wait at liveness.
            assert!(metrics.iter().all(flume::Receiver::is_disconnected));
            for receiver in subscriptions {
                assert!(matches!(
                    receiver.recv().unwrap(),
                    Err(Error::RuntimeShutdown)
                ));
            }
        }
        core.conclude();
        let metrics = handle.terminal_metrics().unwrap().unwrap();
        assert_eq!(metrics.dropped_boundary_work, acting);
    }

    #[test]
    fn teardown_before_conclude_keeps_metrics_unavailable_even_after_a_terminal_input() {
        let (handle, mut core) = owner();
        let step = core.handle(
            receive(
                OwnerReceive::Fatal(super::super::ReceiveFatal::TransportClosed { cause: None }),
                Instant::now(),
            ),
            &mut Datagrams,
            Instant::now(),
            false,
        );
        run_turn(&mut core, step);
        assert!(matches!(
            handle.disconnected_error(),
            Error::ConnectionClosed { .. }
        ));
        drop(core);
        assert!(matches!(
            handle.terminal_metrics(),
            Some(Err(Error::OwnerTerminated))
        ));
        assert!(matches!(
            handle.disconnected_error(),
            Error::ConnectionClosed { .. }
        ));
    }

    #[test]
    fn staged_halt_stop_cannot_cross_shell_write_deadline_or_reappear_after_expiry() {
        let now = Instant::now();
        let deadline = now + Duration::from_millis(10);
        let (handle, mut core) = owner();
        let mut request = declared_request(true);
        request.context_mut().dispatch_deadline = Some(deadline);
        request.context_mut().retry.total_budget =
            Some(crate::runtime::engine::NonZeroDuration::new(Duration::from_millis(10)).unwrap());
        let (completion, admission) = handle
            .enqueue_admission::<super::super::RuntimeOutcome>(request, None)
            .unwrap();
        let boundary = core.receivers.admissions.recv().unwrap();
        let TurnStep::Drive { mut effects, .. } = core.handle(
            OwnerEvent::Admission(Ok(boundary)),
            &mut Datagrams,
            now,
            false,
        ) else {
            panic!("admission drives effects");
        };
        // Time may pass before the immediate write starts, but due work never
        // runs between staging it and this explicit pre-write expiry check.
        assert!(core.next_transmit(&mut effects, deadline).is_none());
        assert!(admission.recv().unwrap().is_ok());
        assert!(matches!(
            completion.try_observed().map(|event| event.value),
            Some(super::super::RuntimeOutcome::Command(
                super::super::CommandOutcome::Failed(_)
            ))
        ));
        assert!(completion.try_observed().is_none(), "exactly one terminal");
        assert_eq!(handle.permits.available(), 1);
        assert_eq!(core.state.state(), SessionState::Running);
    }

    #[test]
    fn halt_acceptance_fences_all_prior_channel_work_and_allows_later_admission() {
        use super::super::{boundary::AdmissionValidity, halt::HaltBoundary};
        let now = Instant::now();
        let deadline = now + Duration::from_secs(1);
        let (handle, mut core) = owner_with_capacity(4);
        let (_first, first) = handle
            .enqueue_admission::<super::super::RuntimeOutcome>(declared_request(false), None)
            .unwrap();
        let (reply, receipt) = flume::bounded(1);
        handle
            .cancellations
            .send(CancellationBoundary::Halt(HaltBoundary {
                prepared: crate::prepared::PreparedHalt {
                    target: TargetSlot::CAMERA_1,
                    axes: Some(crate::AffectedAxes::ZOOM),
                    requests: [None, Some(Ok(declared_request(true))), None],
                    budget: Duration::from_secs(1),
                },
                validity: AdmissionValidity::until(deadline),
                reply,
            }))
            .unwrap();
        // This arrives after halt enqueue but before owner acceptance. It
        // must belong to the same fence as the first channel-retained move.
        let (_second, second) = handle
            .enqueue_admission::<super::super::RuntimeOutcome>(declared_request(false), None)
            .unwrap();
        let halt = core.receivers.cancellations.recv().unwrap();
        let mut step = core.handle(OwnerEvent::Cancellation(halt), &mut Datagrams, now, false);
        while let TurnStep::Drive { mut effects, then } = step {
            while let Some(staged) = core.next_transmit(&mut effects, now) {
                core.finish_transmit(
                    None,
                    &staged,
                    WriteOutcome::Sent(TransmissionMeta::Raw),
                    now,
                    &mut effects,
                );
            }
            step = core.resume(then, &mut Datagrams);
        }
        let halt = receipt.recv().unwrap().unwrap();
        assert!(matches!(halt.slots[1], Some(Ok(_))));
        for reply in [first, second] {
            let boundary = core.receivers.admissions.recv().unwrap();
            let step = core.handle(
                OwnerEvent::Admission(Ok(boundary)),
                &mut Datagrams,
                now,
                false,
            );
            let _ = run_turn(&mut core, step);
            assert!(matches!(
                reply.recv().unwrap(),
                Err(Error::MotionSuperseded { .. })
            ));
        }
        let (_later, later) = handle
            .enqueue_admission::<super::super::RuntimeOutcome>(declared_request(false), None)
            .unwrap();
        let boundary = core.receivers.admissions.recv().unwrap();
        let step = core.handle(
            OwnerEvent::Admission(Ok(boundary)),
            &mut Datagrams,
            now,
            false,
        );
        let _ = run_turn(&mut core, step);
        assert!(later.recv().unwrap().is_ok());
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
            receive(
                OwnerReceive::Fatal(crate::runtime::owner::ReceiveFatal::TransportClosed {
                    cause: None,
                }),
                Instant::now(),
            ),
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
                receive(
                    OwnerReceive::NoProgress(crate::runtime::owner::NoProgressReason::EagerIdle),
                    now,
                ),
                &mut Datagrams,
                now,
                false,
            );
            let TurnStep::Pace { pause, outcome } = step else {
                panic!("an idle receive pauses the receive source: {step:?}");
            };
            assert_eq!(pause, ReceivePause::Idle);
            assert_eq!(outcome, TurnOutcome::YieldBoundaries);
            assert_eq!(core.receive_pause(pause, now), expected);
        }

        // A byte-bearing receive, even one that completed no frame, restarts
        // the idle run.
        let step = core.handle(
            receive(
                OwnerReceive::Frames(crate::runtime::owner::FrameBatch::new()),
                now,
            ),
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
    fn a_transient_fault_arms_its_pause_before_effects_are_driven() {
        let (_handle, mut core) = owner();
        let start = Instant::now();
        for (offset, expected) in [
            (Duration::ZERO, Duration::from_millis(10)),
            (Duration::from_millis(1), Duration::from_millis(20)),
        ] {
            let at = start + offset;
            let fault = Error::transport_error(std::io::Error::other("transient receive fault"));
            let step = core.handle(
                receive(OwnerReceive::read_error(fault), at),
                &mut Datagrams,
                at,
                false,
            );
            assert!(matches!(step, TurnStep::Drive { .. }));
            assert_eq!(
                core.coordinator.receive_not_before(),
                Some(at + expected),
                "the fault pause exists before any write can clear it"
            );
            assert!(matches!(
                run_turn(&mut core, step),
                TurnStep::Done(TurnOutcome::YieldBoundaries)
            ));
        }
        assert!(core.is_running());
    }

    #[test]
    fn a_full_idle_timeout_needs_no_additional_pause() {
        let (_handle, mut core) = owner();
        let now = Instant::now();
        let step = core.handle(
            receive(
                OwnerReceive::NoProgress(crate::runtime::owner::NoProgressReason::IdleTimeout),
                now,
            ),
            &mut Datagrams,
            now,
            false,
        );
        assert!(matches!(step, TurnStep::Done(TurnOutcome::Continue)));
        assert_eq!(core.idle_receives.length(), 0);
        assert_eq!(core.coordinator.receive_not_before(), None);
    }

    #[test]
    fn a_successful_write_resets_idle_pacing_and_ends_a_fault_pause() {
        let (handle, mut core) = owner();
        let now = Instant::now();
        for _ in 0..5 {
            core.idle_receives.record();
        }
        let (_completion, admission) = handle
            .enqueue_admission::<super::super::RuntimeOutcome>(declared_request(false), None)
            .unwrap();
        let boundary = core.receivers.admissions.recv().unwrap();
        let TurnStep::Drive { mut effects, .. } = core.handle(
            OwnerEvent::Admission(Ok(boundary)),
            &mut Datagrams,
            now,
            false,
        ) else {
            panic!("admission drives a write")
        };
        core.pace_receive(ReceivePause::Fault { run: 1 }, now);
        let staged = core.next_transmit(&mut effects, now).unwrap();
        assert!(core.coordinator.receive_not_before().is_some());
        core.finish_transmit(
            None,
            &staged,
            WriteOutcome::Sent(TransmissionMeta::Raw),
            now,
            &mut effects,
        );
        assert!(admission.recv().unwrap().is_ok());
        assert_eq!(core.coordinator.receive_not_before(), None);
        assert_eq!(core.idle_receives.length(), 0);
        assert_eq!(
            core.receive_pause(ReceivePause::Idle, now),
            Duration::from_millis(10)
        );
    }

    #[test]
    fn an_undecodable_datagram_is_discarded_and_the_session_survives() {
        let (_handle, mut core) = owner();
        let now = Instant::now();
        let step = core.handle(
            receive(
                OwnerReceive::NoProgress(super::super::NoProgressReason::MalformedDatagram),
                now,
            ),
            &mut Datagrams,
            now,
            false,
        );
        assert!(matches!(step, TurnStep::Done(TurnOutcome::YieldBoundaries)));
        assert!(core.is_running());
    }
}
