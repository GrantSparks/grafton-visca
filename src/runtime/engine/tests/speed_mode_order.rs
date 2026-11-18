//! Per-target ordering of pan/tilt speed-mode setters and their dependent writes.
use super::*;
use crate::types::PanTiltSpeedMode;

fn setter(mode: PanTiltSpeedMode) -> RuntimeRequest {
    let mut request = command(1);
    let RuntimeRequest::Command { applied_state, .. } = &mut request else {
        unreachable!()
    };
    *applied_state = Some(AppliedStateProjection::set(
        crate::KnownState::PanTiltSpeedMode(mode),
    ));
    request
}

fn extended_drive(target: u8) -> RuntimeRequest {
    let mut request = command(target);
    request.context_mut().requires_extended_pan_tilt_speed = true;
    request
}

fn queue(
    engine: &mut ProtocolEngine,
    ticket: u64,
    request: RuntimeRequest,
    now: Instant,
) -> RequestId {
    admitted(&engine.handle_turn(
        Input::Admit {
            ticket: AdmissionTicket(ticket),
            request,
        },
        now,
        EngineTurn::InputOnly,
    ))
}

fn socket_one_ack(sequence: u32) -> Input {
    frame(
        1,
        Some(sequence),
        DecodedResponse::Ack {
            socket: Some(ViscaSocket::S1),
        },
    )
}
fn socket_one_completion(sequence: u32) -> Input {
    frame(
        1,
        Some(sequence),
        DecodedResponse::Completion {
            socket: Some(ViscaSocket::S1),
        },
    )
}

#[test]
fn dependent_writes_follow_the_older_setter_across_priorities_and_retry_backoff() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let mut first = setter(PanTiltSpeedMode::Standard24);
    first.context_mut().control.class = ControlClass::Background;
    let first = queue(&mut engine, 1, first, now);
    let second = queue(&mut engine, 2, setter(PanTiltSpeedMode::Fine50), now);
    let mut drive = extended_drive(1);
    drive.context_mut().control.class = ControlClass::User;
    let drive = queue(&mut engine, 3, drive, now);
    assert_occupancy_agrees(&engine, now);
    let effects = engine.advance(now);
    assert_eq!(request_transmit(&effects).1, first);
    let sent = send_ok(&mut engine, &effects, Some(1), now);
    assert!(request_transmit_optional(&sent).is_none());

    let ack_deadline = engine.next_wake(now).unwrap();
    assert!(ack_deadline > now);
    let backoff = engine.advance(ack_deadline);
    assert!(request_transmit_optional(&backoff).is_none());
    assert!(matches!(
        phase_of(&engine, first),
        Some(Phase::Backoff { .. })
    ));
    let retry_at = engine.next_wake(ack_deadline).unwrap();
    assert!(retry_at > ack_deadline);
    let retry = engine.advance(retry_at);
    assert_eq!(request_transmit(&retry).1, first);
    send_ok(&mut engine, &retry, Some(1), retry_at);
    let acked = engine.handle(socket_one_ack(1), retry_at);
    assert!(request_transmit_optional(&acked).is_none());

    let applied = engine.handle(socket_one_completion(1), retry_at);
    assert_eq!(terminal_id(&applied), Some(first));
    assert_eq!(request_transmit(&applied).1, second);
    send_ok(&mut engine, &applied, Some(2), retry_at);
    engine.handle(socket_one_ack(2), retry_at);
    let applied = engine.handle(socket_one_completion(2), retry_at);
    assert_eq!(terminal_id(&applied), Some(second));
    assert_eq!(request_transmit(&applied).1, drive);
}

#[test]
fn a_displaced_setter_blocks_only_dependent_writes_to_its_target() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (mode, mode_id) = admit(&mut engine, 1, setter(PanTiltSpeedMode::Standard24), now);
    send_ok(&mut engine, &mode, Some(1), now);
    engine.handle(socket_one_ack(1), now);

    let (standard, standard_id) = admit(&mut engine, 2, command(1), now);
    assert_eq!(request_transmit(&standard).1, standard_id);
    send_ok(&mut engine, &standard, Some(2), now);
    engine.handle(socket_one_ack(2), now);
    assert!(matches!(
        phase_of(&engine, mode_id),
        Some(Phase::ExecutingDisplaced { .. })
    ));

    let (blocked, _) = admit(&mut engine, 3, extended_drive(1), now);
    assert!(request_transmit_optional(&blocked).is_none());
    assert_occupancy_agrees(&engine, now);
    let (other, other_id) = admit(&mut engine, 4, extended_drive(2), now);
    assert_eq!(request_transmit(&other).1, other_id);
    send_ok(&mut engine, &other, Some(3), now);
    let (stop, stop_id) = admit(&mut engine, 5, urgent_command(1), now);
    assert_eq!(request_transmit(&stop).1, stop_id);
}
