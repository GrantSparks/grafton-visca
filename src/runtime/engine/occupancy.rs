//! Decision-local lane occupancy, derived once from the authoritative entries.
use super::*;

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
    pub(super) uncorrelated_command: bool,
    positional: Option<PositionalCommand>,
    /// Request IDs order setters across priorities. A dispatched setter retains
    /// the dependency through socket displacement and retry backoff.
    first_speed_mode_setter: Option<RequestId>,
    speed_mode_setter_dispatched: bool,
    shapes: InquiryShapes,
    shapes_overflow: bool,
}
impl TargetOccupancy {
    pub(super) fn speed_mode_blocks(&self, id: RequestId, request: &RuntimeRequest) -> bool {
        let Some(first) = self.first_speed_mode_setter else {
            return false;
        };
        (id > first && request.sets_pan_tilt_speed_mode())
            || (request.context().requires_extended_pan_tilt_speed
                && (id > first || self.speed_mode_setter_dispatched))
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
    pub(super) fn build(entries: &EntryTable) -> Self {
        let mut occupancy = Self::default();
        for (id, entry) in entries.iter() {
            let lifecycle = entry.lifecycle();
            let target = &mut occupancy.targets[entry.request.context().target];
            if entry.request.sets_pan_tilt_speed_mode() {
                // EntryTable iterates in monotonically allocated request ID order.
                target.first_speed_mode_setter.get_or_insert(*id);
                target.speed_mode_setter_dispatched |= !matches!(
                    lifecycle,
                    Lifecycle::Ready {
                        previous_attempt: None
                    }
                );
            }
            target.commands += usize::from(lifecycle.is_command_on_wire());
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
