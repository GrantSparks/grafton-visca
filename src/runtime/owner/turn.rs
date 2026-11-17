//! Executor-free policy shared by the async and blocking owner shells.
//!
//! This module is the landing point for the owner-turn unification in issue
//! #723, and holds the source-arbitration coordinator from #776 (decision
//! D25). It contains no transport, channel, executor, or wall-clock access:
//! callers sample time and map the returned durations onto their native wait
//! mechanism.

use std::{
    num::NonZeroU32,
    time::{Duration, Instant},
};

use crate::runtime::engine::RawCorrelationReleaseSet;

#[cfg(any(feature = "async", feature = "blocking"))]
mod coordinator;
#[cfg(any(
    feature = "blocking",
    all(
        test,
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]
pub(super) use coordinator::Source;

// The async actor tests build only with an executor adapter.
#[cfg(all(
    test,
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
pub(super) use coordinator::SourcePhase;
#[cfg(any(feature = "async", feature = "blocking"))]
pub(super) use coordinator::{
    BoundaryPoll, BoundarySource, Disposition, NonBoundarySelected, NonBoundarySource,
    OwnerCoordinator, PlannedTurn, ReceiveArm, ReceiveClass, Selected, SelectedInput, Selection,
    SelectionPoll, TimerArm, TurnOutcome, TurnPlan, VacantSlot,
};

/// Pause applied after the first transient receive fault or immediately-idle
/// read so a transport cannot hot-spin an owner.
pub(super) const TRANSIENT_RECEIVE_PAUSE: Duration = Duration::from_millis(10);

/// Ceiling on the escalating receive pause.
pub(super) const MAXIMUM_TRANSIENT_RECEIVE_PAUSE: Duration = Duration::from_millis(250);

/// Consecutive transient receive faults required before a run may be treated
/// as a permanent transport failure.
pub(super) const TRANSIENT_RECEIVE_FAULT_LIMIT: u32 = 12;

/// Minimum wall-clock span of a fault run before it is permanent.
pub(super) const TRANSIENT_RECEIVE_FAULT_SPAN: Duration = Duration::from_secs(1);

/// A fault-free gap this long starts a new run.
pub(super) const TRANSIENT_RECEIVE_FAULT_RESET: Duration = Duration::from_secs(5);

/// Escalating pause for the `run`-th consecutive receive fault or idle read.
pub(super) fn transient_receive_pause(run: u32) -> Duration {
    let doublings = run.saturating_sub(1).min(6);
    TRANSIENT_RECEIVE_PAUSE
        .saturating_mul(1u32 << doublings)
        .min(MAXIMUM_TRANSIENT_RECEIVE_PAUSE)
}

/// Clamp a receive pause so it cannot delay an owner or caller deadline.
pub(super) fn clamp_receive_pause(
    pause: Duration,
    deadline: Option<Instant>,
    now: Instant,
) -> Duration {
    deadline.map_or(pause, |deadline| {
        pause.min(deadline.saturating_duration_since(now))
    })
}

/// The non-empty release scope carried by every pending proof state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PendingReleases(RawCorrelationReleaseSet);

impl PendingReleases {
    fn new(set: RawCorrelationReleaseSet) -> Option<Self> {
        (!set.is_empty()).then_some(Self(set))
    }
}

/// One owner-side, receive-first raw-correlation release turn.
///
/// The engine owns the typed release facts and their deadlines. This small
/// executor-free coordinator owns only the shell state needed to prove that a
/// due release received one ordered input turn before either owner lets due
/// work dispatch a successor. Both owner shells use this same latch, no-input
/// fence, and retained-prefix wait representation (#723).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum RawReleaseTurn {
    #[default]
    Idle,
    Latched {
        set: PendingReleases,
    },
    Fenced {
        set: PendingReleases,
    },
    AwaitingInput {
        set: PendingReleases,
        until: Instant,
    },
    // A no-input probe can finish while the framer still awaits an earlier
    // prefix's tail. Keep its grace deadline for idle pacing until the timer
    // consumes the proof; otherwise an eager idle transport can hot-spin.
    FencedAwaitingInput {
        set: PendingReleases,
        until: Instant,
    },
}

impl RawReleaseTurn {
    pub(super) fn observe(
        &mut self,
        releases: RawCorrelationReleaseSet,
    ) -> Option<RawCorrelationReleaseSet> {
        if matches!(self, Self::Idle) {
            self.replace(releases);
        }
        self.latched()
    }

    pub(super) const fn latched(&self) -> Option<RawCorrelationReleaseSet> {
        match self {
            Self::Idle => None,
            Self::Latched { set }
            | Self::Fenced { set }
            | Self::AwaitingInput { set, .. }
            | Self::FencedAwaitingInput { set, .. } => Some(set.0),
        }
    }

    pub(super) const fn probe_scope(&self) -> Option<RawCorrelationReleaseSet> {
        match self {
            Self::Latched { set } | Self::AwaitingInput { set, .. } => Some(set.0),
            Self::Idle | Self::Fenced { .. } | Self::FencedAwaitingInput { .. } => None,
        }
    }

    pub(super) const fn is_pending(&self) -> bool {
        !matches!(self, Self::Idle)
    }

    /// Replace a completed proof with a newly exposed release scope.
    pub(super) fn replace(&mut self, releases: RawCorrelationReleaseSet) {
        *self = PendingReleases::new(releases).map_or(Self::Idle, |set| Self::Latched { set });
    }

    pub(super) fn replace_if_changed(&mut self, releases: RawCorrelationReleaseSet) -> bool {
        if releases.is_empty() || self.latched() == Some(releases) {
            return false;
        }
        self.replace(releases);
        true
    }

    pub(super) fn fence_no_input(&mut self, releases: RawCorrelationReleaseSet) {
        if let Some(set) = PendingReleases::new(releases) {
            *self = match self.await_until() {
                Some(until) if self.latched() == Some(releases) => {
                    Self::FencedAwaitingInput { set, until }
                }
                _ => Self::Fenced { set },
            };
        }
    }

    pub(super) fn is_fenced(&self, releases: RawCorrelationReleaseSet) -> bool {
        matches!(self, Self::Fenced { set } | Self::FencedAwaitingInput { set, .. } if set.0 == releases)
    }

    pub(super) fn clear_fence(&mut self) {
        *self = match *self {
            Self::Fenced { set } => Self::Latched { set },
            Self::FencedAwaitingInput { set, until } => Self::AwaitingInput { set, until },
            state => state,
        };
    }

    pub(super) const fn await_until(&self) -> Option<Instant> {
        match self {
            Self::AwaitingInput { until, .. } | Self::FencedAwaitingInput { until, .. } => {
                Some(*until)
            }
            _ => None,
        }
    }

    pub(super) fn wait_for_input_until(&mut self, until: Instant) {
        *self = match *self {
            Self::Idle => Self::Idle,
            Self::Fenced { set } | Self::FencedAwaitingInput { set, .. } => {
                Self::FencedAwaitingInput { set, until }
            }
            Self::Latched { set } | Self::AwaitingInput { set, .. } => {
                Self::AwaitingInput { set, until }
            }
        };
    }

    pub(super) fn complete(&mut self) {
        *self = Self::Idle;
    }
}

/// One run of receives that returned immediately without bytes.
///
/// This is deliberately independent of [`TransientFaultRun`]: an idle receive
/// is not a transport fault and must neither spend retry budget nor prove that
/// a prior fault run recovered. Both owner shells nevertheless apply the same
/// escalating pause so an eager driver cannot hot-spin.
#[derive(Debug, Default)]
pub(super) struct IdleReceiveRun {
    length: u32,
}

impl IdleReceiveRun {
    /// Record one idle receive and return its shared pacing interval.
    pub(super) fn record(&mut self) -> Duration {
        self.length = self.length.saturating_add(1);
        transient_receive_pause(self.length)
    }

    /// Received bytes or a successful write restart the idle-read run.
    pub(super) fn reset(&mut self) {
        self.length = 0;
    }

    #[cfg(test)]
    pub(super) const fn length(&self) -> u32 {
        self.length
    }
}

/// One run of consecutive transient receive faults.
#[derive(Debug, Default)]
pub(super) struct TransientFaultRun {
    active: Option<FaultRun>,
}

#[derive(Debug, Clone, Copy)]
struct FaultRun {
    length: NonZeroU32,
    first_at: Instant,
    last_at: Instant,
}

impl TransientFaultRun {
    /// Record one transient fault and report the run it belongs to.
    pub(super) fn record(&mut self, at: Instant) -> (u32, Duration) {
        let run = match self.active {
            Some(run)
                if at.saturating_duration_since(run.last_at) < TRANSIENT_RECEIVE_FAULT_RESET =>
            {
                FaultRun {
                    length: run.length.saturating_add(1),
                    last_at: at,
                    ..run
                }
            }
            _ => FaultRun {
                length: NonZeroU32::MIN,
                first_at: at,
                last_at: at,
            },
        };
        let observation = (run.length.get(), at.saturating_duration_since(run.first_at));
        self.active = Some(run);
        observation
    }

    /// A successful byte-bearing read proves the transport recovered.
    pub(super) fn reset(&mut self) {
        self.active = None;
    }

    /// Whether this run is long enough, and old enough, to be permanent.
    pub(super) const fn is_permanent(length: u32, span: Duration) -> bool {
        length >= TRANSIENT_RECEIVE_FAULT_LIMIT
            && span.as_nanos() >= TRANSIENT_RECEIVE_FAULT_SPAN.as_nanos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fenced_partial_input_keeps_its_grace_until_the_proof_is_consumed() {
        let until = Instant::now() + Duration::from_millis(100);
        let set = RawCorrelationReleaseSet::default()
            .with_terminal_for_test(crate::target::TargetSlot::CAMERA_1);
        let mut release = RawReleaseTurn::default();
        assert_eq!(release.observe(RawCorrelationReleaseSet::default()), None);
        assert_eq!(release.observe(set), Some(set));
        release.wait_for_input_until(until);
        release.fence_no_input(set);
        assert!(release.is_fenced(set));
        assert_eq!(release.await_until(), Some(until));
        assert_eq!(release.probe_scope(), None);
        release.clear_fence();
        assert_eq!(release.await_until(), Some(until));
        assert_eq!(release.probe_scope(), Some(set));
        release.complete();
        assert!(!release.is_pending());
        assert_eq!(release.await_until(), None);
    }

    #[test]
    fn receive_pause_escalates_and_clamps() {
        assert_eq!(transient_receive_pause(1), TRANSIENT_RECEIVE_PAUSE);
        assert_eq!(transient_receive_pause(2), Duration::from_millis(20));
        assert_eq!(transient_receive_pause(3), Duration::from_millis(40));
        assert_eq!(
            transient_receive_pause(u32::MAX),
            MAXIMUM_TRANSIENT_RECEIVE_PAUSE
        );

        let now = Instant::now();
        assert_eq!(
            clamp_receive_pause(
                MAXIMUM_TRANSIENT_RECEIVE_PAUSE,
                Some(now + Duration::from_millis(3)),
                now,
            ),
            Duration::from_millis(3)
        );
    }

    #[test]
    fn idle_receive_run_uses_the_shared_pause_and_resets_on_bytes() {
        let mut run = IdleReceiveRun::default();
        assert_eq!(run.record(), TRANSIENT_RECEIVE_PAUSE);
        assert_eq!(run.record(), Duration::from_millis(20));
        assert_eq!(run.length(), 2);
        run.reset();
        assert_eq!(run.length(), 0);
        assert_eq!(run.record(), TRANSIENT_RECEIVE_PAUSE);
    }

    #[test]
    fn transient_fault_run_resets_after_recovery_gap() {
        let start = Instant::now();
        let mut run = TransientFaultRun::default();
        assert_eq!(run.record(start).0, 1);
        assert_eq!(run.record(start + Duration::from_millis(20)).0, 2);
        assert_eq!(
            run.record(start + Duration::from_millis(20) + TRANSIENT_RECEIVE_FAULT_RESET)
                .0,
            1
        );
        run.reset();
        assert!(run.active.is_none());
    }

    #[test]
    fn permanent_fault_requires_count_and_elapsed_time() {
        assert!(!TransientFaultRun::is_permanent(
            TRANSIENT_RECEIVE_FAULT_LIMIT,
            TRANSIENT_RECEIVE_FAULT_SPAN - Duration::from_nanos(1),
        ));
        assert!(!TransientFaultRun::is_permanent(
            TRANSIENT_RECEIVE_FAULT_LIMIT - 1,
            TRANSIENT_RECEIVE_FAULT_SPAN,
        ));
        assert!(TransientFaultRun::is_permanent(
            TRANSIENT_RECEIVE_FAULT_LIMIT,
            TRANSIENT_RECEIVE_FAULT_SPAN,
        ));
    }
}
