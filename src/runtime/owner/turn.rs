//! Executor-free policy shared by the async and blocking owner shells.
//!
//! This module is the landing point for the owner-turn unification in issue
//! #723.  It contains no transport, channel, executor, or wall-clock access:
//! callers sample time and map the returned durations onto their native wait
//! mechanism.

use std::time::{Duration, Instant};

use crate::runtime::engine::RawCorrelationReleaseSet;

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

/// One owner-side, receive-first raw-correlation release turn.
///
/// The engine owns the typed release facts and their deadlines. This small
/// executor-free coordinator owns only the shell state needed to prove that a
/// due release received one ordered input turn before either owner lets due
/// work dispatch a successor. Both owner shells use this same latch, no-input
/// fence, and retained-prefix wait representation (#723).
#[derive(Debug, Default)]
pub(super) struct RawReleaseTurn {
    latched: Option<RawCorrelationReleaseSet>,
    no_input_fence: Option<RawCorrelationReleaseSet>,
    await_until: Option<Instant>,
}

impl RawReleaseTurn {
    /// Latch the first non-empty release set until a receive turn completes it.
    /// A later control mutation cannot replace the old correlation proof.
    pub(super) fn observe(
        &mut self,
        releases: RawCorrelationReleaseSet,
    ) -> Option<RawCorrelationReleaseSet> {
        if self.latched.is_none() && !releases.is_empty() {
            self.replace(releases);
        }
        if self.latched.is_none() {
            self.clear_fence();
            self.await_until = None;
        }
        self.latched
    }

    pub(super) const fn latched(&self) -> Option<RawCorrelationReleaseSet> {
        self.latched
    }

    pub(super) const fn is_pending(&self) -> bool {
        self.latched.is_some()
    }

    /// Replace a completed proof with a newly exposed, distinct release set.
    /// The replacement requires its own receive-first turn.
    pub(super) fn replace(&mut self, releases: RawCorrelationReleaseSet) {
        debug_assert!(!releases.is_empty());
        self.latched = Some(releases);
        self.clear_fence();
        self.await_until = None;
    }

    /// Record that a no-data receive was sampled for this exact release set.
    #[cfg_attr(not(feature = "async"), allow(dead_code))]
    pub(super) fn fence_no_input(&mut self, releases: RawCorrelationReleaseSet) {
        debug_assert!(!releases.is_empty());
        self.no_input_fence = Some(releases);
    }

    #[cfg_attr(not(feature = "async"), allow(dead_code))]
    pub(super) fn is_fenced(&self, releases: RawCorrelationReleaseSet) -> bool {
        self.no_input_fence.is_some_and(|fenced| fenced == releases)
    }

    pub(super) fn clear_fence(&mut self) {
        self.no_input_fence = None;
    }

    pub(super) const fn await_until(&self) -> Option<Instant> {
        self.await_until
    }

    pub(super) fn wait_for_input_until(&mut self, deadline: Instant) {
        self.await_until = Some(deadline);
    }

    /// Complete the current input proof. The engine due pass is performed by
    /// the shell immediately before or after this call at the same sampled
    /// instant.
    pub(super) fn complete(&mut self) {
        self.latched = None;
        self.clear_fence();
        self.await_until = None;
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

    /// A byte-bearing receive proves the transport is no longer idle.
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
    pub(super) length: u32,
    pub(super) first_at: Option<Instant>,
    pub(super) last_at: Option<Instant>,
}

impl TransientFaultRun {
    /// Record one transient fault and report the run it belongs to.
    pub(super) fn record(&mut self, at: Instant) -> (u32, Duration) {
        let continues = self
            .last_at
            .is_some_and(|last| at.saturating_duration_since(last) < TRANSIENT_RECEIVE_FAULT_RESET);
        if continues {
            self.length = self.length.saturating_add(1);
        } else {
            self.length = 1;
            self.first_at = Some(at);
        }
        self.last_at = Some(at);
        let span = self
            .first_at
            .map_or(Duration::ZERO, |first| at.saturating_duration_since(first));
        (self.length, span)
    }

    /// A successful byte-bearing read proves the transport recovered.
    pub(super) fn reset(&mut self) {
        self.length = 0;
        self.first_at = None;
        self.last_at = None;
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
        assert_eq!(run.length, 0);
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
