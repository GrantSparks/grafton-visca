//! Session status and cause form one immutable terminal verdict (#939).
use super::*;

fn assert_end(
    engine: &mut ProtocolEngine,
    effects: &[Effect],
    status: EndStatus,
    expected: &Error,
    now: Instant,
) {
    let ends: Vec<_> = effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::SessionEnded(end) => Some(end),
            _ => None,
        })
        .collect();
    assert_eq!(ends.len(), 1);
    assert_eq!(ends[0].status, status);
    assert_eq!(format!("{:?}", ends[0].cause), format!("{expected:?}"));
    assert_eq!(engine.state(), status.state());
    assert_eq!(
        format!("{:?}", engine.terminal_error().unwrap()),
        format!("{expected:?}")
    );
    assert!(engine
        .handle(Input::Shutdown(ShutdownReason::Explicit), now)
        .is_empty());
    assert!(engine
        .handle(
            Input::Shutdown(ShutdownReason::FramingFailure {
                cause: Arc::new(Error::OwnerTerminated),
            }),
            now,
        )
        .is_empty());
    let turn = engine.begin_input_turn(now);
    let refusal = engine.admit_in_turn(&turn, inquiry(1)).unwrap_err();
    assert_eq!(format!("{refusal:?}"), format!("{expected:?}"));
    engine.assert_invariants().unwrap();
}

#[test]
fn owner_terminal_inputs_emit_their_complete_verdict_once() {
    let now = Instant::now();
    let peer = Arc::new(Error::from(std::io::Error::other("peer close")));
    let framing = Arc::new(Error::InvalidResponse {
        expected: "complete Sony envelope".into(),
        actual: vec![0x01],
    });
    for (reason, status, expected) in [
        (
            ShutdownReason::Explicit,
            EndStatus::Shutdown,
            Error::RuntimeShutdown,
        ),
        (
            ShutdownReason::TransportClosed { cause: None },
            EndStatus::Closed,
            Error::connection_closed(None),
        ),
        (
            ShutdownReason::TransportClosed {
                cause: Some(Arc::clone(&peer)),
            },
            EndStatus::Closed,
            Error::connection_closed(Some(peer)),
        ),
        (
            ShutdownReason::FramingFailure {
                cause: Arc::clone(&framing),
            },
            EndStatus::Poisoned,
            Error::stream_poisoned(framing),
        ),
    ] {
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        assert!(engine.terminal_error().is_none());
        let effects = engine.handle(Input::Shutdown(reason), now);
        assert_end(&mut engine, &effects, status, &expected, now);
    }
}

#[test]
fn strict_deadline_poison_emits_and_retains_the_engine_verdict_once() {
    let now = Instant::now();
    let mut engine = strict_poison_engine();
    let (effects, _) = admit(&mut engine, 1, command(1), now);
    send_ok(&mut engine, &effects, None, now);
    let deadline = now + Duration::from_millis(20);
    let effects = engine.advance(deadline);
    assert_end(
        &mut engine,
        &effects,
        EndStatus::Poisoned,
        &Error::stream_poisoned(Arc::new(Error::UnsequencedCommandUnconfirmed)),
        deadline,
    );
}

#[test]
fn strict_receive_fault_emits_and_retains_the_engine_verdict_once() {
    let now = Instant::now();
    let mut engine = strict_poison_engine();
    let (effects, _) = admit(&mut engine, 1, command(1), now);
    send_ok(&mut engine, &effects, None, now);
    let effects = engine.handle(
        Input::ReceiveFault {
            error: Error::io_timeout(),
        },
        now,
    );
    assert_end(
        &mut engine,
        &effects,
        EndStatus::Poisoned,
        &Error::stream_poisoned(Arc::new(Error::UnsequencedCommandUnconfirmed)),
        now,
    );
}

#[test]
fn stream_write_poison_delivers_the_exact_transport_cause_once() {
    let now = Instant::now();
    for envelope in [EnvelopeKind::Raw, EnvelopeKind::Sony] {
        let mut engine = engine(envelope, TransportKind::Stream);
        let (admission, id) = admit(&mut engine, 1, command(1), now);
        let (transmission, _, _) = request_transmit(&admission);
        let cause = Error::transport_error(std::io::Error::other("short write"));
        let effects = engine.handle(
            Input::TransmissionFinished {
                request: id,
                transmission,
                result: WriteOutcome::Failed(cause.clone()),
            },
            now,
        );
        assert_end(
            &mut engine,
            &effects,
            EndStatus::Poisoned,
            &Error::stream_poisoned(Arc::new(cause)),
            now,
        );
    }
}
