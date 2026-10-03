//! Executor-free receipt observation shared by the owner shells (D24, #780).
//!
//! A receipt wait is a loop over one native race (the observation slots,
//! owner liveness and the observer deadline) whose every decision is made
//! here: what a wake records into the handle-side cache, when the wait ends,
//! and with which verdict. A shell supplies only the race itself.

use std::{
    ops::ControlFlow,
    time::{Duration, Instant},
};

use crate::{
    camera::MovementTolerance,
    completion,
    prepared::{PositionQueryPlan, SettlementPlan},
    AffectedAxes, Error,
};

use super::boundary::OwnerHandleCore;
use super::{
    CancellationObserver, OperationObservation, ReceiptCore, RuntimeOutcome, TerminalObserver,
};
use crate::ResponseDecoder;

/// A command's linear observation right, bound to the owner handle `H` that
/// admitted it.
#[derive(Debug)]
pub(crate) struct CommandReceipt<H> {
    pub(super) core: ReceiptCore,
    pub(super) owner: H,
}

/// An inquiry's linear observation right and its exact decoder, bound to the
/// owner handle `H` that admitted it.
#[derive(Debug)]
pub(crate) struct InquiryReceipt<R, H> {
    pub(super) core: ReceiptCore,
    pub(super) decoder: ResponseDecoder<R>,
    pub(super) owner: H,
}

/// The observation of one admitted operation: its cached observation state,
/// settlement plan, and the owner handle `H` that admitted it. Every wait
/// borrows it, so an abandoned or timed-out wait releases only that wait
/// (#777). Only this receipt class exposes cancellation.
///
/// Each owner shell implements the waits in its own blocking style; the
/// decisions they make live in this module.
#[derive(Debug)]
pub(crate) struct OperationReceipt<K, H>
where
    K: completion::Kind,
{
    pub(super) observation: OperationObservation,
    pub(super) affected_axes: AffectedAxes,
    pub(super) settlement: completion::Settlement<K>,
    pub(super) owner: H,
}

impl<K, H> OperationReceipt<K, H>
where
    K: completion::Kind,
{
    pub(crate) fn id(&self) -> u64 {
        self.observation.id().get()
    }
}

/// What ended one wait for an observation slot.
#[derive(Debug)]
pub(super) enum ObservationWake {
    /// The terminal slot delivered (`None`: the owner dropped it unresolved).
    Terminal(Option<RuntimeOutcome>),
    /// The cancellation slot delivered (`None`: the owner dropped it, which it
    /// does only when the operation's terminal outcome is already delivered).
    Cancellation(Option<Error>),
    /// The owner is gone.
    OwnerGone,
    /// The observer deadline passed.
    Deadline,
}

impl ObservationWake {
    /// The outcome of a command or inquiry receipt's single wait.
    ///
    /// Once the owner is gone or the deadline passed, the slot is read once
    /// more: a terminal outcome that raced teardown, or arrived exactly at the
    /// deadline, still decides.
    pub(super) fn conclude(
        self,
        core: &ReceiptCore,
        owner: &OwnerHandleCore,
    ) -> Result<RuntimeOutcome, Error> {
        match self {
            Self::Terminal(Some(outcome)) => Ok(outcome),
            Self::Terminal(None) | Self::OwnerGone => {
                core.try_outcome().ok_or_else(|| owner.disconnected_error())
            }
            Self::Deadline => core
                .try_outcome()
                .ok_or_else(|| super::observation_timeout(core.id)),
            Self::Cancellation(_) => Err(Error::InvalidState(
                "a receipt without a cancellation slot observed one".into(),
            )),
        }
    }
}

/// One wait until `verdict` can be read from an operation's observation
/// state.
///
/// Every value a slot delivers is recorded into the handle's cache as soon as
/// the shell receives it, so abandoning the wait never loses one.
pub(super) struct OperationWait<'a, V> {
    observation: &'a mut OperationObservation,
    verdict: V,
    /// Cleared once the owner drops the cancellation slot, so the wait stops
    /// selecting a slot that can no longer deliver.
    cancellation_open: bool,
}

impl<'a, T, V> OperationWait<'a, V>
where
    V: Fn(&mut OperationObservation) -> Option<T>,
{
    pub(super) fn new(observation: &'a mut OperationObservation, verdict: V) -> Self {
        Self {
            observation,
            verdict,
            cancellation_open: true,
        }
    }

    /// The verdict, once the cache holds it.
    pub(super) fn verdict(&mut self) -> Option<T> {
        (self.verdict)(self.observation)
    }

    /// The slots the next race waits on: the terminal slot and, while it can
    /// still deliver, the cancellation slot.
    pub(super) fn slots(&self) -> (&TerminalObserver, Option<&CancellationObserver>) {
        (
            self.observation.terminal_observer(),
            self.observation
                .cancellation_observer()
                .filter(|_| self.cancellation_open),
        )
    }

    /// Record one wake. Ends the wait once the owner is gone or the deadline
    /// passed; the cache is read once more first, so a terminal outcome that
    /// raced teardown, or arrived exactly at the deadline, still decides.
    pub(super) fn absorb(
        &mut self,
        wake: ObservationWake,
        owner: &OwnerHandleCore,
    ) -> ControlFlow<Result<T, Error>> {
        match wake {
            ObservationWake::Terminal(Some(outcome)) => {
                self.observation.record_terminal(outcome);
            }
            ObservationWake::Cancellation(Some(error)) => {
                self.observation.record_cancellation_failure(error);
            }
            ObservationWake::Cancellation(None) => self.cancellation_open = false,
            ObservationWake::Terminal(None) | ObservationWake::OwnerGone => {
                return ControlFlow::Break(
                    self.verdict().ok_or_else(|| owner.disconnected_error()),
                );
            }
            ObservationWake::Deadline => {
                let id = self.observation.id();
                return ControlFlow::Break(
                    self.verdict().ok_or_else(|| super::observation_timeout(id)),
                );
            }
        }
        ControlFlow::Continue(())
    }
}

/// The absolute observer deadline `timeout` after `now`.
pub(super) fn observer_deadline(now: Instant, timeout: Duration) -> Result<Instant, Error> {
    now.checked_add(timeout)
        .ok_or_else(|| Error::InvalidParameter {
            parameter: "observer timeout",
            value: format!("{timeout:?}").into(),
            reason: "duration exceeds the monotonic clock range".into(),
        })
}

/// The budget a targeted settlement wait runs under: the caller's, or the
/// profile's configured default.
pub(super) fn settlement_budget(
    settlement: &completion::Settlement<completion::Targeted>,
    timeout: Option<Duration>,
) -> Result<Duration, Error> {
    match timeout {
        Some(timeout) => Ok(timeout),
        None => settlement.default_budget().ok_or_else(|| {
            Error::InvalidState(
                "targeted operation omitted its configured settlement budget".into(),
            )
        }),
    }
}

/// How settlement is proved by position polling once an operation applied.
#[derive(Debug, Clone, Copy)]
pub(super) struct PositionPoll<'a> {
    pub(super) queries: &'a PositionQueryPlan,
    pub(super) axes: AffectedAxes,
    pub(super) tolerance: MovementTolerance,
    pub(super) interval: Duration,
}

/// The position polling that proves settlement of the applied operation
/// `observation`, or `None` when the profile declares its applied protocol
/// completion to be the settlement evidence.
pub(super) fn position_poll<'a>(
    settlement: &'a completion::Settlement<completion::Targeted>,
    observation: &OperationObservation,
    affected_axes: AffectedAxes,
) -> Result<Option<PositionPoll<'a>>, Error> {
    match settlement.plan()? {
        SettlementPlan::CompletionIsSettled { target, .. } => {
            debug_assert_eq!(*target, observation.target());
            Ok(None)
        }
        SettlementPlan::Poll {
            target,
            queries,
            axes,
            tolerance,
            interval,
            ..
        } => {
            debug_assert_eq!(*target, observation.target());
            debug_assert_eq!(*axes, affected_axes);
            Ok(Some(PositionPoll {
                queries,
                axes: *axes,
                tolerance: *tolerance,
                interval: *interval,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observer_deadline_is_exact_and_rejects_clock_overflow() {
        let now = Instant::now();
        let timeout = Duration::from_millis(100);
        assert!(
            matches!(observer_deadline(now, timeout), Ok(deadline) if deadline == now + timeout)
        );
        assert!(matches!(
            observer_deadline(now, Duration::MAX),
            Err(Error::InvalidParameter {
                parameter: "observer timeout",
                ..
            })
        ));
    }
}
