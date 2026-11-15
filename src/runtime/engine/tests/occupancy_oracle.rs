//! Independent full scans for decision-local occupancy and dispatch gates.
//! Expected phase sets intentionally do not call the new lifecycle predicates.
use super::*;

fn command_on_wire(lifecycle: Lifecycle) -> bool {
    matches!(
        lifecycle,
        Lifecycle::Sending {
            reply: SendingReply::AckThenCompletion
                | SendingReply::CompletionOnly
                | SendingReply::NoReply,
            ..
        } | Lifecycle::AwaitingAck { .. }
            | Lifecycle::AwaitingCompletion { .. }
            | Lifecycle::Executing { .. }
            | Lifecycle::AwaitingCancellationResolution { .. }
    )
}
fn inquiry_on_wire(lifecycle: Lifecycle) -> bool {
    matches!(
        lifecycle,
        Lifecycle::Sending {
            reply: SendingReply::Inquiry,
            ..
        } | Lifecycle::AwaitingReply { .. }
    )
}
fn unacknowledged(lifecycle: Lifecycle) -> bool {
    matches!(
        lifecycle,
        Lifecycle::Sending {
            reply: SendingReply::AckThenCompletion
                | SendingReply::CompletionOnly
                | SendingReply::NoReply,
            ..
        } | Lifecycle::AwaitingAck { .. }
            | Lifecycle::AwaitingCompletion { .. }
    )
}
fn uncorrelated_write(lifecycle: Lifecycle) -> bool {
    matches!(
        lifecycle,
        Lifecycle::Sending {
            reply: SendingReply::CompletionOnly | SendingReply::NoReply,
            ..
        }
    )
}
fn uncorrelated_command(lifecycle: Lifecycle) -> bool {
    matches!(
        lifecycle,
        Lifecycle::Sending {
            reply: SendingReply::CompletionOnly | SendingReply::NoReply,
            ..
        } | Lifecycle::AwaitingCompletion { .. }
    )
}

pub(super) fn assert_agrees<E: EngineMethods + ?Sized>(engine: &E, now: Instant) {
    let occupancy = Occupancy::build(&engine.entries);
    let total_inquiries = engine
        .entries
        .values()
        .filter(|entry| inquiry_on_wire(entry.lifecycle()))
        .count();
    assert_eq!(occupancy.total_inquiries, total_inquiries);
    let due = engine
        .entries
        .iter()
        .filter_map(|(id, entry)| entry.due(*id))
        .min_by_key(|due| due.key());
    assert_eq!(
        occupancy.due.map(|due| (due.key(), due.request)),
        due.map(|due| (due.key(), due.request))
    );
    for target in TargetSlot::ALL {
        let entries = || {
            engine
                .entries
                .values()
                .filter(|entry| entry.request.context().target == target)
        };
        let actual = &occupancy.targets[target];
        assert_eq!(
            actual.commands,
            entries()
                .filter(|entry| command_on_wire(entry.lifecycle()))
                .count(),
            "commands on {target:?}"
        );
        assert_eq!(
            actual.inquiries,
            entries()
                .filter(|entry| inquiry_on_wire(entry.lifecycle()))
                .count(),
            "inquiries on {target:?}"
        );
        assert_eq!(
            actual.unacknowledged,
            entries()
                .filter(|entry| unacknowledged(entry.lifecycle()))
                .count(),
            "unacknowledged on {target:?}"
        );
        assert_eq!(
            actual.uncorrelated_write,
            entries().any(|entry| uncorrelated_write(entry.lifecycle())),
            "uncorrelated write on {target:?}"
        );
        assert_eq!(
            actual.uncorrelated_command,
            entries().any(|entry| uncorrelated_command(entry.lifecycle())),
            "uncorrelated command on {target:?}"
        );
        // Raw admission bounds exact declarations. Sony may overflow this
        // unused shape view conservatively; its gate never queries it.
        if engine.mode().is_raw() {
            let probes = [
                PayloadShape::Unknown,
                PayloadShape::Bytes(0),
                PayloadShape::Nibbles(1),
                PayloadShape::Bytes(2),
                PayloadShape::Nibbles(4),
                PayloadShape::Bytes(16),
            ]
            .into_iter()
            .chain(entries().filter_map(|entry| entry.request.payload_shape()))
            .chain(engine.mode().ledger().inquiry_shapes(target));
            for shape in probes {
                let overlaps = entries()
                    .filter(|entry| inquiry_on_wire(entry.lifecycle()))
                    .filter_map(|entry| entry.request.payload_shape())
                    .any(|live| match (live, shape) {
                        (PayloadShape::Unknown, _) | (_, PayloadShape::Unknown) => true,
                        (
                            PayloadShape::Bytes(left) | PayloadShape::Nibbles(left),
                            PayloadShape::Bytes(right) | PayloadShape::Nibbles(right),
                        ) => left == right,
                    });
                assert_eq!(
                    actual.overlaps(shape),
                    overlaps,
                    "shape {shape:?} on {target:?}"
                );
            }
        }
    }
    for (id, entry) in engine
        .entries
        .iter()
        .filter(|(_, entry)| matches!(entry.lifecycle(), Lifecycle::Ready { .. }))
    {
        assert_eq!(
            engine.dispatch_blocked_in(entry, now, &occupancy),
            engine.dispatch_blocked(entry, now),
            "gate for {id:?} at {now:?}, lifecycle {:?}",
            entry.lifecycle()
        );
    }
}
