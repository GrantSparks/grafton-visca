//! Dispatch precedence of owner halt STOPs over later motion.
use super::*;
use crate::AffectedAxes;

fn zoom_halt_stop(order: u64, start: Instant, budget: Duration) -> RuntimeRequest {
    let mut stop = without_not_executable_retry(urgent_command(1));
    let context = stop.context_mut();
    context.motion = Some(MotionEffect {
        axes: AffectedAxes::ZOOM,
    });
    context.submission_order = order;
    context.dispatch_deadline = Some(start + budget);
    context.retry.total_budget = Some(NonZeroDuration::new(budget).unwrap());
    stop
}

fn buffer_full(engine: &mut ProtocolEngine, id: RequestId, at: Instant) -> Vec<Effect> {
    engine.handle(
        frame(
            1,
            Some(sony_sequence(id)),
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
            },
        ),
        at,
    )
}

fn complete(engine: &mut ProtocolEngine, target: u8, id: RequestId, at: Instant) -> Vec<Effect> {
    engine.handle(
        frame(
            target,
            Some(sony_sequence(id)),
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        at,
    )
}

#[test]
fn halt_retries_hold_only_later_overlapping_motion_and_release_on_ack() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let halt = zoom_halt_stop(1, now, Duration::from_secs(1));
    let (write, stop) = admit(&mut engine, 1, halt, now);
    send_ok(&mut engine, &write, Some(sony_sequence(stop)), now);
    let (queued, overlapping) = admit(
        &mut engine,
        2,
        declared_motion(1, AffectedAxes::ZOOM | AffectedAxes::FOCUS, 2),
        now,
    );
    assert!(request_transmit_optional(&queued).is_none());

    let refused = buffer_full(&mut engine, stop, now);
    let (_, _, retry_at) = retry_scheduled(&refused).expect("STOP retries buffer-full");
    assert!(request_transmit_optional(&refused).is_none());
    for (ticket, target, axis) in [(3, 1, AffectedAxes::FOCUS), (4, 2, AffectedAxes::ZOOM)] {
        let (write, other) = admit(
            &mut engine,
            ticket,
            declared_motion(target, axis, ticket),
            now,
        );
        assert_eq!(request_transmit(&write).1, other);
        send_ok(&mut engine, &write, Some(sony_sequence(other)), now);
        let applied = complete(&mut engine, target, other, now);
        assert!(matches!(
            terminal_outcome(&applied, other),
            Some(RuntimeOutcome::Command(CommandOutcome::Applied))
        ));
        assert!(request_transmit_optional(&applied).is_none());
    }
    assert_occupancy_agrees(&engine, now);
    assert_eq!(engine.next_wake(now), Some(retry_at));
    let retry = engine.advance(retry_at);
    assert_eq!(request_transmit(&retry).1, stop);
    send_ok(&mut engine, &retry, Some(sony_sequence(stop)), retry_at);
    let accepted = engine.handle(
        frame(
            1,
            Some(sony_sequence(stop)),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        retry_at,
    );
    assert_eq!(request_transmit(&accepted).1, overlapping);
    assert!(matches!(
        phase_of(&engine, stop),
        Some(Phase::Executing { .. })
    ));
    engine.assert_invariants().unwrap();
}

#[test]
fn expired_ready_halt_releases_motion_without_writing_again() {
    let now = Instant::now();
    let budget = Duration::from_millis(25);
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let mut halt = zoom_halt_stop(1, now, budget);
    halt.context_mut().control.minimum_spacing = Duration::from_millis(100);
    let (write, stop) = admit(&mut engine, 1, halt, now);
    send_ok(&mut engine, &write, Some(sony_sequence(stop)), now);
    let refused = buffer_full(&mut engine, stop, now);
    let (_, _, retry_at) = retry_scheduled(&refused).expect("STOP retries buffer-full");
    let (queued, later) = admit(
        &mut engine,
        2,
        declared_motion(1, AffectedAxes::ZOOM, 2),
        now,
    );
    assert!(request_transmit_optional(&queued).is_none());

    let paced = engine.advance(retry_at);
    assert!(request_transmit_optional(&paced).is_none());
    assert!(matches!(phase_of(&engine, stop), Some(Phase::Ready)));
    assert_eq!(engine.next_wake(retry_at), Some(now + budget));
    let expired = engine.advance(now + budget);
    assert!(matches!(
        terminal_failure(&expired, stop),
        Some(Error::CommandBufferFull)
    ));
    assert_eq!(request_transmit(&expired).1, later);
    assert!(engine.entry(stop).is_none());
    engine.assert_invariants().unwrap();
}

#[test]
fn second_halt_fences_older_motion_and_keeps_the_first_stop_ordered() {
    let now = Instant::now();
    let budget = Duration::from_secs(1);
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (first_write, first) = admit(&mut engine, 1, zoom_halt_stop(1, now, budget), now);
    send_ok(&mut engine, &first_write, Some(sony_sequence(first)), now);
    let refused = buffer_full(&mut engine, first, now);
    let (_, _, retry_at) = retry_scheduled(&refused).expect("first STOP retries buffer-full");
    let (queued, intervening) = admit(
        &mut engine,
        2,
        declared_motion(1, AffectedAxes::ZOOM, 2),
        now,
    );
    assert!(request_transmit_optional(&queued).is_none());

    let turn = engine.begin_input_turn(now);
    let fenced = engine.halt_in_turn(&turn, camera(1), AffectedAxes::ZOOM, 3);
    assert!(matches!(
        terminal_failure(&fenced, intervening),
        Some(Error::MotionSuperseded { .. })
    ));
    let (second_write, second) = admit(&mut engine, 3, zoom_halt_stop(3, now, budget), now);
    assert_eq!(request_transmit(&second_write).1, second);
    send_ok(&mut engine, &second_write, Some(sony_sequence(second)), now);
    let (queued, later) = admit(
        &mut engine,
        4,
        declared_motion(1, AffectedAxes::ZOOM, 4),
        now,
    );
    assert!(request_transmit_optional(&queued).is_none());
    let second_applied = complete(&mut engine, 1, second, now);
    assert!(matches!(
        terminal_outcome(&second_applied, second),
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
    ));
    assert!(request_transmit_optional(&second_applied).is_none());

    let retry = engine.advance(retry_at);
    assert_eq!(request_transmit(&retry).1, first);
    send_ok(&mut engine, &retry, Some(sony_sequence(first)), retry_at);
    let first_applied = complete(&mut engine, 1, first, retry_at);
    assert!(matches!(
        terminal_outcome(&first_applied, first),
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
    ));
    assert_eq!(request_transmit(&first_applied).1, later);
    engine.assert_invariants().unwrap();
}
