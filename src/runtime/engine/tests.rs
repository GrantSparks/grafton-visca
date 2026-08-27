#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::{
    collections::{BTreeMap, VecDeque},
    num::NonZeroU64,
    sync::Arc,
    time::Duration,
};

use smallvec::{smallvec, SmallVec};

use super::*;

const POWER: InquiryRoute = InquiryRoute(1);
const ZOOM: InquiryRoute = InquiryRoute(2);
const FOCUS: InquiryRoute = InquiryRoute(3);

fn camera(value: u8) -> CameraId {
    CameraId::new(value).unwrap()
}

fn policy(envelope: EnvelopeKind, transport: TransportKind) -> ProtocolPolicy {
    ProtocolPolicy {
        capacity: 16,
        envelope,
        transport,
        inquiry_capacity: 8,
        command_spacing: Duration::ZERO,
        inquiry_spacing: Duration::ZERO,
        inquiry_cooldown: Duration::from_millis(25),
    }
}

fn retrying() -> RetryPolicy {
    RetryPolicy {
        max_retries: 3,
        initial_backoff: Duration::from_millis(10),
        maximum_backoff: Duration::from_millis(100),
        total_budget: Duration::from_secs(10),
        ack_timeout: true,
        completion_timeout: true,
        inquiry_timeout: true,
        buffer_full: true,
        movement_not_executable: true,
        builtin_inquiry_syntax: true,
    }
}

fn context(target: u8, cancellation: CancellationPolicy) -> RequestContext {
    RequestContext {
        target: camera(target),
        timeout: TimeoutPolicy {
            ack: Duration::from_millis(20),
            completion: Duration::from_millis(40),
            inquiry: Duration::from_millis(30),
            cancellation: Duration::from_millis(10),
            ambiguity: Duration::from_millis(50),
        },
        retry: retrying(),
        control: ControlPolicy::default(),
        cancellation,
    }
}

fn wire(address: u8) -> Arc<EncodedMessage> {
    Arc::new(EncodedMessage::new(&[address, 0x01, 0x04, 0x00, 0xff]).unwrap())
}

fn command(target: u8, cancellation: CancellationPolicy) -> RuntimeRequest {
    RuntimeRequest::Command {
        wire: wire(0x80 | target),
        context: context(target, cancellation),
        applied_state: None,
    }
}

fn inquiry(target: u8, route: InquiryRoute) -> RuntimeRequest {
    inquiry_with_retry(target, route, retrying())
}

fn inquiry_with_retry(target: u8, route: InquiryRoute, retry: RetryPolicy) -> RuntimeRequest {
    let mut inquiry_context = context(target, CancellationPolicy::Supported);
    inquiry_context.retry = retry;
    RuntimeRequest::Inquiry {
        wire: wire(0x80 | target),
        context: inquiry_context,
        route,
    }
}

fn engine(envelope: EnvelopeKind, transport: TransportKind) -> ProtocolEngine {
    let mut engine = ProtocolEngine::new(policy(envelope, transport)).unwrap();
    engine
        .register_target(
            camera(1),
            TargetPolicy {
                command_sockets: 2,
                cancellation: CancellationPolicy::Supported,
            },
        )
        .unwrap();
    engine
        .register_target(
            camera(2),
            TargetPolicy {
                command_sockets: 2,
                cancellation: CancellationPolicy::Supported,
            },
        )
        .unwrap();
    engine
}

fn admitted(effects: &[Effect]) -> RequestId {
    effects
        .iter()
        .find_map(|effect| match effect {
            Effect::Admitted { id, .. } => Some(*id),
            _ => None,
        })
        .expect("admission effect")
}

fn request_transmit(effects: &[Effect]) -> (TransmissionId, RequestId, Arc<EncodedMessage>) {
    effects
        .iter()
        .find_map(|effect| match effect {
            Effect::Transmit {
                transmission,
                request,
                kind: Transmission::Request { wire, .. },
            } => Some((*transmission, *request, Arc::clone(wire))),
            _ => None,
        })
        .expect("request transmission")
}

fn cancel_transmit(effects: &[Effect]) -> (TransmissionId, RequestId, ViscaSocket) {
    effects
        .iter()
        .find_map(|effect| match effect {
            Effect::Transmit {
                transmission,
                request,
                kind: Transmission::Cancel { socket, .. },
            } => Some((*transmission, *request, *socket)),
            _ => None,
        })
        .expect("cancel transmission")
}

fn send_ok(
    engine: &mut ProtocolEngine,
    effects: &[Effect],
    sequence: Option<u32>,
    now: Instant,
) -> Vec<Effect> {
    let (transmission, _, _) = request_transmit(effects);
    engine.handle(
        Input::TransmissionFinished {
            transmission,
            result: Ok(TransmissionMeta { sequence }),
        },
        now,
    )
}

fn frame(target: u8, sequence: Option<(u32, SequenceWidth)>, response: DecodedResponse) -> Input {
    Input::Frame(DecodedFrame {
        target: camera(target),
        sequence: sequence.map(|(value, width)| EnvelopeSequence { value, width }),
        response,
    })
}

fn terminal_id(effects: &[Effect]) -> Option<RequestId> {
    effects.iter().find_map(|effect| match effect {
        Effect::Terminal { id, .. } => Some(*id),
        _ => None,
    })
}

#[test]
fn inert_wire_is_inline_and_reused_across_retry_without_reallocation() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let prepared = wire(0x81);
    assert!(prepared.is_inline());
    let request = RuntimeRequest::Command {
        wire: Arc::clone(&prepared),
        context: context(1, CancellationPolicy::Supported),
        applied_state: None,
    };
    let admitted_effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request,
        },
        start + Duration::from_micros(2),
    );
    let (first_tx, id, first_wire) = request_transmit(&admitted_effects);
    assert!(Arc::ptr_eq(&prepared, &first_wire));
    engine.handle(
        Input::TransmissionFinished {
            transmission: first_tx,
            result: Ok(TransmissionMeta { sequence: None }),
        },
        start,
    );
    let deadline = start + Duration::from_millis(20);
    let timeout_effects = engine.handle(Input::Wake, deadline);
    assert!(timeout_effects
        .iter()
        .any(|effect| matches!(effect, Effect::RetryScheduled { id: retry, .. } if *retry == id)));
    let retry_effects = engine.advance(deadline + Duration::from_millis(10));
    let (_, retried, retry_wire) = request_transmit(&retry_effects);
    assert_eq!(retried, id);
    assert!(Arc::ptr_eq(&prepared, &retry_wire));
    engine.assert_invariants().unwrap();
}

#[test]
fn explicit_target_not_wire_bytes_drives_independent_socket_ownership() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    // Both requests intentionally carry Camera 1 in the inert bytes.
    let mut target_two = command(2, CancellationPolicy::Supported);
    if let RuntimeRequest::Command { wire, .. } = &mut target_two {
        *wire = wire_for_test(&[0x81, 0x01, 0xff]);
    }
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let id_one = admitted(&first);
    send_ok(&mut engine, &first, None, start);
    let second = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: target_two,
        },
        start,
    );
    let id_two = admitted(&second);
    send_ok(&mut engine, &second, None, start);
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    engine.handle(
        frame(
            2,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    let done_two = engine.handle(
        frame(
            2,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&done_two), Some(id_two));
    assert!(engine.entry(id_one).is_some());
    engine.assert_invariants().unwrap();
}

fn wire_for_test(bytes: &[u8]) -> Arc<EncodedMessage> {
    Arc::new(EncodedMessage::new(bytes).unwrap())
}

#[test]
fn stale_transmission_and_queue_tickets_are_inert() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let (tx, id, _) = request_transmit(&effects);
    engine.handle(
        Input::TransmissionFinished {
            transmission: tx,
            result: Ok(TransmissionMeta { sequence: None }),
        },
        start,
    );
    let duplicate = engine.handle(
        Input::TransmissionFinished {
            transmission: tx,
            result: Err(Error::Timeout),
        },
        start,
    );
    assert!(duplicate
        .iter()
        .any(|effect| matches!(effect, Effect::Ignored(IgnoreReason::StaleTransmission))));
    let entry = engine.entry(id).unwrap();
    engine.inject_queue_ticket(
        false,
        3,
        QueueTicket {
            request: id,
            generation: GenerationTicket(entry.generation.0.wrapping_add(1)),
            queue_generation: 99,
        },
    );
    let effects = engine.advance(start);
    assert!(!effects
        .iter()
        .any(|effect| matches!(effect, Effect::Transmit { request, .. } if *request == id)));
    engine.assert_invariants().unwrap();
}

#[test]
fn request_transmission_and_generation_allocators_wrap_without_aliasing() {
    let start = Instant::now();
    let mut policy = policy(EnvelopeKind::Raw, TransportKind::Datagram);
    policy.command_spacing = Duration::from_secs(1);
    let mut engine = ProtocolEngine::new(policy).unwrap();
    engine
        .register_target(
            camera(1),
            TargetPolicy {
                command_sockets: 2,
                cancellation: CancellationPolicy::Supported,
            },
        )
        .unwrap();
    engine.seed_allocators(u64::MAX, u64::MAX, u64::MAX);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let first_id = admitted(&first);
    let (first_tx, _, _) = request_transmit(&first);
    assert_eq!(first_id.get(), u64::MAX);
    assert_eq!(first_tx.get(), u64::MAX);
    engine.handle(
        Input::TransmissionFinished {
            transmission: first_tx,
            result: Ok(TransmissionMeta { sequence: None }),
        },
        start,
    );
    let second = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let second_id = admitted(&second);
    assert_eq!(second_id.get(), 1);
    assert_ne!(
        engine.entry(first_id).unwrap().generation,
        engine.entry(second_id).unwrap().generation
    );
    let dispatched = engine.advance(start + Duration::from_secs(1));
    let (second_tx, request, _) = request_transmit(&dispatched);
    assert_eq!(request, second_id);
    assert_eq!(second_tx.get(), 1);
    engine.assert_invariants().unwrap();
}

#[test]
fn sony_exact_and_unique_lower16_are_target_safe_and_owner_deduplicated() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let a = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: inquiry_with_retry(1, POWER, RetryPolicy::NEVER),
        },
        start,
    );
    let a_id = admitted(&a);
    send_ok(&mut engine, &a, Some(0x1111_beef), start);
    // Re-registering the same sequence for the same owner does not create a
    // false lower16 collision.
    engine.register_sequence(a_id, 0x1111_beef, CorrelationKind::Request);
    let unique = engine.handle(
        frame(
            1,
            Some((0xbeef, SequenceWidth::Lower16)),
            DecodedResponse::InquiryReply {
                route: Some(POWER),
                payload: smallvec![1],
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&unique), Some(a_id));

    let b = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: inquiry(1, ZOOM),
        },
        start,
    );
    let b_id = admitted(&b);
    send_ok(&mut engine, &b, Some(0x2222_beef), start);
    let c = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: inquiry(1, FOCUS),
        },
        start,
    );
    let c_id = admitted(&c);
    send_ok(&mut engine, &c, Some(0x3333_beef), start);
    let collision = engine.handle(
        frame(
            1,
            Some((0xbeef, SequenceWidth::Lower16)),
            DecodedResponse::InquiryReply {
                route: Some(ZOOM),
                payload: smallvec![2],
            },
        ),
        start,
    );
    assert!(collision.iter().any(|effect| matches!(
        effect,
        Effect::Ignored(IgnoreReason::AmbiguousLower16Sequence)
    )));
    let exact = engine.handle(
        frame(
            1,
            Some((0x3333_beef, SequenceWidth::Full32)),
            DecodedResponse::InquiryReply {
                route: Some(FOCUS),
                payload: smallvec![3],
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&exact), Some(c_id));
    let now_unique = engine.handle(
        frame(
            1,
            Some((0xbeef, SequenceWidth::Lower16)),
            DecodedResponse::InquiryReply {
                route: Some(ZOOM),
                payload: smallvec![2],
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&now_unique), Some(b_id));

    let wrong_target = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(4),
            request: inquiry_with_retry(1, POWER, RetryPolicy::NEVER),
        },
        start,
    );
    send_ok(&mut engine, &wrong_target, Some(0x1234_5678), start);
    let ignored = engine.handle(
        frame(
            2,
            Some((0x1234_5678, SequenceWidth::Full32)),
            DecodedResponse::InquiryReply {
                route: Some(POWER),
                payload: smallvec![0],
            },
        ),
        start,
    );
    assert!(ignored.iter().any(|effect| matches!(
        effect,
        Effect::Ignored(IgnoreReason::TargetIncompatibleSequence)
    )));
    engine.assert_invariants().unwrap();
}

#[test]
fn raw_inquiries_route_by_unique_content_then_per_target_fifo() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let mut ids = Vec::new();
    for (ticket, target, route) in [(1, 1, POWER), (2, 1, ZOOM), (3, 1, POWER), (4, 2, POWER)] {
        let admitted_effects = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(ticket),
                request: inquiry(target, route),
            },
            start,
        );
        ids.push(admitted(&admitted_effects));
        send_ok(&mut engine, &admitted_effects, None, start);
    }
    let zoom = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: Some(ZOOM),
                payload: smallvec![9],
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&zoom), Some(ids[1]));
    let target_two_fifo = engine.handle(
        frame(
            2,
            None,
            DecodedResponse::InquiryReply {
                route: None,
                payload: smallvec![0],
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&target_two_fifo), Some(ids[3]));
    let ambiguous_power_fifo = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: Some(POWER),
                payload: smallvec![1],
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&ambiguous_power_fifo), Some(ids[0]));
    let unknown_fifo = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: None,
                payload: smallvec![2],
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&unknown_fifo), Some(ids[2]));
    engine.assert_invariants().unwrap();
}

#[test]
fn raw_error_policy_is_socket_then_inquiry_fifo_then_recent_command() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let old = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let old_id = admitted(&old);
    send_ok(&mut engine, &old, None, start);
    let new = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1, CancellationPolicy::Supported),
        },
        start + Duration::from_micros(1),
    );
    let new_id = admitted(&new);
    send_ok(&mut engine, &new, None, start + Duration::from_micros(1));
    let recent = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x01,
            },
        ),
        start + Duration::from_micros(2),
    );
    assert_eq!(terminal_id(&recent), Some(new_id));
    assert!(engine.entry(old_id).is_some());

    let inquiry = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: inquiry(1, POWER),
        },
        start + Duration::from_micros(3),
    );
    let inquiry_id = admitted(&inquiry);
    send_ok(
        &mut engine,
        &inquiry,
        None,
        start + Duration::from_micros(3),
    );
    let fifo = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x01,
            },
        ),
        start + Duration::from_micros(4),
    );
    assert_eq!(terminal_id(&fifo), Some(inquiry_id));
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S2),
            },
        ),
        start + Duration::from_micros(5),
    );
    let socket = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: Some(ViscaSocket::S2),
                code: 0x01,
            },
        ),
        start + Duration::from_micros(6),
    );
    assert_eq!(terminal_id(&socket), Some(old_id));
    engine.assert_invariants().unwrap();
}

#[test]
fn response_at_exact_deadline_wins_and_equal_deadlines_use_admission_order() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let first_id = admitted(&first);
    send_ok(&mut engine, &first, None, start);
    let at_deadline = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(20),
    );
    assert!(engine.entry(first_id).is_some());
    assert!(!at_deadline.iter().any(|effect| matches!(
        effect,
        Effect::RetryScheduled { id, .. } if *id == first_id
    )));

    let a = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: inquiry_with_retry(1, POWER, RetryPolicy::NEVER),
        },
        start + Duration::from_millis(20),
    );
    let a_id = admitted(&a);
    send_ok(&mut engine, &a, None, start + Duration::from_millis(20));
    let b = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: inquiry_with_retry(2, POWER, RetryPolicy::NEVER),
        },
        start + Duration::from_millis(20),
    );
    let b_id = admitted(&b);
    send_ok(&mut engine, &b, None, start + Duration::from_millis(20));
    let due = engine.advance(start + Duration::from_millis(50));
    let terminals: Vec<_> = due
        .iter()
        .filter_map(|effect| match effect {
            Effect::Terminal { id, .. } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(terminals, vec![a_id, b_id]);
    engine.assert_invariants().unwrap();
}

#[test]
fn exact_first_dispatch_preserves_global_inquiry_preference_and_queues_the_loser() {
    let start = Instant::now();
    let mut inquiry_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let command_effects = inquiry_engine.admit_without_due(
        AdmissionTicket(1),
        command(1, CancellationPolicy::Supported),
        start,
    );
    let command_id = admitted(&command_effects);
    let inquiry_effects =
        inquiry_engine.admit_without_due(AdmissionTicket(2), inquiry(2, POWER), start);
    let inquiry_id = admitted(&inquiry_effects);
    let command_before = (
        inquiry_engine.entry(command_id).unwrap().phase(),
        inquiry_engine.entry(command_id).unwrap().cancellation(),
    );
    let inquiry_before = (
        inquiry_engine.entry(inquiry_id).unwrap().phase(),
        inquiry_engine.entry(inquiry_id).unwrap().cancellation(),
    );

    assert!(matches!(
        inquiry_engine.first_dispatch_without_due(command_id, start),
        FirstDispatch::Blocked
    ));
    assert_eq!(
        inquiry_engine
            .entry(command_id)
            .map(|entry| (entry.phase(), entry.cancellation())),
        Some(command_before)
    );
    assert_eq!(
        inquiry_engine
            .entry(inquiry_id)
            .map(|entry| (entry.phase(), entry.cancellation())),
        Some(inquiry_before)
    );
    assert!(inquiry_engine.transmissions.is_empty());
    assert!(inquiry_engine.last_request_sent.is_none());

    let inquiry_dispatch = match inquiry_engine.first_dispatch_without_due(inquiry_id, start) {
        FirstDispatch::Effects(effects) => effects,
        other => panic!("expected inquiry dispatch effects, got {other:?}"),
    };
    let (_, dispatched_id, _) = request_transmit(&inquiry_dispatch);
    assert_eq!(dispatched_id, inquiry_id);

    // Issue #561: losing the race leaves the command queued, never terminal.
    assert_eq!(
        inquiry_engine
            .entry(command_id)
            .map(|entry| (entry.phase(), entry.cancellation())),
        Some(command_before)
    );
    let command_dispatch = match inquiry_engine.first_dispatch_without_due(command_id, start) {
        FirstDispatch::Effects(effects) => effects,
        other => panic!("expected the queued command to dispatch next, got {other:?}"),
    };
    assert_eq!(request_transmit(&command_dispatch).1, command_id);
    assert!(inquiry_engine.entry(inquiry_id).is_some());
    inquiry_engine.assert_invariants().unwrap();

    let mut priority_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let mut urgent = command(2, CancellationPolicy::Supported);
    match &mut urgent {
        RuntimeRequest::Command { context, .. } => {
            context.control.class = ControlClass::Urgent;
        }
        RuntimeRequest::Inquiry { .. } => unreachable!(),
    }
    let urgent_effects = priority_engine.admit_without_due(AdmissionTicket(3), urgent, start);
    let urgent_id = admitted(&urgent_effects);
    let normal_effects = priority_engine.admit_without_due(
        AdmissionTicket(4),
        command(1, CancellationPolicy::Supported),
        start,
    );
    let normal_id = admitted(&normal_effects);
    assert!(matches!(
        priority_engine.first_dispatch_without_due(normal_id, start),
        FirstDispatch::Blocked
    ));
    assert!(priority_engine.transmissions.is_empty());
    let urgent_dispatch = match priority_engine.first_dispatch_without_due(urgent_id, start) {
        FirstDispatch::Effects(effects) => effects,
        other => panic!("expected urgent dispatch effects, got {other:?}"),
    };
    assert_eq!(request_transmit(&urgent_dispatch).1, urgent_id);
    priority_engine.assert_invariants().unwrap();
}

#[test]
fn ordered_input_turn_applies_all_frames_before_an_equal_deadline() {
    let start = Instant::now();
    let deadline = start + Duration::from_millis(40);
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);

    // A is already executing on S1 and its completion deadline is exactly the
    // timestamp at which the owner receives the two-frame batch below.
    let a = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let a_id = admitted(&a);
    send_ok(&mut engine, &a, None, start);
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );

    // B's ACK deadline is the same instant. Cancellation intent makes its ACK
    // produce a cancel transmission, which the owner must drain before it
    // applies the next frame from the wire batch.
    let b_started = start + Duration::from_millis(20);
    let b = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1, CancellationPolicy::Supported),
        },
        b_started,
    );
    let b_id = admitted(&b);
    send_ok(&mut engine, &b, None, b_started);
    let recorded = engine.handle(
        Input::Cancel { id: b_id },
        deadline - Duration::from_millis(1),
    );
    assert!(recorded
        .iter()
        .any(|effect| matches!(effect, Effect::CancellationRecorded { id } if *id == b_id)));

    let turn = engine.begin_input_turn(deadline);
    let mut ordered_effects = Vec::new();

    // Wire frame 1: ACK(B). Drain its cancel-transmit effect recursively at
    // the same sampled instant before consuming wire frame 2.
    let ack_b = engine.handle_in_turn(
        &turn,
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S2),
            },
        ),
    );
    let (cancel_tx, cancel_id, cancel_socket) = cancel_transmit(&ack_b);
    assert_eq!(cancel_id, b_id);
    assert_eq!(cancel_socket, ViscaSocket::S2);
    ordered_effects.extend(ack_b);
    engine.assert_invariants().unwrap();
    ordered_effects.extend(engine.handle_in_turn(
        &turn,
        Input::TransmissionFinished {
            transmission: cancel_tx,
            result: Ok(TransmissionMeta { sequence: None }),
        },
    ));
    assert!(matches!(
        engine.entry(b_id).map(Entry::cancellation),
        Some(CancelState::AwaitingTerminal { .. })
    ));
    engine.assert_invariants().unwrap();

    // Wire frame 2: Completion(A). It must be applied before due work runs,
    // even though A's deadline equals the turn timestamp.
    ordered_effects.extend(engine.handle_in_turn(
        &turn,
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
    ));
    engine.assert_invariants().unwrap();
    ordered_effects.extend(engine.finish_input_turn(turn));

    let cancel_index = ordered_effects
        .iter()
        .position(|effect| {
            matches!(
                effect,
                Effect::Transmit {
                    request,
                    kind: Transmission::Cancel { .. },
                    ..
                } if *request == b_id
            )
        })
        .expect("B cancel transmission");
    let applied_index = ordered_effects
        .iter()
        .position(|effect| {
            matches!(
                effect,
                Effect::Terminal {
                    id,
                    outcome: RuntimeOutcome::Applied,
                } if *id == a_id
            )
        })
        .expect("A applied terminal");
    assert!(cancel_index < applied_index);
    assert!(ordered_effects.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id,
            outcome: RuntimeOutcome::Applied,
        } if *id == a_id
    )));
    assert!(!ordered_effects.iter().any(|effect| matches!(
        effect,
        Effect::RetryScheduled { id, .. } if *id == a_id
    )));
    assert!(!ordered_effects.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id,
            outcome: RuntimeOutcome::Failed(_),
        } if *id == a_id
    )));
    assert!(engine.entry(a_id).is_none());
    engine.assert_invariants().unwrap();
}

fn engine_with_target(
    envelope: EnvelopeKind,
    transport: TransportKind,
    sockets: u8,
    cancellation: CancellationPolicy,
) -> ProtocolEngine {
    let mut engine = ProtocolEngine::new(policy(envelope, transport)).unwrap();
    engine
        .register_target(
            camera(1),
            TargetPolicy {
                command_sockets: sockets,
                cancellation,
            },
        )
        .unwrap();
    engine
}

#[test]
fn unsupported_target_cancels_queued_locally_but_sent_without_intent() {
    let start = Instant::now();
    let mut engine = engine_with_target(
        EnvelopeKind::Raw,
        TransportKind::Datagram,
        1,
        CancellationPolicy::Unsupported,
    );
    let active = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Unsupported),
        },
        start,
    );
    let active_id = admitted(&active);
    send_ok(&mut engine, &active, None, start);
    let queued = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1, CancellationPolicy::Unsupported),
        },
        start,
    );
    let queued_id = admitted(&queued);
    assert!(request_transmit_optional(&queued).is_none());
    let local = engine.handle(Input::Cancel { id: queued_id }, start);
    assert!(local
        .iter()
        .any(|effect| matches!(effect, Effect::CancellationRecorded { id } if *id == queued_id)));
    assert!(local.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id,
            outcome: RuntimeOutcome::Cancelled
        } if *id == queued_id
    )));
    assert!(!local.iter().any(|effect| matches!(
        effect,
        Effect::Transmit {
            kind: Transmission::Cancel { .. },
            ..
        }
    )));

    let unsupported = engine.handle(Input::Cancel { id: active_id }, start);
    assert!(unsupported.iter().any(|effect| matches!(
        effect,
        Effect::CancellationObservation {
            id,
            observation: CancellationObservation::Failed(Error::NotSupported)
        } if *id == active_id
    )));
    assert_eq!(
        engine.entry(active_id).unwrap().cancellation(),
        CancelState::None
    );
    let timeout = engine.advance(start + Duration::from_millis(20));
    assert!(timeout.iter().any(|effect| matches!(
        effect,
        Effect::RetryScheduled { id, .. } if *id == active_id
    )));
    engine.assert_invariants().unwrap();
}

fn request_transmit_optional(
    effects: &[Effect],
) -> Option<(TransmissionId, RequestId, Arc<EncodedMessage>)> {
    effects.iter().find_map(|effect| match effect {
        Effect::Transmit {
            transmission,
            request,
            kind: Transmission::Request { wire, .. },
        } => Some((*transmission, *request, Arc::clone(wire))),
        _ => None,
    })
}

#[test]
fn sending_and_pre_ack_cancellation_record_intent_then_emit_one_cancel_on_ack() {
    use crate::command::semantics::WriteOnlyState;

    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let mut request = command(1, CancellationPolicy::Supported);
    if let RuntimeRequest::Command { applied_state, .. } = &mut request {
        *applied_state =
            Some(AppliedStateProjection::set(WriteOnlyState::Spotlight, &[1]).unwrap());
    }
    let admitted_effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request,
        },
        start,
    );
    let (request_tx, id, _) = request_transmit(&admitted_effects);
    let recorded = engine.handle(Input::Cancel { id }, start);
    assert!(recorded
        .iter()
        .any(|effect| matches!(effect, Effect::CancellationRecorded { id: seen } if *seen == id)));
    assert!(!recorded.iter().any(|effect| matches!(
        effect,
        Effect::Transmit {
            kind: Transmission::Cancel { .. },
            ..
        }
    )));
    engine.handle(
        Input::TransmissionFinished {
            transmission: request_tx,
            result: Ok(TransmissionMeta {
                sequence: Some(0x1001),
            }),
        },
        start,
    );
    let ack = engine.handle(
        frame(
            1,
            Some((0x1001, SequenceWidth::Full32)),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(1),
    );
    let (cancel_tx, cancel_id, socket) = cancel_transmit(&ack);
    assert_eq!(cancel_id, id);
    assert_eq!(socket, ViscaSocket::S1);
    let duplicate = engine.handle(
        frame(
            1,
            Some((0x1001, SequenceWidth::Full32)),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(2),
    );
    assert!(!duplicate.iter().any(|effect| matches!(
        effect,
        Effect::Transmit {
            kind: Transmission::Cancel { .. },
            ..
        }
    )));
    engine.handle(
        Input::TransmissionFinished {
            transmission: cancel_tx,
            result: Ok(TransmissionMeta {
                sequence: Some(0x2001),
            }),
        },
        start + Duration::from_millis(2),
    );
    let cancelled = engine.handle(
        frame(
            1,
            Some((0x2001, SequenceWidth::Full32)),
            DecodedResponse::Error {
                socket: Some(ViscaSocket::S1),
                code: 0x04,
            },
        ),
        start + Duration::from_millis(3),
    );
    assert!(cancelled.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id: seen,
            outcome: RuntimeOutcome::Cancelled
        } if *seen == id
    )));
    assert!(!cancelled
        .iter()
        .any(|effect| matches!(effect, Effect::AppliedState { .. })));
    engine.assert_invariants().unwrap();
}

#[test]
fn late_ack_ambiguity_keeps_capacity_and_correlation_until_quarantine() {
    let start = Instant::now();
    let mut engine = engine_with_target(
        EnvelopeKind::Sony,
        TransportKind::Datagram,
        1,
        CancellationPolicy::Supported,
    );
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let (tx, id, _) = request_transmit(&first);
    engine.handle(
        Input::TransmissionFinished {
            transmission: tx,
            result: Ok(TransmissionMeta {
                sequence: Some(0x1010),
            }),
        },
        start,
    );
    engine.handle(Input::Cancel { id }, start);
    let ack_timeout = engine.advance(start + Duration::from_millis(20));
    assert!(ack_timeout.iter().any(|effect| matches!(
        effect,
        Effect::Transition {
            id: seen,
            to: Phase::AwaitingLateAck { .. },
            ..
        } if *seen == id
    )));
    let queued = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1, CancellationPolicy::Supported),
        },
        start + Duration::from_millis(21),
    );
    assert!(request_transmit_optional(&queued).is_none());
    let completed_at_boundary = engine.handle(
        frame(
            1,
            Some((0x1010, SequenceWidth::Full32)),
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(50),
    );
    assert!(completed_at_boundary.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id: seen,
            outcome: RuntimeOutcome::Applied
        } if *seen == id
    )));
    // Completion at the exact ambiguity deadline wins and releases capacity;
    // ordinary dispatch can now select the queued command.
    assert!(request_transmit_optional(&completed_at_boundary).is_some());
    engine.assert_invariants().unwrap();
}

#[test]
fn ambiguity_expiry_is_unconfirmed_and_releases_only_after_deadline() {
    let start = Instant::now();
    let mut engine = engine_with_target(
        EnvelopeKind::Sony,
        TransportKind::Datagram,
        1,
        CancellationPolicy::Supported,
    );
    let effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let (tx, id, _) = request_transmit(&effects);
    engine.handle(
        Input::TransmissionFinished {
            transmission: tx,
            result: Ok(TransmissionMeta {
                sequence: Some(0xabcd),
            }),
        },
        start,
    );
    engine.handle(Input::Cancel { id }, start);
    engine.advance(start + Duration::from_millis(20));
    assert_eq!(engine.active_len(), 1);
    let expired = engine.advance(start + Duration::from_millis(50));
    assert!(expired.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id: seen,
            outcome: RuntimeOutcome::Failed(Error::CancellationUnconfirmed)
        } if *seen == id
    )));
    assert_eq!(engine.active_len(), 0);
    engine.assert_invariants().unwrap();
}

#[test]
fn datagram_cancel_failure_resolves_token_but_original_remains_routable() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let id = admitted(&effects);
    send_ok(&mut engine, &effects, None, start);
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(1),
    );
    let cancellation = engine.handle(Input::Cancel { id }, start + Duration::from_millis(2));
    let (cancel_tx, _, _) = cancel_transmit(&cancellation);
    let failed = engine.handle(
        Input::TransmissionFinished {
            transmission: cancel_tx,
            result: Err(Error::TransportError("cancel write failed".into())),
        },
        start + Duration::from_millis(2),
    );
    assert!(failed.iter().any(|effect| matches!(
        effect,
        Effect::CancellationObservation {
            id: seen,
            observation: CancellationObservation::Failed(Error::TransportError(_))
        } if *seen == id
    )));
    assert!(engine.entry(id).is_some());
    assert_eq!(engine.socket_owner(camera(1), ViscaSocket::S1), Some(id));
    let completion = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(3),
    );
    assert!(completion.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id: seen,
            outcome: RuntimeOutcome::Applied
        } if *seen == id
    )));
    assert!(!completion
        .iter()
        .any(|effect| matches!(effect, Effect::CancellationObservation { .. })));
    engine.assert_invariants().unwrap();
}

#[test]
fn stream_cancel_failure_poisons_and_terminalizes_in_admission_order() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let first_id = admitted(&first);
    send_ok(&mut engine, &first, None, start);
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    let second = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(2, CancellationPolicy::Supported),
        },
        start,
    );
    let second_id = admitted(&second);
    send_ok(&mut engine, &second, None, start);
    let cancellation = engine.handle(Input::Cancel { id: first_id }, start);
    let (cancel_tx, _, _) = cancel_transmit(&cancellation);
    let poison = engine.handle(
        Input::TransmissionFinished {
            transmission: cancel_tx,
            result: Err(Error::Timeout),
        },
        start,
    );
    assert_eq!(engine.state(), SessionState::Poisoned);
    let terminals: Vec<_> = poison
        .iter()
        .filter_map(|effect| match effect {
            Effect::Terminal { id, .. } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(terminals, vec![first_id, second_id]);
    assert!(
        poison
            .iter()
            .filter(|effect| matches!(
                effect,
                Effect::Terminal {
                    outcome: RuntimeOutcome::Failed(Error::StreamPoisoned { .. }),
                    ..
                }
            ))
            .count()
            == 2
    );
    engine.assert_invariants().unwrap();
}

#[test]
fn datagram_request_failure_isolated_close_and_shutdown_are_distinct() {
    let start = Instant::now();
    let mut datagram_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let first = datagram_engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let first_id = admitted(&first);
    let (first_tx, _, _) = request_transmit(&first);
    let second = datagram_engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: inquiry(2, POWER),
        },
        start,
    );
    let second_id = admitted(&second);
    send_ok(&mut datagram_engine, &second, None, start);
    let isolated = datagram_engine.handle(
        Input::TransmissionFinished {
            transmission: first_tx,
            result: Err(Error::TransportError("datagram write".into())),
        },
        start,
    );
    assert_eq!(terminal_id(&isolated), Some(first_id));
    assert!(datagram_engine.entry(second_id).is_some());
    let closed = datagram_engine.handle(
        Input::Close {
            reason: Some("peer closed".into()),
        },
        start,
    );
    assert_eq!(datagram_engine.state(), SessionState::Closed);
    assert!(closed.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id,
            outcome: RuntimeOutcome::Failed(Error::ConnectionClosed { .. })
        } if *id == second_id
    )));

    let mut fresh = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let pending = fresh.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: inquiry(1, POWER),
        },
        start,
    );
    let pending_id = admitted(&pending);
    let shutdown = fresh.handle(Input::Shutdown(ShutdownReason::Explicit), start);
    assert_eq!(fresh.state(), SessionState::Shutdown);
    assert!(shutdown.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id,
            outcome: RuntimeOutcome::Failed(Error::RuntimeShutdown)
        } if *id == pending_id
    )));
    fresh.assert_invariants().unwrap();
}

#[test]
fn applied_state_projection_is_target_qualified_and_emitted_only_on_applied() {
    let start = Instant::now();
    let projection = AppliedStateProjection::set(
        crate::command::semantics::WriteOnlyState::PanTiltLimits,
        &[1, 2],
    )
    .unwrap();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let request = RuntimeRequest::Command {
        wire: wire(0x82),
        context: context(2, CancellationPolicy::Supported),
        applied_state: Some(projection),
    };
    let effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request,
        },
        start,
    );
    let id = admitted(&effects);
    send_ok(&mut engine, &effects, None, start);
    engine.handle(
        frame(
            2,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S2),
            },
        ),
        start,
    );
    let completion = engine.handle(
        frame(
            2,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S2),
            },
        ),
        start,
    );
    let applied_position = completion
        .iter()
        .position(|effect| matches!(effect, Effect::AppliedState { .. }))
        .unwrap();
    let terminal_position = completion
        .iter()
        .position(|effect| matches!(effect, Effect::Terminal { .. }))
        .unwrap();
    assert!(applied_position < terminal_position);
    assert!(completion.iter().any(|effect| matches!(
        effect,
        Effect::AppliedState {
            effect: AppliedStateEffect {
                request,
                target,
                projection: seen,
            }
        } if *request == id && *target == camera(2) && *seen == projection
    )));
    engine.assert_invariants().unwrap();
}

#[test]
fn applied_state_actions_are_closed_and_failure_does_not_emit_one() {
    use crate::command::semantics::WriteOnlyState;

    let start = Instant::now();
    let projections = [
        AppliedStateProjection::set(WriteOnlyState::Spotlight, &[1]).unwrap(),
        AppliedStateProjection::clear(WriteOnlyState::ImageFreeze),
        AppliedStateProjection::invalidate(WriteOnlyState::TallyMode),
    ];

    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    for (offset, projection) in projections.into_iter().enumerate() {
        let effects = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(offset as u64 + 1),
                request: RuntimeRequest::Command {
                    wire: wire(0x81),
                    context: context(1, CancellationPolicy::Supported),
                    applied_state: Some(projection),
                },
            },
            start,
        );
        let id = admitted(&effects);
        send_ok(&mut engine, &effects, None, start);
        engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start,
        );
        let completion = engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Completion {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start,
        );
        assert!(completion.iter().any(|effect| matches!(
            effect,
            Effect::AppliedState {
                effect: AppliedStateEffect {
                    request,
                    projection: seen,
                    ..
                }
            } if *request == id && *seen == projection
        )));
    }

    let mut context = context(1, CancellationPolicy::Supported);
    context.retry = RetryPolicy::NEVER;
    let failed = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(9),
            request: RuntimeRequest::Command {
                wire: wire(0x81),
                context,
                applied_state: Some(
                    AppliedStateProjection::set(WriteOnlyState::Spotlight, &[0]).unwrap(),
                ),
            },
        },
        start,
    );
    let (transmission, id, _) = request_transmit(&failed);
    let failed = engine.handle(
        Input::TransmissionFinished {
            transmission,
            result: Err(Error::TransportError("request write".into())),
        },
        start,
    );
    assert!(failed.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id: terminal,
            outcome: RuntimeOutcome::Failed(_),
        } if *terminal == id
    )));
    assert!(!failed
        .iter()
        .any(|effect| matches!(effect, Effect::AppliedState { .. })));
    engine.assert_invariants().unwrap();
}

#[test]
fn dispatch_is_priority_fifo_with_equal_priority_inquiry_preference() {
    let start = Instant::now();
    let mut configured = policy(EnvelopeKind::Raw, TransportKind::Datagram);
    configured.command_spacing = Duration::from_millis(10);
    let mut engine = ProtocolEngine::new(configured).unwrap();
    for target in [camera(1), camera(2)] {
        engine
            .register_target(
                target,
                TargetPolicy {
                    command_sockets: 2,
                    cancellation: CancellationPolicy::Supported,
                },
            )
            .unwrap();
    }
    let seed = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    send_ok(&mut engine, &seed, None, start);
    let queued_command = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(2, CancellationPolicy::Supported),
        },
        start,
    );
    let command_id = admitted(&queued_command);
    let inquiry = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: inquiry(2, POWER),
        },
        start,
    );
    let inquiry_id = admitted(&inquiry);
    assert!(request_transmit_optional(&queued_command).is_none());
    assert!(request_transmit_optional(&inquiry).is_none());
    let selected = engine.advance(start + Duration::from_millis(10));
    assert_eq!(request_transmit(&selected).1, inquiry_id);
    assert_ne!(request_transmit(&selected).1, command_id);

    // FIFO among commands of the same private priority.
    let mut fifo = engine_with_target(
        EnvelopeKind::Raw,
        TransportKind::Datagram,
        1,
        CancellationPolicy::Supported,
    );
    let occupying = fifo.handle(
        Input::Admit {
            ticket: AdmissionTicket(10),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    send_ok(&mut fifo, &occupying, None, start);
    fifo.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    let first_queued = fifo.handle(
        Input::Admit {
            ticket: AdmissionTicket(11),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let first_id = admitted(&first_queued);
    let second_queued = fifo.handle(
        Input::Admit {
            ticket: AdmissionTicket(12),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let second_id = admitted(&second_queued);
    let released = fifo.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    assert_eq!(request_transmit(&released).1, first_id);
    assert_ne!(request_transmit(&released).1, second_id);
    fifo.assert_invariants().unwrap();
}

#[test]
fn raw_inquiry_retry_releases_fifo_and_requeues_at_tail_with_same_wire() {
    let start = Instant::now();
    let mut configured = policy(EnvelopeKind::Raw, TransportKind::Datagram);
    configured.inquiry_capacity = 1;
    let mut engine = ProtocolEngine::new(configured).unwrap();
    engine
        .register_target(
            camera(1),
            TargetPolicy {
                command_sockets: 2,
                cancellation: CancellationPolicy::Supported,
            },
        )
        .unwrap();
    let first_wire = wire(0x81);
    let first_request = RuntimeRequest::Inquiry {
        wire: Arc::clone(&first_wire),
        context: context(1, CancellationPolicy::Supported),
        route: POWER,
    };
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: first_request,
        },
        start,
    );
    let first_id = admitted(&first);
    send_ok(&mut engine, &first, None, start);
    let second = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: inquiry(1, ZOOM),
        },
        start,
    );
    let second_id = admitted(&second);
    assert!(request_transmit_optional(&second).is_none());
    let retry = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x02,
            },
        ),
        start,
    );
    assert!(retry.iter().any(|effect| matches!(
        effect,
        Effect::RetryScheduled { id, .. } if *id == first_id
    )));
    assert!(request_transmit_optional(&retry).is_none());
    // The contextual syntax cooldown blocks all inquiries. At release, the
    // already-queued second inquiry remains ahead of the retried first inquiry.
    let promoted = engine.advance(start + Duration::from_millis(25));
    let (_, dispatched, _) = request_transmit(&promoted);
    assert_eq!(dispatched, second_id);
    let second_sent = send_ok(
        &mut engine,
        &promoted,
        None,
        start + Duration::from_millis(25),
    );
    assert!(request_transmit_optional(&second_sent).is_none());
    let second_reply = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: Some(ZOOM),
                payload: smallvec![1],
            },
        ),
        start + Duration::from_millis(25),
    );
    let (_, retried, retried_wire) = request_transmit(&second_reply);
    assert_eq!(retried, first_id);
    assert!(Arc::ptr_eq(&first_wire, &retried_wire));
    engine.handle(
        Input::TransmissionFinished {
            transmission: request_transmit(&second_reply).0,
            result: Ok(TransmissionMeta { sequence: None }),
        },
        start + Duration::from_millis(25),
    );
    assert_eq!(engine.raw_inquiry_front(camera(1)), Some(first_id));
    engine.assert_invariants().unwrap();
}

#[test]
fn replaying_identical_ordered_trace_produces_identical_effects() {
    fn replay(start: Instant) -> Vec<String> {
        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let mut output = Vec::new();
        let admitted_effects = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(7),
                request: command(1, CancellationPolicy::Supported),
            },
            start,
        );
        output.extend(admitted_effects.iter().map(|effect| format!("{effect:?}")));
        let tx = request_transmit(&admitted_effects).0;
        let sent = engine.handle(
            Input::TransmissionFinished {
                transmission: tx,
                result: Ok(TransmissionMeta {
                    sequence: Some(0x1234),
                }),
            },
            start,
        );
        output.extend(sent.iter().map(|effect| format!("{effect:?}")));
        let ack = engine.handle(
            frame(
                1,
                Some((0x1234, SequenceWidth::Full32)),
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S2),
                },
            ),
            start + Duration::from_millis(1),
        );
        output.extend(ack.iter().map(|effect| format!("{effect:?}")));
        let done = engine.handle(
            frame(
                1,
                Some((0x1234, SequenceWidth::Full32)),
                DecodedResponse::Completion {
                    socket: Some(ViscaSocket::S2),
                },
            ),
            start + Duration::from_millis(2),
        );
        output.extend(done.iter().map(|effect| format!("{effect:?}")));
        engine.assert_invariants().unwrap();
        output
    }
    let start = Instant::now();
    assert_eq!(replay(start), replay(start));
}

#[test]
fn phase_one_protocol_fixture_replays_through_production_engine() {
    const TRACE: &str =
        include_str!("../../../tests/fixtures/issue_542/protocol/correlation-routing.trace");

    let base = Instant::now();
    let mut engine: Option<ProtocolEngine> = None;
    let mut envelope: Option<EnvelopeKind> = None;
    let mut names = BTreeMap::<RequestId, String>::new();
    let mut pending_observation: Option<String> = None;
    let mut ticket = 0_u64;

    for raw_line in TRACE.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("trace ") {
            continue;
        }
        if line.starts_with("scenario ") {
            engine = None;
            envelope = None;
            names.clear();
            pending_observation = None;
            ticket = 0;
            continue;
        }
        if line == "end" {
            assert!(pending_observation.is_none());
            if let Some(engine) = &engine {
                engine.assert_invariants().unwrap();
            }
            continue;
        }
        let columns: Vec<_> = line.split('|').map(str::trim).collect();
        assert!(columns.len() >= 3, "invalid fixture record: {line}");
        let micros = columns[0].parse::<u64>().unwrap();
        let now = base + Duration::from_micros(micros);
        let kind = columns[1];
        let fields = fixture_fields(columns[2]);
        match kind {
            "transmit" => {
                let record_envelope = match fixture_required(&fields, "envelope") {
                    "raw" => EnvelopeKind::Raw,
                    "sony" => EnvelopeKind::Sony,
                    other => panic!("unknown fixture envelope {other}"),
                };
                if engine.is_none() {
                    let mut created =
                        ProtocolEngine::new(policy(record_envelope, TransportKind::Datagram))
                            .unwrap();
                    for target in 1..=3 {
                        created
                            .register_target(
                                camera(target),
                                TargetPolicy {
                                    command_sockets: 2,
                                    cancellation: CancellationPolicy::Supported,
                                },
                            )
                            .unwrap();
                    }
                    engine = Some(created);
                    envelope = Some(record_envelope);
                }
                assert_eq!(envelope, Some(record_envelope));
                ticket += 1;
                let target = fixture_required(&fields, "target").parse::<u8>().unwrap();
                let mut fixture_context = context(target, CancellationPolicy::Supported);
                fixture_context.retry = RetryPolicy::NEVER;
                let request = match fixture_required(&fields, "kind") {
                    "command" => RuntimeRequest::Command {
                        wire: wire(0x80 | target),
                        context: fixture_context,
                        applied_state: None,
                    },
                    "inquiry" => RuntimeRequest::Inquiry {
                        wire: wire(0x80 | target),
                        context: fixture_context,
                        route: fixture_route(fields.get("route").map(String::as_str)),
                    },
                    other => panic!("unknown fixture request kind {other}"),
                };
                let engine = engine.as_mut().unwrap();
                let admitted_effects = engine.handle(
                    Input::Admit {
                        ticket: AdmissionTicket(ticket),
                        request,
                    },
                    now,
                );
                let id = admitted(&admitted_effects);
                let name = fixture_required(&fields, "id").to_owned();
                names.insert(id, name);
                let transmission = request_transmit(&admitted_effects).0;
                let sequence = fields.get("sequence").map(|value| fixture_u32(value));
                let result_effects = engine.handle(
                    Input::TransmissionFinished {
                        transmission,
                        result: Ok(TransmissionMeta { sequence }),
                    },
                    now,
                );
                assert!(!result_effects
                    .iter()
                    .any(|effect| matches!(effect, Effect::Terminal { .. } | Effect::Ignored(_))));
            }
            "frame" => {
                let engine = engine.as_mut().unwrap();
                let target = fixture_required(&fields, "target").parse::<u8>().unwrap();
                let sequence = fields.get("sequence").map(|value| EnvelopeSequence {
                    value: fixture_u32(value),
                    width: if fields.get("sequence-width").map(String::as_str) == Some("lower16") {
                        SequenceWidth::Lower16
                    } else {
                        SequenceWidth::Full32
                    },
                });
                let response_name = fixture_required(&fields, "response");
                let response = match response_name {
                    "ack" => DecodedResponse::Ack {
                        socket: Some(fixture_socket(&fields)),
                    },
                    "completion" => DecodedResponse::Completion {
                        socket: Some(fixture_socket(&fields)),
                    },
                    "reply" => DecodedResponse::InquiryReply {
                        route: fields
                            .get("route")
                            .filter(|route| route.as_str() != "unknown")
                            .map(|route| fixture_route(Some(route))),
                        payload: SmallVec::from_slice(
                            fixture_required(&fields, "payload").as_bytes(),
                        ),
                    },
                    "error" => DecodedResponse::Error {
                        socket: fields.get("socket").map(|_| fixture_socket(&fields)),
                        code: fixture_u32(fixture_required(&fields, "code")) as u8,
                    },
                    other => panic!("unknown fixture response {other}"),
                };
                let effects = engine.handle(
                    Input::Frame(DecodedFrame {
                        target: camera(target),
                        sequence,
                        response,
                    }),
                    now,
                );
                pending_observation = Some(fixture_observation(
                    &effects,
                    &names,
                    &fields,
                    response_name,
                    target,
                ));
            }
            "expect" => {
                let expected = format!(
                    "observer={} outcome={}",
                    fixture_required(&fields, "observer"),
                    fixture_required(&fields, "outcome")
                );
                assert_eq!(pending_observation.take().unwrap(), expected, "{line}");
            }
            other => panic!("unknown fixture record {other}"),
        }
    }
}

fn fixture_fields(input: &str) -> BTreeMap<String, String> {
    input
        .split_whitespace()
        .map(|field| {
            let (key, value) = field.split_once('=').unwrap();
            (key.to_owned(), value.to_owned())
        })
        .collect()
}

fn fixture_required<'a>(fields: &'a BTreeMap<String, String>, key: &str) -> &'a str {
    fields.get(key).map(String::as_str).unwrap()
}

fn fixture_u32(value: &str) -> u32 {
    value.strip_prefix("0x").map_or_else(
        || value.parse().unwrap(),
        |hex| u32::from_str_radix(hex, 16).unwrap(),
    )
}

fn fixture_socket(fields: &BTreeMap<String, String>) -> ViscaSocket {
    ViscaSocket::from_socket_number(fixture_required(fields, "socket").parse::<u8>().unwrap())
        .unwrap()
}

fn fixture_route(route: Option<&str>) -> InquiryRoute {
    match route {
        Some("power") => POWER,
        Some("zoom") => ZOOM,
        Some("focus") => FOCUS,
        None | Some("unknown") => InquiryRoute::UNKNOWN,
        Some(other) => panic!("unknown fixture route {other}"),
    }
}

fn fixture_observation(
    effects: &[Effect],
    names: &BTreeMap<RequestId, String>,
    fields: &BTreeMap<String, String>,
    response: &str,
    target: u8,
) -> String {
    if let Some((id, socket)) = effects.iter().find_map(|effect| match effect {
        Effect::Transition {
            id,
            to: Phase::Executing { socket, .. },
            ..
        } => Some((*id, *socket)),
        _ => None,
    }) {
        return format!(
            "observer=engine outcome=ack:{}:target={target}:socket={}",
            names.get(&id).unwrap(),
            socket.as_socket_number()
        );
    }
    if let Some((id, outcome)) = effects.iter().find_map(|effect| match effect {
        Effect::Terminal { id, outcome } => Some((*id, outcome)),
        _ => None,
    }) {
        let name = names.get(&id).unwrap();
        let outcome = match outcome {
            RuntimeOutcome::Applied => "applied".to_owned(),
            RuntimeOutcome::Reply { .. } => format!(
                "reply:{}:{}",
                fixture_required(fields, "route"),
                fixture_required(fields, "payload")
            ),
            RuntimeOutcome::Failed(_) => {
                format!("error:{}", fixture_required(fields, "code"))
            }
            RuntimeOutcome::Cancelled => "cancelled".to_owned(),
        };
        return format!("observer={name} outcome={outcome}");
    }
    let reason = effects.iter().find_map(|effect| match effect {
        Effect::Ignored(reason) => Some(*reason),
        _ => None,
    });
    let outcome = match reason {
        Some(IgnoreReason::TargetIncompatibleSequence) => "ignored:target-incompatible-sequence",
        Some(IgnoreReason::AmbiguousLower16Sequence) => "ignored:ambiguous-lower16-sequence",
        Some(IgnoreReason::UnmatchedSequencedFrame) => "ignored:unmatched-sequenced-reply",
        Some(IgnoreReason::UnmatchedFrame) if response == "completion" => {
            "ignored:unmatched-target-socket"
        }
        Some(_) => "ignored:unmatched-raw-frame",
        None => panic!("frame produced no fixture observation: {effects:?}"),
    };
    format!("observer=diagnostics outcome={outcome}")
}

#[test]
fn arbitrary_stale_and_reordered_inputs_preserve_invariants() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let mut random = 0x5425_49ab_cdef_0123_u64;
    let mut ticket = 0_u64;
    let mut now = start;
    for _ in 0..2_000_u64 {
        random = random
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        now += Duration::from_millis(1);
        match random % 8 {
            0 => {
                ticket = ticket.wrapping_add(1);
                let target = ((random >> 8) & 1) as u8 + 1;
                let request = if random & 0x10000 == 0 {
                    command(target, CancellationPolicy::Supported)
                } else {
                    inquiry(target, InquiryRoute(((random >> 20) as u16 % 3) + 1))
                };
                let _ = engine.handle(
                    Input::Admit {
                        ticket: AdmissionTicket(ticket),
                        request,
                    },
                    now,
                );
            }
            1 => {
                if let Some(transmission) = engine.transmissions.keys().next().copied() {
                    let result = if random & 0x20000 == 0 {
                        Ok(TransmissionMeta { sequence: None })
                    } else {
                        Err(Error::Timeout)
                    };
                    let _ = engine.handle(
                        Input::TransmissionFinished {
                            transmission,
                            result,
                        },
                        now,
                    );
                }
            }
            2 => {
                let target = ((random >> 8) & 1) as u8 + 1;
                let socket = if random & 0x100 == 0 {
                    ViscaSocket::S1
                } else {
                    ViscaSocket::S2
                };
                let response = match (random >> 12) % 4 {
                    0 => DecodedResponse::Ack {
                        socket: Some(socket),
                    },
                    1 => DecodedResponse::Completion {
                        socket: Some(socket),
                    },
                    2 => DecodedResponse::Error {
                        socket: (random & 0x8000 != 0).then_some(socket),
                        code: [0x01, 0x02, 0x03, 0x04, 0x41][(random as usize) % 5],
                    },
                    _ => DecodedResponse::InquiryReply {
                        route: (random & 0x4000 != 0)
                            .then_some(InquiryRoute(((random >> 20) as u16 % 3) + 1)),
                        payload: smallvec![random as u8],
                    },
                };
                let _ = engine.handle(frame(target, None, response), now);
            }
            3 => {
                if let Some(id) = engine.entries.keys().next().copied() {
                    let _ = engine.handle(Input::Cancel { id }, now);
                }
            }
            4 => {
                let _ = engine.advance(now);
            }
            5 => {
                let stale = TransmissionId::from_nonzero(NonZeroU64::new(random | 1).unwrap());
                let _ = engine.handle(
                    Input::TransmissionFinished {
                        transmission: stale,
                        result: Ok(TransmissionMeta { sequence: None }),
                    },
                    now,
                );
            }
            6 => {
                if let Some(wake) = engine.next_wake() {
                    now = wake.max(now);
                    let _ = engine.advance(now);
                }
            }
            _ => {
                let _ = engine.handle(
                    frame(
                        ((random >> 8) & 1) as u8 + 1,
                        None,
                        DecodedResponse::Unknown,
                    ),
                    now,
                );
            }
        }
        engine.assert_invariants().unwrap();
    }
}

mod generated_invariant_properties {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        /// Generated ordered, stale, and reordered inputs must never leave a
        /// derived engine index inconsistent with its authoritative entries.
        #[test]
        fn arbitrary_ordered_and_stale_inputs_preserve_invariants_property(
            actions in prop::collection::vec(any::<u64>(), 1..256)
        ) {
            let start = Instant::now();
            let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
            let mut ticket = 0_u64;
            let mut now = start;

            for action in actions {
                now += Duration::from_micros((action & 0x3f).saturating_add(1));
                match action % 8 {
                    0 => {
                        ticket = ticket.wrapping_add(1);
                        let target = ((action >> 8) & 1) as u8 + 1;
                        let request = if action & 0x10000 == 0 {
                            command(target, CancellationPolicy::Supported)
                        } else {
                            inquiry(
                                target,
                                InquiryRoute(((action >> 20) as u16 % 3) + 1),
                            )
                        };
                        let _ = engine.handle(
                            Input::Admit {
                                ticket: AdmissionTicket(ticket),
                                request,
                            },
                            now,
                        );
                    }
                    1 => {
                        if let Some(transmission) = engine.transmissions.keys().next().copied() {
                            let result = if action & 0x20000 == 0 {
                                Ok(TransmissionMeta { sequence: None })
                            } else {
                                Err(Error::Timeout)
                            };
                            let _ = engine.handle(
                                Input::TransmissionFinished {
                                    transmission,
                                    result,
                                },
                                now,
                            );
                        }
                    }
                    2 => {
                        let target = ((action >> 8) & 1) as u8 + 1;
                        let socket = if action & 0x100 == 0 {
                            ViscaSocket::S1
                        } else {
                            ViscaSocket::S2
                        };
                        let response = match (action >> 12) % 4 {
                            0 => DecodedResponse::Ack { socket: Some(socket) },
                            1 => DecodedResponse::Completion { socket: Some(socket) },
                            2 => DecodedResponse::Error {
                                socket: (action & 0x8000 != 0).then_some(socket),
                                code: [0x01, 0x02, 0x03, 0x04, 0x41]
                                    [(action as usize) % 5],
                            },
                            _ => DecodedResponse::InquiryReply {
                                route: (action & 0x4000 != 0).then_some(InquiryRoute(
                                    ((action >> 20) as u16 % 3) + 1,
                                )),
                                payload: smallvec![action as u8],
                            },
                        };
                        let _ = engine.handle(frame(target, None, response), now);
                    }
                    3 => {
                        if let Some(id) = engine.entries.keys().next().copied() {
                            let _ = engine.handle(Input::Cancel { id }, now);
                        }
                    }
                    4 => {
                        let _ = engine.advance(now);
                    }
                    5 => {
                        let stale = TransmissionId::from_nonzero(
                            NonZeroU64::new(action | 1).expect("normalized property id"),
                        );
                        let _ = engine.handle(
                            Input::TransmissionFinished {
                                transmission: stale,
                                result: Ok(TransmissionMeta { sequence: None }),
                            },
                            now,
                        );
                    }
                    6 => {
                        if let Some(wake) = engine.next_wake() {
                            now = wake.max(now);
                            let _ = engine.advance(now);
                        }
                    }
                    _ => {
                        let _ = engine.handle(
                            frame(
                                ((action >> 8) & 1) as u8 + 1,
                                None,
                                DecodedResponse::Unknown,
                            ),
                            now,
                        );
                    }
                }
                prop_assert!(engine.assert_invariants().is_ok());
            }
        }
    }
}

#[test]
fn raw_late_ack_at_ambiguity_boundary_is_routed_and_cancel_remains_active() {
    let start = Instant::now();
    let mut engine = engine_with_target(
        EnvelopeKind::Raw,
        TransportKind::Datagram,
        1,
        CancellationPolicy::Supported,
    );
    let admitted_effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let id = admitted(&admitted_effects);
    send_ok(&mut engine, &admitted_effects, None, start);
    engine.handle(Input::Cancel { id }, start);
    engine.advance(start + Duration::from_millis(20));

    let boundary = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(50),
    );
    assert!(boundary.iter().any(|effect| matches!(
        effect,
        Effect::Transmit {
            request,
            kind: Transmission::Cancel { .. },
            ..
        } if *request == id
    )));
    assert!(!boundary
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { id: terminal, .. } if *terminal == id)));
    assert!(matches!(
        engine.entry(id).unwrap().cancellation(),
        CancelState::Sending { .. }
    ));
    engine.assert_invariants().unwrap();
}

#[test]
fn full_width_sony_miss_never_uses_lower16_fallback() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: inquiry_with_retry(1, POWER, RetryPolicy::NEVER),
        },
        start,
    );
    let id = admitted(&effects);
    send_ok(&mut engine, &effects, Some(0x1111_beef), start);
    let miss = engine.handle(
        frame(
            1,
            Some((0x2222_beef, SequenceWidth::Full32)),
            DecodedResponse::InquiryReply {
                route: Some(POWER),
                payload: smallvec![1],
            },
        ),
        start,
    );
    assert!(miss.iter().any(|effect| matches!(
        effect,
        Effect::Ignored(IgnoreReason::UnmatchedSequencedFrame)
    )));
    assert!(engine.entry(id).is_some());
    engine.assert_invariants().unwrap();
}

#[test]
fn cancellation_response_timeout_resolves_observer_but_retains_quarantine() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let id = admitted(&effects);
    send_ok(&mut engine, &effects, None, start);
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(1),
    );
    let cancellation = engine.handle(Input::Cancel { id }, start + Duration::from_millis(2));
    let (cancel_tx, _, _) = cancel_transmit(&cancellation);
    engine.handle(
        Input::TransmissionFinished {
            transmission: cancel_tx,
            result: Ok(TransmissionMeta { sequence: None }),
        },
        start + Duration::from_millis(2),
    );
    let observer_timeout = engine.advance(start + Duration::from_millis(12));
    assert!(observer_timeout.iter().any(|effect| matches!(
        effect,
        Effect::CancellationObservation {
            id: seen,
            observation: CancellationObservation::Failed(Error::Timeout)
        } if *seen == id
    )));
    assert!(engine.entry(id).is_some());
    assert_eq!(engine.socket_owner(camera(1), ViscaSocket::S1), Some(id));
    let quarantine = engine.advance(start + Duration::from_millis(52));
    assert!(quarantine.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id: seen,
            outcome: RuntimeOutcome::Failed(Error::CancellationUnconfirmed)
        } if *seen == id
    )));
    engine.assert_invariants().unwrap();
}

#[test]
fn datagram_cancel_failure_keeps_ownership_until_ambiguity_deadline() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let mut request_context = context(1, CancellationPolicy::Supported);
    request_context.timeout.completion = Duration::from_millis(5);
    request_context.timeout.ambiguity = Duration::from_millis(50);
    let effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: RuntimeRequest::Command {
                wire: wire(0x81),
                context: request_context,
                applied_state: None,
            },
        },
        start,
    );
    let id = admitted(&effects);
    send_ok(&mut engine, &effects, None, start);
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(1),
    );
    let cancellation = engine.handle(Input::Cancel { id }, start + Duration::from_millis(2));
    let (cancel_tx, _, _) = cancel_transmit(&cancellation);
    engine.handle(
        Input::TransmissionFinished {
            transmission: cancel_tx,
            result: Err(Error::TransportError("cancel write".into())),
        },
        start + Duration::from_millis(2),
    );
    let completion_timeout = engine.advance(start + Duration::from_millis(6));
    assert!(!completion_timeout
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { id: seen, .. } if *seen == id)));
    assert_eq!(engine.socket_owner(camera(1), ViscaSocket::S1), Some(id));
    let expired = engine.advance(start + Duration::from_millis(52));
    assert_eq!(terminal_id(&expired), Some(id));
    engine.assert_invariants().unwrap();
}

#[test]
fn retryable_rejection_after_cancel_intent_is_cancelled_without_retry() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let id = admitted(&effects);
    send_ok(&mut engine, &effects, None, start);
    engine.handle(Input::Cancel { id }, start);
    let rejected = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x03,
            },
        ),
        start + Duration::from_millis(1),
    );
    assert!(rejected.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id: seen,
            outcome: RuntimeOutcome::Cancelled
        } if *seen == id
    )));
    assert!(!rejected
        .iter()
        .any(|effect| matches!(effect, Effect::RetryScheduled { .. })));
    engine.assert_invariants().unwrap();
}

#[test]
fn inquiry_syntax_retry_requires_explicit_builtin_policy() {
    let start = Instant::now();

    let mut custom_retry = retrying();
    custom_retry.builtin_inquiry_syntax = false;
    let mut custom_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let custom_admission = custom_engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: inquiry_with_retry(1, POWER, custom_retry),
        },
        start,
    );
    let custom_id = admitted(&custom_admission);
    send_ok(&mut custom_engine, &custom_admission, None, start);
    let custom_error = custom_engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x02,
            },
        ),
        start,
    );
    assert!(custom_error.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id,
            outcome: RuntimeOutcome::Failed(Error::SyntaxError),
        } if *id == custom_id
    )));
    assert!(!custom_error
        .iter()
        .any(|effect| matches!(effect, Effect::RetryScheduled { .. })));

    let mut builtin_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let builtin_admission = builtin_engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: inquiry_with_retry(1, POWER, retrying()),
        },
        start,
    );
    let builtin_id = admitted(&builtin_admission);
    send_ok(&mut builtin_engine, &builtin_admission, None, start);
    let builtin_error = builtin_engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x02,
            },
        ),
        start,
    );
    assert!(builtin_error.iter().any(|effect| matches!(
        effect,
        Effect::RetryScheduled { id, .. } if *id == builtin_id
    )));
    assert!(!builtin_error.iter().any(|effect| matches!(
        effect,
        Effect::Terminal { id, .. } if *id == builtin_id
    )));
}

#[test]
fn blocked_target_does_not_block_other_targets_or_accumulate_stale_tickets() {
    let start = Instant::now();
    let mut configured = policy(EnvelopeKind::Raw, TransportKind::Datagram);
    configured.capacity = 4;
    let mut engine = ProtocolEngine::new(configured).unwrap();
    for target in [camera(1), camera(2)] {
        engine
            .register_target(
                target,
                TargetPolicy {
                    command_sockets: 1,
                    cancellation: CancellationPolicy::Supported,
                },
            )
            .unwrap();
    }
    let occupying = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    send_ok(&mut engine, &occupying, None, start);
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    let blocked = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    assert!(request_transmit_optional(&blocked).is_none());
    for ticket in 3..20 {
        let queued = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(ticket),
                request: command(1, CancellationPolicy::Supported),
            },
            start,
        );
        let id = admitted(&queued);
        engine.handle(Input::Cancel { id }, start);
    }
    assert_eq!(
        engine
            .command_queues
            .iter()
            .map(VecDeque::len)
            .sum::<usize>(),
        1
    );
    let other_target = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(20),
            request: command(2, CancellationPolicy::Supported),
        },
        start,
    );
    assert!(request_transmit_optional(&other_target).is_some());
    engine.assert_invariants().unwrap();
}

#[test]
fn retry_backoff_must_fit_inside_total_budget() {
    let start = Instant::now();
    let mut retry = retrying();
    retry.total_budget = Duration::from_millis(15);
    retry.initial_backoff = Duration::from_millis(10);
    let mut request_context = context(1, CancellationPolicy::Supported);
    request_context.retry = retry;
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: RuntimeRequest::Command {
                wire: wire(0x81),
                context: request_context,
                applied_state: None,
            },
        },
        start,
    );
    let id = admitted(&effects);
    send_ok(&mut engine, &effects, None, start);
    let exhausted = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x03,
            },
        ),
        start + Duration::from_millis(5),
    );
    assert!(exhausted.iter().any(|effect| matches!(
        effect,
        Effect::Terminal { id: seen, .. } if *seen == id
    )));
    assert!(!exhausted
        .iter()
        .any(|effect| matches!(effect, Effect::RetryScheduled { .. })));
    engine.assert_invariants().unwrap();
}

// ---------------------------------------------------------------------------
// Issue #565: 1.x transport fault tolerance.
// ---------------------------------------------------------------------------

fn ignored_reasons(effects: &[Effect]) -> Vec<IgnoreReason> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Ignored(reason) => Some(*reason),
            _ => None,
        })
        .collect()
}

fn socket_of(engine: &ProtocolEngine, id: RequestId) -> Option<ViscaSocket> {
    match engine.entry(id)?.phase() {
        Phase::Executing { socket, .. } | Phase::AwaitingCancellationResolution { socket, .. } => {
            Some(socket)
        }
        _ => None,
    }
}

/// A transient receive fault retries every command still awaiting an ACK and
/// leaves the session running. Restores 1.x `SchedulerEvent::NetworkError`.
#[test]
fn transient_receive_fault_retries_awaiting_ack_work_and_keeps_the_session() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let first_id = admitted(&first);
    send_ok(&mut engine, &first, None, start);
    let second = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(2, CancellationPolicy::Supported),
        },
        start,
    );
    let second_id = admitted(&second);
    send_ok(&mut engine, &second, None, start);

    let fault = engine.handle(
        Input::ReceiveFault {
            error: Error::Io(Arc::new(std::io::Error::from(
                std::io::ErrorKind::ConnectionRefused,
            ))),
        },
        start,
    );

    assert_eq!(engine.state(), SessionState::Running);
    let retried: Vec<_> = fault
        .iter()
        .filter_map(|effect| match effect {
            Effect::RetryScheduled { id, attempt, .. } => Some((*id, *attempt)),
            _ => None,
        })
        .collect();
    assert_eq!(retried, vec![(first_id, 1), (second_id, 1)]);
    assert!(
        !fault
            .iter()
            .any(|effect| matches!(effect, Effect::Terminal { .. })),
        "a transient receive fault must not fail retryable work"
    );
    assert!(engine.entry(first_id).is_some());
    assert!(engine.entry(second_id).is_some());
    engine.assert_invariants().unwrap();
}

/// An inquiry awaiting its reply is untouched, matching the 1.x command-only
/// scan, and a request that cannot retry fails with the transport error
/// without ending the session.
#[test]
fn receive_fault_fails_only_unretryable_work_and_never_the_session() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let mut once = context(1, CancellationPolicy::Supported);
    once.retry = RetryPolicy::NEVER;
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: RuntimeRequest::Command {
                wire: wire(0x81),
                context: once,
                applied_state: None,
            },
        },
        start,
    );
    let command_id = admitted(&admission);
    send_ok(&mut engine, &admission, None, start);
    let inquiry_admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: inquiry(2, POWER),
        },
        start,
    );
    let inquiry_id = admitted(&inquiry_admission);
    send_ok(&mut engine, &inquiry_admission, None, start);

    let fault = engine.handle(
        Input::ReceiveFault {
            error: Error::TransportError("ICMP port unreachable".into()),
        },
        start,
    );

    assert_eq!(engine.state(), SessionState::Running);
    assert!(fault.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id,
            outcome: RuntimeOutcome::Failed(Error::TransportError(_)),
        } if *id == command_id
    )));
    assert!(
        matches!(
            engine.entry(inquiry_id).map(Entry::phase),
            Some(Phase::AwaitingReply { .. })
        ),
        "an inquiry keeps its own reply deadline across a transient receive fault"
    );
    engine.assert_invariants().unwrap();
}

/// A receive fault applied to a terminated session is inert.
#[test]
fn receive_fault_after_termination_is_inert() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    engine.handle(Input::Close { reason: None }, start);
    let fault = engine.handle(
        Input::ReceiveFault {
            error: Error::Timeout,
        },
        start,
    );
    assert_eq!(
        ignored_reasons(&fault),
        vec![IgnoreReason::SessionNotRunning]
    );
    assert_eq!(engine.state(), SessionState::Closed);
}

/// A stream request-write failure is a session verdict on purpose: the poison
/// reason names the exact transport cause, and every affected request reports
/// the one error that classifies as needing a replacement session (#564).
#[test]
fn stream_write_failure_poisons_with_the_transport_cause_in_the_reason() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let first_id = admitted(&first);
    send_ok(&mut engine, &first, None, start);
    let second = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(2, CancellationPolicy::Supported),
        },
        start,
    );
    let second_id = admitted(&second);
    let (second_tx, _, _) = request_transmit(&second);

    let failure = engine.handle(
        Input::TransmissionFinished {
            transmission: second_tx,
            result: Err(Error::TransportError("short write".into())),
        },
        start,
    );

    let terminals: Vec<_> = failure
        .iter()
        .filter_map(|effect| match effect {
            Effect::Terminal { id, outcome } => Some((*id, outcome.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        terminals.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        vec![first_id, second_id]
    );
    for (_, outcome) in &terminals {
        let RuntimeOutcome::Failed(Error::StreamPoisoned { reason }) = outcome else {
            panic!("every request on a poisoned stream reports the session verdict: {outcome:?}");
        };
        assert!(
            reason.contains("short write"),
            "the exact transport cause must survive in the poison reason: {reason}"
        );
    }
    assert_eq!(engine.state(), SessionState::Poisoned);
    engine.assert_invariants().unwrap();
}

/// A datagram write failure stays isolated to its own request.
#[test]
fn datagram_write_failure_fails_exactly_one_request() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let first_id = admitted(&first);
    send_ok(&mut engine, &first, None, start);
    let second = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(2, CancellationPolicy::Supported),
        },
        start,
    );
    let second_id = admitted(&second);
    let (second_tx, _, _) = request_transmit(&second);
    let failure = engine.handle(
        Input::TransmissionFinished {
            transmission: second_tx,
            result: Err(Error::TransportError("datagram refused".into())),
        },
        start,
    );
    assert_eq!(terminal_id(&failure), Some(second_id));
    assert_eq!(engine.state(), SessionState::Running);
    assert!(engine.entry(first_id).is_some());
    engine.assert_invariants().unwrap();
}

/// `90 40 FF` carries no socket nibble: 1.x assigned the first free socket.
#[test]
fn socketless_ack_assigns_the_first_free_socket() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let id = admitted(&admission);
    send_ok(&mut engine, &admission, None, start);
    let acked = engine.handle(frame(1, None, DecodedResponse::Ack { socket: None }), start);
    assert!(ignored_reasons(&acked).is_empty());
    assert_eq!(socket_of(&engine, id), Some(ViscaSocket::S1));

    // And the socketless completion that such a camera sends finishes it.
    let done = engine.handle(
        frame(1, None, DecodedResponse::Completion { socket: None }),
        start,
    );
    assert_eq!(terminal_id(&done), Some(id));
    engine.assert_invariants().unwrap();
}

/// With socket one already held, a socketless ACK takes socket two.
#[test]
fn socketless_ack_takes_the_second_socket_when_the_first_is_busy() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let first_id = admitted(&first);
    send_ok(&mut engine, &first, None, start);
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    let second = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let second_id = admitted(&second);
    send_ok(&mut engine, &second, None, start);
    let acked = engine.handle(frame(1, None, DecodedResponse::Ack { socket: None }), start);
    assert!(ignored_reasons(&acked).is_empty());
    assert_eq!(socket_of(&engine, first_id), Some(ViscaSocket::S1));
    assert_eq!(socket_of(&engine, second_id), Some(ViscaSocket::S2));
    engine.assert_invariants().unwrap();
}

/// The assignment policy itself, including the exhausted case 1.x guarded
/// against: a camera whose sockets are all taken gets no invented assignment,
/// and a one-socket target has no second socket to fall back to.
#[test]
fn socket_assignment_follows_the_1x_fallback_order() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let first_id = admitted(&first);
    send_ok(&mut engine, &first, None, start);
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    let second = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let second_id = admitted(&second);
    send_ok(&mut engine, &second, None, start);

    // Socket one is taken by another request: both the named-socket fallback
    // and the socketless pick move to socket two.
    assert_eq!(
        engine.assign_socket(camera(1), Some(ViscaSocket::S1), second_id),
        Some(ViscaSocket::S2)
    );
    assert_eq!(
        engine.assign_socket(camera(1), None, second_id),
        Some(ViscaSocket::S2)
    );
    // A request keeps the socket it already owns.
    assert_eq!(
        engine.assign_socket(camera(1), Some(ViscaSocket::S1), first_id),
        Some(ViscaSocket::S1)
    );

    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S2),
            },
        ),
        start,
    );
    // Every socket is now held by another request: nothing is invented.
    let stranger = RequestId::from_nonzero(NonZeroU64::new(u64::MAX).unwrap());
    assert_eq!(engine.assign_socket(camera(1), None, stranger), None);
    assert_eq!(
        engine.assign_socket(camera(1), Some(ViscaSocket::S1), stranger),
        None
    );
    engine.assert_invariants().unwrap();

    // A one-socket target has no other socket to fall back to.
    let mut single = engine_with_target(
        EnvelopeKind::Raw,
        TransportKind::Datagram,
        1,
        CancellationPolicy::Supported,
    );
    let only = single.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let only_id = admitted(&only);
    send_ok(&mut single, &only, None, start);
    single.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    assert_eq!(socket_of(&single, only_id), Some(ViscaSocket::S1));
    assert_eq!(single.assign_socket(camera(1), None, stranger), None);
    assert_eq!(
        single.assign_socket(camera(1), Some(ViscaSocket::S1), stranger),
        None
    );
    single.assert_invariants().unwrap();
}

/// A socketless ACK that matches no in-flight command is inert, exactly like
/// any other unattributable frame.
#[test]
fn unattributable_socketless_ack_is_inert() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let stray = engine.handle(frame(1, None, DecodedResponse::Ack { socket: None }), start);
    assert_eq!(ignored_reasons(&stray), vec![IgnoreReason::UnmatchedFrame]);
    engine.assert_invariants().unwrap();
}

/// An ACK naming an occupied socket falls back to the free one (1.x
/// reassignment) instead of being dropped and eating the ACK deadline.
#[test]
fn ack_naming_an_occupied_socket_falls_back_to_the_free_socket() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let first_id = admitted(&first);
    send_ok(&mut engine, &first, None, start);
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    let second = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let second_id = admitted(&second);
    send_ok(&mut engine, &second, None, start);

    // The camera repeats socket one, which the first request still owns.
    let reassigned = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );

    assert!(ignored_reasons(&reassigned).is_empty());
    assert_eq!(socket_of(&engine, first_id), Some(ViscaSocket::S1));
    assert_eq!(socket_of(&engine, second_id), Some(ViscaSocket::S2));
    // Each request still completes on the socket it actually owns.
    let done = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S2),
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&done), Some(second_id));
    engine.assert_invariants().unwrap();
}

/// Issue #297: an ACK that arrives before the write result for the very frame
/// it answers is latched, not dropped, and applied as soon as the write is
/// confirmed. This is the engine-level guard: it holds no matter how a future
/// owner orders its reader against its writer.
#[test]
fn ack_racing_its_own_write_result_is_latched_and_applied() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let id = admitted(&admission);
    let (transmission, _, _) = request_transmit(&admission);
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::Sending { .. })
    ));

    // The camera answers before this owner applies the write result.
    let early = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S2),
            },
        ),
        start,
    );
    assert!(
        ignored_reasons(&early).is_empty(),
        "the racing ACK must not be dropped: {early:?}"
    );
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::Sending { .. })
    ));
    engine.assert_invariants().unwrap();

    let confirmed = engine.handle(
        Input::TransmissionFinished {
            transmission,
            result: Ok(TransmissionMeta { sequence: None }),
        },
        start,
    );
    assert!(confirmed.iter().any(|effect| matches!(
        effect,
        Effect::Transition {
            to: Phase::Executing {
                socket: ViscaSocket::S2,
                ..
            },
            ..
        }
    )));
    assert_eq!(socket_of(&engine, id), Some(ViscaSocket::S2));

    let done = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S2),
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&done), Some(id));
    engine.assert_invariants().unwrap();
}

/// A latch belongs to exactly one attempt: a retried request starts clean.
#[test]
fn a_latched_ack_never_survives_into_the_next_attempt() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let id = admitted(&admission);
    let (transmission, _, _) = request_transmit(&admission);
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    engine.handle(
        Input::TransmissionFinished {
            transmission,
            result: Ok(TransmissionMeta { sequence: None }),
        },
        start,
    );
    assert_eq!(socket_of(&engine, id), Some(ViscaSocket::S1));
    // Force a retry; the completion deadline releases the socket.
    let retried = engine.advance(start + Duration::from_millis(40));
    assert!(retried
        .iter()
        .any(|effect| matches!(effect, Effect::RetryScheduled { .. })));
    let resent = engine.advance(start + Duration::from_millis(60));
    let (retry_tx, _, _) = request_transmit(&resent);
    engine.handle(
        Input::TransmissionFinished {
            transmission: retry_tx,
            result: Ok(TransmissionMeta { sequence: None }),
        },
        start + Duration::from_millis(60),
    );
    assert!(
        matches!(
            engine.entry(id).map(Entry::phase),
            Some(Phase::AwaitingAck { .. })
        ),
        "the retried attempt must await its own ACK"
    );
    engine.assert_invariants().unwrap();
}

/// A socketless completion is attributable only while exactly one command owns
/// a socket on that target; otherwise it stays inert instead of guessing.
#[test]
fn socketless_completion_needs_a_sole_socket_holder() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let first_id = admitted(&first);
    send_ok(&mut engine, &first, None, start);
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    let second = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let second_id = admitted(&second);
    send_ok(&mut engine, &second, None, start);
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S2),
            },
        ),
        start,
    );

    let ambiguous = engine.handle(
        frame(1, None, DecodedResponse::Completion { socket: None }),
        start,
    );
    assert_eq!(
        ignored_reasons(&ambiguous),
        vec![IgnoreReason::UnmatchedFrame]
    );
    assert!(engine.entry(first_id).is_some());
    assert!(engine.entry(second_id).is_some());

    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    let resolved = engine.handle(
        frame(1, None, DecodedResponse::Completion { socket: None }),
        start,
    );
    assert_eq!(terminal_id(&resolved), Some(second_id));
    engine.assert_invariants().unwrap();
}

/// Sony correlates by sequence, so a socketless completion completes exactly
/// the request the envelope names.
#[test]
fn sony_socketless_completion_finishes_the_sequenced_request() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Stream);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let id = admitted(&admission);
    send_ok(&mut engine, &admission, Some(7), start);
    engine.handle(
        frame(
            1,
            Some((7, SequenceWidth::Full32)),
            DecodedResponse::Ack { socket: None },
        ),
        start,
    );
    assert_eq!(socket_of(&engine, id), Some(ViscaSocket::S1));
    let done = engine.handle(
        frame(
            1,
            Some((7, SequenceWidth::Full32)),
            DecodedResponse::Completion { socket: None },
        ),
        start,
    );
    assert_eq!(terminal_id(&done), Some(id));
    engine.assert_invariants().unwrap();
}
