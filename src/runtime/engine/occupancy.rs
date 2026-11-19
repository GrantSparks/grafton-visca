//! Decision-local lane occupancy, derived once from the authoritative entries.
use super::*;
use crate::{state_cache::PerStateKey, StateKey};

#[derive(Debug, Clone, Copy)]
pub(super) struct PositionalCommand {
    family: SendingReply,
    halt: bool,
    submission_order: u64,
}

#[derive(Debug, Default)]
pub(super) struct TargetOccupancy {
    pub(super) commands: usize,
    pub(super) inquiries: usize,
    pub(super) unacknowledged: usize,
    pub(super) unassigned_command: bool,
    pub(super) uncorrelated_command: bool,
    pub(super) pan_tilt_drive_cancel_pending: bool,
    positional: Option<PositionalCommand>,
    /// Admission order applies across priorities. A setter retains precedence
    /// through socket displacement and retry backoff until terminal settlement.
    first_setters: PerStateKey<Option<RequestId>>,
    speed_mode_setter_dispatched: bool,
    /// Earliest owner halt cutoff with a STOP that may still write each axis.
    pending_halt_cutoffs: [Option<u64>; HALT_AXES.len()],
    shapes: InquiryShapes,
    shapes_overflow: bool,
}
impl TargetOccupancy {
    pub(super) fn state_write_blocks(&self, id: RequestId, request: &RuntimeRequest) -> bool {
        request
            .applied_state_key()
            .is_some_and(|key| self.first_setters[key].is_some_and(|first| first < id))
            || (request.context().requires_extended_pan_tilt_speed
                && self.first_setters[StateKey::PanTiltSpeedMode]
                    .is_some_and(|first| first < id || self.speed_mode_setter_dispatched))
    }

    pub(super) fn halt_stop_blocks(&self, request: &RuntimeRequest) -> bool {
        let context = request.context();
        !request.is_stop()
            && context.motion.is_some_and(|motion| {
                self.pending_halt_cutoffs
                    .into_iter()
                    .zip(HALT_AXES)
                    .any(|(cutoff, axis)| {
                        motion.axes.contains(axis)
                            && cutoff.is_some_and(|cutoff| cutoff < context.submission_order)
                    })
            })
    }

    pub(super) fn overlaps(&self, shape: PayloadShape) -> bool {
        self.shapes_overflow || self.shapes.iter().any(|live| live.overlaps(shape))
    }
    pub(super) fn urgent_bypass(&self, entry: &Entry, stream: bool) -> bool {
        self.unacknowledged == 1
            && self.positional.is_some_and(|candidate| {
                (stream && candidate.family == SendingReply::CompletionOnly)
                    || (matches!(
                        candidate.family,
                        SendingReply::Stop | SendingReply::AckThenCompletion
                    ) && !(candidate.halt
                        && entry.request.context().dispatch_deadline.is_some()
                        && candidate.submission_order == entry.request.context().submission_order))
            })
    }
}

#[derive(Debug, Default)]
pub(super) struct Occupancy {
    pub(super) targets: PerTarget<TargetOccupancy>,
    pub(super) due: Option<DueWork>,
}
impl Occupancy {
    pub(super) fn build(entries: &EntryTable, now: Instant) -> Self {
        let mut occupancy = Self::default();
        for (id, entry) in entries.iter() {
            let lifecycle = entry.lifecycle();
            let target = &mut occupancy.targets[entry.request.context().target];
            if let Some(key) = entry.request.applied_state_key() {
                // EntryTable iterates in monotonically allocated request ID order.
                target.first_setters[key].get_or_insert(*id);
                if key == StateKey::PanTiltSpeedMode {
                    target.speed_mode_setter_dispatched |= !matches!(
                        lifecycle,
                        Lifecycle::Ready {
                            previous_attempt: None
                        }
                    );
                }
            }
            target.pan_tilt_drive_cancel_pending |= entry.request.context().pan_tilt_drive
                && lifecycle.cancellation_ambiguity().is_some();
            if let Some((axes, cutoff)) = entry.writable_halt(now) {
                for (pending, axis) in target.pending_halt_cutoffs.iter_mut().zip(HALT_AXES) {
                    if axes.contains(axis) {
                        *pending = Some(pending.map_or(cutoff, |previous| previous.min(cutoff)));
                    }
                }
            }
            target.commands += usize::from(lifecycle.is_command_on_wire());
            target.unassigned_command |= matches!(lifecycle, Lifecycle::ExecutingUnassigned { .. });
            if lifecycle.is_live_inquiry() {
                target.inquiries += 1;
                if let Some(shape) = entry.request.payload_shape() {
                    target.shapes_overflow |= !target.shapes.try_insert(shape);
                }
            }
            target.uncorrelated_command |= lifecycle.is_uncorrelated_command();
            if raw_unacknowledged_command_candidate(entry) {
                target.unacknowledged += 1;
                target.positional = Some(PositionalCommand {
                    family: entry.reply_family(),
                    halt: entry.request.context().dispatch_deadline.is_some(),
                    submission_order: entry.request.context().submission_order,
                });
            }
            if let Some(due) = entry.due(*id) {
                if occupancy
                    .due
                    .is_none_or(|previous| due.key() < previous.key())
                {
                    occupancy.due = Some(due);
                }
            }
        }
        occupancy
    }
}
