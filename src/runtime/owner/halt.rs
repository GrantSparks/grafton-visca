//! One owner halt transaction; STOP slots live only in its bounded report.
use super::boundary::AdmissionValidity;
use super::{AppliedEffect, EngineTurn, OwnerState, ReceiptCore, TerminalObserver};
use crate::{prepared::PreparedHalt, Error, HaltOutcome, HaltReport};
use std::time::Instant;

#[derive(Debug)]
pub(super) struct HaltBoundary {
    pub(super) prepared: PreparedHalt,
    pub(super) deadline: Instant,
    pub(super) validity: AdmissionValidity,
    pub(super) reply: flume::Sender<Result<HaltReceipt, Error>>,
}

#[derive(Debug)]
pub(super) struct HaltReceipt {
    pub(super) slots: [Option<Result<ReceiptCore, Error>>; 3],
    pub(super) deadline: Instant,
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
        if let Some(axes) = prepared.axes {
            for effect in self.engine.halt(prepared.target, axes, cutoff) {
                let _ = self.apply_effect(effect);
            }
        }
        // All admissions are input-only: no axis's write or observation can
        // delay admission of another STOP. Released ordinary permits and the
        // existing target control reserve participate in the same allocator.
        let turn = self.begin_input_turn(now);
        let slots = prepared.requests.map(|request| {
            request.map(|request| {
                let mut request = request.inspect_err(|error| {
                    self.record_admission_rejection(
                        prepared.target,
                        super::RequestLane::Command,
                        error,
                    );
                })?;
                let remaining = deadline.saturating_duration_since(now);
                if remaining.is_zero() {
                    let error = Error::admission_timeout();
                    self.record_admission_rejection(
                        request.context().target,
                        super::RequestLane::of(&request),
                        &error,
                    );
                    return Err(error);
                }
                request.context_mut().submission_order = cutoff;
                request.context_mut().dispatch_deadline = Some(deadline);
                // The engine budget expires queued STOPs at the SAME absolute
                // deadline; a stale STOP must not follow newer post-halt motion.
                request.context_mut().retry.total_budget = remaining;
                let permit = self
                    .permits
                    .try_acquire(request.context())
                    .inspect_err(|error| {
                        self.record_admission_rejection(
                            request.context().target,
                            super::RequestLane::of(&request),
                            error,
                        );
                    })?;
                let target = request.context().target;
                let (completion, observer) = TerminalObserver::pair();
                let (reply, admitted) = flume::bounded(1);
                let input = self.stage_admission_with(request, permit, observer, reply);
                for effect in self.input_in_turn(&turn, input) {
                    if let AppliedEffect::Transmit(_) = self.apply_effect(effect) {
                        return Err(Error::InvalidState(
                            "input-only halt admission unexpectedly transmitted".into(),
                        ));
                    }
                }
                let admitted = admitted.try_recv().map_err(|_| {
                    Error::InvalidState("halt admission omitted its outcome".into())
                })??;
                Ok(ReceiptCore::admitted(
                    admitted, target, completion, remaining,
                ))
            })
        });
        // Input-only finalization preserves all protocol pacing; the shell
        // starts ordinary dispatch after publishing the bounded receipt.
        let _ = self.finish_input_turn(turn, EngineTurn::INPUT_ONLY);
        HaltReceipt { slots, deadline }
    }
}
