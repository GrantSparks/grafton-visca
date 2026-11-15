//! One owner halt transaction; STOP slots live only in its bounded report.
use super::boundary::AdmissionValidity;
use super::{EngineTurn, OwnerState, ReceiptCore, TerminalObserver};
use crate::{prepared::PreparedHalt, Error, HaltOutcome, HaltReport};
use std::time::Instant;

#[derive(Debug)]
pub(super) struct HaltBoundary {
    pub(super) prepared: PreparedHalt,
    pub(super) validity: AdmissionValidity,
    pub(super) reply: flume::Sender<Result<HaltReceipt, Error>>,
}

#[derive(Debug)]
pub(super) struct HaltReceipt {
    pub(super) slots: [Option<Result<ReceiptCore, Error>>; 3],
    pub(super) deadline: Instant,
}

/// One supported axis's outcome: whether its STOP applied before the halt's
/// deadline.
pub(super) fn outcome(applied: Result<(), Error>) -> HaltOutcome {
    match applied {
        Ok(()) => HaltOutcome::Applied,
        Err(error) => HaltOutcome::Failed(error),
    }
}

pub(super) fn report(outcomes: [HaltOutcome; 3]) -> HaltReport {
    let [pan_tilt, zoom, focus] = outcomes;
    HaltReport::new(pan_tilt, zoom, focus)
}

impl OwnerState {
    pub(super) fn accept_halt(
        &mut self,
        prepared: PreparedHalt,
        deadline: Instant,
        cutoff: u64,
        now: Instant,
    ) -> HaltReceipt {
        self.observed_at = now;
        let turn = self.begin_input_turn(now);
        if let Some(axes) = prepared.axes {
            for effect in self
                .engine
                .halt_in_turn(&turn.0, prepared.target, axes, cutoff)
            {
                let _ = self.apply_effect(effect);
            }
        }
        // All admissions are input-only: no axis's write or observation can
        // delay admission of another STOP. Released ordinary permits and the
        // existing target control reserve participate in the same allocator.
        let slots = prepared.requests.map(|request| {
            request.map(|request| {
                let mut request = request.inspect_err(|error| {
                    self.record_admission_rejection(prepared.target, super::Lane::Command, error);
                })?;
                let remaining = deadline.saturating_duration_since(now);
                request.context_mut().submission_order = cutoff;
                request.context_mut().dispatch_deadline = Some(deadline);
                // The engine budget expires queued STOPs at the SAME absolute
                // deadline; a stale STOP must not follow newer post-halt motion.
                request.context_mut().retry.total_budget = Some(
                    crate::runtime::engine::NonZeroDuration::new(remaining)
                        .map_err(|_| Error::admission_timeout())
                        .inspect_err(|error| {
                            self.record_admission_rejection(
                                prepared.target,
                                super::Lane::Command,
                                error,
                            )
                        })?,
                );
                let permit = self
                    .permits
                    .try_acquire(request.context())
                    .inspect_err(|error| {
                        self.record_admission_rejection(
                            request.context().target,
                            request.lane(),
                            error,
                        );
                    })?;
                let (completion, observer) = TerminalObserver::pair();
                let admitted = self.admit_in_turn(&turn, request, permit, observer)?;
                Ok(ReceiptCore::admitted(admitted, completion, remaining))
            })
        });
        // Input-only finalization preserves all protocol pacing; the shell
        // starts ordinary dispatch after publishing the bounded receipt.
        let _ = self.finish_input_turn(turn, EngineTurn::InputOnly);
        HaltReceipt { slots, deadline }
    }
}
