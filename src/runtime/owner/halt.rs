//! One owner halt transaction; STOP slots live only in its bounded report.
use super::boundary::{AdmissionRejection, BoundaryClaim};
use super::{Effect, EngineTurn, OwnerInputTurn, OwnerState, ReceiptCore, TerminalObserver};
use crate::{prepared::PreparedHalt, Error, HaltOutcome, HaltReport};
use std::collections::VecDeque;
use std::time::Instant;

#[derive(Debug)]
pub(super) struct HaltBoundary {
    pub(super) prepared: PreparedHalt,
    pub(super) claim: BoundaryClaim,
    pub(super) reply: flume::Sender<Result<HaltReceipt, Error>>,
}

impl HaltBoundary {
    pub(super) fn reject(self, state: &mut OwnerState, rejection: AdmissionRejection) {
        if rejection.record {
            for _ in self.prepared.requests.iter().flatten() {
                state.record_admission_rejection(
                    self.prepared.target,
                    super::Lane::Command,
                    &rejection.error,
                );
            }
        }
        let _ = self.reply.try_send(Err(rejection.error));
    }
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
    /// Supersede prior motion in one input turn. Its effects are driven by the
    /// shell before admitting STOPs, so released permits are available and any
    /// transmission or session end follows the ordinary ordered effect path.
    pub(super) fn begin_halt(
        &mut self,
        prepared: &PreparedHalt,
        cutoff: u64,
        now: Instant,
    ) -> (OwnerInputTurn, VecDeque<Effect>) {
        let turn = self.begin_input_turn(now);
        if let Some(axes) = prepared.axes {
            self.engine.halt_in_turn_into(
                &turn.0,
                prepared.target,
                axes,
                cutoff,
                &mut self.buffers.effect_scratch,
            );
        }
        (turn, self.buffers.take_effects())
    }

    pub(super) fn accept_halt(
        &mut self,
        turn: OwnerInputTurn,
        prepared: PreparedHalt,
        deadline: Instant,
        cutoff: u64,
        remaining: crate::runtime::engine::NonZeroDuration,
        engine_turn: EngineTurn,
    ) -> (HaltReceipt, VecDeque<Effect>) {
        // All admissions are input-only: no axis's write or observation can
        // delay admission of another STOP. Released ordinary permits and the
        // existing target control reserve participate in the same allocator.
        let slots = prepared.requests.map(|request| {
            request.map(|request| {
                let mut request = request.inspect_err(|error| {
                    self.record_admission_rejection(prepared.target, super::Lane::Command, error);
                })?;
                request.context_mut().submission_order = cutoff;
                request.context_mut().dispatch_deadline = Some(deadline);
                // The engine budget expires queued STOPs at the SAME absolute
                // deadline; a stale STOP must not follow newer post-halt motion.
                request.context_mut().retry.total_budget = Some(remaining);
                let permit = self.permits.try_acquire(&request).inspect_err(|error| {
                    self.record_admission_rejection(
                        request.context().target,
                        request.lane(),
                        error,
                    );
                })?;
                let (completion, observer) = TerminalObserver::pair();
                let admitted = self.admit_in_turn(&turn, request.into_inner(), permit, observer)?;
                ReceiptCore::admitted(admitted, completion, remaining.get())
            })
        });
        let effects = self.finish_input_turn(turn, engine_turn);
        (HaltReceipt { slots, deadline }, effects)
    }
}
