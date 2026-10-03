//! Executor-free tests for [`OwnerCoordinator`].
//!
//! The unit tests pin each planning and classification branch. The `model`
//! module drives the coordinator through generated bounded event sequences
//! with a tiny deterministic shell and checks conservation, ordering, slot,
//! fairness and liveness invariants after every step (D25).
#![allow(clippy::panic, clippy::unwrap_used)]

use std::time::{Duration, Instant};

use super::*;
use crate::CameraId;

fn set(targets: &[u8]) -> RawCorrelationReleaseSet {
    targets
        .iter()
        .fold(RawCorrelationReleaseSet::default(), |set, target| {
            set.with_terminal_for_test(CameraId::new(*target).unwrap())
        })
}

fn empty() -> RawCorrelationReleaseSet {
    RawCorrelationReleaseSet::default()
}

fn select(plan: TurnPlan<u32>) -> Selection {
    match plan {
        TurnPlan::Select(selection) => selection,
        TurnPlan::Redeliver(boundary) => panic!("unexpected redelivery of {boundary}"),
    }
}

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

#[test]
fn every_boundary_first_turn_requests_one_handoff_and_the_ceiling_forces_one() {
    let mut coordinator = OwnerCoordinator::<u32>::default();
    coordinator.set_fairness_ceiling(2);
    assert!(!coordinator.begin_turn(), "an ordinary receive-first turn");

    let receive = Selected::Receive {
        due_at_receive: empty(),
        class: ReceiveClass::Other,
    };
    for _ in 0..2 {
        assert!(coordinator.finish(receive, None, TurnOutcome::Continue));
    }
    assert!(
        coordinator.begin_turn(),
        "the ceiling forces a boundary turn"
    );
    let now = Instant::now();
    let forced = select(coordinator.plan(now, None, empty(), || false));
    assert_eq!(forced.phase, SourcePhase::BoundariesFirst);

    // The forced turn restarted the streak.
    assert!(coordinator.finish(receive, None, TurnOutcome::Continue));
    assert!(!coordinator.begin_turn());

    // A non-progressing receive yields the next turn to boundaries.
    assert!(coordinator.finish(receive, None, TurnOutcome::YieldBoundaries));
    assert!(coordinator.begin_turn());
    let yielded = select(coordinator.plan(now, None, empty(), || false));
    assert_eq!(yielded.phase, SourcePhase::BoundariesFirst);

    assert!(!coordinator.finish(Selected::Other, None, TurnOutcome::Stop));
}

#[test]
fn a_boundary_turn_resets_the_receive_streak() {
    let mut coordinator = OwnerCoordinator::<u32>::default();
    coordinator.set_fairness_ceiling(2);
    let receive = Selected::Receive {
        due_at_receive: empty(),
        class: ReceiveClass::Other,
    };
    assert!(coordinator.finish(receive, None, TurnOutcome::ContinueBuffered));
    assert!(coordinator.finish(Selected::Boundary, None, TurnOutcome::Continue));
    assert!(coordinator.finish(receive, None, TurnOutcome::Continue));
    assert!(!coordinator.begin_turn(), "one receive since the boundary");
}

#[test]
fn an_ordinary_turn_selects_every_source_with_the_engine_timer() {
    let mut coordinator = OwnerCoordinator::<u32>::default();
    let now = Instant::now();
    let wake = now + ms(5);
    coordinator.begin_turn();
    let selection = select(coordinator.plan(now, Some(wake), empty(), || false));
    assert_eq!(
        selection,
        Selection {
            phase: SourcePhase::ReceiveFirst,
            receive: ReceiveArm::Poll,
            timer: TimerArm::Engine {
                at: Some(wake),
                due: false,
            },
            boundaries_eligible: true,
            timer_precedes_control: false,
        }
    );
    assert_eq!(
        selection.order().collect::<Vec<_>>(),
        [
            Source::Receive,
            Source::Shutdown,
            Source::Cancellation,
            Source::Admission,
            Source::Control,
            Source::Timer,
        ]
    );
}

#[test]
fn a_pending_release_plans_each_proof_state() {
    let now = Instant::now();
    let s1 = set(&[1]);

    // Unfenced, no grace: poll receive first against the engine timer.
    let mut coordinator = OwnerCoordinator::<u32>::default();
    coordinator.begin_turn();
    let probe = select(coordinator.plan(now, Some(now), s1, || false));
    assert_eq!(probe.phase, SourcePhase::ReceiveFirst);
    assert_eq!(probe.receive, ReceiveArm::Poll);
    assert_eq!(
        probe.timer,
        TimerArm::Engine {
            at: Some(now),
            due: true,
        }
    );

    // A no-input probe fences the set; the next turn's proof is complete.
    let no_input = Selected::Receive {
        due_at_receive: s1,
        class: ReceiveClass::NoInput,
    };
    assert_eq!(
        coordinator.classify(no_input, s1),
        Disposition::Apply {
            suppress_due: false,
        }
    );
    assert!(coordinator.finish(no_input, None, TurnOutcome::YieldBoundaries));
    coordinator.begin_turn();
    let proved = select(coordinator.plan(now, Some(now), s1, || false));
    assert_eq!(proved.receive, ReceiveArm::ProofComplete);
    assert_eq!(
        proved.phase,
        SourcePhase::BoundariesFirst,
        "the proof-complete turn keeps the ordinary yielded phase"
    );

    // The timer clears the fence; a retained prefix then waits for its grace.
    assert_eq!(
        coordinator.classify(Selected::Timer, s1),
        Disposition::Apply {
            suppress_due: false,
        }
    );
    let grace = now + ms(100);
    coordinator.release_mut().wait_for_input_until(grace);
    assert!(coordinator.finish(Selected::Timer, None, TurnOutcome::ContinueBuffered));
    coordinator.begin_turn();
    let waiting = select(coordinator.plan(now, Some(now), s1, || false));
    assert_eq!(waiting.phase, SourcePhase::ReceiveFirst);
    assert_eq!(waiting.receive, ReceiveArm::Poll);
    assert_eq!(waiting.timer, TimerArm::Grace { until: grace });

    // Once grace has elapsed, the grace timer leads.
    coordinator.begin_turn();
    let elapsed = select(coordinator.plan(grace, Some(now), s1, || false));
    assert_eq!(elapsed.phase, SourcePhase::BoundariesFirst);
    assert_eq!(elapsed.timer, TimerArm::Grace { until: grace });
    assert_eq!(
        elapsed.order().last(),
        Some(Source::Receive),
        "an elapsed grace timer leads an eager receive"
    );
}

/// The fairness ceiling stays authoritative while a release awaits its input
/// proof: a forced turn puts every boundary source and the release timer ahead
/// of a receive flood. That timer turn is what resolves retained input and
/// starts a grace budget, so it is the release's liveness guarantee.
#[test]
fn a_forced_turn_lets_the_release_timer_lead_a_receive_flood() {
    let now = Instant::now();
    let s1 = set(&[1]);
    let receive = Selected::Receive {
        due_at_receive: empty(),
        class: ReceiveClass::Other,
    };
    for complete_frame in [false, true] {
        let mut coordinator = OwnerCoordinator::<u32>::default();
        coordinator.set_fairness_ceiling(1);
        assert!(coordinator.finish(receive, None, TurnOutcome::ContinueBuffered));
        assert!(coordinator.begin_turn(), "a forced boundary turn");
        let selection = select(coordinator.plan(now, Some(now), s1, || complete_frame));
        if complete_frame {
            // A complete frame already held by the framer is arrived input:
            // decode it before the release.
            assert_eq!(selection.phase, SourcePhase::ReceiveFirst);
        } else {
            assert_eq!(
                selection.order().collect::<Vec<_>>(),
                [
                    Source::Shutdown,
                    Source::Cancellation,
                    Source::Admission,
                    Source::Control,
                    Source::Timer,
                    Source::Receive,
                ]
            );
        }
    }
}

/// A boundary-first phase inherited from an idle read before the hold
/// deadline cannot certify the new release's input proof: stale input may have
/// arrived while that read was parked, so receive goes first.
#[test]
fn a_newly_due_release_overrides_an_inherited_boundary_first_phase() {
    let now = Instant::now();
    let mut coordinator = OwnerCoordinator::<u32>::default();
    let idle = Selected::Receive {
        due_at_receive: empty(),
        class: ReceiveClass::NoInput,
    };
    assert!(coordinator.finish(idle, None, TurnOutcome::YieldBoundaries));
    assert!(coordinator.begin_turn());
    let selection = select(coordinator.plan(now, Some(now), set(&[1]), || false));
    assert_eq!(selection.phase, SourcePhase::ReceiveFirst);
}

#[test]
fn a_timer_selected_before_a_hold_matured_restarts() {
    let now = Instant::now();
    let mut coordinator = OwnerCoordinator::<u32>::default();
    coordinator.begin_turn();
    let _ = select(coordinator.plan(now, Some(now + ms(5)), empty(), || false));
    assert_eq!(
        coordinator.classify(Selected::Timer, set(&[1])),
        Disposition::Restart
    );
    coordinator.begin_turn();
    let probe = select(coordinator.plan(now + ms(5), Some(now + ms(5)), set(&[1]), || false));
    assert_eq!(probe.phase, SourcePhase::ReceiveFirst);
    assert_eq!(probe.receive, ReceiveArm::Poll);
}

#[test]
fn a_transient_fault_probe_suppresses_due_without_fencing() {
    let now = Instant::now();
    let s1 = set(&[1]);
    let mut coordinator = OwnerCoordinator::<u32>::default();
    coordinator.begin_turn();
    let _ = coordinator.plan(now, Some(now), s1, || false);
    let fault = Selected::Receive {
        due_at_receive: s1,
        class: ReceiveClass::TransientFault,
    };
    assert_eq!(
        coordinator.classify(fault, s1),
        Disposition::Apply { suppress_due: true }
    );
    assert!(coordinator.finish(fault, None, TurnOutcome::YieldBoundaries));
    coordinator.begin_turn();
    let selection = select(coordinator.plan(now, Some(now), s1, || false));
    assert_eq!(selection.receive, ReceiveArm::Poll, "no fence was recorded");
}

#[test]
fn a_receive_that_crossed_a_hold_deadline_is_its_probe() {
    let now = Instant::now();
    let s1 = set(&[1]);
    let mut coordinator = OwnerCoordinator::<u32>::default();
    coordinator.begin_turn();
    let _ = coordinator.plan(now, Some(now + ms(5)), empty(), || false);
    let fault = Selected::Receive {
        due_at_receive: s1,
        class: ReceiveClass::TransientFault,
    };
    assert_eq!(
        coordinator.classify(fault, s1),
        Disposition::Apply { suppress_due: true },
        "a fault stamped at or after H suppresses due work"
    );
    let stale = Selected::Receive {
        due_at_receive: empty(),
        class: ReceiveClass::TransientFault,
    };
    assert_eq!(
        coordinator.classify(stale, s1),
        Disposition::Apply {
            suppress_due: false,
        },
        "a read that completed before H is not a probe"
    );
}

#[test]
fn a_crossing_no_input_read_leaves_the_new_set_unproved() {
    let now = Instant::now();
    let s1 = set(&[1]);
    let mut coordinator = OwnerCoordinator::<u32>::default();
    coordinator.begin_turn();
    let _ = coordinator.plan(now, Some(now + ms(5)), empty(), || false);
    let idle = Selected::Receive {
        due_at_receive: s1,
        class: ReceiveClass::NoInput,
    };
    assert_eq!(
        coordinator.classify(idle, s1),
        Disposition::Apply {
            suppress_due: false,
        }
    );
    assert!(coordinator.finish(idle, None, TurnOutcome::YieldBoundaries));
    coordinator.begin_turn();
    let next = select(coordinator.plan(now + ms(5), Some(now + ms(5)), s1, || false));
    assert_eq!(
        next.receive,
        ReceiveArm::Poll,
        "the newly latched set needs a probe"
    );
    assert_eq!(next.phase, SourcePhase::ReceiveFirst);
}

#[test]
fn a_retained_boundary_blocks_boundaries_until_the_release_resolves() {
    let now = Instant::now();
    let s1 = set(&[1]);
    let mut coordinator = OwnerCoordinator::<u32>::default();
    coordinator.begin_turn();
    let _ = coordinator.plan(now, Some(now), s1, || false);
    assert_eq!(
        coordinator.classify(Selected::Boundary, s1),
        Disposition::Retain
    );
    assert_eq!(coordinator.retain(7), Ok(()));
    coordinator.restart_after_retain();
    assert!(coordinator.has_retained());

    coordinator.begin_turn();
    let held = select(coordinator.plan(now, Some(now), s1, || false));
    assert!(!held.boundaries_eligible);
    assert_eq!(
        held.order().collect::<Vec<_>>(),
        [
            Source::Receive,
            Source::Shutdown,
            Source::Control,
            Source::Timer,
        ]
    );

    // The release completes; the next plan redelivers before polling.
    coordinator.release_mut().complete();
    assert!(coordinator.finish(Selected::Timer, None, TurnOutcome::Continue));
    coordinator.begin_turn();
    match coordinator.plan(now, None, empty(), || false) {
        TurnPlan::Redeliver(boundary) => {
            // Shutdown was pending: hand it back for the terminal drain.
            coordinator.restore(boundary);
        }
        TurnPlan::Select(selection) => panic!("expected redelivery, got {selection:?}"),
    }
    assert_eq!(coordinator.take_retained(), Some(7));
    assert_eq!(coordinator.take_retained(), None);
}

#[test]
fn a_due_wake_grants_control_one_turn_per_deadline() {
    let now = Instant::now();
    let wake = now;
    let mut coordinator = OwnerCoordinator::<u32>::default();
    coordinator.begin_turn();
    let first = select(coordinator.plan(now, Some(wake), empty(), || false));
    assert!(!first.timer_precedes_control);
    assert_eq!(
        first.order().collect::<Vec<_>>(),
        [
            Source::Receive,
            Source::Shutdown,
            Source::Cancellation,
            Source::Admission,
            Source::Control,
            Source::Timer,
        ]
    );
    let consumed = coordinator.control_consumed_wake(Some(wake), now);
    assert_eq!(consumed, Some(wake));
    assert!(coordinator.finish(Selected::Other, consumed, TurnOutcome::Continue));

    coordinator.begin_turn();
    let second = select(coordinator.plan(now, Some(wake), empty(), || false));
    assert!(second.timer_precedes_control);
    assert_eq!(second.order().last(), Some(Source::Control));

    // A replaced deadline gets its own allowance.
    coordinator.begin_turn();
    let replaced = select(coordinator.plan(now + ms(1), Some(now + ms(1)), empty(), || false));
    assert!(!replaced.timer_precedes_control);

    // A future wake was not consumed by control.
    assert_eq!(
        coordinator.control_consumed_wake(Some(now + ms(9)), now),
        None
    );
}

#[test]
fn the_timer_winning_clears_the_control_allowance() {
    let now = Instant::now();
    let mut coordinator = OwnerCoordinator::<u32>::default();
    coordinator.begin_turn();
    let _ = coordinator.plan(now, Some(now), empty(), || false);
    assert!(coordinator.finish(Selected::Other, Some(now), TurnOutcome::Continue));
    assert!(coordinator.finish(Selected::Timer, None, TurnOutcome::Continue));
    coordinator.begin_turn();
    let selection = select(coordinator.plan(now, Some(now), empty(), || false));
    assert!(!selection.timer_precedes_control);
}

/// Generated bounded event sequences against a deterministic model shell.
mod model {
    use std::collections::{BTreeMap, VecDeque};

    use proptest::prelude::*;

    use super::*;

    const CEILING: usize = 2;
    const ADMISSION_CAPACITY: usize = 2;
    const GRACE: Duration = Duration::from_millis(100);
    const IDLE_PAUSE: Duration = Duration::from_millis(10);
    const FAULT_LIMIT: u32 = 12;
    /// Consecutive frame or idle receive wins while an eligible boundary is
    /// ready. A progressing receive flood is bounded by the fairness ceiling;
    /// idle probes during a retained-prefix grace by the grace itself, since
    /// each idle read paces by `IDLE_PAUSE`.
    ///
    /// Transient-fault receives are excluded. While a raw release awaits its
    /// input proof, a fault neither proves the release nor lengthens the
    /// fairness streak, so a fault flood keeps receive first until the fault
    /// run becomes permanent and ends the session (`FAULT_LIMIT`). Selecting a
    /// boundary earlier would not help: the release cannot resolve without a
    /// proof, so the boundary could only be retained, after its admission
    /// claim was spent. Pinned by
    /// `a_fault_flood_during_a_pending_release_defers_boundaries`.
    const FAIRNESS_BOUND: usize =
        CEILING + (GRACE.as_millis() / IDLE_PAUSE.as_millis()) as usize + 2;
    /// Consecutive receive wins while a pending release's timer was ready to
    /// be delivered. The fairness ceiling forces a turn that puts the timer
    /// ahead of receive, which resolves retained input and starts a grace
    /// budget. Fault runs are excluded as above.
    const RELEASE_TIMER_BOUND: usize = CEILING;
    const TAIL_TURNS: usize = 400;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) enum Recv {
        Pending,
        NoInput,
        Fault,
        Frames,
        /// Bytes arrive, but the framer still retains a partial prefix.
        Buffered,
    }

    #[derive(Clone, Copy, Debug)]
    pub(super) enum Step {
        Admission,
        ExpiredAdmission,
        Cancellation,
        BufferedCancellation,
        Control,
        HoldNow,
        HoldSoon,
        Receive(Recv),
        Prefix,
        Tick(u64),
        Turn,
    }

    pub(super) const ALPHABET: [Step; 16] = [
        Step::Admission,
        Step::ExpiredAdmission,
        Step::Cancellation,
        Step::BufferedCancellation,
        Step::Control,
        Step::HoldNow,
        Step::HoldSoon,
        Step::Receive(Recv::Pending),
        Step::Receive(Recv::NoInput),
        Step::Receive(Recv::Fault),
        Step::Receive(Recv::Frames),
        Step::Prefix,
        Step::Tick(10),
        Step::Tick(150),
        Step::Receive(Recv::Buffered),
        Step::Turn,
    ];

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Kind {
        Admission,
        Cancellation,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Token {
        id: u32,
        kind: Kind,
        /// An admission whose claim has expired, or a cancellation whose
        /// terminal observation is already buffered: answered when selected.
        answer_early: bool,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Answer {
        Applied,
        Early,
        Drained,
    }

    #[derive(Debug)]
    enum Event {
        Receive(Recv),
        Shutdown,
        Boundary(Token),
        Control,
        Timer,
    }

    struct World {
        now: Instant,
        coordinator: OwnerCoordinator<Token>,
        admissions: VecDeque<Token>,
        cancellations: VecDeque<Token>,
        control: bool,
        shutdown: bool,
        receive: Recv,
        prefix: bool,
        holds: Vec<(Instant, u8)>,
        next_target: u8,
        next_id: u32,
        enqueued: Vec<Token>,
        answers: BTreeMap<u32, Answer>,
        answer_order: Vec<u32>,
        parked: Option<Selection>,
        stopped: bool,
        faults: u32,
        receive_wins_with_boundary_ready: usize,
        receive_wins_with_release_timer_ready: usize,
        control_wins_for_due_wake: Option<(Option<Instant>, usize)>,
    }

    impl World {
        fn new() -> Self {
            let mut coordinator = OwnerCoordinator::default();
            coordinator.set_fairness_ceiling(CEILING);
            Self {
                now: Instant::now(),
                coordinator,
                admissions: VecDeque::new(),
                cancellations: VecDeque::new(),
                control: false,
                shutdown: false,
                receive: Recv::Pending,
                prefix: false,
                holds: Vec::new(),
                next_target: 1,
                next_id: 0,
                enqueued: Vec::new(),
                answers: BTreeMap::new(),
                answer_order: Vec::new(),
                parked: None,
                stopped: false,
                faults: 0,
                receive_wins_with_boundary_ready: 0,
                receive_wins_with_release_timer_ready: 0,
                control_wins_for_due_wake: None,
            }
        }

        fn due(&self, at: Instant) -> RawCorrelationReleaseSet {
            let targets: Vec<u8> = self
                .holds
                .iter()
                .filter(|(deadline, _)| *deadline <= at)
                .map(|(_, target)| *target)
                .collect();
            set(&targets)
        }

        fn next_wake(&self) -> Option<Instant> {
            self.holds.iter().map(|(deadline, _)| *deadline).min()
        }

        fn release_due(&mut self, at: Instant) {
            self.holds.retain(|(deadline, _)| *deadline > at);
        }

        fn enqueue(&mut self, kind: Kind, answer_early: bool) {
            let lane_full = match kind {
                Kind::Admission => self.admissions.len() >= ADMISSION_CAPACITY,
                Kind::Cancellation => !self.cancellations.is_empty(),
            };
            if lane_full || self.stopped {
                return;
            }
            let token = Token {
                id: self.next_id,
                kind,
                answer_early,
            };
            self.next_id += 1;
            self.enqueued.push(token);
            match kind {
                Kind::Admission => self.admissions.push_back(token),
                Kind::Cancellation => self.cancellations.push_back(token),
            }
        }

        fn answer(&mut self, token: Token, answer: Answer) {
            assert!(
                self.answers.insert(token.id, answer).is_none(),
                "token {} answered twice",
                token.id
            );
            self.answer_order.push(token.id);
        }

        fn step(&mut self, step: Step) {
            match step {
                Step::Admission => self.enqueue(Kind::Admission, false),
                Step::ExpiredAdmission => self.enqueue(Kind::Admission, true),
                Step::Cancellation => self.enqueue(Kind::Cancellation, false),
                Step::BufferedCancellation => self.enqueue(Kind::Cancellation, true),
                Step::Control => self.control = true,
                Step::HoldNow | Step::HoldSoon => {
                    let delay = if matches!(step, Step::HoldNow) {
                        Duration::ZERO
                    } else {
                        ms(20)
                    };
                    let target = self.next_target;
                    self.next_target = self.next_target % 7 + 1;
                    self.holds.retain(|(_, existing)| *existing != target);
                    self.holds.push((self.now + delay, target));
                }
                Step::Receive(receive) => self.receive = receive,
                Step::Prefix => self.prefix = true,
                Step::Tick(value) => self.now += ms(value),
                Step::Turn => self.turn(),
            }
        }

        fn timer_ready(&self, timer: TimerArm) -> bool {
            match timer {
                TimerArm::Engine { at, due } => due || at.is_some_and(|at| at <= self.now),
                TimerArm::Grace { until } => until <= self.now,
            }
        }

        fn ready(&self, selection: &Selection, source: Source) -> bool {
            match source {
                Source::Receive => {
                    selection.receive == ReceiveArm::ProofComplete || self.receive != Recv::Pending
                }
                Source::Shutdown => self.shutdown,
                Source::Cancellation => !self.cancellations.is_empty(),
                Source::Admission => !self.admissions.is_empty(),
                Source::Control => self.control,
                Source::Timer => self.timer_ready(selection.timer),
            }
        }

        fn take(&mut self, selection: &Selection, source: Source) -> Event {
            match source {
                Source::Receive if selection.receive == ReceiveArm::ProofComplete => Event::Timer,
                Source::Receive => Event::Receive(self.receive),
                Source::Shutdown => Event::Shutdown,
                Source::Cancellation => {
                    assert!(selection.boundaries_eligible);
                    Event::Boundary(self.cancellations.pop_front().unwrap())
                }
                Source::Admission => {
                    assert!(selection.boundaries_eligible);
                    Event::Boundary(self.admissions.pop_front().unwrap())
                }
                Source::Control => {
                    self.control = false;
                    Event::Control
                }
                Source::Timer => Event::Timer,
            }
        }

        fn turn(&mut self) {
            if self.stopped {
                return;
            }
            let selection = match self.parked.take() {
                Some(selection) => selection,
                None => {
                    self.coordinator.begin_turn();
                    let now = self.now;
                    let (wake, due) = (self.next_wake(), self.due(now));
                    match self.coordinator.plan(now, wake, due, || false) {
                        TurnPlan::Redeliver(token) => {
                            let event = if self.shutdown {
                                self.coordinator.restore(token);
                                Event::Shutdown
                            } else {
                                Event::Boundary(token)
                            };
                            // A redelivery turn is a boundary turn.
                            self.receive_wins_with_boundary_ready = 0;
                            self.receive_wins_with_release_timer_ready = 0;
                            self.dispatch(event);
                            return;
                        }
                        TurnPlan::Select(selection) => selection,
                    }
                }
            };
            let winner = selection
                .order()
                .find(|source| self.ready(&selection, *source));
            let winner = match winner {
                Some(winner) => winner,
                None => {
                    // Nothing is ready: the shell parks until its timer, or
                    // until the script makes another source ready.
                    let at = match selection.timer {
                        TimerArm::Engine { at, .. } => at,
                        TimerArm::Grace { until } => Some(until),
                    };
                    match at {
                        Some(at) if at > self.now => {
                            self.now = at;
                            Source::Timer
                        }
                        _ => {
                            self.parked = Some(selection);
                            return;
                        }
                    }
                }
            };
            self.check_fairness(&selection, winner);
            let event = self.take(&selection, winner);
            self.dispatch(event);
        }

        fn check_fairness(&mut self, selection: &Selection, winner: Source) {
            // A completed no-input proof delivers the timer event through the
            // receive source; it is a timer turn, not a read.
            let winner =
                if winner == Source::Receive && selection.receive == ReceiveArm::ProofComplete {
                    Source::Timer
                } else {
                    winner
                };
            let boundary_ready = selection.order().any(|source| {
                matches!(
                    source,
                    Source::Shutdown | Source::Cancellation | Source::Admission
                ) && self.ready(selection, source)
            });
            let fault = winner == Source::Receive && self.receive == Recv::Fault;
            if fault {
                // Excluded; see `FAIRNESS_BOUND`.
            } else if winner == Source::Receive && boundary_ready {
                self.receive_wins_with_boundary_ready += 1;
                assert!(
                    self.receive_wins_with_boundary_ready <= FAIRNESS_BOUND,
                    "a ready boundary waited behind {} receive wins",
                    self.receive_wins_with_boundary_ready
                );
            } else {
                self.receive_wins_with_boundary_ready = 0;
            }

            // A pending release's ready timer is delivered within the bound.
            let release_timer_ready =
                self.coordinator.release().is_pending() && self.timer_ready(selection.timer);
            if winner == Source::Receive && release_timer_ready && !fault {
                self.receive_wins_with_release_timer_ready += 1;
                assert!(
                    self.receive_wins_with_release_timer_ready <= RELEASE_TIMER_BOUND,
                    "a ready release timer waited behind {} receive wins",
                    self.receive_wins_with_release_timer_ready
                );
            } else {
                self.receive_wins_with_release_timer_ready = 0;
            }

            // A due engine wake grants control at most one turn. A plan
            // without a due engine wake ends that wake's allowance, so a later
            // timer at the same instant starts afresh.
            if !matches!(selection.timer, TimerArm::Engine { due: true, .. }) {
                self.control_wins_for_due_wake = None;
            }
            if let TimerArm::Engine { at, due: true } = selection.timer {
                match winner {
                    Source::Control => {
                        let count = match self.control_wins_for_due_wake {
                            Some((deadline, count)) if deadline == at => count + 1,
                            _ => 1,
                        };
                        assert!(count <= 1, "control won twice ahead of a due wake");
                        self.control_wins_for_due_wake = Some((at, count));
                    }
                    Source::Timer => self.control_wins_for_due_wake = None,
                    _ => {}
                }
            }
        }

        fn dispatch(&mut self, event: Event) {
            let selected_at = self.now;
            let selected = match &event {
                Event::Timer => Selected::Timer,
                Event::Boundary(_) => Selected::Boundary,
                Event::Receive(receive) => Selected::Receive {
                    due_at_receive: self.due(selected_at),
                    class: match receive {
                        Recv::NoInput => ReceiveClass::NoInput,
                        Recv::Fault => ReceiveClass::TransientFault,
                        Recv::Frames | Recv::Buffered | Recv::Pending => ReceiveClass::Other,
                    },
                },
                Event::Shutdown | Event::Control => Selected::Other,
            };
            let suppress_due = match self.coordinator.classify(selected, self.due(selected_at)) {
                Disposition::Restart => return,
                Disposition::Retain => {
                    let Event::Boundary(token) = event else {
                        panic!("only a boundary is retained, not {event:?}");
                    };
                    if token.answer_early {
                        self.answer(token, Answer::Early);
                    } else {
                        assert_eq!(
                            self.coordinator.retain(token),
                            Ok(()),
                            "the retained slot was occupied"
                        );
                    }
                    self.coordinator.restart_after_retain();
                    return;
                }
                Disposition::Apply { suppress_due } => suppress_due,
            };
            let consumed = if matches!(event, Event::Control) {
                self.coordinator
                    .control_consumed_wake(self.next_wake(), self.now)
            } else {
                None
            };
            let outcome = self.handle(event, suppress_due);
            if !self.coordinator.finish(selected, consumed, outcome) {
                self.stop();
            }
        }

        fn idle_pause(&mut self) {
            let deadline = self
                .coordinator
                .release()
                .await_until()
                .filter(|deadline| *deadline > self.now)
                .or_else(|| self.next_wake());
            let pause = deadline.map_or(IDLE_PAUSE, |deadline| {
                IDLE_PAUSE.min(deadline.saturating_duration_since(self.now))
            });
            self.now += pause;
        }

        fn handle(&mut self, event: Event, suppress_due: bool) -> TurnOutcome {
            match event {
                Event::Shutdown => TurnOutcome::Stop,
                Event::Control => TurnOutcome::Continue,
                Event::Boundary(token) => {
                    assert!(
                        self.due(self.now).is_empty(),
                        "a boundary's engine turn ran behind a due raw release"
                    );
                    let answer = if token.answer_early {
                        Answer::Early
                    } else {
                        Answer::Applied
                    };
                    self.answer(token, answer);
                    TurnOutcome::Continue
                }
                Event::Timer => self.wake(),
                Event::Receive(Recv::NoInput) => {
                    self.idle_pause();
                    TurnOutcome::YieldBoundaries
                }
                Event::Receive(Recv::Fault) => {
                    self.faults += 1;
                    if self.faults >= FAULT_LIMIT {
                        return TurnOutcome::Stop;
                    }
                    self.idle_pause();
                    TurnOutcome::YieldBoundaries
                }
                Event::Receive(Recv::Frames) => {
                    self.faults = 0;
                    self.prefix = false;
                    if !suppress_due {
                        // A complete input turn runs due work at the sampled
                        // instant, which settles any due hold.
                        self.release_due(self.now);
                        self.coordinator.release_mut().complete();
                    }
                    TurnOutcome::Continue
                }
                Event::Receive(Recv::Buffered) => {
                    // An input-only turn: the framer still holds a prefix, so
                    // no due work runs and the release stays pending.
                    self.faults = 0;
                    self.prefix = true;
                    TurnOutcome::ContinueBuffered
                }
                Event::Receive(Recv::Pending) => {
                    unreachable!("a pending receive is never selected")
                }
            }
        }

        /// The shell's timer turn, mirroring the async actor's `Wake` handler.
        fn wake(&mut self) -> TurnOutcome {
            let now = self.now;
            let due = self.due(now);
            let latched = self
                .coordinator
                .release_mut()
                .observe(due)
                .unwrap_or_default();
            if !latched.is_empty() {
                if self.coordinator.release_mut().replace_if_changed(due) {
                    return TurnOutcome::Continue;
                }
                if self.prefix {
                    match self.coordinator.release().await_until() {
                        Some(grace) if grace <= now => self.prefix = false,
                        Some(grace) => {
                            self.coordinator.release_mut().wait_for_input_until(grace);
                            return TurnOutcome::ContinueBuffered;
                        }
                        None => {
                            self.coordinator
                                .release_mut()
                                .wait_for_input_until(now + GRACE);
                            return TurnOutcome::ContinueBuffered;
                        }
                    }
                }
            }
            self.coordinator.release_mut().complete();
            self.release_due(now);
            TurnOutcome::Continue
        }

        fn stop(&mut self) {
            self.stopped = true;
            if let Some(token) = self.coordinator.take_retained() {
                self.answer(token, Answer::Drained);
            }
            while let Some(token) = self.cancellations.pop_front() {
                self.answer(token, Answer::Drained);
            }
            while let Some(token) = self.admissions.pop_front() {
                self.answer(token, Answer::Drained);
            }
        }

        /// Shut down and check the end-of-run invariants.
        fn finish_and_check(mut self) {
            self.shutdown = true;
            for _ in 0..TAIL_TURNS {
                if self.stopped {
                    break;
                }
                self.turn();
                if self.parked.is_some() {
                    // Nothing ready and no timer: only an external source
                    // could wake the shell. Shutdown is ready, so a parked
                    // selection here is a liveness failure.
                    panic!("parked with shutdown pending");
                }
            }
            assert!(self.stopped, "shutdown was not selected within the tail");
            assert!(!self.coordinator.has_retained());

            // Conservation: every boundary is answered exactly once.
            assert_eq!(self.answers.len(), self.enqueued.len());
            for token in &self.enqueued {
                assert!(self.answers.contains_key(&token.id), "{token:?} lost");
            }
            // FIFO within each lane, the retained boundary included.
            for kind in [Kind::Admission, Kind::Cancellation] {
                let enqueued: Vec<u32> = self
                    .enqueued
                    .iter()
                    .filter(|token| token.kind == kind)
                    .map(|token| token.id)
                    .collect();
                let answered: Vec<u32> = self
                    .answer_order
                    .iter()
                    .copied()
                    .filter(|id| enqueued.contains(id))
                    .collect();
                assert_eq!(answered, enqueued, "{kind:?} lane answered out of order");
            }
        }
    }

    pub(super) fn run(steps: impl IntoIterator<Item = Step>) {
        let mut world = World::new();
        for step in steps {
            world.step(step);
        }
        world.finish_and_check();
    }

    /// Two live admissions arrive at a due raw hold whose retained prefix
    /// keeps the release in its grace, with receive pending (#775).
    #[test]
    fn two_admissions_behind_a_retained_release_are_both_applied() {
        run([
            Step::HoldNow,
            Step::Prefix,
            Step::Turn,
            Step::Turn,
            Step::Admission,
            Step::Admission,
            Step::Turn,
            Step::Turn,
            Step::Turn,
            Step::Tick(150),
            Step::Turn,
            Step::Turn,
            Step::Turn,
            Step::Turn,
        ]);
    }

    /// Current behaviour (see `FAIRNESS_BOUND`): a transient-fault flood while
    /// a raw release awaits its input proof keeps receive first, so a queued
    /// admission stays queued, with its admission deadline unclaimed, until
    /// the fault run ends the session; the terminal drain then answers it.
    #[test]
    fn a_fault_flood_during_a_pending_release_defers_boundaries() {
        let mut world = World::new();
        for step in [Step::HoldNow, Step::Receive(Recv::Fault), Step::Admission] {
            world.step(step);
        }
        for _ in 0..FAULT_LIMIT {
            world.step(Step::Turn);
        }
        assert!(world.stopped, "the fault run ended the session");
        assert_eq!(world.answers.get(&0), Some(&Answer::Drained));
    }

    /// A buffered-input flood with no prior timer turn: the forced fairness
    /// turn delivers the release timer, which starts the prefix's grace; once
    /// the grace elapses the timer leads and the release resolves, so a queued
    /// admission is applied rather than waiting for shutdown.
    #[test]
    fn a_buffered_flood_cannot_stall_a_pending_release() {
        let mut world = World::new();
        for step in [
            Step::HoldNow,
            Step::Receive(Recv::Buffered),
            Step::Admission,
        ] {
            world.step(step);
        }
        // The first forced turn goes to the queued admission (retained); the
        // next one, with admission ineligible, goes to the timer.
        for _ in 0..2 * (CEILING + 1) {
            world.step(Step::Turn);
        }
        assert!(
            world.coordinator.release().await_until().is_some(),
            "the forced turn's timer started the grace"
        );
        world.step(Step::Tick(150));
        for _ in 0..(CEILING + 4) {
            world.step(Step::Turn);
        }
        assert!(
            !world.coordinator.release().is_pending(),
            "the release resolved"
        );
        assert_eq!(world.answers.get(&0), Some(&Answer::Applied));
        world.finish_and_check();
    }

    #[test]
    fn a_receive_flood_cannot_starve_a_queued_boundary() {
        run([
            Step::Receive(Recv::Frames),
            Step::Admission,
            Step::Cancellation,
            Step::Turn,
            Step::Turn,
            Step::Turn,
            Step::Turn,
            Step::Turn,
            Step::Turn,
        ]);
    }

    /// Every sequence of length five over the boundary, release and receive
    /// symbols that matter most for retention.
    #[test]
    fn exhaustive_short_sequences_preserve_the_invariants() {
        const KEY: [Step; 9] = [
            Step::Admission,
            Step::Cancellation,
            Step::HoldNow,
            Step::Prefix,
            Step::Receive(Recv::Frames),
            Step::Receive(Recv::NoInput),
            Step::Receive(Recv::Buffered),
            Step::Tick(150),
            Step::Turn,
        ];
        const LENGTH: u32 = 5;
        for mut index in 0..KEY.len().pow(LENGTH) {
            let mut steps = Vec::with_capacity(LENGTH as usize + 4);
            for _ in 0..LENGTH {
                steps.push(KEY[index % KEY.len()]);
                index /= KEY.len();
            }
            // Give every sequence a chance to retain a second boundary.
            steps.extend([Step::Admission, Step::Turn, Step::Turn, Step::Turn]);
            run(steps);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4096))]

        /// Long generated sequences over the full alphabet.
        #[test]
        fn generated_sequences_preserve_the_invariants(
            steps in prop::collection::vec(0_usize..ALPHABET.len(), 1..96)
        ) {
            run(steps.into_iter().map(|index| ALPHABET[index]));
        }
    }
}
