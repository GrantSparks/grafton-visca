//! Deterministic planner cost and opt-in wall-clock measurements (#907).
//! Debug tests count visits; release timing uses uninstrumented production iteration.
use super::*;

const ENTRIES: usize = 67;

fn blocked_engine(
    envelope: EnvelopeKind,
    transport: TransportKind,
    now: Instant,
) -> ProtocolEngine {
    let engine = saturated_engine(envelope, transport, now, |_| command(1));
    assert!(engine.select_dispatch(now).is_none());
    engine
}

fn saturated_engine(
    envelope: EnvelopeKind,
    transport: TransportKind,
    now: Instant,
    ready_request: impl Fn(usize) -> RuntimeRequest,
) -> ProtocolEngine {
    let mut configured = policy(envelope, transport);
    configured.capacity = std::num::NonZeroUsize::new(ENTRIES).unwrap();
    let mut engine = ProtocolEngine::new(
        configured,
        target_policies(
            &[camera(1)],
            TargetPolicy {
                command_sockets: CommandSocketCount::ONE,
                ..TargetPolicy::test_default()
            },
        ),
        now,
    )
    .unwrap();
    for index in 0..ENTRIES {
        let request = if index == ENTRIES - 1 {
            let mut request = command(1);
            request.context_mut().control.class = ControlClass::User;
            request
        } else {
            ready_request(index)
        };
        let effects = engine.handle_turn(
            Input::Admit {
                ticket: AdmissionTicket(index as u64 + 1),
                request,
            },
            now,
            EngineTurn::InputOnly,
        );
        assert!(effects
            .iter()
            .any(|effect| matches!(effect, Effect::Admitted { .. })));
    }
    // Reserve the only socket with the newest, highest-priority request.
    let effects = engine.advance(now);
    let (transmission, blocker, _) = request_transmit(&effects);
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
    finish_write_input_only(
        &mut engine,
        blocker,
        transmission,
        Ok(TransmissionMeta::for_sequence(
            (envelope == EnvelopeKind::Sony).then_some(0x1001),
        )),
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

#[cfg(debug_assertions)]
#[test]
fn dispatch_stops_scanning_at_the_first_eligible_mixed_queue_entry() {
    let now = Instant::now();
    for inquiry_index in 0..=1 {
        let engine = saturated_engine(EnvelopeKind::Sony, TransportKind::Datagram, now, |index| {
            if index == inquiry_index {
                inquiry(1)
            } else {
                command(1)
            }
        });
        super::super::entry_table::reset_entry_visits();
        let request = engine.select_dispatch(now).unwrap().request;
        assert_eq!(request.get(), u64::try_from(inquiry_index + 1).unwrap());
        // One occupancy pass, at most one blocked command, then the inquiry.
        // Split lanes checked all ENTRIES - 2 socket-blocked ready commands.
        assert_eq!(
            super::super::entry_table::entry_visits(),
            ENTRIES + inquiry_index + 1
        );
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
