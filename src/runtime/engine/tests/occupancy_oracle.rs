//! Independent scans for decision-local occupancy and dispatch gates.
//! Expected phase sets intentionally do not call the new lifecycle predicates.
use super::*;
use crate::runtime::owner::AdmissionPermitPool;

fn command_on_wire(lifecycle: Lifecycle) -> bool {
    matches!(
        lifecycle,
        Lifecycle::Sending {
            reply: SendingReply::Stop
                | SendingReply::AckThenCompletion
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
            reply: SendingReply::Stop
                | SendingReply::AckThenCompletion
                | SendingReply::CompletionOnly
                | SendingReply::NoReply,
            ..
        } | Lifecycle::AwaitingAck { .. }
            | Lifecycle::AwaitingCompletion { .. }
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

#[derive(Default)]
struct ScannedTarget {
    commands: usize,
    inquiries: usize,
    unacknowledged: usize,
    uncorrelated_command: bool,
    declared_shapes: SmallVec<[PayloadShape; 4]>,
    live_shapes: SmallVec<[PayloadShape; 4]>,
}

pub(super) fn assert_agrees<E: EngineMethods + ?Sized>(engine: &E, now: Instant) {
    let occupancy = Occupancy::build(&engine.entries);
    let due = engine
        .entries
        .iter()
        .filter_map(|(id, entry)| entry.due(*id))
        .min_by_key(|due| due.key());
    assert_eq!(
        occupancy.due.map(|due| (due.key(), due.request)),
        due.map(|due| (due.key(), due.request))
    );
    let mut scanned = PerTarget::<ScannedTarget>::default();
    let raw = engine.mode().is_raw();
    for entry in engine.entries.values() {
        let expected = &mut scanned[entry.request.context().target];
        let lifecycle = entry.lifecycle();
        let live_inquiry = inquiry_on_wire(lifecycle);
        expected.commands += usize::from(command_on_wire(lifecycle));
        expected.inquiries += usize::from(live_inquiry);
        expected.unacknowledged += usize::from(unacknowledged(lifecycle));
        expected.uncorrelated_command |= uncorrelated_command(lifecycle);
        if let (true, Some(shape)) = (raw, entry.request.payload_shape()) {
            expected.declared_shapes.push(shape);
            if live_inquiry {
                expected.live_shapes.push(shape);
            }
        }
    }
    for (target, expected) in scanned.iter() {
        let actual = &occupancy.targets[target];
        assert_eq!(actual.commands, expected.commands, "commands on {target:?}");
        assert_eq!(
            actual.inquiries, expected.inquiries,
            "inquiries on {target:?}"
        );
        assert_eq!(
            actual.unacknowledged, expected.unacknowledged,
            "unacknowledged on {target:?}"
        );
        assert_eq!(
            actual.uncorrelated_command, expected.uncorrelated_command,
            "uncorrelated command on {target:?}"
        );
        // Raw admission bounds exact declarations. Sony may overflow this
        // unused shape view conservatively; its gate never queries it.
        if raw {
            let probes = [
                PayloadShape::Unknown,
                PayloadShape::Bytes(0),
                PayloadShape::Nibbles(1),
                PayloadShape::Bytes(2),
                PayloadShape::Nibbles(4),
                PayloadShape::Bytes(16),
            ]
            .into_iter()
            .chain(expected.declared_shapes.iter().copied())
            .chain(engine.mode().ledger().inquiry_shapes(target));
            for shape in probes {
                let overlaps = expected.live_shapes.iter().any(|live| match (live, shape) {
                    (PayloadShape::Unknown, _) | (_, PayloadShape::Unknown) => true,
                    (
                        PayloadShape::Bytes(left) | PayloadShape::Nibbles(left),
                        PayloadShape::Bytes(right) | PayloadShape::Nibbles(right),
                    ) => *left == right,
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

/// Admission is the only inquiry count limit. Overlap/correlation can defer
/// dispatch further, but cannot create an extra ordinary slot.
#[test]
fn a_single_admission_slot_excludes_a_second_inquiry_for_every_envelope() {
    let now = Instant::now();
    for (envelope, transport) in [
        (EnvelopeKind::Raw, TransportKind::Datagram),
        (EnvelopeKind::Raw, TransportKind::Stream),
        (EnvelopeKind::Sony, TransportKind::Datagram),
    ] {
        let mut engine = engine(envelope, transport);
        engine.policy.capacity = std::num::NonZeroUsize::MIN;
        let permits = AdmissionPermitPool::new(1, PerTarget::default());
        let first_permit = permits.try_acquire_ordinary().unwrap();
        let (write, first) = admit(
            &mut engine,
            1,
            inquiry_with_shape(1, PayloadShape::Bytes(1)),
            now,
        );
        let sequence = (envelope == EnvelopeKind::Sony).then_some(0x1001);
        send_ok(&mut engine, &write, sequence, now);
        assert!(matches!(
            permits.try_acquire_ordinary(),
            Err(Error::RuntimeQueueFull { capacity: 1 })
        ));
        let replied = engine.handle(
            frame(
                1,
                sequence,
                DecodedResponse::InquiryReply {
                    payload: smallvec![0x01],
                },
            ),
            now,
        );
        assert!(matches!(
            terminal_outcome(&replied, first),
            Some(RuntimeOutcome::Inquiry(InquiryOutcome::Reply { .. }))
        ));
        drop(first_permit);
        let _second_permit = permits.try_acquire_ordinary().unwrap();
        let (write, next) = admit(
            &mut engine,
            3,
            inquiry_with_shape(1, PayloadShape::Bytes(2)),
            now,
        );
        assert_eq!(request_transmit(&write).1, next);
        send_ok(&mut engine, &write, sequence.map(|_| 0x1002), now);
        assert_agrees(&engine, now);
        engine.assert_invariants().unwrap();
    }
}

proptest::proptest! {
    #[test]
    fn ordinary_slots_bound_inquiry_occupancy_under_admission_reply_and_deadline_orderings(
        capacity in 1usize..=4,
        mode in 0u8..3,
        actions in proptest::collection::vec(proptest::num::u8::ANY, 1..100),
    ) {
        let (envelope, transport) = match mode {
            0 => (EnvelopeKind::Raw, TransportKind::Datagram),
            1 => (EnvelopeKind::Raw, TransportKind::Stream),
            _ => (EnvelopeKind::Sony, TransportKind::Datagram),
        };
        let mut engine = engine(envelope, transport);
        engine.policy.capacity = std::num::NonZeroUsize::new(capacity).unwrap();
        let mut now = Instant::now();
        for (index, action) in actions.into_iter().enumerate() {
            now += Duration::from_millis(if action & 7 == 7 { 35 } else { 1 });
            let target = 1;
            let mut effects = if action & 7 < 5 && engine.active_len() == capacity {
                engine.advance(now)
            } else if action & 7 < 5 {
                engine.handle(Input::Admit {
                    ticket: AdmissionTicket(index as u64),
                    request: inquiry_with_shape(target, PayloadShape::Bytes(1 + usize::from(action & 3))),
                }, now)
            } else {
                let candidate = engine.entries.iter().find(|(_, entry)| {
                    entry.request.context().target == camera(target) && entry.lifecycle().is_awaiting_reply()
                });
                let (sequence, length) = candidate.map_or((None, 1), |(_, entry)| {
                    let length = match entry.request.payload_shape().unwrap() {
                        PayloadShape::Bytes(length) | PayloadShape::Nibbles(length) => length,
                        PayloadShape::Unknown => 1,
                    };
                    (entry.current_sequence(), length)
                });
                engine.handle(frame(target, sequence,
                    DecodedResponse::InquiryReply { payload: std::iter::repeat_n(1, length).collect() }), now)
            };
            while let Some((_, id, _)) = request_transmit_optional(&effects) {
                let sequence = (envelope == EnvelopeKind::Sony).then(|| sony_sequence(id));
                effects = send_ok(&mut engine, &effects, sequence, now);
            }
            assert_agrees(&engine, now);
            engine.assert_invariants().unwrap();
            for target in [camera(1), camera(2)] {
                let in_flight = engine.inquiries_inflight_for(target);
                proptest::prop_assert!(in_flight <= capacity);
                if engine.entries.values().any(|entry| {
                    entry.request.context().target == target
                        && matches!(entry.lifecycle(), Lifecycle::Ready { .. })
                }) {
                    proptest::prop_assert!(in_flight < capacity);
                }
            }
        }
    }
}
