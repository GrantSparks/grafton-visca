//! Executor-free owner source arbitration (#776, decision D25).
//!
//! [`OwnerCoordinator`] owns every *decision* an owner turn makes about which
//! source it may select and what a selected event is allowed to do:
//!
//! - the receive-versus-boundary [`SourcePhase`], the receive fairness ceiling
//!   and the forced boundary turn it implies;
//! - the one-turn control allowance granted to an already-due timer;
//! - the pause that paces the next read after an idle or faulted receive,
//!   during which every other source stays selectable (#675, #780);
//! - the receive-first raw-correlation release proof ([`RawReleaseTurn`]);
//! - the single retained-boundary slot that holds an admission or cancellation
//!   selected while a raw release was due (#775), and the eligibility rule
//!   that follows from it.
//!
//! Both owners drive it: the async actor and the blocking worker thread
//! (D24, #780). A shell supplies only I/O: it samples the clock, asks
//! [`plan`] for a [`TurnPlan`], polls its sources in the order
//! [`Selection::order`] gives, reports what was selected through [`classify`],
//! and reports the turn's result through [`finish`]. Nothing here touches a
//! channel, a transport, an executor or the wall clock, so every decision can
//! be exercised by plain unit and generated-sequence tests.
//!
//! The boundary payload type `B` is opaque: the coordinator stores and returns
//! it but never inspects it.
//!
//! [`plan`]: OwnerCoordinator::plan
//! [`classify`]: OwnerCoordinator::classify
//! [`finish`]: OwnerCoordinator::finish

use std::time::Instant;

use super::RawReleaseTurn;
use crate::runtime::engine::RawCorrelationReleaseSet;

/// Whether one owner turn keeps the session alive, and which source should
/// lead the next selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::runtime::owner) enum TurnOutcome {
    /// Keep running with protocol input first.
    Continue,
    /// Keep protocol input first because the stream framer still retains
    /// ordered input. Buffered work still counts toward the ordinary receive
    /// fairness ceiling: an adversarial stream can alternate buffered and
    /// transport-backed batches forever.
    ContinueBuffered,
    /// This receive turn made no protocol progress, so poll the ordered
    /// boundary sources first on the next selection after one cooperative
    /// handoff.
    YieldBoundaries,
    /// The session is over.
    Stop,
}

impl TurnOutcome {
    pub(in crate::runtime::owner) const fn next_source_phase(self) -> Option<SourcePhase> {
        match self {
            Self::Continue | Self::ContinueBuffered => Some(SourcePhase::ReceiveFirst),
            Self::YieldBoundaries => Some(SourcePhase::BoundariesFirst),
            Self::Stop => None,
        }
    }
}

/// Which source wins when receive and a boundary source are ready at once.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::runtime::owner) enum SourcePhase {
    /// Poll meaningful protocol input before a simultaneous boundary.
    #[default]
    ReceiveFirst,
    /// After a non-progressing receive, give the ordered boundary sources first
    /// refusal.
    BoundariesFirst,
}

/// What the receive source contributes to this selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::runtime::owner) enum ReceiveArm {
    /// Poll the transport (or retained framer input).
    Poll,
    /// Poll the transport, but not before `until`: the previous receive was
    /// idle or a transient fault, and its paced pause has not elapsed. Every
    /// other source stays selectable meanwhile, so a pause never delays a
    /// boundary (#675). A pause is clamped to the next engine or grace
    /// deadline, so it never defers a receive past a timer it must precede,
    /// and a write ends it, so it never delays the reply the write invites.
    Paced { until: Instant },
    /// An exact no-input probe already completed for the latched release set:
    /// the receive source is immediately ready with a timer event instead of
    /// another read.
    ProofComplete,
}

/// What the timer source waits for in this selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::runtime::owner) enum TimerArm {
    /// The engine's next wake. `due` records that it had already elapsed when
    /// the turn was planned, so a shell must not rely on a zero-length sleep.
    Engine { at: Option<Instant>, due: bool },
    /// An engine-owned retained-prefix grace deadline (#713).
    Grace { until: Instant },
}

/// One planned selection.
///
/// [`order`](Self::order) is the single definition of which sources may be
/// selected and which wins when several are ready at once; the async actor
/// polls its sources in exactly this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::runtime::owner) struct Selection {
    pub(in crate::runtime::owner) phase: SourcePhase,
    pub(in crate::runtime::owner) receive: ReceiveArm,
    pub(in crate::runtime::owner) timer: TimerArm,
    /// Whether cancellation and admission may be selected. They are
    /// ineligible exactly while the retained-boundary slot is occupied, so a
    /// second boundary is never dequeued with nowhere to keep it (#775).
    pub(in crate::runtime::owner) boundaries_eligible: bool,
    /// A due timer has already granted one control observation, so it now
    /// precedes control.
    pub(in crate::runtime::owner) timer_precedes_control: bool,
}

/// One owner event source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::runtime::owner) enum Source {
    Receive,
    Shutdown,
    Cancellation,
    Admission,
    Control,
    Timer,
}

impl Selection {
    /// The sources this selection may pick, in left-biased all-ready order.
    ///
    /// Shutdown, cancellation and admission keep a fixed order, followed by
    /// control and the timer (the timer first once it has granted control its
    /// one turn). Receive precedes all of them in [`SourcePhase::ReceiveFirst`]
    /// and follows them in [`SourcePhase::BoundariesFirst`]. Cancellation and
    /// admission are omitted while a boundary is retained.
    pub(in crate::runtime::owner) fn order(&self) -> impl Iterator<Item = Source> + Clone {
        let receive_first = self.phase == SourcePhase::ReceiveFirst;
        let eligible = self.boundaries_eligible;
        let tail = if self.timer_precedes_control {
            [Source::Timer, Source::Control]
        } else {
            [Source::Control, Source::Timer]
        };
        receive_first
            .then_some(Source::Receive)
            .into_iter()
            .chain([Source::Shutdown])
            .chain(
                [Source::Cancellation, Source::Admission]
                    .into_iter()
                    .filter(move |_| eligible),
            )
            .chain(tail)
            .chain((!receive_first).then_some(Source::Receive))
    }
}

/// The plan for one owner turn.
#[derive(Debug)]
pub(in crate::runtime::owner) enum TurnPlan<B> {
    /// No raw release is due any more, so the retained boundary is delivered
    /// before any source is polled. The shell must still give an already
    /// pending shutdown precedence (and hand the boundary back with
    /// [`OwnerCoordinator::restore`] if it does).
    Redeliver(B),
    /// Select one source as described.
    Select(Selection),
}

/// What kind of event a shell selected, as far as arbitration is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::runtime::owner) enum Selected {
    /// The timer (engine wake, grace deadline, or completed no-input proof).
    Timer,
    /// A live admission or cancellation payload.
    Boundary,
    /// A receive result.
    Receive {
        /// The raw release set due when the read completed. Only this instant,
        /// not a later executor resume, proves that input was sampled at or
        /// after a hold deadline.
        due_at_receive: RawCorrelationReleaseSet,
        class: ReceiveClass,
    },
    /// Shutdown, control, or a disconnected admission lane.
    Other,
}

/// How a receive result bears on a raw-release input proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::runtime::owner) enum ReceiveClass {
    /// The read consumed nothing: an idle no-data result, or an idle fault
    /// normalized to one. This is exact no-input evidence.
    NoInput,
    /// A genuine transient fault. It cannot prove that no stale frame is ready
    /// behind it.
    TransientFault,
    /// Anything else (frames, close, a permanent fault).
    Other,
}

/// What the shell must do with a selected event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::runtime::owner) enum Disposition {
    /// A timer selected from a view that predates a raw hold expiry: it has not
    /// earned the receive-first release pass. Discard it and plan again.
    Restart,
    /// A boundary selected while a raw release is due must not enter its
    /// engine turn yet. The shell answers it if it can (a lost admission claim,
    /// a cancellation whose terminal result is already buffered) and otherwise
    /// hands it to [`OwnerCoordinator::retain`]; either way it then plans again.
    Retain,
    /// Run the event's engine turn. `suppress_due` is set when a raw release
    /// probe hit a transient fault, so that turn must not run due work.
    Apply { suppress_due: bool },
}

/// Facts sampled by [`OwnerCoordinator::plan`] and used again after selection.
#[derive(Debug, Clone, Copy, Default)]
struct Planned {
    wake: Option<Instant>,
    releases: RawCorrelationReleaseSet,
    release_pending: bool,
    release_fenced: bool,
}

/// Executor-free source arbitration for one owner. See the module docs.
#[derive(Debug)]
pub(in crate::runtime::owner) struct OwnerCoordinator<B> {
    phase: SourcePhase,
    receive_first_streak: usize,
    fairness_ceiling: usize,
    forced: bool,
    /// The due wake that already granted its one control observation. Keyed
    /// by the exact deadline: a boundary can replace a wake, and a timer can
    /// become due while a selection is parked.
    control_allowance_consumed_for: Option<Instant>,
    planned: Planned,
    release: RawReleaseTurn,
    retained: Option<B>,
    /// The instant before which receive must not be polled again.
    receive_not_before: Option<Instant>,
}

impl<B> Default for OwnerCoordinator<B> {
    fn default() -> Self {
        Self {
            phase: SourcePhase::ReceiveFirst,
            receive_first_streak: 0,
            fairness_ceiling: 1,
            forced: false,
            control_allowance_consumed_for: None,
            planned: Planned::default(),
            release: RawReleaseTurn::default(),
            retained: None,
            receive_not_before: None,
        }
    }
}

impl<B> OwnerCoordinator<B> {
    /// Set the number of consecutive receive-first wins after which one
    /// boundary-first turn is forced (#675). Zero is treated as one.
    pub(in crate::runtime::owner) fn set_fairness_ceiling(&mut self, ceiling: usize) {
        self.fairness_ceiling = ceiling.max(1);
    }

    /// Start a turn. Returns whether the shell must make one cooperative
    /// handoff before sampling the clock: polling boundaries first is not by
    /// itself a handoff when they are all pending, so every boundary-first
    /// turn (forced or yielded) gets one.
    pub(in crate::runtime::owner) fn begin_turn(&mut self) -> bool {
        let yielded = self.phase == SourcePhase::BoundariesFirst;
        self.forced = self.phase == SourcePhase::ReceiveFirst
            && self.receive_first_streak >= self.fairness_ceiling;
        if self.forced {
            self.receive_first_streak = 0;
        }
        self.forced || yielded
    }

    /// Plan the selection for this turn.
    ///
    /// `now` must be sampled after the handoff requested by
    /// [`begin_turn`](Self::begin_turn), so a timer that became due meanwhile
    /// does not inherit a stale positive delay. `complete_frame_buffered` is
    /// consulted only on a forced turn while a raw release is pending: a
    /// complete stream frame already held by the framer is arrived protocol
    /// input that must be decoded before the release, so it keeps receive
    /// first.
    pub(in crate::runtime::owner) fn plan(
        &mut self,
        now: Instant,
        next_wake: Option<Instant>,
        releases_due_now: RawCorrelationReleaseSet,
        complete_frame_buffered: impl FnOnce() -> bool,
    ) -> TurnPlan<B> {
        let wake_due = next_wake.is_some_and(|wake| wake <= now);
        let _ = self.release.observe(releases_due_now);
        let releases = self.release.latched().unwrap_or_default();
        let release_pending = self.release.is_pending();
        let release_fenced = self.release.is_fenced(releases);
        self.planned = Planned {
            wake: next_wake,
            releases,
            release_pending,
            release_fenced,
        };
        // Do not carry a consumed allowance over to an absent, future, or
        // replaced timer, including a replacement that is already due.
        if !wake_due || self.control_allowance_consumed_for != next_wake {
            self.control_allowance_consumed_for = None;
        }
        let timer_precedes_control = wake_due
            && self
                .control_allowance_consumed_for
                .is_some_and(|consumed| next_wake == Some(consumed));
        let engine_timer = TimerArm::Engine {
            at: next_wake,
            due: wake_due,
        };
        let phase = if self.forced {
            SourcePhase::BoundariesFirst
        } else {
            self.phase
        };
        let boundaries_eligible = self.retained.is_none();
        self.receive_not_before = self.receive_not_before.filter(|until| *until > now);
        let poll = match self.receive_not_before {
            Some(until) => ReceiveArm::Paced { until },
            None => ReceiveArm::Poll,
        };
        let selection = |phase, receive, timer| Selection {
            phase,
            receive,
            timer,
            boundaries_eligible,
            timer_precedes_control,
        };

        if !release_pending {
            if let Some(retained) = self.retained.take() {
                return TurnPlan::Redeliver(retained);
            }
            return TurnPlan::Select(selection(phase, poll, engine_timer));
        }

        let complete_frame_buffered = self.forced && complete_frame_buffered();
        let grace = self.release.await_until();
        let release_phase = if grace.is_some_and(|deadline| deadline <= now) {
            // Once grace has elapsed, its timer leads an eagerly ready receive
            // so no further zero-time turn can delay it.
            SourcePhase::BoundariesFirst
        } else if self.forced && !complete_frame_buffered {
            // The fairness ceiling stays authoritative: a forced turn puts
            // every boundary source and the release timer ahead of a receive
            // flood. That timer turn is what resolves retained input and starts
            // a grace budget, so it guarantees the release makes progress.
            SourcePhase::BoundariesFirst
        } else {
            // A newly due release still needs its first ordered receive proof.
            // A boundary-first phase inherited from a pre-H idle read cannot
            // certify it: stale input may have arrived while that read was
            // parked.
            SourcePhase::ReceiveFirst
        };
        let plan = match (release_fenced, grace) {
            // The exact no-input probe already proved the latched set: the
            // receive source delivers the timer event without another read.
            (true, None) => selection(phase, ReceiveArm::ProofComplete, engine_timer),
            // A retained prefix owns a real time budget (#713). Even a fenced
            // set keeps polling receive, since its tail may still arrive.
            (_, Some(until)) => selection(release_phase, poll, TimerArm::Grace { until }),
            // Before due work can release correlation or dispatch, poll
            // receive under the same boundary order as every other turn.
            (false, None) => selection(release_phase, poll, engine_timer),
        };
        TurnPlan::Select(plan)
    }

    /// Defer the next receive until `until`, after an idle or transient-fault
    /// receive. Selection keeps every other source live during the pause.
    pub(in crate::runtime::owner) fn pace_receive(&mut self, until: Instant) {
        self.receive_not_before = Some(until);
    }

    /// End a pending receive pause because the owner just wrote. Pacing only
    /// keeps an idle transport from spinning the owner; a write invites a
    /// reply, which must be read as soon as it arrives rather than after a
    /// pause chosen before the write, possibly past the reply's own deadline.
    pub(in crate::runtime::owner) fn end_receive_pause(&mut self) {
        self.receive_not_before = None;
    }

    /// Hand a boundary returned by [`TurnPlan::Redeliver`] back because an
    /// already pending shutdown takes precedence over it.
    pub(in crate::runtime::owner) fn restore(&mut self, boundary: B) {
        debug_assert!(self.retained.is_none());
        self.retained = Some(boundary);
    }

    /// Decide what the selected event may do.
    ///
    /// `due_after_selection` is the raw release set due at the instant the
    /// selection completed; that one instant is carried through the event's
    /// engine turn.
    pub(in crate::runtime::owner) fn classify(
        &mut self,
        selected: Selected,
        due_after_selection: RawCorrelationReleaseSet,
    ) -> Disposition {
        let planned = self.planned;
        if !due_after_selection.is_empty() {
            match selected {
                // A raw deadline matured while an ordinary selection was
                // parked. This timer was selected from the pre-expiry view and
                // has not earned the receive-first release pass.
                Selected::Timer if !planned.release_pending => {
                    self.phase = SourcePhase::ReceiveFirst;
                    return Disposition::Restart;
                }
                // A selected boundary has not touched engine state yet. Its
                // ordinary input turn would advance due work behind unread or
                // retained evidence, so it waits for the release.
                Selected::Boundary => return Disposition::Retain,
                _ => {}
            }
        }

        let (due_at_receive, class) = match selected {
            Selected::Receive {
                due_at_receive,
                class,
            } => (due_at_receive, Some(class)),
            _ => (RawCorrelationReleaseSet::default(), None),
        };
        // The receive probes the latched set, or crossed into a hold that
        // matured while it was parked. Only the first can be fenced: a set
        // that was not latched at plan time is latched afresh by the next plan,
        // which starts its own proof, so a crossing no-input read records
        // nothing. A crossing transient fault still suppresses due work.
        let probes_latched = planned.release_pending && !planned.release_fenced;
        let crossed_into_release = !planned.release_pending && !due_at_receive.is_empty();
        if probes_latched && class == Some(ReceiveClass::NoInput) {
            self.release.fence_no_input(planned.releases);
        }
        let probe = probes_latched || crossed_into_release;
        if selected == Selected::Timer {
            // A timer either advances the release or defers it behind retained
            // framing; either way the next turn needs a fresh probe.
            self.release.clear_fence();
        }
        Disposition::Apply {
            suppress_due: probe && class == Some(ReceiveClass::TransientFault),
        }
    }

    /// Keep a boundary that [`classify`](Self::classify) returned
    /// [`Disposition::Retain`] for, until the release resolves.
    ///
    /// The slot holds one boundary. Selection makes cancellation and admission
    /// ineligible while it is occupied, so an occupied slot here is a broken
    /// invariant; the boundary is then handed back rather than overwriting the
    /// retained one, and the shell must answer it itself.
    pub(in crate::runtime::owner) fn retain(&mut self, boundary: B) -> Result<(), B> {
        if self.retained.is_some() {
            debug_assert!(false, "a boundary was selected while the slot was occupied");
            return Err(boundary);
        }
        self.retained = Some(boundary);
        Ok(())
    }

    /// Record that a [`Disposition::Retain`] boundary was retained or answered:
    /// plan again with receive first. The receive fairness streak is
    /// deliberately left alone, since the boundary is invisible to scheduling
    /// until the release resolves.
    pub(in crate::runtime::owner) fn restart_after_retain(&mut self) {
        self.phase = SourcePhase::ReceiveFirst;
    }

    /// If control won while the planned wake was due, the wake that control
    /// consumed. `current_wake` and `observed_at` are re-sampled after the
    /// selection: a timer can mature while the tail is parked, and the
    /// left-biased control source can still win that poll.
    pub(in crate::runtime::owner) fn control_consumed_wake(
        &self,
        current_wake: Option<Instant>,
        observed_at: Instant,
    ) -> Option<Instant> {
        current_wake
            .filter(|current| Some(*current) == self.planned.wake)
            .filter(|wake| *wake <= observed_at)
    }

    /// Account for a finished turn. Returns `false` once the session is over.
    ///
    /// Every receive that keeps the session running lengthens the receive-first
    /// streak; any other event resets it.
    pub(in crate::runtime::owner) fn finish(
        &mut self,
        selected: Selected,
        control_consumed_wake: Option<Instant>,
        outcome: TurnOutcome,
    ) -> bool {
        if selected == Selected::Timer {
            self.control_allowance_consumed_for = None;
        } else if let Some(wake) = control_consumed_wake {
            self.control_allowance_consumed_for = Some(wake);
        }
        let receive = matches!(selected, Selected::Receive { .. });
        self.receive_first_streak = match outcome {
            TurnOutcome::Continue | TurnOutcome::ContinueBuffered if receive => {
                self.receive_first_streak.saturating_add(1)
            }
            _ => 0,
        };
        match outcome.next_source_phase() {
            Some(next) => {
                self.phase = next;
                true
            }
            None => false,
        }
    }

    /// Take the retained boundary for a terminal drain. Also used by the
    /// shell's `Drop`, so a boundary retained when an owner unwinds is still
    /// answered.
    pub(in crate::runtime::owner) fn take_retained(&mut self) -> Option<B> {
        self.retained.take()
    }

    /// Whether a boundary is retained.
    #[cfg(test)]
    pub(in crate::runtime::owner) const fn has_retained(&self) -> bool {
        self.retained.is_some()
    }

    /// The raw-release proof, read-only.
    #[cfg(test)]
    pub(in crate::runtime::owner) const fn release(&self) -> &RawReleaseTurn {
        &self.release
    }

    /// The raw-release proof, for the timer and frame handlers that complete,
    /// replace or defer it.
    pub(in crate::runtime::owner) fn release_mut(&mut self) -> &mut RawReleaseTurn {
        &mut self.release
    }
}

#[cfg(test)]
mod tests;
