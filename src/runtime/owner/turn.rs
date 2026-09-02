//! Executor-free policy shared by the async and blocking owner shells.
//!
//! This module is the landing point for the owner-turn unification in issue
//! #723.  It contains no transport, channel, executor, or wall-clock access:
//! callers sample time and map the returned durations onto their native wait
//! mechanism.

use std::time::{Duration, Instant};

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
