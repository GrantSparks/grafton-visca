//! Deterministic planner cost and opt-in wall-clock measurements (#907).
//! Debug tests count visits; release timing uses uninstrumented production iteration.
use super::*;

const ENTRIES: usize = 67;

fn blocked_engine(
    envelope: EnvelopeKind,
    transport: TransportKind,
    now: Instant,
) -> ProtocolEngine {
    let mut configured = policy(envelope, transport);
    configured.capacity = ENTRIES;
    let mut engine = ProtocolEngine::new(configured, now).unwrap();
    engine
        .register_target(
            camera(1),
            TargetPolicy {
                command_sockets: CommandSocketCount::ONE,
                ..TargetPolicy::test_default()
            },
        )
        .unwrap();
    for index in 0..ENTRIES {
        let mut request = command(1, CancellationPolicy::Supported);
        if index == ENTRIES - 1 {
            let RuntimeRequest::Command { context, .. } = &mut request else {
                unreachable!()
            };
            context.control.class = ControlClass::User;
        }
        let effects = engine.handle_turn(
            Input::Admit {
                ticket: AdmissionTicket(index as u64 + 1),
                request,
                slot: AdmissionSlot::Ordinary,
            },
            now,
            EngineTurn::InputOnly,
        );
        assert!(effects
            .iter()
            .any(|effect| matches!(effect, Effect::Admitted { .. })));
    }
    // Dispatch the last entry so short-circuiting scans must visit all N entries.
    let effects = engine.advance(now);
    let (_, blocker, _) = request_transmit(&effects);
    assert_eq!(blocker.get(), u64::try_from(ENTRIES).unwrap());
    assert_eq!(
        engine
            .entry(blocker)
            .unwrap()
            .request
            .context()
            .control
            .class,
        ControlClass::User
    );
    send_ok(
        &mut engine,
        &effects,
        (envelope == EnvelopeKind::Sony).then_some(0x1001),
        now,
    );
    assert!(matches!(
        phase_of(&engine, blocker),
        Some(Phase::AwaitingAck { .. })
    ));
    assert_eq!(
        engine
            .entries
            .values()
            .filter(|entry| matches!(entry.phase(), Phase::Ready))
            .count(),
        ENTRIES - 1
    );
    engine.assert_invariants().unwrap();
    assert_eq!(engine.entries.len(), ENTRIES);
    assert!(engine.select_dispatch(now).is_none());
    engine
}

#[cfg(debug_assertions)]
#[test]
fn planner_entry_visits() {
    let now = Instant::now();
    for envelope in [EnvelopeKind::Sony, EnvelopeKind::Raw] {
        for transport in [TransportKind::Datagram, TransportKind::Stream] {
            let engine = blocked_engine(envelope, transport, now);
            super::super::entry_table::reset_entry_visits();
            let _ = engine.next_wake(now);
            let wake = super::super::entry_table::entry_visits();
            super::super::entry_table::reset_entry_visits();
            assert!(engine.select_dispatch(now).is_none());
            let dispatch = super::super::entry_table::entry_visits();
            eprintln!("{envelope:?}/{transport:?}: N={ENTRIES} next_wake={wake} select_dispatch={dispatch}");
            assert!(wake <= 2 * ENTRIES + 8, "next_wake scanned {wake} entries");
            assert!(
                dispatch <= 2 * ENTRIES + 8,
                "select_dispatch scanned {dispatch} entries"
            );
        }
    }
}

#[test]
#[ignore = "host-dependent release-mode planner timing"]
fn planner_timing() {
    let now = Instant::now();
    const ITERATIONS: u32 = 10_000;
    for envelope in [EnvelopeKind::Sony, EnvelopeKind::Raw] {
        for transport in [TransportKind::Datagram, TransportKind::Stream] {
            let engine = blocked_engine(envelope, transport, now);
            let started = Instant::now();
            for _ in 0..ITERATIONS {
                std::hint::black_box(
                    std::hint::black_box(&engine).next_wake(std::hint::black_box(now)),
                );
                std::hint::black_box(
                    std::hint::black_box(&engine).select_dispatch(std::hint::black_box(now)),
                );
            }
            eprintln!("{envelope:?}/{transport:?}: N={ENTRIES}, next_wake+blocked_dispatch {:?}/iteration over {ITERATIONS} iterations", started.elapsed() / ITERATIONS);
        }
    }
}

#[cfg(debug_assertions)]
#[test]
fn every_full_table_iterator_reports_its_visits() {
    let engine = blocked_engine(EnvelopeKind::Sony, TransportKind::Datagram, Instant::now());
    for iterate in [
        |table: &EntryTable| {
            table.iter().count();
        },
        |table: &EntryTable| {
            table.values().count();
        },
        |table: &EntryTable| {
            table.active_entries().count();
        },
        |table: &EntryTable| {
            table.into_iter().count();
        },
        |table: &EntryTable| {
            table.iter().rev().count();
        },
    ] {
        super::super::entry_table::reset_entry_visits();
        iterate(&engine.entries);
        assert_eq!(super::super::entry_table::entry_visits(), ENTRIES);
    }
}
