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
        raw_inquiry_release_hold: Duration::from_millis(50),
        raw_release_grace: Duration::from_millis(100),
        strict_unconfirmed_poison: false,
    }
}

/// A raw datagram policy that keeps the pre-fix whole-session poison behavior,
/// so a test can pin the strict opt-in mode explicitly.
fn strict_poison_policy() -> ProtocolPolicy {
    ProtocolPolicy {
        strict_unconfirmed_poison: true,
        ..policy(EnvelopeKind::Raw, TransportKind::Datagram)
    }
}

/// A two-target raw datagram engine in the strict `strict_unconfirmed_poison`
/// opt-in mode, mirroring [`engine`] for the whole-session poison tests.
fn strict_poison_engine() -> ProtocolEngine {
    let mut engine = ProtocolEngine::new(strict_poison_policy()).unwrap();
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
    engine
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
        reply_shape: ReplyShape::AckThenCompletion,
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

fn urgent_command(target: u8, cancellation: CancellationPolicy) -> RuntimeRequest {
    let mut request = command(target, cancellation);
    let RuntimeRequest::Command { context, .. } = &mut request else {
        unreachable!("command helper always constructs a command");
    };
    context.control.class = ControlClass::Urgent;
    request
}

/// A raw command that declares a non-default reply shape (issue #700).
fn command_with_reply_shape(
    target: u8,
    cancellation: CancellationPolicy,
    reply_shape: ReplyShape,
) -> RuntimeRequest {
    let mut context = context(target, cancellation);
    context.reply_shape = reply_shape;
    RuntimeRequest::Command {
        wire: wire(0x80 | target),
        context,
        applied_state: None,
    }
}

fn command_with_reply_shape_and_retry(
    target: u8,
    cancellation: CancellationPolicy,
    reply_shape: ReplyShape,
    retry: RetryPolicy,
) -> RuntimeRequest {
    let mut request = command_with_reply_shape(target, cancellation, reply_shape);
    let RuntimeRequest::Command { context, .. } = &mut request else {
        unreachable!("reply shapes apply only to commands");
    };
    context.retry = retry;
    request
}

/// Admits `request` and returns `(effects, id)` after admission.
fn admit(
    engine: &mut ProtocolEngine,
    ticket: u64,
    request: RuntimeRequest,
    now: Instant,
) -> (Vec<Effect>, RequestId) {
    let effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(ticket),
            request,
        },
        now,
    );
    let id = admitted(&effects);
    (effects, id)
}

/// The authoritative phase of an admitted entry, for lifecycle assertions.
fn phase_of(engine: &ProtocolEngine, id: RequestId) -> Option<Phase> {
    engine.entries.get(&id).map(Entry::phase)
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

/// The public raw owner uses one inquiry flight. This is the topology in which
/// a released FIFO owner has a queued successor and therefore needs the bounded
/// target ambiguity hold.
fn single_flight_raw_engine() -> ProtocolEngine {
    let mut configured = policy(EnvelopeKind::Raw, TransportKind::Datagram);
    configured.inquiry_capacity = 1;
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
    cancel_transmit_optional(effects).expect("cancel transmission")
}

fn cancel_transmit_optional(
    effects: &[Effect],
) -> Option<(TransmissionId, RequestId, ViscaSocket)> {
    effects.iter().find_map(|effect| match effect {
        Effect::Transmit {
            transmission,
            request,
            kind: Transmission::Cancel { socket, .. },
        } => Some((*transmission, *request, *socket)),
        _ => None,
    })
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
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
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
            result: Ok(TransmissionMeta {
                sequence: Some(0x1001),
            }),
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
fn request_transmission_and_generation_allocators_stop_at_exhaustion() {
    let mut allocator = IdAllocator::seeded(u64::MAX);
    assert_eq!(allocator.candidate().map(NonZeroU64::get), Some(u64::MAX));
    assert!(allocator.candidate().is_none());

    let start = Instant::now();
    let mut request_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    request_engine.seed_allocators(u64::MAX, 1, 1);
    let first = request_engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let first_id = admitted(&first);
    assert_eq!(first_id.get(), u64::MAX);
    let second = request_engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    assert!(second.iter().any(|effect| matches!(
        effect,
        Effect::AdmissionRejected {
            ticket: AdmissionTicket(2),
            error: Error::RuntimeIdentityExhausted,
        }
    )));

    let mut generation_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    generation_engine.seed_allocators(1, 1, u64::MAX);
    let first = generation_engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    assert_eq!(admitted(&first).get(), 1);
    let second = generation_engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    assert!(second.iter().any(|effect| matches!(
        effect,
        Effect::AdmissionRejected {
            ticket: AdmissionTicket(2),
            error: Error::RuntimeIdentityExhausted,
        }
    )));

    let mut transmission_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    transmission_engine.seed_allocators(1, u64::MAX, 1);
    let first = transmission_engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let first_id = admitted(&first);
    let (first_tx, _, _) = request_transmit(&first);
    assert_eq!(first_tx.get(), u64::MAX);
    send_ok(&mut transmission_engine, &first, None, start);
    let second = transmission_engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let second_id = admitted(&second);
    let dispatched = transmission_engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    assert!(dispatched.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id,
            outcome: RuntimeOutcome::Failed(Error::RuntimeIdentityExhausted),
        } if *id == second_id
    )));
    assert!(transmission_engine.entry(first_id).is_some());
    request_engine.assert_invariants().unwrap();
    generation_engine.assert_invariants().unwrap();
    transmission_engine.assert_invariants().unwrap();
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
fn sony_exact_request_and_cancellation_collision_is_ignored() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let admitted_effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let (request_tx, id, _) = request_transmit(&admitted_effects);
    engine.handle(
        Input::TransmissionFinished {
            transmission: request_tx,
            result: Ok(TransmissionMeta {
                sequence: Some(0x1111_beef),
            }),
        },
        start,
    );
    engine.handle(
        frame(
            1,
            Some((0x1111_beef, SequenceWidth::Full32)),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    let cancellation = engine.handle(Input::Cancel { id }, start);
    let (cancel_tx, cancel_id, _) = cancel_transmit(&cancellation);
    assert_eq!(cancel_id, id);
    engine.handle(
        Input::TransmissionFinished {
            transmission: cancel_tx,
            result: Ok(TransmissionMeta {
                // Deliberately collide with the original request identity.
                sequence: Some(0x1111_beef),
            }),
        },
        start,
    );

    let collision = engine.handle(
        frame(
            1,
            Some((0x1111_beef, SequenceWidth::Full32)),
            DecodedResponse::Completion { socket: None },
        ),
        start,
    );
    assert!(collision.iter().any(|effect| matches!(
        effect,
        Effect::Ignored(IgnoreReason::UnmatchedSequencedFrame)
    )));
    assert!(!collision
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { .. })));
    assert!(!collision
        .iter()
        .any(|effect| matches!(effect, Effect::CancellationObservation { .. })));
    assert!(engine.entry(id).is_some());
    engine.assert_invariants().unwrap();
}

#[test]
fn sony_lower16_request_and_cancellation_collision_is_ambiguous() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let admitted_effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let (request_tx, id, _) = request_transmit(&admitted_effects);
    engine.handle(
        Input::TransmissionFinished {
            transmission: request_tx,
            result: Ok(TransmissionMeta {
                sequence: Some(0x1111_beef),
            }),
        },
        start,
    );
    engine.handle(
        frame(
            1,
            Some((0x1111_beef, SequenceWidth::Full32)),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    let cancellation = engine.handle(Input::Cancel { id }, start);
    let (cancel_tx, cancel_id, _) = cancel_transmit(&cancellation);
    assert_eq!(cancel_id, id);
    engine.handle(
        Input::TransmissionFinished {
            transmission: cancel_tx,
            result: Ok(TransmissionMeta {
                // Distinct full identities still collide in the lower-16
                // fallback, so this reply must remain ambiguous.
                sequence: Some(0x2222_beef),
            }),
        },
        start,
    );

    let collision = engine.handle(
        frame(
            1,
            Some((0xbeef, SequenceWidth::Lower16)),
            DecodedResponse::Completion { socket: None },
        ),
        start,
    );
    assert!(collision.iter().any(|effect| matches!(
        effect,
        Effect::Ignored(IgnoreReason::AmbiguousLower16Sequence)
    )));
    assert!(!collision
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { .. })));
    assert!(!collision
        .iter()
        .any(|effect| matches!(effect, Effect::CancellationObservation { .. })));
    assert!(engine.entry(id).is_some());
    engine.assert_invariants().unwrap();
}

#[test]
fn sony_retry_reuses_first_successful_sequence_and_ignores_stale_result() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let (first_tx, id, first_wire) = request_transmit(&first);
    assert!(matches!(
        &first[0..],
        [
            Effect::Admitted { .. },
            Effect::Transition { .. },
            Effect::Transmit {
                kind: Transmission::Request {
                    requested_sequence: None,
                    ..
                },
                ..
            }
        ]
    ));
    engine.handle(
        Input::TransmissionFinished {
            transmission: first_tx,
            result: Ok(TransmissionMeta {
                sequence: Some(0x1020_3040),
            }),
        },
        start,
    );
    assert_eq!(
        engine.entry(id).and_then(|entry| entry.current_sequence),
        Some(0x1020_3040)
    );

    let timeout = engine.advance(start + Duration::from_millis(20));
    let retry_ready = timeout
        .iter()
        .find_map(|effect| match effect {
            Effect::RetryScheduled { ready_at, .. } => Some(*ready_at),
            _ => None,
        })
        .expect("ACK timeout schedules a retry");
    let retry = engine.advance(retry_ready);
    let (retry_tx, retry_id, retry_wire) = request_transmit(&retry);
    let requested = retry.iter().find_map(|effect| match effect {
        Effect::Transmit {
            kind: Transmission::Request {
                requested_sequence, ..
            },
            ..
        } => *requested_sequence,
        _ => None,
    });
    assert_eq!(retry_id, id);
    assert_ne!(retry_tx, first_tx);
    assert!(Arc::ptr_eq(&first_wire, &retry_wire));
    assert_eq!(requested, Some(0x1020_3040));

    // The old attempt was removed when the retry became authoritative. Its
    // late result must not overwrite the request's current sequence.
    let stale = engine.finish_write_without_due(
        first_tx,
        Ok(TransmissionMeta {
            sequence: Some(0xdead_beef),
        }),
        retry_ready,
    );
    assert!(stale
        .iter()
        .any(|effect| matches!(effect, Effect::Ignored(IgnoreReason::StaleTransmission))));
    assert_eq!(
        engine.entry(id).and_then(|entry| entry.current_sequence),
        Some(0x1020_3040)
    );

    // A new logical request starts with no requested sequence, even while the
    // previous request is in its retry transmission.
    let next = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1, CancellationPolicy::Supported),
        },
        retry_ready,
    );
    let next_requested = next.iter().find_map(|effect| match effect {
        Effect::Transmit {
            kind: Transmission::Request {
                requested_sequence, ..
            },
            ..
        } => Some(*requested_sequence),
        _ => None,
    });
    assert_eq!(next_requested, Some(None));
    engine.assert_invariants().unwrap();
}

#[test]
fn sony_stale_command_errors_do_not_spend_retry_during_backoff_or_ready() {
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
    let (first_tx, id, _) = request_transmit(&first);
    let first_sequence = 0x1020_3040;
    engine.handle(
        Input::TransmissionFinished {
            transmission: first_tx,
            result: Ok(TransmissionMeta {
                sequence: Some(first_sequence),
            }),
        },
        start,
    );

    // The first busy response is current and schedules exactly one retry.
    let retry_error = engine.handle(
        frame(
            1,
            Some((first_sequence, SequenceWidth::Full32)),
            DecodedResponse::Error {
                socket: None,
                code: 0x03,
            },
        ),
        start + Duration::from_millis(1),
    );
    let (retry_id, attempt, retry_ready) =
        retry_scheduled(&retry_error).expect("the current busy response retries");
    assert_eq!(retry_id, id);
    assert_eq!(attempt, 1);
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::Backoff { ready_at, .. }) if ready_at == retry_ready
    ));
    assert_eq!(engine.entry(id).unwrap().attempt, 1);
    assert_eq!(engine.next_wake(), Some(retry_ready));

    // The old attempt's duplicate is still routable by sequence, but must be
    // inert while the request is in backoff.
    let stale_backoff = engine.handle(
        frame(
            1,
            Some((first_sequence, SequenceWidth::Full32)),
            DecodedResponse::Error {
                socket: None,
                code: 0x03,
            },
        ),
        start + Duration::from_millis(2),
    );
    assert!(stale_backoff
        .iter()
        .any(|effect| matches!(effect, Effect::Ignored(IgnoreReason::UnmatchedFrame))));
    assert!(!stale_backoff
        .iter()
        .any(|effect| matches!(effect, Effect::RetryScheduled { .. })));
    assert!(!stale_backoff
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { .. })));
    assert_eq!(engine.entry(id).unwrap().attempt, 1);
    assert_eq!(engine.next_wake(), Some(retry_ready));

    // Hold the sole command socket with another request so the retry reaches
    // Ready at its original deadline. This makes the second stale phase
    // observable instead of dispatching immediately.
    let blocker = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1, CancellationPolicy::Supported),
        },
        start + Duration::from_millis(2),
    );
    let (_, blocker_id, _) = request_transmit(&blocker);
    let blocker_sequence = 0x2030_4050;
    send_ok(
        &mut engine,
        &blocker,
        Some(blocker_sequence),
        start + Duration::from_millis(2),
    );
    let blocker_ack = engine.handle(
        frame(
            1,
            Some((blocker_sequence, SequenceWidth::Full32)),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(2),
    );
    assert!(!blocker_ack
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { .. })));

    let promoted = engine.advance(retry_ready);
    assert!(promoted.iter().any(|effect| matches!(
        effect,
        Effect::Transition {
            id: seen,
            from: Phase::Backoff { ready_at, .. },
            to: Phase::Ready { .. },
            ..
        } if *seen == id && *ready_at == retry_ready
    )));
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::Ready { .. })
    ));
    assert!(request_transmit_optional(&promoted).is_none());

    // A duplicate retryable error and a stale permanent error are both inert
    // while the retry is queued. In particular, neither changes its attempt
    // budget nor terminalizes the request.
    let ready_before = (engine.entry(id).unwrap().attempt, engine.next_wake());
    let stale_ready = engine.handle(
        frame(
            1,
            Some((first_sequence, SequenceWidth::Full32)),
            DecodedResponse::Error {
                socket: None,
                code: 0x03,
            },
        ),
        retry_ready + Duration::from_millis(1),
    );
    let stale_permanent = engine.handle(
        frame(
            1,
            Some((first_sequence, SequenceWidth::Full32)),
            DecodedResponse::Error {
                socket: None,
                code: 0x01,
            },
        ),
        retry_ready + Duration::from_millis(1),
    );
    for effects in [&stale_ready, &stale_permanent] {
        assert!(effects
            .iter()
            .any(|effect| matches!(effect, Effect::Ignored(IgnoreReason::UnmatchedFrame))));
        assert!(!effects
            .iter()
            .any(|effect| matches!(effect, Effect::RetryScheduled { .. })));
        assert!(!effects
            .iter()
            .any(|effect| matches!(effect, Effect::Terminal { .. })));
    }
    assert_eq!(
        (engine.entry(id).unwrap().attempt, engine.next_wake()),
        ready_before
    );

    // Once the blocker completes, the request dispatches once and reuses the
    // sequence from the original attempt; stale errors did not move its retry.
    let released = engine.handle(
        frame(
            1,
            Some((blocker_sequence, SequenceWidth::Full32)),
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        retry_ready + Duration::from_millis(2),
    );
    let (_, dispatched, _) = request_transmit(&released);
    assert_eq!(dispatched, id);
    let requested_sequence = released.iter().find_map(|effect| match effect {
        Effect::Transmit {
            request,
            kind: Transmission::Request {
                requested_sequence, ..
            },
            ..
        } if *request == id => Some(*requested_sequence),
        _ => None,
    });
    assert_eq!(requested_sequence, Some(Some(first_sequence)));
    assert_eq!(
        released
            .iter()
            .filter(|effect| matches!(
                effect,
                Effect::Transmit { request, kind: Transmission::Request { .. }, .. }
                    if *request == id
            ))
            .count(),
        1
    );
    send_ok(
        &mut engine,
        &released,
        Some(first_sequence),
        retry_ready + Duration::from_millis(2),
    );
    assert_eq!(engine.entry(id).unwrap().attempt, 1);
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::AwaitingAck { .. })
    ));
    assert!(engine.entry(blocker_id).is_none());
    engine.assert_invariants().unwrap();
}

#[test]
fn sony_stale_inquiry_errors_do_not_spend_retry_during_backoff_or_ready() {
    let start = Instant::now();
    let mut configured = policy(EnvelopeKind::Sony, TransportKind::Datagram);
    configured.inquiry_capacity = 1;
    configured.inquiry_cooldown = Duration::ZERO;
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

    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: inquiry(1, POWER),
        },
        start,
    );
    let (_, id, _) = request_transmit(&first);
    let first_sequence = 0x3040_5060;
    send_ok(&mut engine, &first, Some(first_sequence), start);

    // Queue another inquiry before the first one retries. It will be the
    // capacity blocker that leaves the retried inquiry observable in Ready.
    let queued = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: inquiry(1, ZOOM),
        },
        start + Duration::from_millis(1),
    );
    assert!(request_transmit_optional(&queued).is_none());
    let retry_error = engine.handle(
        frame(
            1,
            Some((first_sequence, SequenceWidth::Full32)),
            DecodedResponse::Error {
                socket: None,
                code: 0x02,
            },
        ),
        start + Duration::from_millis(2),
    );
    let (retry_id, attempt, retry_ready) =
        retry_scheduled(&retry_error).expect("the current syntax response retries");
    assert_eq!(retry_id, id);
    assert_eq!(attempt, 1);

    let stale_backoff = engine.handle(
        frame(
            1,
            Some((first_sequence, SequenceWidth::Full32)),
            DecodedResponse::Error {
                socket: None,
                code: 0x02,
            },
        ),
        start + Duration::from_millis(3),
    );
    assert!(stale_backoff
        .iter()
        .any(|effect| matches!(effect, Effect::Ignored(IgnoreReason::UnmatchedFrame))));
    assert!(!stale_backoff
        .iter()
        .any(|effect| matches!(effect, Effect::RetryScheduled { .. })));
    assert_eq!(engine.entry(id).unwrap().attempt, 1);

    // The queued inquiry wins dispatch when the retry becomes ready, leaving
    // the old inquiry in Ready and preserving the original retry deadline.
    let promoted = engine.advance(retry_ready);
    let (_, queued_id, _) = request_transmit(&promoted);
    assert_eq!(queued_id, admitted(&queued));
    send_ok(&mut engine, &promoted, Some(0x4050_6070), retry_ready);
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::Ready { .. })
    ));

    let ready_before = (engine.entry(id).unwrap().attempt, engine.next_wake());
    let stale_ready = engine.handle(
        frame(
            1,
            Some((first_sequence, SequenceWidth::Full32)),
            DecodedResponse::Error {
                socket: None,
                code: 0x02,
            },
        ),
        retry_ready + Duration::from_millis(1),
    );
    let stale_permanent = engine.handle(
        frame(
            1,
            Some((first_sequence, SequenceWidth::Full32)),
            DecodedResponse::Error {
                socket: None,
                code: 0x01,
            },
        ),
        retry_ready + Duration::from_millis(1),
    );
    for effects in [&stale_ready, &stale_permanent] {
        assert!(effects
            .iter()
            .any(|effect| matches!(effect, Effect::Ignored(IgnoreReason::UnmatchedFrame))));
        assert!(!effects
            .iter()
            .any(|effect| matches!(effect, Effect::RetryScheduled { .. })));
        assert!(!effects
            .iter()
            .any(|effect| matches!(effect, Effect::Terminal { .. })));
    }
    assert_eq!(
        (engine.entry(id).unwrap().attempt, engine.next_wake()),
        ready_before
    );

    let released = engine.handle(
        frame(
            1,
            Some((0x4050_6070, SequenceWidth::Full32)),
            DecodedResponse::InquiryReply {
                route: Some(ZOOM),
                payload: smallvec![1],
            },
        ),
        retry_ready + Duration::from_millis(2),
    );
    let (retry_tx, dispatched, _) = request_transmit(&released);
    assert_eq!(dispatched, id);
    let requested_sequence = released.iter().find_map(|effect| match effect {
        Effect::Transmit {
            request,
            kind: Transmission::Request {
                requested_sequence, ..
            },
            ..
        } if *request == id => Some(*requested_sequence),
        _ => None,
    });
    assert_eq!(requested_sequence, Some(Some(first_sequence)));
    assert_eq!(
        released
            .iter()
            .filter(|effect| matches!(
                effect,
                Effect::Transmit { request, kind: Transmission::Request { .. }, .. }
                    if *request == id
            ))
            .count(),
        1
    );
    engine.handle(
        Input::TransmissionFinished {
            transmission: retry_tx,
            result: Ok(TransmissionMeta {
                sequence: Some(first_sequence),
            }),
        },
        retry_ready + Duration::from_millis(2),
    );
    assert_eq!(engine.entry(id).unwrap().attempt, 1);
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::AwaitingReply { .. })
    ));
    assert!(engine.entry(admitted(&queued)).is_none());
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
fn raw_error_policy_requires_unique_socketless_evidence() {
    let start = Instant::now();

    // With no inquiry owner, a socketless error has one exact unacknowledged
    // command candidate and therefore resolves that command.
    {
        let mut command_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let admission = command_engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command(1, CancellationPolicy::Supported),
            },
            start,
        );
        let id = admitted(&admission);
        send_ok(&mut command_engine, &admission, None, start);
        let resolved = command_engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Error {
                    socket: None,
                    code: 0x01,
                },
            ),
            start,
        );
        assert_eq!(terminal_id(&resolved), Some(id));
        command_engine.assert_invariants().unwrap();
    }

    // With no command candidate, socketless errors retain the established
    // per-target inquiry FIFO, including its oldest-first order.
    {
        let mut inquiry_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let first = inquiry_engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: inquiry(1, POWER),
            },
            start,
        );
        let first_id = admitted(&first);
        send_ok(&mut inquiry_engine, &first, None, start);
        let second = inquiry_engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(2),
                request: inquiry(1, ZOOM),
            },
            start,
        );
        let second_id = admitted(&second);
        send_ok(&mut inquiry_engine, &second, None, start);

        let first_error = inquiry_engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Error {
                    socket: None,
                    code: 0x01,
                },
            ),
            start,
        );
        assert_eq!(terminal_id(&first_error), Some(first_id));
        let second_error = inquiry_engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Error {
                    socket: None,
                    code: 0x01,
                },
            ),
            start,
        );
        assert_eq!(terminal_id(&second_error), Some(second_id));
        inquiry_engine.assert_invariants().unwrap();
    }

    // An inquiry and an unacknowledged command are both live: a socketless
    // error is ambiguous and must not terminally resolve or retry either one.
    {
        let mut collision_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let inquiry_admission = collision_engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: inquiry(1, POWER),
            },
            start,
        );
        let inquiry_id = admitted(&inquiry_admission);
        send_ok(&mut collision_engine, &inquiry_admission, None, start);
        let command_admission = collision_engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(2),
                request: command(1, CancellationPolicy::Supported),
            },
            start,
        );
        let command_id = admitted(&command_admission);
        send_ok(&mut collision_engine, &command_admission, None, start);

        let ambiguous = collision_engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Error {
                    socket: None,
                    code: 0x03,
                },
            ),
            start,
        );
        assert_eq!(
            ignored_reasons(&ambiguous),
            vec![IgnoreReason::UnmatchedFrame]
        );
        assert!(terminal_outcome(&ambiguous, inquiry_id).is_none());
        assert!(terminal_outcome(&ambiguous, command_id).is_none());
        assert!(retry_scheduled(&ambiguous).is_none());
        assert!(matches!(
            collision_engine.entry(inquiry_id).map(Entry::phase),
            Some(Phase::AwaitingReply { .. })
        ));
        assert!(matches!(
            collision_engine.entry(command_id).map(Entry::phase),
            Some(Phase::AwaitingAck { .. })
        ));

        // Resolve both requests through their authoritative response shapes so
        // the collision test also proves that ambiguity did not poison them.
        let inquiry_reply = collision_engine.handle(
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
        assert_eq!(terminal_id(&inquiry_reply), Some(inquiry_id));
        let acknowledged = collision_engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start,
        );
        assert!(acknowledged.iter().any(|effect| matches!(
            effect,
            Effect::Transition {
                id,
                to: Phase::Executing { .. },
                ..
            } if *id == command_id
        )));
        let completed = collision_engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Completion {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start,
        );
        assert_eq!(terminal_id(&completed), Some(command_id));
        collision_engine.assert_invariants().unwrap();
    }

    // An inquiry whose write is still in flight is already live for
    // correlation purposes, even before it enters the reply FIFO.  It must
    // therefore also block socketless command attribution.
    {
        let mut sending_collision_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let inquiry_admission = sending_collision_engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: inquiry(1, POWER),
            },
            start,
        );
        let inquiry_id = admitted(&inquiry_admission);
        let command_admission = sending_collision_engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(2),
                request: command(1, CancellationPolicy::Supported),
            },
            start,
        );
        let command_id = admitted(&command_admission);
        let ambiguous = sending_collision_engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Error {
                    socket: None,
                    code: 0x03,
                },
            ),
            start,
        );
        assert_eq!(
            ignored_reasons(&ambiguous),
            vec![IgnoreReason::UnmatchedFrame]
        );
        assert!(terminal_outcome(&ambiguous, inquiry_id).is_none());
        assert!(terminal_outcome(&ambiguous, command_id).is_none());
        assert!(retry_scheduled(&ambiguous).is_none());
        assert!(matches!(
            sending_collision_engine.entry(inquiry_id).map(Entry::phase),
            Some(Phase::Sending { .. })
        ));
        assert!(matches!(
            sending_collision_engine.entry(command_id).map(Entry::phase),
            Some(Phase::Sending { .. })
        ));
        sending_collision_engine.assert_invariants().unwrap();
    }

    // A named socket is authoritative even when it is unowned; it cannot fall
    // back to the target's unacknowledged command candidate.
    {
        let mut unowned_socket_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let admission = unowned_socket_engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command(1, CancellationPolicy::Supported),
            },
            start,
        );
        let id = admitted(&admission);
        send_ok(&mut unowned_socket_engine, &admission, None, start);
        let ignored = unowned_socket_engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Error {
                    socket: Some(ViscaSocket::S2),
                    code: 0x01,
                },
            ),
            start,
        );
        assert_eq!(
            ignored_reasons(&ignored),
            vec![IgnoreReason::UnmatchedFrame]
        );
        assert!(terminal_outcome(&ignored, id).is_none());
        assert!(matches!(
            unowned_socket_engine.entry(id).map(Entry::phase),
            Some(Phase::AwaitingAck { .. })
        ));
        unowned_socket_engine.assert_invariants().unwrap();
    }

    // Once ACK establishes a socket, a socketless error has no command
    // candidate and must never be attributed by temporal recency.  The owned
    // socket form remains exact and still resolves the Executing command.
    {
        let mut executing_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let admission = executing_engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command(1, CancellationPolicy::Supported),
            },
            start,
        );
        let id = admitted(&admission);
        send_ok(&mut executing_engine, &admission, None, start);
        executing_engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start,
        );
        let socketless = executing_engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Error {
                    socket: None,
                    code: 0x01,
                },
            ),
            start,
        );
        assert_eq!(
            ignored_reasons(&socketless),
            vec![IgnoreReason::UnmatchedFrame]
        );
        assert!(terminal_outcome(&socketless, id).is_none());
        assert!(matches!(
            executing_engine.entry(id).map(Entry::phase),
            Some(Phase::Executing {
                socket: ViscaSocket::S1,
                ..
            })
        ));
        let owned = executing_engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Error {
                    socket: Some(ViscaSocket::S1),
                    code: 0x01,
                },
            ),
            start,
        );
        assert_eq!(terminal_id(&owned), Some(id));
        executing_engine.assert_invariants().unwrap();
    }
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
fn correlated_frames_win_at_equality_and_are_stale_one_nanosecond_later() {
    for envelope in [EnvelopeKind::Raw, EnvelopeKind::Sony] {
        let sequence = (envelope == EnvelopeKind::Sony).then_some((0x1001, SequenceWidth::Full32));
        let transmission_sequence = sequence.map(|(value, _)| value);

        // Both envelopes correlate an ACK to the one AwaitingAck request, but
        // raw does so by its target lane while Sony uses the envelope sequence.
        assert_correlated_frame_deadline_boundary(
            |start| {
                let mut engine = engine(envelope, TransportKind::Datagram);
                let (admission, id) = admit(
                    &mut engine,
                    1,
                    command_with_reply_shape_and_retry(
                        1,
                        CancellationPolicy::Supported,
                        ReplyShape::AckThenCompletion,
                        RetryPolicy::NEVER,
                    ),
                    start,
                );
                send_ok(&mut engine, &admission, transmission_sequence, start);
                let deadline = match phase_of(&engine, id) {
                    Some(Phase::AwaitingAck { deadline, .. }) => deadline,
                    phase => panic!("expected AwaitingAck, got {phase:?}"),
                };
                (
                    engine,
                    id,
                    deadline,
                    frame(
                        1,
                        sequence,
                        DecodedResponse::Ack {
                            socket: Some(ViscaSocket::S1),
                        },
                    ),
                )
            },
            DeadlineKind::Ack,
            |engine, id, _| {
                assert!(matches!(
                    phase_of(engine, id),
                    Some(Phase::Executing { .. })
                ));
            },
        );

        // Inquiry replies use their own route/FIFO or sequence correlation and
        // must obey the inquiry deadline rather than a command deadline.
        assert_correlated_frame_deadline_boundary(
            |start| {
                let mut engine = engine(envelope, TransportKind::Datagram);
                let (admission, id) = admit(
                    &mut engine,
                    2,
                    inquiry_with_retry(1, POWER, RetryPolicy::NEVER),
                    start,
                );
                send_ok(&mut engine, &admission, transmission_sequence, start);
                let deadline = match phase_of(&engine, id) {
                    Some(Phase::AwaitingReply { deadline, .. }) => deadline,
                    phase => panic!("expected AwaitingReply, got {phase:?}"),
                };
                (
                    engine,
                    id,
                    deadline,
                    frame(
                        1,
                        sequence,
                        DecodedResponse::InquiryReply {
                            route: Some(POWER),
                            payload: smallvec![1],
                        },
                    ),
                )
            },
            DeadlineKind::InquiryReply,
            |engine, id, effects| {
                assert!(engine.entry(id).is_none());
                assert!(matches!(
                    terminal_outcome(effects, id),
                    Some(RuntimeOutcome::Reply { .. })
                ));
            },
        );

        // Socket ownership (raw) and exact sequence correlation (Sony) both
        // identify an executing command's completion.
        assert_correlated_frame_deadline_boundary(
            |start| {
                let mut engine = engine(envelope, TransportKind::Datagram);
                let (admission, id) = admit(
                    &mut engine,
                    3,
                    command_with_reply_shape_and_retry(
                        1,
                        CancellationPolicy::Supported,
                        ReplyShape::AckThenCompletion,
                        RetryPolicy::NEVER,
                    ),
                    start,
                );
                send_ok(&mut engine, &admission, transmission_sequence, start);
                engine.handle(
                    frame(
                        1,
                        sequence,
                        DecodedResponse::Ack {
                            socket: Some(ViscaSocket::S1),
                        },
                    ),
                    start,
                );
                let deadline = match phase_of(&engine, id) {
                    Some(Phase::Executing { deadline, .. }) => deadline,
                    phase => panic!("expected Executing, got {phase:?}"),
                };
                (
                    engine,
                    id,
                    deadline,
                    frame(
                        1,
                        sequence,
                        DecodedResponse::Completion {
                            socket: Some(ViscaSocket::S1),
                        },
                    ),
                )
            },
            DeadlineKind::Completion,
            |engine, id, effects| {
                assert!(engine.entry(id).is_none());
                assert!(matches!(
                    terminal_outcome(effects, id),
                    Some(RuntimeOutcome::Applied)
                ));
            },
        );

        // A completion-only command has no ACK phase or socket, but its
        // completion is still a correlated deadline-bound protocol response.
        assert_correlated_frame_deadline_boundary(
            |start| {
                let mut engine = engine(envelope, TransportKind::Datagram);
                let (admission, id) = admit(
                    &mut engine,
                    4,
                    command_with_reply_shape_and_retry(
                        1,
                        CancellationPolicy::Supported,
                        ReplyShape::CompletionOnly,
                        RetryPolicy::NEVER,
                    ),
                    start,
                );
                send_ok(&mut engine, &admission, transmission_sequence, start);
                let deadline = match phase_of(&engine, id) {
                    Some(Phase::AwaitingCompletion { deadline, .. }) => deadline,
                    phase => panic!("expected AwaitingCompletion, got {phase:?}"),
                };
                (
                    engine,
                    id,
                    deadline,
                    frame(1, sequence, DecodedResponse::Completion { socket: None }),
                )
            },
            DeadlineKind::Completion,
            |engine, id, effects| {
                assert!(engine.entry(id).is_none());
                assert!(matches!(
                    terminal_outcome(effects, id),
                    Some(RuntimeOutcome::Applied)
                ));
            },
        );
    }

    // Sony can receive an exact completion before an ACK. It is valid at the
    // ACK boundary, but no longer after that still-active AwaitingAck deadline.
    assert_correlated_frame_deadline_boundary(
        |start| {
            let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
            let (admission, id) = admit(
                &mut engine,
                5,
                command_with_reply_shape_and_retry(
                    1,
                    CancellationPolicy::Supported,
                    ReplyShape::AckThenCompletion,
                    RetryPolicy::NEVER,
                ),
                start,
            );
            let sequence = 0x2001;
            send_ok(&mut engine, &admission, Some(sequence), start);
            let deadline = match phase_of(&engine, id) {
                Some(Phase::AwaitingAck { deadline, .. }) => deadline,
                phase => panic!("expected AwaitingAck, got {phase:?}"),
            };
            (
                engine,
                id,
                deadline,
                frame(
                    1,
                    Some((sequence, SequenceWidth::Full32)),
                    DecodedResponse::Completion { socket: None },
                ),
            )
        },
        DeadlineKind::Ack,
        |engine, id, effects| {
            assert!(engine.entry(id).is_none());
            assert!(matches!(
                terminal_outcome(effects, id),
                Some(RuntimeOutcome::Applied)
            ));
        },
    );
}

#[test]
fn an_overdue_request_does_not_make_an_unrelated_correlated_frame_stale() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);

    let (overdue_admission, overdue_id) = admit(
        &mut engine,
        1,
        command_with_reply_shape_and_retry(
            1,
            CancellationPolicy::Supported,
            ReplyShape::AckThenCompletion,
            RetryPolicy::NEVER,
        ),
        start,
    );
    send_ok(&mut engine, &overdue_admission, None, start);

    let (reply_admission, reply_id) = admit(
        &mut engine,
        2,
        inquiry_with_retry(2, POWER, RetryPolicy::NEVER),
        start,
    );
    send_ok(&mut engine, &reply_admission, None, start);

    // The command's ACK deadline was 20ms, but this target-two reply remains
    // within its own 30ms deadline. The engine must correlate the frame before
    // deciding whether it is stale, then run the target-one due transition.
    let effects = engine.handle(
        frame(
            2,
            None,
            DecodedResponse::InquiryReply {
                route: Some(POWER),
                payload: smallvec![1],
            },
        ),
        start + Duration::from_millis(20) + Duration::from_nanos(1),
    );
    assert!(ignored_reasons(&effects).is_empty());
    assert!(matches!(
        terminal_outcome(&effects, reply_id),
        Some(RuntimeOutcome::Reply { .. })
    ));
    assert_eq!(
        deadline_expiries(&effects, overdue_id),
        [(DeadlineKind::Ack, false)]
    );
    let reply = position_of(
        &effects,
        |effect| matches!(effect, Effect::Terminal { id, .. } if *id == reply_id),
    )
    .expect("unrelated reply is applied");
    let expired = position_of(&effects, |effect| {
        matches!(
            effect,
            Effect::DeadlineExpired {
                id,
                deadline: DeadlineKind::Ack,
                ..
            } if *id == overdue_id
        )
    })
    .expect("overdue command is processed after the reply");
    assert!(reply < expired);
    engine.assert_invariants().unwrap();
}

#[test]
fn cancellation_terminal_frames_are_inclusive_at_ambiguity_and_stale_afterward() {
    for envelope in [EnvelopeKind::Raw, EnvelopeKind::Sony] {
        let setup = |start| {
            let mut engine = engine(envelope, TransportKind::Datagram);
            let (admission, id) = admit(
                &mut engine,
                1,
                command_with_reply_shape_and_retry(
                    1,
                    CancellationPolicy::Supported,
                    ReplyShape::AckThenCompletion,
                    RetryPolicy::NEVER,
                ),
                start,
            );
            let request_sequence = (envelope == EnvelopeKind::Sony).then_some(0x3001);
            send_ok(&mut engine, &admission, request_sequence, start);
            engine.handle(
                frame(
                    1,
                    request_sequence.map(|value| (value, SequenceWidth::Full32)),
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                start,
            );
            let cancellation = engine.handle(Input::Cancel { id }, start);
            let (transmission, _, _) = cancel_transmit(&cancellation);
            let cancellation_sequence = (envelope == EnvelopeKind::Sony).then_some(0x3002);
            engine.handle(
                Input::TransmissionFinished {
                    transmission,
                    result: Ok(TransmissionMeta {
                        sequence: cancellation_sequence,
                    }),
                },
                start,
            );
            let deadline = match phase_of(&engine, id) {
                Some(Phase::AwaitingCancellationResolution { deadline, .. }) => deadline,
                phase => panic!("expected AwaitingCancellationResolution, got {phase:?}"),
            };
            (
                engine,
                id,
                deadline,
                frame(
                    1,
                    cancellation_sequence.map(|value| (value, SequenceWidth::Full32)),
                    DecodedResponse::Error {
                        socket: Some(ViscaSocket::S1),
                        code: 0x04,
                    },
                ),
            )
        };

        let start = Instant::now();
        let (mut equal_engine, equal_id, deadline, equal_input) = setup(start);
        let equal = equal_engine.handle(equal_input, deadline);
        assert!(ignored_reasons(&equal).is_empty());
        assert!(matches!(
            terminal_outcome(&equal, equal_id),
            Some(RuntimeOutcome::Cancelled)
        ));
        equal_engine.assert_invariants().unwrap();

        let (mut late_engine, late_id, deadline, late_input) = setup(start);
        let late = late_engine.handle(late_input, deadline + Duration::from_nanos(1));
        assert_eq!(ignored_reasons(&late), vec![IgnoreReason::UnmatchedFrame]);
        match envelope {
            EnvelopeKind::Raw => assert!(matches!(
                terminal_failure(&late, late_id),
                Some(Error::UnsequencedCommandUnconfirmed)
            )),
            EnvelopeKind::Sony => assert!(matches!(
                terminal_failure(&late, late_id),
                Some(Error::CancellationUnconfirmed)
            )),
        }
        let ignored = position_of(&late, |effect| {
            matches!(effect, Effect::Ignored(IgnoreReason::UnmatchedFrame))
        })
        .expect("late terminal frame is ignored");
        let terminal = position_of(
            &late,
            |effect| matches!(effect, Effect::Terminal { id, .. } if *id == late_id),
        )
        .expect("ambiguity due transition");
        assert!(ignored < terminal);
        late_engine.assert_invariants().unwrap();
    }
}

#[test]
fn cancellation_ambiguity_closes_an_executing_response_before_its_completion_deadline() {
    let setup = |start| {
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let mut request = command_with_reply_shape_and_retry(
            1,
            CancellationPolicy::Supported,
            ReplyShape::AckThenCompletion,
            RetryPolicy::NEVER,
        );
        let RuntimeRequest::Command { context, .. } = &mut request else {
            unreachable!("test constructs a command");
        };
        context.timeout.completion = Duration::from_secs(1);
        let (admission, id) = admit(&mut engine, 1, request, start);
        send_ok(&mut engine, &admission, None, start);
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
        let cancellation = engine.handle(Input::Cancel { id }, start + Duration::from_millis(1));
        assert!(cancel_transmit_optional(&cancellation).is_some());
        let ambiguity_deadline = match engine.entry(id).map(Entry::cancellation) {
            Some(CancelState::Sending {
                ambiguity_deadline, ..
            }) => ambiguity_deadline,
            cancellation => panic!("expected a sent cancellation, got {cancellation:?}"),
        };
        assert!(matches!(
            phase_of(&engine, id),
            Some(Phase::Executing { .. })
        ));
        (
            engine,
            id,
            ambiguity_deadline,
            frame(
                1,
                None,
                DecodedResponse::Completion {
                    socket: Some(ViscaSocket::S1),
                },
            ),
        )
    };

    let start = Instant::now();
    let (mut equal_engine, equal_id, deadline, equal_input) = setup(start);
    let equal = equal_engine.handle(equal_input, deadline);
    assert!(ignored_reasons(&equal).is_empty());
    assert!(matches!(
        terminal_outcome(&equal, equal_id),
        Some(RuntimeOutcome::Applied)
    ));
    equal_engine.assert_invariants().unwrap();

    let (mut late_engine, late_id, deadline, late_input) = setup(start);
    let late = late_engine.handle(late_input, deadline + Duration::from_nanos(1));
    assert_eq!(ignored_reasons(&late), vec![IgnoreReason::UnmatchedFrame]);
    assert!(matches!(
        terminal_failure(&late, late_id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    late_engine.assert_invariants().unwrap();
}

#[test]
fn exact_first_dispatch_preserves_admission_order_and_queues_the_loser() {
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
    let inquiry_before = (
        inquiry_engine.entry(inquiry_id).unwrap().phase(),
        inquiry_engine.entry(inquiry_id).unwrap().cancellation(),
    );

    assert!(inquiry_engine.transmissions.is_empty());
    assert!(inquiry_engine.last_request_sent.is_none());
    let command_dispatch = match inquiry_engine.first_dispatch_without_due(command_id, start) {
        FirstDispatch::Effects(effects) => effects,
        other => panic!("expected command dispatch effects, got {other:?}"),
    };
    assert_eq!(request_transmit(&command_dispatch).1, command_id);

    // Issue #561: losing the same-class race leaves the inquiry queued, never
    // terminal, even when the caller asks for its exact first dispatch.
    assert_eq!(
        inquiry_engine
            .entry(inquiry_id)
            .map(|entry| (entry.phase(), entry.cancellation())),
        Some(inquiry_before)
    );

    let inquiry_dispatch = match inquiry_engine.first_dispatch_without_due(inquiry_id, start) {
        FirstDispatch::Effects(effects) => effects,
        other => panic!("expected inquiry dispatch effects, got {other:?}"),
    };
    let (_, dispatched_id, _) = request_transmit(&inquiry_dispatch);
    assert_eq!(dispatched_id, inquiry_id);

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
fn requested_executing_cancellation_blocks_first_dispatch_without_due() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let first = engine.admit_without_due(
        AdmissionTicket(1),
        command(1, CancellationPolicy::Supported),
        start,
    );
    let first_id = admitted(&first);
    let first_dispatch = match engine.first_dispatch_without_due(first_id, start) {
        FirstDispatch::Effects(effects) => effects,
        other => panic!("expected first request dispatch effects, got {other:?}"),
    };
    let (first_tx, _, _) = request_transmit(&first_dispatch);
    engine.finish_write_without_due(first_tx, Ok(TransmissionMeta { sequence: None }), start);
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
    engine.policy.command_spacing = Duration::from_millis(10);

    let requested_at = start + Duration::from_millis(1);
    let requested = engine.handle(Input::Cancel { id: first_id }, requested_at);
    assert!(cancel_transmit_optional(&requested).is_none());
    assert!(matches!(
        engine.entry(first_id).map(Entry::cancellation),
        Some(CancelState::Requested { .. })
    ));

    let ordinary = engine.admit_without_due(
        AdmissionTicket(2),
        command(2, CancellationPolicy::Supported),
        requested_at,
    );
    let ordinary_id = admitted(&ordinary);
    let before = (
        engine
            .entry(first_id)
            .map(|entry| (entry.phase(), entry.cancellation())),
        engine
            .entry(ordinary_id)
            .map(|entry| (entry.phase(), entry.cancellation())),
        engine.transmissions.len(),
        engine.last_request_sent,
    );
    assert!(matches!(
        engine.first_dispatch_without_due(ordinary_id, requested_at),
        FirstDispatch::Blocked
    ));
    assert_eq!(
        (
            engine
                .entry(first_id)
                .map(|entry| (entry.phase(), entry.cancellation())),
            engine
                .entry(ordinary_id)
                .map(|entry| (entry.phase(), entry.cancellation())),
            engine.transmissions.len(),
            engine.last_request_sent,
        ),
        before
    );

    let cancellation = engine.advance(start + Duration::from_millis(10));
    assert_eq!(cancel_transmit(&cancellation).1, first_id);
    assert!(matches!(
        engine.entry(first_id).map(Entry::cancellation),
        Some(CancelState::Sending { .. })
    ));
    assert_eq!(engine.next_wake(), Some(start + Duration::from_millis(20)));
    let released = engine.advance(start + Duration::from_millis(20));
    assert_eq!(request_transmit(&released).1, ordinary_id);
    engine.assert_invariants().unwrap();
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
    // Issue #671: the active command's lost ACK is quarantined per-request, not
    // poisoned. At the ACK deadline it moves into its late-ACK quarantine and
    // the session stays live; nothing is retried (a raw command is never
    // replayed) and no terminal is emitted yet.
    let timeout = engine.advance(start + Duration::from_millis(20));
    assert_eq!(engine.state(), SessionState::Running);
    assert!(matches!(
        engine.entry(active_id).map(Entry::phase),
        Some(Phase::AwaitingLateAck { .. })
    ));
    assert!(!timeout
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { .. })));
    assert!(!timeout
        .iter()
        .any(|effect| matches!(effect, Effect::RetryScheduled { .. })));
    // The one request fails at its ambiguity deadline (20ms + 50ms); the session
    // is still running for any other work.
    let ambiguity_deadline = start + Duration::from_millis(70);
    assert_eq!(engine.next_wake(), Some(ambiguity_deadline));
    let resolved = engine.advance(ambiguity_deadline);
    assert!(resolved.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id,
            outcome: RuntimeOutcome::Failed(Error::UnsequencedCommandUnconfirmed),
        } if *id == active_id
    )));
    assert_eq!(engine.state(), SessionState::Running);
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
fn requested_cancellation_waits_for_shared_command_spacing() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    engine.policy.command_spacing = Duration::from_millis(10);
    let admitted_effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let (_, id, _) = request_transmit(&admitted_effects);
    send_ok(&mut engine, &admitted_effects, None, start);
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

    let before_spacing = start + Duration::from_millis(1);
    let cancellation = engine.handle(Input::Cancel { id }, before_spacing);
    assert!(!cancellation.iter().any(|effect| matches!(
        effect,
        Effect::Transmit {
            kind: Transmission::Cancel { .. },
            ..
        }
    )));
    assert!(matches!(
        engine.entry(id).map(Entry::cancellation),
        Some(CancelState::Requested { .. })
    ));
    assert_eq!(engine.next_wake(), Some(start + Duration::from_millis(10)));
    engine.assert_invariants().unwrap();
}

#[test]
fn pending_cancellation_transmits_before_ordinary_work_and_advances_spacing() {
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

    engine.policy.command_spacing = Duration::from_millis(10);
    let queued = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1, CancellationPolicy::Supported),
        },
        start + Duration::from_millis(1),
    );
    let queued_id = admitted(&queued);
    assert!(request_transmit_optional(&queued).is_none());
    let requested = engine.handle(
        Input::Cancel { id: first_id },
        start + Duration::from_millis(1),
    );
    assert!(cancel_transmit_optional(&requested).is_none());

    let first_eligible = start + Duration::from_millis(10);
    let cancellation = engine.advance(first_eligible);
    assert_eq!(cancel_transmit(&cancellation).1, first_id);
    assert!(request_transmit_optional(&cancellation).is_none());
    assert_eq!(engine.last_request_sent, Some(first_eligible));
    assert!(matches!(
        engine.entry(queued_id).map(Entry::phase),
        Some(Phase::Ready { .. })
    ));
    assert_eq!(engine.next_wake(), Some(start + Duration::from_millis(20)));

    let ordinary = engine.advance(start + Duration::from_millis(20));
    assert_eq!(request_transmit(&ordinary).1, queued_id);
    assert!(!ordinary.iter().any(|effect| matches!(
        effect,
        Effect::Transmit {
            kind: Transmission::Cancel { .. },
            ..
        }
    )));
    engine.assert_invariants().unwrap();
}

#[test]
fn zero_spacing_drains_pending_cancellations_in_admission_order() {
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
    engine.policy.command_spacing = Duration::from_millis(10);

    let ordinary = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: command(2, CancellationPolicy::Supported),
        },
        start + Duration::from_millis(1),
    );
    let ordinary_id = admitted(&ordinary);
    assert!(request_transmit_optional(&ordinary).is_none());
    assert!(cancel_transmit_optional(&engine.handle(
        Input::Cancel { id: first_id },
        start + Duration::from_millis(1),
    ))
    .is_none());
    assert!(cancel_transmit_optional(&engine.handle(
        Input::Cancel { id: second_id },
        start + Duration::from_millis(1),
    ))
    .is_none());

    // The pending intents were admitted under a positive spacing floor.  A
    // zero-spacing profile can now drain both in urgent order in one turn.
    engine.policy.command_spacing = Duration::ZERO;
    let drained = engine.advance(start + Duration::from_millis(1));
    let cancellation_ids: Vec<_> = drained
        .iter()
        .filter_map(|effect| match effect {
            Effect::Transmit {
                request,
                kind: Transmission::Cancel { .. },
                ..
            } => Some(*request),
            _ => None,
        })
        .collect();
    assert_eq!(cancellation_ids, vec![first_id, second_id]);
    assert_eq!(request_transmit(&drained).1, ordinary_id);
    let ordinary_index = drained
        .iter()
        .position(|effect| {
            matches!(
                effect,
                Effect::Transmit {
                    request,
                    kind: Transmission::Request { .. },
                    ..
                } if *request == ordinary_id
            )
        })
        .expect("ordinary transmission");
    let last_cancel_index = drained
        .iter()
        .rposition(|effect| {
            matches!(
                effect,
                Effect::Transmit {
                    kind: Transmission::Cancel { .. },
                    ..
                }
            )
        })
        .expect("cancellation transmission");
    assert!(last_cancel_index < ordinary_index);
    engine.assert_invariants().unwrap();
}

#[test]
fn cancellation_ambiguity_deadline_wins_over_later_pacing_eligibility() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    engine.policy.command_spacing = Duration::from_millis(100);
    let mut request_context = context(1, CancellationPolicy::Supported);
    request_context.timeout.completion = Duration::from_secs(1);
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
        start,
    );
    let requested_at = start + Duration::from_millis(1);
    assert!(
        request_transmit_optional(&engine.handle(Input::Cancel { id }, requested_at,)).is_none()
    );
    let ambiguity_deadline = start + Duration::from_millis(51);
    assert_eq!(engine.next_wake(), Some(ambiguity_deadline));

    let expired = engine.advance(ambiguity_deadline);
    assert!(!expired.iter().any(|effect| matches!(
        effect,
        Effect::Transmit {
            kind: Transmission::Cancel { .. },
            ..
        }
    )));
    assert!(expired.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id: seen,
            outcome: RuntimeOutcome::Failed(Error::UnsequencedCommandUnconfirmed),
        } if *seen == id
    )));
    // Issue #671: the ambiguity expiry fails only this request; the raw session
    // stays live rather than poisoning.
    assert_eq!(engine.state(), SessionState::Running);
    engine.assert_invariants().unwrap();
}

#[test]
fn cancel_in_late_ack_quarantine_extends_eligibility_to_its_ambiguity_deadline() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let request = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let id = admitted(&request);
    send_ok(&mut engine, &request, None, start);

    // The ordinary raw ACK timeout starts an inert #671 quarantine. It is
    // still marked `None`, so a late ACK cannot re-open it before cancellation.
    let original_deadline = start + Duration::from_millis(70);
    let timed_out = engine.advance(start + Duration::from_millis(20));
    assert!(!timed_out
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { id: seen, .. } if *seen == id)));
    assert!(matches!(
        phase_of(&engine, id),
        Some(Phase::AwaitingLateAck { deadline }) if deadline == original_deadline
    ));
    assert_eq!(
        engine.entry(id).map(Entry::cancellation),
        Some(CancelState::None)
    );

    // A later cancellation creates a live late-ACK path through its own 50ms
    // ambiguity window, not the earlier quarantine deadline.
    let extended_deadline = start + Duration::from_millis(80);
    let cancelled = engine.handle(Input::Cancel { id }, start + Duration::from_millis(30));
    assert!(cancelled
        .iter()
        .any(|effect| matches!(effect, Effect::CancellationRecorded { id: seen } if *seen == id)));
    assert!(matches!(
        phase_of(&engine, id),
        Some(Phase::AwaitingLateAck { deadline }) if deadline == extended_deadline
    ));
    assert_eq!(
        engine.entry(id).map(Entry::cancellation),
        Some(CancelState::Requested {
            ambiguity_deadline: extended_deadline,
        })
    );
    assert_eq!(engine.next_wake(), Some(extended_deadline));

    // The original deadline is now inert; the later ACK is still accepted and
    // immediately produces the socket cancellation it made possible.
    let old_deadline = engine.advance(original_deadline);
    assert!(!old_deadline
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { id: seen, .. } if *seen == id)));
    let late_ack = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(75),
    );
    assert!(matches!(
        phase_of(&engine, id),
        Some(Phase::Executing {
            socket: ViscaSocket::S1,
            ..
        })
    ));
    assert_eq!(cancel_transmit(&late_ack).1, id);
    engine.assert_invariants().unwrap();
}

#[test]
fn completion_before_cancellation_pacing_due_wins_and_removes_intent() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    engine.policy.command_spacing = Duration::from_millis(10);
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
        start,
    );
    let requested_at = start + Duration::from_millis(1);
    assert!(
        request_transmit_optional(&engine.handle(Input::Cancel { id }, requested_at,)).is_none()
    );

    let completed = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(5),
    );
    assert!(completed.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id: seen,
            outcome: RuntimeOutcome::Applied,
        } if *seen == id
    )));
    assert!(!completed.iter().any(|effect| matches!(
        effect,
        Effect::Transmit {
            kind: Transmission::Cancel { .. },
            ..
        }
    )));
    assert!(engine.entry(id).is_none());

    let after_pacing = engine.advance(start + Duration::from_millis(10));
    assert!(!after_pacing.iter().any(|effect| matches!(
        effect,
        Effect::Transmit {
            kind: Transmission::Cancel { .. },
            ..
        }
    )));
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
        &[
            i64::from(crate::command::PanTiltLimitCorner::UpRight.to_byte()),
            1,
            2,
        ],
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
        // Each terminal raw command retains its socket evidence briefly. This
        // cache-projection test is not exercising stale-frame handling, so
        // advance past that bounded hold before reusing S1.
        let now = start + Duration::from_millis(offset as u64 * 51);
        let effects = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(offset as u64 + 1),
                request: RuntimeRequest::Command {
                    wire: wire(0x81),
                    context: context(1, CancellationPolicy::Supported),
                    applied_state: Some(projection),
                },
            },
            now,
        );
        let id = admitted(&effects);
        send_ok(&mut engine, &effects, None, now);
        engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            now,
        );
        let completion = engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Completion {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            now,
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
        start + Duration::from_millis(153),
    );
    let (transmission, id, _) = request_transmit(&failed);
    let failed = engine.handle(
        Input::TransmissionFinished {
            transmission,
            result: Err(Error::TransportError("request write".into())),
        },
        start + Duration::from_millis(153),
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
fn dispatch_is_priority_fifo_in_admission_order_across_lanes() {
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
    assert_eq!(request_transmit(&selected).1, command_id);
    assert_ne!(request_transmit(&selected).1, inquiry_id);

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
fn newer_same_class_inquiries_cannot_starve_an_older_command() {
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

    // A burst of later inquiries accumulates behind the shared pacing floor.
    // It must not form a preferred lane that can perpetually leap over the
    // already-eligible command once that floor opens.
    let mut inquiry_ids = Vec::new();
    for ticket in 3..=8 {
        let inquiry = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(ticket),
                request: inquiry(2, POWER),
            },
            start,
        );
        inquiry_ids.push(admitted(&inquiry));
        assert!(request_transmit_optional(&inquiry).is_none());
    }

    let selected = engine.advance(start + Duration::from_millis(10));
    assert_eq!(request_transmit(&selected).1, command_id);
    for inquiry_id in inquiry_ids {
        assert!(matches!(
            engine.entry(inquiry_id).map(Entry::phase),
            Some(Phase::Ready { .. })
        ));
    }
    engine.assert_invariants().unwrap();
}

#[test]
fn retune_to_one_socket_allows_preexisting_second_command_to_drain_via_fallback() {
    let start = Instant::now();
    let mut engine = engine_with_target(
        EnvelopeKind::Raw,
        TransportKind::Datagram,
        2,
        CancellationPolicy::Supported,
    );
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

    // This request was dispatched under the two-socket policy and is still
    // awaiting its ACK when the live capacity is lowered.
    let second = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let second_id = admitted(&second);
    send_ok(&mut engine, &second, None, start);
    assert_eq!(
        engine.entry(second_id).unwrap().dispatched_socket_capacity,
        Some(2)
    );

    let mut retuned_sockets = [None; 9];
    retuned_sockets[1] = Some(1);
    engine
        .retune(Duration::ZERO, Duration::ZERO, retuned_sockets)
        .unwrap();
    assert_eq!(engine.command_sockets(camera(1)), 1);

    // New work observes the reduced concurrency limit immediately.
    let third = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: command(1, CancellationPolicy::Supported),
        },
        start + Duration::from_millis(1),
    );
    let third_id = admitted(&third);
    assert!(request_transmit_optional(&third).is_none());

    // The camera reuses the busy first socket in its ACK, so the second
    // pre-retune request must fall back to the still-physical S2.
    let fallback = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(2),
    );
    assert!(!fallback
        .iter()
        .any(|effect| matches!(effect, Effect::Ignored(IgnoreReason::SocketConflict))));
    assert!(matches!(
        phase_of(&engine, second_id),
        Some(Phase::Executing {
            socket: ViscaSocket::S2,
            ..
        })
    ));
    assert_eq!(
        engine.socket_owner(camera(1), ViscaSocket::S2),
        Some(second_id)
    );

    // The lower limit continues to gate future work until both legacy
    // in-flight commands have drained.
    let first_complete = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(3),
    );
    assert!(request_transmit_optional(&first_complete).is_none());
    assert!(matches!(
        phase_of(&engine, third_id),
        Some(Phase::Ready { .. })
    ));
    assert!(engine.entry(first_id).is_none());

    let second_complete = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S2),
            },
        ),
        start + Duration::from_millis(4),
    );
    assert_eq!(request_transmit(&second_complete).1, third_id);
    assert_eq!(
        engine.entry(third_id).unwrap().dispatched_socket_capacity,
        Some(1)
    );
    engine.assert_invariants().unwrap();
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

    // Releasing A's raw inquiry FIFO owner also leaves its target in the
    // bounded ambiguity hold. A delayed attempt-N payload has no wire identity
    // and must not finish B while B is queued or after it is requeued.
    let stale_attempt = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: Some(POWER),
                payload: smallvec![0x0a],
            },
        ),
        start + Duration::from_nanos(1),
    );
    assert_eq!(
        ignored_reasons(&stale_attempt),
        vec![IgnoreReason::UnmatchedFrame]
    );
    assert!(terminal_outcome(&stale_attempt, second_id).is_none());

    // The syntax cooldown makes A retry-eligible before the target hold ends,
    // but neither B nor the requeued A may use the released correlation early.
    let before_release = engine.advance(start + Duration::from_millis(25));
    assert!(request_transmit_optional(&before_release).is_none());

    // At A's exact hold expiry, B remains ahead of the retried A in the FIFO.
    let promoted = engine.advance(start + Duration::from_millis(50));
    let (_, dispatched, _) = request_transmit(&promoted);
    assert_eq!(dispatched, second_id);
    let second_sent = send_ok(
        &mut engine,
        &promoted,
        None,
        start + Duration::from_millis(50),
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
        start + Duration::from_millis(50),
    );
    assert!(matches!(
        terminal_outcome(&second_reply, second_id),
        Some(RuntimeOutcome::Reply { payload, .. }) if payload.as_slice() == [1]
    ));
    // A matched reply cannot produce a late duplicate, so B's success releases
    // the FIFO-tail retry immediately without adding another hold (#712).
    let (retry_tx, retried_id, retried_wire) = request_transmit(&second_reply);
    assert_eq!(retried_id, first_id);
    assert!(Arc::ptr_eq(&first_wire, &retried_wire));
    engine.handle(
        Input::TransmissionFinished {
            transmission: retry_tx,
            result: Ok(TransmissionMeta { sequence: None }),
        },
        start + Duration::from_millis(50),
    );
    assert_eq!(engine.raw_inquiry_front(camera(1)), Some(first_id));

    let legitimate_retry = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: Some(POWER),
                payload: smallvec![0x0b],
            },
        ),
        start + Duration::from_millis(50),
    );
    assert!(matches!(
        terminal_outcome(&legitimate_retry, first_id),
        Some(RuntimeOutcome::Reply { payload, .. }) if payload.as_slice() == [0x0b]
    ));
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
                pending_observation =
                    Some(fixture_observation(engine, &effects, &names, response_name));
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

/// Names the inquiry route the engine actually attributed to a reply.
fn fixture_route_name(route: Option<InquiryRoute>) -> &'static str {
    match route {
        None => "unknown",
        Some(route) if route == POWER => "power",
        Some(route) if route == ZOOM => "zoom",
        Some(route) if route == FOCUS => "focus",
        Some(route) if route == InquiryRoute::UNKNOWN => "unknown",
        Some(other) => panic!("engine reported an unknown inquiry route {other:?}"),
    }
}

/// Recovers the VISCA error code the engine's terminal error stands for.
///
/// This is the inverse of [`Error::from_code`], so an engine that collapses
/// distinct camera error codes onto one error renders the wrong code and the
/// fixture's expectation column rejects it.
fn fixture_error_code(error: &Error) -> String {
    let code = match error {
        Error::MessageLengthError => 0x01,
        Error::SyntaxError => 0x02,
        Error::CommandBufferFull => 0x03,
        Error::CommandCanceled => 0x04,
        Error::NoSocket => 0x05,
        Error::CommandNotExecutable => 0x41,
        Error::Unknown(code) => *code,
        other => panic!("a camera error frame produced a non-camera error: {other:?}"),
    };
    format!("0x{code:02X}")
}

/// Every column below is read out of the engine's own effects. Nothing is
/// rebuilt from the fixture's input record, so corrupting a reply payload, a
/// reply route, an attributed socket or an error code fails the expectation.
fn fixture_observation(
    engine: &ProtocolEngine,
    effects: &[Effect],
    names: &BTreeMap<RequestId, String>,
    response: &str,
) -> String {
    if let Some((id, socket)) = effects.iter().find_map(|effect| match effect {
        Effect::Transition {
            id,
            to: Phase::Executing { socket, .. },
            ..
        } => Some((*id, *socket)),
        _ => None,
    }) {
        let target = engine
            .entry(id)
            .expect("an acknowledged request keeps its engine entry")
            .request
            .context()
            .target;
        return format!(
            "observer=engine outcome=ack:{}:target={}:socket={}",
            names.get(&id).unwrap(),
            target.id(),
            socket.as_socket_number()
        );
    }
    if let Some((id, outcome)) = effects.iter().find_map(|effect| match effect {
        Effect::Terminal { id, outcome } => Some((*id, outcome)),
        _ => None,
    }) {
        let name = names.get(&id).unwrap();
        let outcome = match outcome {
            RuntimeOutcome::Written => "written".to_owned(),
            RuntimeOutcome::Applied => "applied".to_owned(),
            RuntimeOutcome::Reply { route, payload } => format!(
                "reply:{}:{}",
                fixture_route_name(*route),
                String::from_utf8_lossy(payload)
            ),
            RuntimeOutcome::Failed(error) => {
                format!("error:{}", fixture_error_code(error))
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

/// Every session shape the engine can actually be configured into.
///
/// Issue #636: the invariant fuzzers predated #620 and only ever ran raw
/// framing over a datagram transport, so the Sony correlation indexes, the
/// stream poisoning path, and every socketless frame shape were generated by
/// nothing.
const FUZZ_CONFIGURATIONS: [(EnvelopeKind, TransportKind); 4] = [
    (EnvelopeKind::Raw, TransportKind::Datagram),
    (EnvelopeKind::Raw, TransportKind::Stream),
    (EnvelopeKind::Sony, TransportKind::Datagram),
    (EnvelopeKind::Sony, TransportKind::Stream),
];

/// A receive-side fault the owner has already classified as transient.
fn fuzz_receive_fault(action: u64) -> Error {
    match action % 4 {
        0 => Error::Timeout,
        1 => Error::NoResponse,
        2 => Error::TransportBusy,
        _ => Error::TransportError("fuzz receive fault".into()),
    }
}

/// A frame the engine can actually attribute, drawn from one live entry.
///
/// Purely arbitrary frames almost never match a request's current phase, so a
/// generator built only from them audits the inert paths and very little else.
/// This branch keeps the same determinism — the entry is chosen by index into
/// the ordered entry map, the socket and sequence are read back out of that
/// entry — while driving real request lifecycles through ACK, completion,
/// camera error and inquiry reply. A command still being written is answered
/// too, which is what exercises the deferred-ACK latch (#636).
fn fuzz_directed_frame(engine: &ProtocolEngine, action: u64) -> Option<Input> {
    if engine.entries.is_empty() {
        return None;
    }
    // Rotate from a generated offset to the first entry that a frame can say
    // anything about, so a run full of queued work still reaches the wire.
    let offset = (action >> 44) as usize % engine.entries.len();
    let entry = engine
        .entries
        .values()
        .cycle()
        .skip(offset)
        .take(engine.entries.len())
        .find(|entry| {
            !matches!(entry.phase, Phase::Ready { .. } | Phase::Backoff { .. })
                && (engine.policy.envelope == EnvelopeKind::Raw
                    || !entry.sequence_history.is_empty())
        })?;
    let other = if action & 0x8000 == 0 {
        ViscaSocket::S1
    } else {
        ViscaSocket::S2
    };
    let response = match entry.phase {
        Phase::Sending { .. } | Phase::AwaitingAck { .. } | Phase::AwaitingLateAck { .. } => {
            DecodedResponse::Ack {
                socket: (action & 0x200 == 0).then_some(other),
            }
        }
        Phase::Executing { socket, .. } | Phase::AwaitingCancellationResolution { socket, .. } => {
            if action & 0x4000 == 0 {
                DecodedResponse::Completion {
                    socket: (action & 0x200 == 0).then_some(socket),
                }
            } else {
                DecodedResponse::Error {
                    socket: Some(socket),
                    code: [0x01, 0x02, 0x03, 0x04, 0x41][(action as usize) % 5],
                }
            }
        }
        Phase::AwaitingCompletion { .. } => DecodedResponse::Completion { socket: None },
        Phase::AwaitingReply { .. } => DecodedResponse::InquiryReply {
            route: entry.request.inquiry_route(),
            payload: smallvec![action as u8],
        },
        Phase::Ready { .. } | Phase::Backoff { .. } => return None,
    };
    Some(Input::Frame(DecodedFrame {
        target: entry.request.context().target,
        // Sony correlation is by envelope sequence, so a directed frame quotes
        // the sequence the request actually registered — at either width.
        sequence: entry
            .sequence_history
            .last()
            .map(|record| EnvelopeSequence {
                value: record.sequence,
                width: if action & 0x400 == 0 {
                    SequenceWidth::Full32
                } else {
                    SequenceWidth::Lower16
                },
            }),
        response,
    }))
}

/// Applies one generated input to `engine`.
///
/// Every choice is a pure function of `action` and the configuration: no wall
/// clock, no entropy, no ambient state. The loop-driven fuzzer and its
/// proptest twin share this so the two can never drift apart, and replaying an
/// action stream reproduces an engine byte for byte.
fn fuzz_step(
    engine: &mut ProtocolEngine,
    envelope: EnvelopeKind,
    transport: TransportKind,
    action: u64,
    terminal_faults: bool,
    ticket: &mut u64,
    now: &mut Instant,
) -> Vec<Effect> {
    let target = ((action >> 8) & 1) as u8 + 1;
    let socket = if action & 0x100 == 0 {
        ViscaSocket::S1
    } else {
        ViscaSocket::S2
    };
    // The socket nibble is genuinely optional on the wire: a camera may answer
    // `90 40 FF` / `90 50 FF` with no socket at all.
    let named_socket = (action & 0x200 == 0).then_some(socket);
    let route = InquiryRoute(((action >> 20) as u16 % 3) + 1);
    let sequence_value = ((action >> 24) & 0xffff) as u32;
    // Sony correlation runs on the envelope sequence; raw framing carries none,
    // and each width is a different index (exact and lower-16).
    let width = if action & 0x400 == 0 {
        SequenceWidth::Full32
    } else {
        SequenceWidth::Lower16
    };
    let sequence = (envelope == EnvelopeKind::Sony).then_some((sequence_value, width));
    let write_sequence = (envelope == EnvelopeKind::Sony).then_some(sequence_value);
    match action % 12 {
        0 => {
            *ticket = ticket.wrapping_add(1);
            let request = if action & 0x10000 == 0 {
                command(target, CancellationPolicy::Supported)
            } else {
                inquiry(target, route)
            };
            engine.handle(
                Input::Admit {
                    ticket: AdmissionTicket(*ticket),
                    request,
                },
                *now,
            )
        }
        1 => {
            let Some(transmission) = engine.transmissions.keys().next().copied() else {
                return Vec::new();
            };
            // A failed datagram write fails exactly one request; a failed
            // stream write poisons the entire session, so it is generated only
            // once `terminal_faults` opens — the run earns its deep protocol
            // coverage first and then audits the poisoned tail.
            let fails = match transport {
                TransportKind::Datagram => action & 0x2_0000 == 0,
                TransportKind::Stream => terminal_faults && (action >> 33).is_multiple_of(8),
            };
            let result = if fails {
                Err(Error::Timeout)
            } else {
                Ok(TransmissionMeta {
                    sequence: write_sequence,
                })
            };
            engine.handle(
                Input::TransmissionFinished {
                    transmission,
                    result,
                },
                *now,
            )
        }
        2 => {
            let response = match (action >> 12) % 4 {
                0 => DecodedResponse::Ack {
                    socket: named_socket,
                },
                1 => DecodedResponse::Completion {
                    socket: named_socket,
                },
                2 => DecodedResponse::Error {
                    socket: named_socket,
                    code: [0x01, 0x02, 0x03, 0x04, 0x41][(action as usize) % 5],
                },
                _ => DecodedResponse::InquiryReply {
                    route: (action & 0x4000 != 0).then_some(route),
                    payload: smallvec![action as u8],
                },
            };
            engine.handle(frame(target, sequence, response), *now)
        }
        3 => {
            let Some(id) = engine.entries.keys().next().copied() else {
                return Vec::new();
            };
            engine.handle(Input::Cancel { id }, *now)
        }
        4 => engine.advance(*now),
        5 => {
            let stale = TransmissionId::from_nonzero(
                NonZeroU64::new(action | 1).expect("an odd value is never zero"),
            );
            engine.handle(
                Input::TransmissionFinished {
                    transmission: stale,
                    result: Ok(TransmissionMeta {
                        sequence: write_sequence,
                    }),
                },
                *now,
            )
        }
        6 => {
            let Some(wake) = engine.next_wake() else {
                return Vec::new();
            };
            *now = wake.max(*now);
            engine.advance(*now)
        }
        7 => engine.handle(
            Input::ReceiveFault {
                error: fuzz_receive_fault(action >> 16),
            },
            *now,
        ),
        8 => engine.handle(
            frame(target, sequence, DecodedResponse::NetworkChange),
            *now,
        ),
        9 | 10 => {
            let Some(input) = fuzz_directed_frame(engine, action) else {
                return Vec::new();
            };
            engine.handle(input, *now)
        }
        _ => {
            // A frame whose envelope sequence contradicts the session shape:
            // raw framing never carries one, Sony framing always does.
            let mismatched = sequence
                .is_none()
                .then_some((sequence_value, SequenceWidth::Lower16));
            engine.handle(frame(target, mismatched, DecodedResponse::Unknown), *now)
        }
    }
}

/// What a fuzz run actually reached, so the generators cannot quietly decay
/// into a stream of inert frames while still "passing".
#[derive(Debug, Default)]
struct FuzzCoverage {
    acknowledged: usize,
    applied: usize,
    replied: usize,
    retried: usize,
    cancelled: usize,
    ignored: usize,
}

impl FuzzCoverage {
    fn record(&mut self, effects: &[Effect]) {
        for effect in effects {
            match effect {
                Effect::Transition {
                    to: Phase::Executing { .. },
                    ..
                } => self.acknowledged += 1,
                Effect::Terminal {
                    outcome: RuntimeOutcome::Applied,
                    ..
                } => self.applied += 1,
                Effect::Terminal {
                    outcome: RuntimeOutcome::Reply { .. },
                    ..
                } => self.replied += 1,
                Effect::Terminal {
                    outcome: RuntimeOutcome::Cancelled,
                    ..
                } => self.cancelled += 1,
                Effect::RetryScheduled { .. } => self.retried += 1,
                Effect::Ignored(_) => self.ignored += 1,
                _ => {}
            }
        }
    }
}

/// Seeds each generated session with one complete, one replied, one retried,
/// and one cancelled request before the stale/reordered stream begins. The
/// seed is deliberately real protocol traffic: raw retries use a conclusive
/// camera rejection, while Sony retries carry and reuse an envelope sequence.
/// That keeps the coverage assertions meaningful after raw ambiguity became a
/// terminal session error rather than a retry trigger.
fn seed_admit(
    engine: &mut ProtocolEngine,
    ticket: &mut u64,
    coverage: &mut FuzzCoverage,
    request: RuntimeRequest,
    now: Instant,
) -> Vec<Effect> {
    *ticket += 1;
    let effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(*ticket),
            request,
        },
        now,
    );
    coverage.record(&effects);
    effects
}

fn seed_fuzz_coverage(
    engine: &mut ProtocolEngine,
    envelope: EnvelopeKind,
    ticket: &mut u64,
    now: &mut Instant,
    coverage: &mut FuzzCoverage,
) {
    let sequence =
        |value| (envelope == EnvelopeKind::Sony).then_some((value, SequenceWidth::Full32));
    let write_sequence = |value| (envelope == EnvelopeKind::Sony).then_some(value);

    // A normal command reaches Applied, exercising ACK and completion
    // attribution for both envelopes.
    let admitted_effects = seed_admit(
        engine,
        ticket,
        coverage,
        command(1, CancellationPolicy::Supported),
        *now,
    );
    let (transmission, id, _) = request_transmit(&admitted_effects);
    let sent = engine.handle(
        Input::TransmissionFinished {
            transmission,
            result: Ok(TransmissionMeta {
                sequence: write_sequence(0x5101),
            }),
        },
        *now,
    );
    coverage.record(&sent);
    let acknowledged = engine.handle(
        frame(
            1,
            sequence(0x5101),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        *now + Duration::from_micros(1),
    );
    coverage.record(&acknowledged);
    let completed = engine.handle(
        frame(
            1,
            sequence(0x5101),
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        *now + Duration::from_micros(2),
    );
    coverage.record(&completed);
    assert!(completed.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id: terminal,
            outcome: RuntimeOutcome::Applied,
        } if *terminal == id
    )));
    *now += Duration::from_micros(2);

    // An inquiry reaches a reply without an ACK phase.
    let admitted_effects = seed_admit(
        engine,
        ticket,
        coverage,
        inquiry(1, POWER),
        *now + Duration::from_micros(1),
    );
    let (transmission, id, _) = request_transmit(&admitted_effects);
    let sent = engine.handle(
        Input::TransmissionFinished {
            transmission,
            result: Ok(TransmissionMeta {
                sequence: write_sequence(0x5102),
            }),
        },
        *now + Duration::from_micros(1),
    );
    coverage.record(&sent);
    let replied = engine.handle(
        frame(
            1,
            sequence(0x5102),
            DecodedResponse::InquiryReply {
                route: Some(POWER),
                payload: smallvec![1],
            },
        ),
        *now + Duration::from_micros(2),
    );
    coverage.record(&replied);
    assert!(replied.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id: terminal,
            outcome: RuntimeOutcome::Reply { .. },
        } if *terminal == id
    )));
    *now += Duration::from_micros(2);

    // A conclusive buffer-full rejection is retryable even for raw VISCA;
    // only an ambiguous timeout after a successful send is unsequenced.
    let admitted_effects = seed_admit(
        engine,
        ticket,
        coverage,
        command(2, CancellationPolicy::Supported),
        *now,
    );
    let (transmission, retry_id, _) = request_transmit(&admitted_effects);
    let sent = engine.handle(
        Input::TransmissionFinished {
            transmission,
            result: Ok(TransmissionMeta {
                sequence: write_sequence(0x5103),
            }),
        },
        *now,
    );
    coverage.record(&sent);
    let rejection = engine.handle(
        frame(
            2,
            sequence(0x5103),
            DecodedResponse::Error {
                socket: None,
                code: 0x03,
            },
        ),
        *now + Duration::from_micros(1),
    );
    coverage.record(&rejection);
    assert!(rejection.iter().any(|effect| matches!(
        effect,
        Effect::RetryScheduled { id, attempt: 1, .. } if *id == retry_id
    )));
    let retry_ready = rejection
        .iter()
        .find_map(|effect| match effect {
            Effect::RetryScheduled { ready_at, .. } => Some(*ready_at),
            _ => None,
        })
        .expect("retry ready time");
    let retry = engine.advance(retry_ready);
    coverage.record(&retry);
    let (retry_transmission, retried, _) = request_transmit(&retry);
    assert_eq!(retried, retry_id);
    let sent = engine.handle(
        Input::TransmissionFinished {
            transmission: retry_transmission,
            result: Ok(TransmissionMeta {
                sequence: write_sequence(0x5103),
            }),
        },
        retry_ready,
    );
    coverage.record(&sent);
    let acknowledged = engine.handle(
        frame(
            2,
            sequence(0x5103),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        retry_ready + Duration::from_micros(1),
    );
    coverage.record(&acknowledged);
    let completed = engine.handle(
        frame(
            2,
            sequence(0x5103),
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        retry_ready + Duration::from_micros(2),
    );
    coverage.record(&completed);
    *now = retry_ready + Duration::from_micros(2);

    // Cancellation is a separate wire operation. A successful response keeps
    // this seed non-terminal for both the raw and Sony correlation paths.
    let admitted_effects = seed_admit(
        engine,
        ticket,
        coverage,
        command(1, CancellationPolicy::Supported),
        *now,
    );
    let (transmission, cancel_id, _) = request_transmit(&admitted_effects);
    let sent = engine.handle(
        Input::TransmissionFinished {
            transmission,
            result: Ok(TransmissionMeta {
                sequence: write_sequence(0x5104),
            }),
        },
        *now,
    );
    coverage.record(&sent);
    let acknowledged = engine.handle(
        frame(
            1,
            sequence(0x5104),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S2),
            },
        ),
        *now + Duration::from_micros(1),
    );
    coverage.record(&acknowledged);
    let cancellation = engine.handle(
        Input::Cancel { id: cancel_id },
        *now + Duration::from_micros(2),
    );
    coverage.record(&cancellation);
    let (cancel_transmission, _, _) = cancel_transmit(&cancellation);
    let sent = engine.handle(
        Input::TransmissionFinished {
            transmission: cancel_transmission,
            result: Ok(TransmissionMeta {
                sequence: write_sequence(0x5105),
            }),
        },
        *now + Duration::from_micros(2),
    );
    coverage.record(&sent);
    let cancellation_response = if envelope == EnvelopeKind::Raw {
        DecodedResponse::Error {
            socket: Some(ViscaSocket::S2),
            code: 0x04,
        }
    } else {
        DecodedResponse::Completion {
            socket: Some(ViscaSocket::S2),
        }
    };
    let cancelled = engine.handle(
        frame(1, sequence(0x5105), cancellation_response),
        *now + Duration::from_micros(3),
    );
    coverage.record(&cancelled);
    assert!(
        cancelled.iter().any(|effect| matches!(
            effect,
            Effect::Terminal {
                id,
                outcome: RuntimeOutcome::Cancelled,
            } if *id == cancel_id
        )),
        "{envelope:?} cancellation effects: {cancelled:?}"
    );
    *now += Duration::from_micros(3);

    // A stale frame is the minimum ignored-input corpus entry.
    let stale = engine.handle(
        frame(1, sequence(0xdead_beef), DecodedResponse::Unknown),
        *now,
    );
    coverage.record(&stale);
    assert!(stale
        .iter()
        .any(|effect| matches!(effect, Effect::Ignored(_))));
    engine.assert_invariants().unwrap();
}

#[test]
fn arbitrary_stale_and_reordered_inputs_preserve_invariants() {
    for (index, (envelope, transport)) in FUZZ_CONFIGURATIONS.into_iter().enumerate() {
        let start = Instant::now();
        let mut engine = engine(envelope, transport);
        // Seeded per configuration so the four runs are not the same stream.
        let mut random = 0x5425_49ab_cdef_0123_u64 ^ (index as u64).wrapping_mul(0x9E37_79B9);
        let mut ticket = 0_u64;
        let mut now = start;
        let mut coverage = FuzzCoverage::default();
        seed_fuzz_coverage(&mut engine, envelope, &mut ticket, &mut now, &mut coverage);
        const STEPS: u64 = 2_000;
        for step in 0..STEPS {
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            now += Duration::from_millis(1);
            let effects = fuzz_step(
                &mut engine,
                envelope,
                transport,
                random,
                step * 4 >= STEPS * 3,
                &mut ticket,
                &mut now,
            );
            coverage.record(&effects);
            engine.assert_invariants().unwrap_or_else(|violation| {
                panic!("{envelope:?}/{transport:?} step {step}: {violation}")
            });
        }
        // A configuration that never got a command acknowledged, applied,
        // replied, retried or cancelled would be auditing nothing at all.
        assert!(
            coverage.acknowledged > 0
                && coverage.applied > 0
                && coverage.replied > 0
                && coverage.retried > 0
                && coverage.cancelled > 0
                && coverage.ignored > 0,
            "{envelope:?}/{transport:?} reached too little of the engine: {coverage:?}"
        );
        // A failed stream write poisons the session (its byte-stream position is
        // unknowable). A raw datagram's unconfirmed outcome no longer does:
        // issue #671 fails only that command and quarantines its correlation, so
        // the session survives. Datagram sessions therefore stay live in default
        // mode regardless of envelope; only a stream ends poisoned here (the
        // fuzz never drives the strict opt-in or an explicit poison input).
        assert_eq!(
            engine.state() == SessionState::Poisoned,
            transport == TransportKind::Stream,
            "{envelope:?}/{transport:?} ended in {:?}",
            engine.state()
        );
    }
}

mod generated_invariant_properties {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        /// Generated ordered, stale, and reordered inputs must never leave a
        /// derived engine index inconsistent with its authoritative entries.
        ///
        /// The session shape is generated too, so Sony correlation, stream
        /// poisoning, socketless frames, receive faults and network-change
        /// frames are all in the search space (#636).
        #[test]
        fn arbitrary_ordered_and_stale_inputs_preserve_invariants_property(
            configuration in 0_usize..FUZZ_CONFIGURATIONS.len(),
            actions in prop::collection::vec(any::<u64>(), 1..256)
        ) {
            let (envelope, transport) = FUZZ_CONFIGURATIONS[configuration];
            let start = Instant::now();
            let mut engine = engine(envelope, transport);
            let mut ticket = 0_u64;
            let mut now = start;

            let steps = actions.len();
            for (step, action) in actions.into_iter().enumerate() {
                now += Duration::from_micros((action & 0x3f).saturating_add(1));
                let _ = fuzz_step(
                    &mut engine,
                    envelope,
                    transport,
                    action,
                    step * 4 >= steps * 3,
                    &mut ticket,
                    &mut now,
                );
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
            outcome: RuntimeOutcome::Failed(Error::UnsequencedCommandUnconfirmed)
        } if *seen == id
    )));
    // Issue #671: failing the request at the quarantine deadline no longer
    // poisons the raw session.
    assert_eq!(engine.state(), SessionState::Running);
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
    custom_engine.assert_invariants().unwrap();
    builtin_engine.assert_invariants().unwrap();
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
    // Backoff is jittered into `[ceiling / 2, ceiling)`, so the *floor* of the
    // first attempt's band has to overshoot the budget for this test to be
    // about the budget rather than about the draw: 30ms/2 = 15ms, and the
    // error below arrives 5ms in.
    retry.initial_backoff = Duration::from_millis(30);
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

/// A transient receive fault retries every sequenced Sony command still
/// awaiting an ACK and leaves the session running. Raw commands instead poison
/// the session because their outcomes have no sequence key. Restores 1.x
/// `SchedulerEvent::NetworkError` for the sequenced path.
#[test]
fn transient_receive_fault_retries_awaiting_ack_work_and_keeps_the_session() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let first_id = admitted(&first);
    send_ok(&mut engine, &first, Some(0x2001), start);
    let second = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(2, CancellationPolicy::Supported),
        },
        start,
    );
    let second_id = admitted(&second);
    send_ok(&mut engine, &second, Some(0x2002), start);

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
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
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
    send_ok(&mut engine, &admission, Some(0x3001), start);
    let inquiry_admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: inquiry(2, POWER),
        },
        start,
    );
    let inquiry_id = admitted(&inquiry_admission);
    send_ok(&mut engine, &inquiry_admission, Some(0x3002), start);

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
    engine.assert_invariants().unwrap();
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

/// Issue #620/#682: a named socket already held by another request falls back
/// to the target's free socket, while a camera whose sockets are all taken gets
/// no invented assignment and a one-socket target has no second socket to use.
#[test]
fn socket_assignment_falls_back_from_an_occupied_named_socket_and_never_invents_one() {
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

    // Socket one is taken by another request: the named-socket ACK falls back to
    // the free socket two (issue #620/#682), and a socketless ACK also uses it.
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

/// Issue #620/#682: an ACK naming a socket another request still owns — the
/// classic case is a lost completion frame that made the camera reuse the
/// socket — falls back to the target's free socket instead of being dropped.
/// The candidate was already uniquely identified (the sole unacknowledged raw
/// command), so this cannot mis-attribute the ACK; it only keeps the command
/// from wedging on `AwaitingAck`, which before issue #671 cascaded into a
/// session poison at that command's ACK deadline. Nothing is poisoned.
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

    // The camera names socket one for the second command (it reused the socket
    // after losing the first command's completion). The second command falls
    // back to the free socket two rather than being wedged.
    let reused = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );

    assert!(ignored_reasons(&reused).is_empty());
    assert_eq!(engine.state(), SessionState::Running);
    assert_eq!(socket_of(&engine, first_id), Some(ViscaSocket::S1));
    assert_eq!(socket_of(&engine, second_id), Some(ViscaSocket::S2));
    assert!(matches!(
        engine.entry(second_id).map(Entry::phase),
        Some(Phase::Executing { .. })
    ));

    // The second command completes on the socket it actually owns; no cascade to
    // a session poison anywhere.
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
    assert_eq!(engine.state(), SessionState::Running);
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

/// A raw ACK that arrives after the write result has one exact
/// `AwaitingAck` candidate and is attributed without admission-order FIFO.
#[test]
fn raw_ack_in_awaiting_ack_uses_the_unique_command_candidate() {
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
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::AwaitingAck { .. })
    ));

    let acknowledged = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    assert!(acknowledged.iter().any(|effect| matches!(
        effect,
        Effect::Transition {
            id: seen,
            to: Phase::Executing {
                socket: ViscaSocket::S1,
                ..
            },
            ..
        } if *seen == id
    )));
    assert_eq!(socket_of(&engine, id), Some(ViscaSocket::S1));
    engine.assert_invariants().unwrap();
}

/// Raw VISCA gates only the unacknowledged window. Sony's sequence envelope can
/// correlate multiple writes before either ACK arrives.
#[test]
fn raw_gate_serializes_pre_ack_while_sony_allows_pipeline() {
    let start = Instant::now();
    {
        let mut raw = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let first = raw.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command(1, CancellationPolicy::Supported),
            },
            start,
        );
        let first_id = admitted(&first);
        let (first_tx, _, _) = request_transmit(&first);
        let second = raw.handle(
            Input::Admit {
                ticket: AdmissionTicket(2),
                request: command(1, CancellationPolicy::Supported),
            },
            start,
        );
        let second_id = admitted(&second);
        assert!(
            request_transmit_optional(&second).is_none(),
            "raw VISCA keeps the second command queued before the first ACK"
        );
        assert!(matches!(
            raw.entry(second_id).map(Entry::phase),
            Some(Phase::Ready { .. })
        ));

        raw.handle(
            Input::TransmissionFinished {
                transmission: first_tx,
                result: Ok(TransmissionMeta { sequence: None }),
            },
            start,
        );
        let ack = raw.handle(
            frame(
                1,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start,
        );
        assert!(matches!(
            raw.entry(first_id).map(Entry::phase),
            Some(Phase::Executing { .. })
        ));
        let (second_tx, dispatched, _) = request_transmit(&ack);
        assert_eq!(dispatched, second_id);
        assert!(matches!(
            raw.entry(second_id).map(Entry::phase),
            Some(Phase::Sending { transmission, .. }) if transmission == second_tx
        ));
        raw.assert_invariants().unwrap();
    }

    {
        let mut sony = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let first = sony.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command(1, CancellationPolicy::Supported),
            },
            start,
        );
        let first_id = admitted(&first);
        let (first_tx, _, _) = request_transmit(&first);
        let second = sony.handle(
            Input::Admit {
                ticket: AdmissionTicket(2),
                request: command(1, CancellationPolicy::Supported),
            },
            start,
        );
        let second_id = admitted(&second);
        let (second_tx, dispatched, _) = request_transmit(&second);
        assert_eq!(dispatched, second_id);
        assert!(matches!(
            sony.entry(first_id).map(Entry::phase),
            Some(Phase::Sending { .. })
        ));
        assert!(matches!(
            sony.entry(second_id).map(Entry::phase),
            Some(Phase::Sending { transmission, .. }) if transmission == second_tx
        ));

        sony.handle(
            Input::TransmissionFinished {
                transmission: first_tx,
                result: Ok(TransmissionMeta {
                    sequence: Some(0x1001),
                }),
            },
            start,
        );
        sony.handle(
            Input::TransmissionFinished {
                transmission: second_tx,
                result: Ok(TransmissionMeta {
                    sequence: Some(0x1002),
                }),
            },
            start,
        );
        assert!(matches!(
            sony.entry(first_id).map(Entry::phase),
            Some(Phase::AwaitingAck { .. })
        ));
        assert!(matches!(
            sony.entry(second_id).map(Entry::phase),
            Some(Phase::AwaitingAck { .. })
        ));
        sony.assert_invariants().unwrap();
    }
}

/// Issue #636: the latch holds the first ACK an attempt raced and is never
/// overwritten. A second ACK arriving on the same still-unwritten frame would
/// otherwise steal the socket the camera actually named first.
#[test]
fn a_second_racing_ack_cannot_steal_the_latch_from_the_first() {
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

    let latched = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    assert!(
        ignored_reasons(&latched).is_empty(),
        "the first racing ACK is latched, not dropped: {latched:?}"
    );

    let duplicate = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S2),
            },
        ),
        start,
    );
    assert_eq!(
        ignored_reasons(&duplicate),
        vec![IgnoreReason::UnmatchedFrame],
        "a second ACK on a latched attempt is inert, not an overwrite"
    );
    engine.assert_invariants().unwrap();

    engine.handle(
        Input::TransmissionFinished {
            transmission,
            result: Ok(TransmissionMeta { sequence: None }),
        },
        start,
    );
    assert_eq!(
        socket_of(&engine, id),
        Some(ViscaSocket::S1),
        "the applied latch must be the socket the camera named first"
    );
    engine.assert_invariants().unwrap();
}

/// A retry belongs to exactly one attempt: a retried Sony request starts with
/// no deferred ACK and reuses its registered sequence rather than inheriting
/// stale attempt state.
#[test]
fn a_latched_ack_never_survives_into_the_next_attempt() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
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
        Input::TransmissionFinished {
            transmission,
            result: Ok(TransmissionMeta {
                sequence: Some(0x1001),
            }),
        },
        start,
    );
    engine.handle(
        frame(
            1,
            Some((0x1001, SequenceWidth::Full32)),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    assert_eq!(socket_of(&engine, id), Some(ViscaSocket::S1));
    // Force a retry; the completion deadline releases the socket.
    let retried = engine.advance(start + Duration::from_millis(40));
    assert!(retried
        .iter()
        .any(|effect| matches!(effect, Effect::RetryScheduled { .. })));
    let retry_ready = retry_scheduled(&retried)
        .expect("a completion timeout schedules a Sony retry")
        .2;
    let resent = engine.advance(retry_ready);
    let (retry_tx, retry_id, retry_wire) = request_transmit(&resent);
    assert_eq!(retry_id, id);
    assert!(matches!(
        resent.iter().find_map(|effect| match effect {
            Effect::Transmit {
                kind:
                    Transmission::Request {
                        requested_sequence, ..
                    },
                ..
            } => Some(*requested_sequence),
            _ => None,
        }),
        Some(Some(0x1001))
    ));
    assert!(Arc::ptr_eq(
        engine.entry(id).unwrap().request.wire(),
        &retry_wire
    ));
    assert!(engine
        .entry(id)
        .is_some_and(|entry| entry.deferred_ack.is_none()));
    engine.handle(
        Input::TransmissionFinished {
            transmission: retry_tx,
            result: Ok(TransmissionMeta {
                sequence: Some(0x1001),
            }),
        },
        retry_ready,
    );
    assert!(
        matches!(
            engine.entry(id).map(Entry::phase),
            Some(Phase::AwaitingAck { .. })
        ),
        "the retried attempt must await its own ACK"
    );
    assert_eq!(
        engine.entry(id).and_then(|entry| entry.current_sequence),
        Some(0x1001)
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

fn command_with_retry(target: u8, retry: RetryPolicy) -> RuntimeRequest {
    let mut command_context = context(target, CancellationPolicy::Supported);
    command_context.retry = retry;
    RuntimeRequest::Command {
        wire: wire(0x80 | target),
        context: command_context,
        applied_state: None,
    }
}

fn sony_sequence(id: RequestId) -> u32 {
    0x1000 + u32::try_from(id.get()).unwrap()
}

fn deadline_expiries(effects: &[Effect], id: RequestId) -> Vec<(DeadlineKind, bool)> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::DeadlineExpired {
                id: seen,
                deadline,
                will_retry,
            } if *seen == id => Some((*deadline, *will_retry)),
            _ => None,
        })
        .collect()
}

/// Pins the strict deadline boundary for a frame already correlated to one
/// active request. The setup runs twice so the accepted equality case and the
/// one-nanosecond-late case cannot influence each other.
fn assert_correlated_frame_deadline_boundary(
    setup: impl Fn(Instant) -> (ProtocolEngine, RequestId, Instant, Input),
    deadline_kind: DeadlineKind,
    exact_assertion: impl Fn(&ProtocolEngine, RequestId, &[Effect]),
) {
    let start = Instant::now();

    let (mut equal_engine, equal_id, deadline, equal_input) = setup(start);
    let equal = equal_engine.handle(equal_input, deadline);
    assert!(ignored_reasons(&equal).is_empty());
    assert!(deadline_expiries(&equal, equal_id).is_empty());
    exact_assertion(&equal_engine, equal_id, &equal);
    equal_engine.assert_invariants().unwrap();

    let (mut late_engine, late_id, deadline, late_input) = setup(start);
    let late = late_engine.handle(late_input, deadline + Duration::from_nanos(1));
    assert_eq!(ignored_reasons(&late), vec![IgnoreReason::UnmatchedFrame]);
    assert_eq!(deadline_expiries(&late, late_id), [(deadline_kind, false)]);
    let ignored = position_of(&late, |effect| {
        matches!(effect, Effect::Ignored(IgnoreReason::UnmatchedFrame))
    })
    .expect("late frame is ignored");
    let expired = position_of(&late, |effect| {
        matches!(
            effect,
            Effect::DeadlineExpired {
                id,
                deadline,
                ..
            } if *id == late_id && *deadline == deadline_kind
        )
    })
    .expect("normal due transition");
    assert!(
        ignored < expired,
        "the late frame precedes its due transition"
    );
    late_engine.assert_invariants().unwrap();
}

fn position_of(effects: &[Effect], predicate: impl Fn(&Effect) -> bool) -> Option<usize> {
    effects.iter().position(predicate)
}

/// Issue #571: an expired ACK deadline names itself and carries the retry
/// decision, and it is emitted before the retry it caused.
#[test]
fn ack_deadline_expiry_reports_its_own_retry_decision_before_the_retry() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let id = admitted(&admission);
    send_ok(&mut engine, &admission, Some(sony_sequence(id)), start);
    let expired = engine.advance(start + Duration::from_millis(20));
    assert_eq!(deadline_expiries(&expired, id), [(DeadlineKind::Ack, true)]);
    let expiry = position_of(&expired, |effect| {
        matches!(effect, Effect::DeadlineExpired { .. })
    })
    .expect("deadline expiry");
    let retry = position_of(&expired, |effect| {
        matches!(effect, Effect::RetryScheduled { .. })
    })
    .expect("retry");
    assert!(expiry < retry, "cause must precede consequence");
    engine.assert_invariants().unwrap();
}

/// Issue #571: the same expiry on a request that may not retry says so, rather
/// than leaving a subscriber to infer it from a missing `RetryScheduled`.
#[test]
fn ack_deadline_expiry_without_retry_policy_reports_no_retry() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command_with_retry(1, RetryPolicy::NEVER),
        },
        start,
    );
    let id = admitted(&admission);
    send_ok(&mut engine, &admission, None, start);
    let expired = engine.advance(start + Duration::from_millis(20));
    assert_eq!(
        deadline_expiries(&expired, id),
        [(DeadlineKind::Ack, false)]
    );
    // Issue #671: the ACK expiry is still reported as a non-retrying deadline,
    // but a raw command is now quarantined per-request rather than terminated
    // outright at the deadline; the terminal follows at the ambiguity deadline.
    assert!(terminal_id(&expired).is_none());
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::AwaitingLateAck { .. })
    ));
    assert_eq!(engine.state(), SessionState::Running);
    let resolved = engine.advance(start + Duration::from_millis(70));
    assert!(matches!(
        terminal_failure(&resolved, id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert_eq!(engine.state(), SessionState::Running);
    engine.assert_invariants().unwrap();
}

/// Issue #571: a completion deadline is distinguished from an ACK deadline.
#[test]
fn completion_deadline_expiry_is_reported_as_a_completion_deadline() {
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
    let expired = engine.advance(start + Duration::from_millis(40));
    assert_eq!(
        deadline_expiries(&expired, id),
        [(DeadlineKind::Completion, false)]
    );
    assert!(expired.iter().all(|effect| {
        !matches!(effect, Effect::RetryScheduled { id: seen, .. } if *seen == id)
    }));
    // Issue #671: the completion deadline is still reported, but it now holds the
    // owned socket quarantined and fails only this request at the ambiguity
    // deadline; the session is not poisoned.
    assert!(!expired
        .iter()
        .any(|effect| matches!(effect, Effect::SessionChanged { .. })));
    assert!(terminal_failure(&expired, id).is_none());
    assert_eq!(engine.state(), SessionState::Running);
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::AwaitingCancellationResolution { .. })
    ));
    let resolved = engine.advance(start + Duration::from_millis(90));
    assert!(matches!(
        terminal_failure(&resolved, id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert_eq!(engine.state(), SessionState::Running);
    engine.assert_invariants().unwrap();
}

/// Issue #571: an inquiry reply deadline is its own kind.
#[test]
fn inquiry_reply_deadline_expiry_is_reported_as_an_inquiry_deadline() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: inquiry(1, POWER),
        },
        start,
    );
    let id = admitted(&admission);
    send_ok(&mut engine, &admission, None, start);
    let expired = engine.advance(start + Duration::from_millis(30));
    assert_eq!(
        deadline_expiries(&expired, id),
        [(DeadlineKind::InquiryReply, true)]
    );
    engine.assert_invariants().unwrap();
}

/// Issue #571: `will_retry` reports the decision the engine actually took, not
/// the policy flag that motivated it. The policy still permits ACK-deadline
/// retries on the second expiry below; the attempt budget is what refuses it.
#[test]
fn deadline_expiry_reports_no_retry_once_the_attempt_budget_is_spent() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let policy = RetryPolicy {
        max_retries: 1,
        ..retrying()
    };
    assert!(policy.ack_timeout);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command_with_retry(1, policy),
        },
        start,
    );
    let id = admitted(&admission);
    let sequence = sony_sequence(id);
    send_ok(&mut engine, &admission, Some(sequence), start);
    let first = engine.advance(start + Duration::from_millis(20));
    assert_eq!(deadline_expiries(&first, id), [(DeadlineKind::Ack, true)]);
    let resent = engine.advance(start + Duration::from_millis(30));
    send_ok(
        &mut engine,
        &resent,
        Some(sequence),
        start + Duration::from_millis(30),
    );
    let second = engine.advance(start + Duration::from_millis(50));
    assert_eq!(deadline_expiries(&second, id), [(DeadlineKind::Ack, false)]);
    assert_eq!(terminal_id(&second), Some(id));
    engine.assert_invariants().unwrap();
}

// --- Issue #566: retry timing, exhaustion, and camera error codes ------------

/// Retry policy with a wide ceiling, so the ACK exponent cap is the binding
/// constraint rather than `maximum_backoff`.
fn ack_capped_retry() -> RetryPolicy {
    RetryPolicy {
        max_retries: 12,
        initial_backoff: Duration::from_millis(1),
        maximum_backoff: Duration::from_secs(10),
        total_budget: Duration::from_secs(600),
        ..retrying()
    }
}

fn retry_scheduled(effects: &[Effect]) -> Option<(RequestId, u32, Instant)> {
    effects.iter().find_map(|effect| match effect {
        Effect::RetryScheduled {
            id,
            attempt,
            ready_at,
        } => Some((*id, *attempt, *ready_at)),
        _ => None,
    })
}

fn terminal_outcome(effects: &[Effect], id: RequestId) -> Option<RuntimeOutcome> {
    effects.iter().find_map(|effect| match effect {
        Effect::Terminal { id: seen, outcome } if *seen == id => Some(outcome.clone()),
        _ => None,
    })
}

/// Terminal failure error for one request, if it failed in this batch.
fn terminal_failure(effects: &[Effect], id: RequestId) -> Option<Error> {
    match terminal_outcome(effects, id) {
        Some(RuntimeOutcome::Failed(error)) => Some(error),
        _ => None,
    }
}

/// Admits one command, confirms its write, and lets its ACK deadline lapse,
/// returning the retry that was scheduled.
fn ack_timeout_retry(
    engine: &mut ProtocolEngine,
    effects: &[Effect],
    at: Instant,
) -> (RequestId, u32, Instant) {
    let id = admitted(effects);
    send_ok(engine, effects, Some(sony_sequence(id)), at);
    let timed_out = engine.handle(Input::Wake, at + Duration::from_millis(20));
    let scheduled = retry_scheduled(&timed_out).expect("a lost ACK schedules a retry");
    assert_eq!(scheduled.0, id);
    scheduled
}

/// Issue #566: the backoff is the 1.x exponential ceiling with an equal-jitter
/// band under it, and the whole sequence is a pure function of the engine's
/// seed, the request identity and the attempt number — no clock, no entropy.
#[test]
fn retry_backoff_follows_the_pinned_jitter_sequence() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    // `retrying()` allows three retries; four draws need a wider budget.
    let mut retry = retrying();
    retry.max_retries = 6;
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command_with_retry(1, retry),
        },
        start,
    );
    let id = admitted(&admission);
    send_ok(&mut engine, &admission, None, start);

    // `retrying()` is initial 10ms, ceiling 100ms. The exponential ceilings are
    // 10, 20, 40 and 80ms; each wait is the deterministic draw inside
    // `[ceiling / 2, ceiling)`.
    let expected = [
        Duration::from_nanos(9_659_429),
        Duration::from_nanos(11_157_005),
        Duration::from_nanos(32_133_684),
        Duration::from_nanos(77_696_168),
    ];
    let ceilings = [
        Duration::from_millis(10),
        Duration::from_millis(20),
        Duration::from_millis(40),
        Duration::from_millis(80),
    ];
    let mut now = start;
    for (index, (wait, ceiling)) in expected.iter().zip(ceilings).enumerate() {
        let attempt = u32::try_from(index).unwrap() + 1;
        let busy = engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Error {
                    socket: None,
                    code: 0x03,
                },
            ),
            now,
        );
        let (retried, seen_attempt, ready_at) = retry_scheduled(&busy)
            .unwrap_or_else(|| panic!("attempt {attempt} must schedule a retry"));
        assert_eq!(retried, id);
        assert_eq!(seen_attempt, attempt);
        assert_eq!(
            ready_at,
            now + *wait,
            "attempt {attempt} must wait exactly the pinned draw"
        );
        assert!(
            *wait >= ceiling / 2 && *wait < ceiling,
            "attempt {attempt} must stay inside its equal-jitter band"
        );

        // Nothing runs before the retry is due, and the frame is reissued once
        // it is.
        assert!(
            request_transmit_optional(&engine.advance(ready_at - Duration::from_nanos(1)))
                .is_none()
        );
        let promoted = engine.advance(ready_at);
        let (transmission, sent, _) = request_transmit(&promoted);
        assert_eq!(sent, id);
        engine.handle(
            Input::TransmissionFinished {
                transmission,
                result: Ok(TransmissionMeta { sequence: None }),
            },
            ready_at,
        );
        now = ready_at;
    }
    engine.assert_invariants().unwrap();
}

/// Two requests that fail on the same instant must not retry on the same
/// instant. This is the whole reason the jitter exists.
#[test]
fn concurrent_retries_of_the_same_instant_are_separated() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let mut ready = Vec::new();
    for (ticket, target) in [(1_u64, 1_u8), (2, 2)] {
        let effects = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(ticket),
                request: command(target, CancellationPolicy::Supported),
            },
            start,
        );
        ready.push(ack_timeout_retry(&mut engine, &effects, start));
    }

    assert_eq!(ready[0].1, 1);
    assert_eq!(ready[1].1, 1);
    assert_ne!(
        ready[0].2, ready[1].2,
        "two requests retrying from one instant must not collide again"
    );
    engine.assert_invariants().unwrap();
}

/// The spread is seed-derived, not clock-derived: the same inputs under a
/// different seed produce a different — but still exact — sequence.
#[test]
fn the_jitter_sequence_moves_with_the_seed() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    engine.seed_jitter(7);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let (_, attempt, ready_at) = ack_timeout_retry(&mut engine, &admission, start);
    assert_eq!(attempt, 1);
    assert_eq!(
        ready_at,
        start + Duration::from_millis(20) + Duration::from_nanos(6_093_771)
    );
    engine.assert_invariants().unwrap();
}

/// Issue #566: 1.x capped the ACK backoff exponent at five and left every
/// other retry trigger uncapped. Only the ACK path stops doubling.
#[test]
fn the_ack_backoff_exponent_is_capped_and_other_triggers_are_not() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command_with_retry(1, ack_capped_retry()),
        },
        start,
    );
    let id = admitted(&admission);
    let sequence = sony_sequence(id);
    send_ok(&mut engine, &admission, Some(sequence), start);

    // 1ms initial, ceiling 10s: without a cap the seventh attempt would double
    // to 64ms, and the eighth to 128ms.
    let mut now = start;
    let mut waits = Vec::new();
    for _ in 0..8 {
        let timed_out = engine.handle(Input::Wake, now + Duration::from_millis(20));
        let (_, attempt, ready_at) = retry_scheduled(&timed_out).expect("ACK timeout retry");
        waits.push(ready_at - (now + Duration::from_millis(20)));
        assert_eq!(attempt, u32::try_from(waits.len()).unwrap());
        let promoted = engine.advance(ready_at);
        let (transmission, _, _) = request_transmit(&promoted);
        engine.handle(
            Input::TransmissionFinished {
                transmission,
                result: Ok(TransmissionMeta {
                    sequence: Some(sequence),
                }),
            },
            ready_at,
        );
        now = ready_at;
    }

    // Attempts 1..=6 use exponents 0..=5; from attempt 7 the exponent stays 5,
    // so the ceiling stops growing and every later wait stays inside 32ms.
    assert_eq!(waits[5], Duration::from_nanos(26_176_335));
    assert_eq!(waits[6], Duration::from_nanos(23_921_702));
    assert_eq!(waits[7], Duration::from_nanos(28_562_151));
    for (index, wait) in waits.iter().enumerate().skip(6) {
        assert!(
            *wait <= Duration::from_millis(32),
            "attempt {} must not exceed the capped ceiling, got {wait:?}",
            index + 1
        );
    }

    // The pure delay function agrees, which is what fixes the exact numbers
    // above to the ACK *cap* rather than to `maximum_backoff`.
    assert_eq!(
        waits[7],
        retry_delay(
            ack_capped_retry(),
            8,
            Backoff::AckCapped,
            Jitter::new().fraction(id, 8),
        )
    );

    // The other half of the claim, driven through the engine rather than by
    // handing `Backoff::Uncapped` to the pure function: an identical policy
    // whose *completion* deadline is what expires must have its exponent left
    // uncapped by the engine's own trigger selection.
    let mut uncapped_engine = self::engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let admission = uncapped_engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command_with_retry(1, ack_capped_retry()),
        },
        start,
    );
    let uncapped_id = admitted(&admission);
    let uncapped_sequence = sony_sequence(uncapped_id);
    send_ok(
        &mut uncapped_engine,
        &admission,
        Some(uncapped_sequence),
        start,
    );

    let mut now = start;
    let mut uncapped_waits = Vec::new();
    for _ in 0..8 {
        // Acknowledge inside the ACK deadline so the only deadline that can
        // fire below is the completion one.
        uncapped_engine.handle(
            frame(
                1,
                Some((uncapped_sequence, SequenceWidth::Full32)),
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            now,
        );
        let expired = now + Duration::from_millis(60);
        let timed_out = uncapped_engine.handle(Input::Wake, expired);
        let (retried, attempt, ready_at) =
            retry_scheduled(&timed_out).expect("completion timeout retry");
        assert_eq!(retried, uncapped_id);
        uncapped_waits.push(ready_at - expired);
        assert_eq!(attempt, u32::try_from(uncapped_waits.len()).unwrap());
        let promoted = uncapped_engine.advance(ready_at);
        let (transmission, _, _) = request_transmit(&promoted);
        uncapped_engine.handle(
            Input::TransmissionFinished {
                transmission,
                result: Ok(TransmissionMeta {
                    sequence: Some(uncapped_sequence),
                }),
            },
            ready_at,
        );
        now = ready_at;
    }

    // Attempts 7 and 8 use exponents 6 and 7, so their ceilings are 64ms and
    // 128ms and every wait sits in the upper half-open band `[ceiling/2,
    // ceiling)`. Both therefore clear the capped ceiling the ACK trigger is
    // held to, which is exactly what selecting `Backoff::AckCapped` for a
    // completion timeout would destroy.
    assert!(
        uncapped_waits[6] >= Duration::from_millis(32),
        "attempt 7 must have grown past the ACK ceiling, got {:?}",
        uncapped_waits[6]
    );
    assert!(
        uncapped_waits[7] >= Duration::from_millis(64),
        "attempt 8 must have doubled again, got {:?}",
        uncapped_waits[7]
    );
    assert_eq!(
        uncapped_waits[7],
        retry_delay(
            ack_capped_retry(),
            8,
            Backoff::Uncapped,
            Jitter::new().fraction(uncapped_id, 8),
        )
    );
    uncapped_engine.assert_invariants().unwrap();
    engine.assert_invariants().unwrap();
}

/// Issue #566: a request that keeps losing its ACK exhausts its attempt budget
/// and fails, rather than retrying forever.
#[test]
fn a_command_that_never_acks_exhausts_its_attempt_budget() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let mut retry = retrying();
    retry.max_retries = 3;
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command_with_retry(1, retry),
        },
        start,
    );
    let id = admitted(&admission);
    let sequence = sony_sequence(id);
    send_ok(&mut engine, &admission, Some(sequence), start);

    let mut now = start;
    for attempt in 1..=3 {
        let timed_out = engine.handle(Input::Wake, now + Duration::from_millis(20));
        let (_, seen, ready_at) = retry_scheduled(&timed_out).expect("ACK timeout retry");
        assert_eq!(seen, attempt);
        assert!(terminal_outcome(&timed_out, id).is_none());
        let promoted = engine.advance(ready_at);
        let (transmission, _, _) = request_transmit(&promoted);
        engine.handle(
            Input::TransmissionFinished {
                transmission,
                result: Ok(TransmissionMeta {
                    sequence: Some(sequence),
                }),
            },
            ready_at,
        );
        now = ready_at;
    }

    let exhausted = engine.handle(Input::Wake, now + Duration::from_millis(20));
    assert!(retry_scheduled(&exhausted).is_none(), "the budget is spent");
    assert!(matches!(
        terminal_failure(&exhausted, id),
        Some(Error::Timeout)
    ));
    assert!(engine.entry(id).is_none());
    engine.assert_invariants().unwrap();
}

/// Issue #566: the total-budget expiry arm — a request already waiting in
/// backoff when its wall-clock budget runs out fails on the budget, carrying
/// the error that caused the last retry rather than an incidental timeout.
#[test]
fn a_retry_waiting_in_backoff_fails_when_the_total_budget_expires() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let mut retry = retrying();
    retry.initial_backoff = Duration::from_millis(10);
    retry.maximum_backoff = Duration::from_millis(10);
    retry.total_budget = Duration::from_millis(12);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command_with_retry(1, retry),
        },
        start,
    );
    let id = admitted(&admission);
    send_ok(&mut engine, &admission, None, start);

    // A busy camera at 1ms schedules a retry inside the 12ms budget; the draw
    // is 5..10ms, so the entry is still in backoff when the budget lapses.
    let busy = engine.handle(
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
    let (_, attempt, ready_at) = retry_scheduled(&busy).expect("a busy camera retries");
    assert_eq!(attempt, 1);
    assert!(ready_at < start + Duration::from_millis(12));

    // Nothing is due before the budget end, and the budget end is what fires.
    let expired = engine.advance(start + Duration::from_millis(12));
    assert!(
        matches!(
            terminal_failure(&expired, id),
            Some(Error::CommandBufferFull)
        ),
        "the budget arm reports the error that caused the last retry"
    );
    assert!(engine.entry(id).is_none());
    engine.assert_invariants().unwrap();
}

fn immediate_retry_budget(total_budget: Duration) -> RetryPolicy {
    RetryPolicy {
        initial_backoff: Duration::ZERO,
        maximum_backoff: Duration::ZERO,
        total_budget,
        ..retrying()
    }
}

fn raw_sending_with_reply_shape(
    reply_shape: ReplyShape,
    retry: RetryPolicy,
    start: Instant,
) -> (ProtocolEngine, RequestId, TransmissionId) {
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command_with_reply_shape_and_retry(
                1,
                CancellationPolicy::Supported,
                reply_shape,
                retry,
            ),
        },
        start,
    );
    let (transmission, id, _) = request_transmit(&admission);
    (engine, id, transmission)
}

fn cancel_raw_sending(engine: &mut ProtocolEngine, id: RequestId, at: Instant) -> Instant {
    let recorded = engine.handle(Input::Cancel { id }, at);
    assert!(recorded
        .iter()
        .any(|effect| matches!(effect, Effect::CancellationRecorded { id: seen } if *seen == id)));
    let ambiguity_deadline = match engine.entry(id).map(Entry::cancellation) {
        Some(CancelState::Requested { ambiguity_deadline }) => ambiguity_deadline,
        cancellation => panic!("expected sending cancellation intent, got {cancellation:?}"),
    };
    assert_eq!(engine.next_wake(), Some(ambiguity_deadline));
    ambiguity_deadline
}

fn assert_initial_attempt_budget_expiry(
    engine: &mut ProtocolEngine,
    id: RequestId,
    budget_deadline: Instant,
) {
    assert_eq!(engine.entry(id).map(|entry| entry.attempt), Some(0));
    assert_eq!(engine.next_wake(), Some(budget_deadline));
    let expired = engine.advance(budget_deadline);
    assert!(matches!(
        terminal_failure(&expired, id),
        Some(Error::Timeout)
    ));
    assert!(
        deadline_expiries(&expired, id).is_empty(),
        "the admission budget, not the later phase deadline, must expire first"
    );
    assert!(engine.entry(id).is_none());
    engine.assert_invariants().unwrap();
}

/// The admission-relative budget must also govern an initial request that has
/// not yet been dispatched because a target-local raw owner is still active.
#[test]
fn initial_ready_request_expires_at_admission_budget_without_transmitting() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let owner = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let owner_id = admitted(&owner);
    send_ok(&mut engine, &owner, None, start);

    let retry = immediate_retry_budget(Duration::from_millis(10));
    let queued = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command_with_retry(1, retry),
        },
        start,
    );
    let queued_id = admitted(&queued);
    assert!(request_transmit_optional(&queued).is_none());
    assert!(matches!(
        phase_of(&engine, queued_id),
        Some(Phase::Ready { .. })
    ));

    let budget_deadline = start + Duration::from_millis(10);
    assert_eq!(engine.next_wake(), Some(budget_deadline));
    let expired = engine.advance(budget_deadline);
    assert!(matches!(
        terminal_failure(&expired, queued_id),
        Some(Error::Timeout)
    ));
    assert!(!expired.iter().any(|effect| matches!(
        effect,
        Effect::Transmit { request, .. } if *request == queued_id
    )));
    assert!(engine.entry(queued_id).is_none());
    assert!(engine.entry(owner_id).is_some());
    engine.assert_invariants().unwrap();
}

/// Every initial response-bearing phase is bounded from admission, even when
/// its ordinary protocol deadline would arrive later.
#[test]
fn initial_active_phases_expire_at_admission_budget() {
    let start = Instant::now();
    let budget_deadline = start + Duration::from_millis(10);

    // Initial write still in flight.
    {
        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let admission = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command_with_retry(1, immediate_retry_budget(Duration::from_millis(10))),
            },
            start,
        );
        let id = admitted(&admission);
        assert!(matches!(phase_of(&engine, id), Some(Phase::Sending { .. })));
        assert_initial_attempt_budget_expiry(&mut engine, id, budget_deadline);
    }

    // Initial ACK wait.
    {
        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let admission = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command_with_retry(1, immediate_retry_budget(Duration::from_millis(10))),
            },
            start,
        );
        let id = admitted(&admission);
        send_ok(&mut engine, &admission, Some(sony_sequence(id)), start);
        let phase_deadline = match phase_of(&engine, id) {
            Some(Phase::AwaitingAck { deadline, .. }) => deadline,
            phase => panic!("expected AwaitingAck, got {phase:?}"),
        };
        assert!(phase_deadline > budget_deadline);
        assert_initial_attempt_budget_expiry(&mut engine, id, budget_deadline);
    }

    // Initial acknowledged command, waiting for completion.
    {
        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let admission = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command_with_retry(1, immediate_retry_budget(Duration::from_millis(10))),
            },
            start,
        );
        let id = admitted(&admission);
        let sequence = sony_sequence(id);
        send_ok(&mut engine, &admission, Some(sequence), start);
        engine.handle(
            frame(
                1,
                Some((sequence, SequenceWidth::Full32)),
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start,
        );
        let phase_deadline = match phase_of(&engine, id) {
            Some(Phase::Executing { deadline, .. }) => deadline,
            phase => panic!("expected Executing, got {phase:?}"),
        };
        assert!(phase_deadline > budget_deadline);
        assert_initial_attempt_budget_expiry(&mut engine, id, budget_deadline);
    }

    // Completion-only commands use their distinct, socketless completion phase.
    {
        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let admission = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command_with_reply_shape_and_retry(
                    1,
                    CancellationPolicy::Supported,
                    ReplyShape::CompletionOnly,
                    immediate_retry_budget(Duration::from_millis(10)),
                ),
            },
            start,
        );
        let id = admitted(&admission);
        send_ok(&mut engine, &admission, Some(sony_sequence(id)), start);
        let phase_deadline = match phase_of(&engine, id) {
            Some(Phase::AwaitingCompletion { deadline, .. }) => deadline,
            phase => panic!("expected AwaitingCompletion, got {phase:?}"),
        };
        assert!(phase_deadline > budget_deadline);
        assert_initial_attempt_budget_expiry(&mut engine, id, budget_deadline);
    }

    // Initial inquiry reply wait.
    {
        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let admission = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: inquiry_with_retry(
                    1,
                    POWER,
                    immediate_retry_budget(Duration::from_millis(10)),
                ),
            },
            start,
        );
        let id = admitted(&admission);
        send_ok(&mut engine, &admission, Some(sony_sequence(id)), start);
        let phase_deadline = match phase_of(&engine, id) {
            Some(Phase::AwaitingReply { deadline, .. }) => deadline,
            phase => panic!("expected AwaitingReply, got {phase:?}"),
        };
        assert!(phase_deadline > budget_deadline);
        assert_initial_attempt_budget_expiry(&mut engine, id, budget_deadline);
    }
}

/// Raw frames that arrive before the write result can be latched, but the
/// initial total budget is still their strict response boundary while Sending.
#[test]
fn initial_raw_sending_latches_respect_total_budget_boundary() {
    let start = Instant::now();
    let budget_deadline = start + Duration::from_millis(10);

    // A deferred ACK at equality can still complete with its write and
    // completion in the one input turn before due work runs.
    {
        let (mut engine, id, transmission) = raw_sending_with_reply_shape(
            ReplyShape::AckThenCompletion,
            immediate_retry_budget(Duration::from_millis(10)),
            start,
        );
        let turn = engine.begin_input_turn(budget_deadline);
        let mut effects = engine.handle_in_turn(
            &turn,
            frame(
                1,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
        );
        assert!(ignored_reasons(&effects).is_empty());
        assert!(engine
            .entry(id)
            .is_some_and(|entry| entry.deferred_ack.is_some()));
        effects.extend(engine.handle_in_turn(
            &turn,
            Input::TransmissionFinished {
                transmission,
                result: Ok(TransmissionMeta { sequence: None }),
            },
        ));
        assert!(matches!(
            phase_of(&engine, id),
            Some(Phase::Executing {
                socket: ViscaSocket::S1,
                ..
            })
        ));
        effects.extend(engine.handle_in_turn(
            &turn,
            frame(
                1,
                None,
                DecodedResponse::Completion {
                    socket: Some(ViscaSocket::S1),
                },
            ),
        ));
        effects.extend(engine.finish_input_turn(turn));
        assert!(matches!(
            terminal_outcome(&effects, id),
            Some(RuntimeOutcome::Applied)
        ));
        engine.assert_invariants().unwrap();
    }

    // The same ACK one nanosecond after the budget is ignored before due work
    // drops the still-unwritten transmission into its raw quarantine.
    {
        let (mut engine, id, _) = raw_sending_with_reply_shape(
            ReplyShape::AckThenCompletion,
            immediate_retry_budget(Duration::from_millis(10)),
            start,
        );
        let late = engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            budget_deadline + Duration::from_nanos(1),
        );
        assert_eq!(ignored_reasons(&late), vec![IgnoreReason::UnmatchedFrame]);
        assert!(terminal_outcome(&late, id).is_none());
        assert!(engine
            .entry(id)
            .is_some_and(|entry| entry.deferred_ack.is_none()));
        assert!(matches!(
            phase_of(&engine, id),
            Some(Phase::AwaitingLateAck { .. })
        ));
        engine.assert_invariants().unwrap();
    }

    // Completion-only has the same deferred-write contract, using its distinct
    // completion latch instead of an ACK/socket path.
    {
        let (mut engine, id, transmission) = raw_sending_with_reply_shape(
            ReplyShape::CompletionOnly,
            immediate_retry_budget(Duration::from_millis(10)),
            start,
        );
        let turn = engine.begin_input_turn(budget_deadline);
        let mut effects = engine.handle_in_turn(
            &turn,
            frame(1, None, DecodedResponse::Completion { socket: None }),
        );
        assert!(ignored_reasons(&effects).is_empty());
        assert!(engine
            .entry(id)
            .is_some_and(|entry| entry.deferred_completion.is_some()));
        effects.extend(engine.handle_in_turn(
            &turn,
            Input::TransmissionFinished {
                transmission,
                result: Ok(TransmissionMeta { sequence: None }),
            },
        ));
        effects.extend(engine.finish_input_turn(turn));
        assert!(matches!(
            terminal_outcome(&effects, id),
            Some(RuntimeOutcome::Applied)
        ));
        engine.assert_invariants().unwrap();
    }

    // Strictly after the budget, the completion cannot become a deferred latch.
    {
        let (mut engine, id, _) = raw_sending_with_reply_shape(
            ReplyShape::CompletionOnly,
            immediate_retry_budget(Duration::from_millis(10)),
            start,
        );
        let late = engine.handle(
            frame(1, None, DecodedResponse::Completion { socket: None }),
            budget_deadline + Duration::from_nanos(1),
        );
        assert_eq!(ignored_reasons(&late), vec![IgnoreReason::UnmatchedFrame]);
        assert!(terminal_outcome(&late, id).is_none());
        assert!(engine
            .entry(id)
            .is_some_and(|entry| entry.deferred_completion.is_none()));
        assert!(matches!(
            phase_of(&engine, id),
            Some(Phase::AwaitingLateAck { .. })
        ));
        engine.assert_invariants().unwrap();
    }
}

/// A write result is an input too: equality remains input-first, while a raw
/// no-reply write that arrives one nanosecond after its total budget must not
/// become a local `Written` success.
#[test]
fn raw_no_reply_write_result_respects_total_budget_boundary() {
    let start = Instant::now();
    let budget_deadline = start + Duration::from_millis(10);
    let retry = immediate_retry_budget(Duration::from_millis(10));

    // At equality the result is still an input in the budget's last valid
    // turn, so plain raw execution reaches its normal local-write terminal.
    {
        let (mut engine, id, transmission) =
            raw_sending_with_reply_shape(ReplyShape::NoReply, retry, start);
        let equal = engine.handle(
            Input::TransmissionFinished {
                transmission,
                result: Ok(TransmissionMeta { sequence: None }),
            },
            budget_deadline,
        );
        assert!(matches!(
            terminal_outcome(&equal, id),
            Some(RuntimeOutcome::Written)
        ));
        assert!(!equal.iter().any(|effect| matches!(
            effect,
            Effect::RetryScheduled { id: retried, .. } if *retried == id
        )));
        assert!(engine.entry(id).is_none());
        assert_eq!(
            engine.next_wake(),
            Some(budget_deadline + Duration::from_millis(50)),
            "the successful no-reply terminal retains its ordinary raw tombstone"
        );
        engine.assert_invariants().unwrap();
    }

    // Strictly after the budget, a raw write result — even a successful one —
    // is physically ambiguous. It must clear the active transmission and hold
    // the unacknowledged slot rather than declaring `Written` or retrying.
    {
        let (mut engine, id, transmission) =
            raw_sending_with_reply_shape(ReplyShape::NoReply, retry, start);
        let late_at = budget_deadline + Duration::from_nanos(1);
        let late = engine.handle(
            Input::TransmissionFinished {
                transmission,
                result: Ok(TransmissionMeta { sequence: None }),
            },
            late_at,
        );
        assert!(terminal_outcome(&late, id).is_none());
        assert!(!late.iter().any(|effect| matches!(
            effect,
            Effect::RetryScheduled { id: retried, .. } if *retried == id
        )));
        let quarantine_deadline = late_at + Duration::from_millis(50);
        assert!(matches!(
            phase_of(&engine, id),
            Some(Phase::AwaitingLateAck { deadline }) if deadline == quarantine_deadline
        ));
        assert_eq!(engine.entry(id).map(|entry| entry.attempt), Some(0));
        assert!(engine.transmissions.is_empty());
        assert_eq!(engine.next_wake(), Some(quarantine_deadline));

        let resolved = engine.advance(quarantine_deadline);
        assert!(matches!(
            terminal_failure(&resolved, id),
            Some(Error::UnsequencedCommandUnconfirmed)
        ));
        assert_eq!(engine.state(), SessionState::Running);
        engine.assert_invariants().unwrap();
    }

    // The same boundary wins over a late local write error. The result is too
    // late to prove that raw bytes did not reach the camera, so it follows the
    // unconfirmed quarantine rather than terminalizing with that transport
    // error.
    {
        let (mut engine, id, transmission) =
            raw_sending_with_reply_shape(ReplyShape::NoReply, retry, start);
        let late_at = budget_deadline + Duration::from_nanos(1);
        let late = engine.handle(
            Input::TransmissionFinished {
                transmission,
                result: Err(Error::TransportError("late raw write result".into())),
            },
            late_at,
        );
        assert!(terminal_outcome(&late, id).is_none());
        assert!(matches!(
            phase_of(&engine, id),
            Some(Phase::AwaitingLateAck { .. })
        ));
        assert!(engine.transmissions.is_empty());
        assert!(!late.iter().any(|effect| matches!(
            effect,
            Effect::RetryScheduled { id: retried, .. } if *retried == id
        )));
        assert_eq!(
            engine.state(),
            SessionState::Running,
            "a late datagram write error remains per-request"
        );
        let resolved = engine.advance(late_at + Duration::from_millis(50));
        assert!(matches!(
            terminal_failure(&resolved, id),
            Some(Error::UnsequencedCommandUnconfirmed)
        ));
        engine.assert_invariants().unwrap();
    }

    // The strict recovery opt-in takes the same late-result branch, but its
    // existing raw-unconfirmed policy poisons immediately instead of retaining
    // a per-request ambiguity hold.
    {
        let mut engine = strict_poison_engine();
        let admission = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command_with_reply_shape_and_retry(
                    1,
                    CancellationPolicy::Supported,
                    ReplyShape::NoReply,
                    retry,
                ),
            },
            start,
        );
        let (transmission, id, _) = request_transmit(&admission);
        let late = engine.handle(
            Input::TransmissionFinished {
                transmission,
                result: Ok(TransmissionMeta { sequence: None }),
            },
            budget_deadline + Duration::from_nanos(1),
        );
        assert!(matches!(
            terminal_failure(&late, id),
            Some(Error::StreamPoisoned { .. })
        ));
        assert_eq!(engine.state(), SessionState::Poisoned);
        assert!(engine.entry(id).is_none());
        assert!(engine.transmissions.is_empty());
        assert_eq!(engine.next_wake(), None);
        engine.assert_invariants().unwrap();
    }
}

/// A completion latched before the budget cannot be consumed by a late raw
/// write result. The write-result boundary must clear that latch before it can
/// become an applied state transition.
#[test]
fn raw_deferred_completion_write_result_respects_total_budget_boundary() {
    use crate::command::semantics::WriteOnlyState;

    let start = Instant::now();
    let budget_deadline = start + Duration::from_millis(10);
    let retry = immediate_retry_budget(Duration::from_millis(10));

    // A completion latched before the deadline is valid when its write result
    // arrives at equality, including the applied-state consequence.
    {
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let mut request = command_with_reply_shape_and_retry(
            1,
            CancellationPolicy::Supported,
            ReplyShape::CompletionOnly,
            retry,
        );
        if let RuntimeRequest::Command { applied_state, .. } = &mut request {
            *applied_state =
                Some(AppliedStateProjection::set(WriteOnlyState::Spotlight, &[1]).unwrap());
        }
        let admission = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request,
            },
            start,
        );
        let (transmission, id, _) = request_transmit(&admission);
        let early = engine.handle(
            frame(1, None, DecodedResponse::Completion { socket: None }),
            budget_deadline - Duration::from_nanos(1),
        );
        assert!(terminal_outcome(&early, id).is_none());
        assert!(engine
            .entry(id)
            .is_some_and(|entry| entry.deferred_completion.is_some()));

        let equal = engine.handle(
            Input::TransmissionFinished {
                transmission,
                result: Ok(TransmissionMeta { sequence: None }),
            },
            budget_deadline,
        );
        assert!(matches!(
            terminal_outcome(&equal, id),
            Some(RuntimeOutcome::Applied)
        ));
        assert!(equal
            .iter()
            .any(|effect| matches!(effect, Effect::AppliedState { .. })));
        assert!(engine.entry(id).is_none());
        engine.assert_invariants().unwrap();
    }

    // One nanosecond later, the same already-latched completion is discarded
    // with the write correlation. No retry or applied-state effect may escape
    // the raw ambiguity quarantine.
    {
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let mut request = command_with_reply_shape_and_retry(
            1,
            CancellationPolicy::Supported,
            ReplyShape::CompletionOnly,
            retry,
        );
        if let RuntimeRequest::Command { applied_state, .. } = &mut request {
            *applied_state =
                Some(AppliedStateProjection::set(WriteOnlyState::Spotlight, &[1]).unwrap());
        }
        let admission = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request,
            },
            start,
        );
        let (transmission, id, _) = request_transmit(&admission);
        engine.handle(
            frame(1, None, DecodedResponse::Completion { socket: None }),
            budget_deadline - Duration::from_nanos(1),
        );
        assert!(engine
            .entry(id)
            .is_some_and(|entry| entry.deferred_completion.is_some()));

        let late_at = budget_deadline + Duration::from_nanos(1);
        let late = engine.handle(
            Input::TransmissionFinished {
                transmission,
                result: Ok(TransmissionMeta { sequence: None }),
            },
            late_at,
        );
        assert!(terminal_outcome(&late, id).is_none());
        assert!(!late.iter().any(|effect| matches!(
            effect,
            Effect::RetryScheduled { id: retried, .. } if *retried == id
        )));
        assert!(!late
            .iter()
            .any(|effect| matches!(effect, Effect::AppliedState { .. })));
        let quarantine_deadline = late_at + Duration::from_millis(50);
        assert!(matches!(
            phase_of(&engine, id),
            Some(Phase::AwaitingLateAck { deadline }) if deadline == quarantine_deadline
        ));
        assert!(engine.entries.get(&id).is_some_and(|entry| {
            entry.deferred_ack.is_none() && entry.deferred_completion.is_none()
        }));
        assert!(engine.transmissions.is_empty());
        assert_eq!(engine.next_wake(), Some(quarantine_deadline));

        let resolved = engine.advance(quarantine_deadline);
        assert!(matches!(
            terminal_failure(&resolved, id),
            Some(Error::UnsequencedCommandUnconfirmed)
        ));
        assert!(!resolved
            .iter()
            .any(|effect| matches!(effect, Effect::AppliedState { .. })));
        engine.assert_invariants().unwrap();
    }
}

/// Sony write metadata is valid at equality, but a result after the total
/// budget cannot register its sequence. The no-due owner seam shares this
/// engine-authoritative boundary and a late transport error retains a prior
/// retry cause rather than taking precedence over budget expiry.
#[test]
fn sony_write_results_respect_total_budget_before_correlation_mutation() {
    let start = Instant::now();
    let budget_deadline = start + Duration::from_millis(10);
    let retry = immediate_retry_budget(Duration::from_millis(10));

    // Equality remains input-first even for Sony metadata. Inspect inside the
    // turn before due work consumes the now-reached budget.
    {
        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let admission = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command_with_retry(1, retry),
            },
            start,
        );
        let (transmission, id, _) = request_transmit(&admission);
        let sequence = 0x1001_beef;
        let turn = engine.begin_input_turn(budget_deadline);
        let equal = engine.handle_in_turn(
            &turn,
            Input::TransmissionFinished {
                transmission,
                result: Ok(TransmissionMeta {
                    sequence: Some(sequence),
                }),
            },
        );
        assert!(terminal_outcome(&equal, id).is_none());
        assert!(matches!(
            phase_of(&engine, id),
            Some(Phase::AwaitingAck { .. })
        ));
        assert_eq!(
            engine.entry(id).and_then(|entry| entry.current_sequence),
            Some(sequence)
        );
        assert!(engine.sequences.contains_key(&sequence));
        assert!(engine.lower_sequences.contains_key(&(sequence as u16)));

        let expired = engine.finish_input_turn(turn);
        assert!(matches!(
            terminal_failure(&expired, id),
            Some(Error::Timeout)
        ));
        assert!(engine.sequences.is_empty());
        assert!(engine.lower_sequences.is_empty());
        engine.assert_invariants().unwrap();
    }

    // The blocking owner's no-due write-result seam cannot bypass the same
    // strict boundary: no late Sony metadata reaches the correlation indexes.
    {
        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let admission = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command_with_retry(1, retry),
            },
            start,
        );
        let (transmission, id, _) = request_transmit(&admission);
        let late = engine.finish_write_without_due(
            transmission,
            Ok(TransmissionMeta {
                sequence: Some(0xdead_beef),
            }),
            budget_deadline + Duration::from_nanos(1),
        );
        assert!(matches!(terminal_failure(&late, id), Some(Error::Timeout)));
        assert!(!late.iter().any(|effect| matches!(
            effect,
            Effect::Transition {
                id: transitioned,
                to: Phase::AwaitingAck { .. },
                ..
            } if *transitioned == id
        )));
        assert!(!late.iter().any(|effect| matches!(
            effect,
            Effect::RetryScheduled { id: retried, .. } if *retried == id
        )));
        assert!(engine.entry(id).is_none());
        assert!(engine.transmissions.is_empty());
        assert!(engine.sequences.is_empty());
        assert!(engine.lower_sequences.is_empty());
        assert_eq!(engine.next_wake(), None);
        engine.assert_invariants().unwrap();
    }

    // The late-result ordering applies equally to `Err`: it cannot replace a
    // retry cause retained by a prior Sony attempt or schedule another retry.
    {
        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let admission = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command_with_retry(1, retry),
            },
            start,
        );
        let id = admitted(&admission);
        let first_sequence = 0x1001;
        send_ok(&mut engine, &admission, Some(first_sequence), start);
        let retried = engine.handle(
            frame(
                1,
                Some((first_sequence, SequenceWidth::Full32)),
                DecodedResponse::Error {
                    socket: None,
                    code: 0x03,
                },
            ),
            start + Duration::from_nanos(1),
        );
        let (transmission, retried_id, _) = request_transmit(&retried);
        assert_eq!(retried_id, id);
        assert!(matches!(
            engine.entry(id).and_then(|entry| entry.last_error.clone()),
            Some(Error::CommandBufferFull)
        ));
        assert!(matches!(phase_of(&engine, id), Some(Phase::Sending { .. })));

        let late = engine.finish_write_without_due(
            transmission,
            Err(Error::TransportError("late Sony write result".into())),
            budget_deadline + Duration::from_nanos(1),
        );
        assert!(matches!(
            terminal_failure(&late, id),
            Some(Error::CommandBufferFull)
        ));
        assert!(!late.iter().any(|effect| matches!(
            effect,
            Effect::RetryScheduled { id: retried, .. } if *retried == id
        )));
        assert!(engine.entry(id).is_none());
        assert!(engine.transmissions.is_empty());
        assert!(engine.sequences.is_empty());
        assert!(engine.lower_sequences.is_empty());
        assert_eq!(
            engine.state(),
            SessionState::Running,
            "a late datagram write error must not poison the session"
        );
        assert_eq!(engine.next_wake(), None);
        engine.assert_invariants().unwrap();
    }
}

/// A compatible stream write failure is an authoritative session verdict even
/// if it arrives strictly after the request's total budget. The Raw case uses
/// the ordinary input ingress; the Sony case exercises the blocking owner's
/// no-due ingress, which shares that same engine path.
#[test]
fn late_stream_write_failure_poisons_before_total_budget_for_raw_and_sony() {
    let start = Instant::now();
    let budget = Duration::from_millis(10);
    let late_at = start + budget + Duration::from_nanos(1);

    for (envelope, peer_sequence, cause, use_no_due_ingress) in [
        (EnvelopeKind::Raw, None, "late raw stream write", false),
        (
            EnvelopeKind::Sony,
            Some(0x1001_beef),
            "late Sony stream write",
            true,
        ),
    ] {
        let mut engine = engine(envelope, TransportKind::Stream);
        let peer = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command_with_retry(1, immediate_retry_budget(budget)),
            },
            start,
        );
        let peer_id = admitted(&peer);
        send_ok(&mut engine, &peer, peer_sequence, start);

        let failed = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(2),
                request: command_with_retry(2, immediate_retry_budget(budget)),
            },
            start,
        );
        let (transmission, failed_id, _) = request_transmit(&failed);
        let result = Err(Error::TransportError(cause.into()));
        let poisoned = if use_no_due_ingress {
            engine.finish_write_without_due(transmission, result, late_at)
        } else {
            engine.handle(
                Input::TransmissionFinished {
                    transmission,
                    result,
                },
                late_at,
            )
        };

        let expected_reason = format!("Transport error: {cause}");
        let terminal_ids: Vec<_> = poisoned
            .iter()
            .filter_map(|effect| match effect {
                Effect::Terminal { id, .. } => Some(*id),
                _ => None,
            })
            .collect();
        assert_eq!(terminal_ids, vec![peer_id, failed_id]);
        for id in [peer_id, failed_id] {
            let Some(Error::StreamPoisoned { reason }) = terminal_failure(&poisoned, id) else {
                panic!("{envelope:?} stream failure must terminalize {id:?} with poison");
            };
            assert_eq!(reason.as_ref(), expected_reason.as_str());
        }
        assert!(poisoned.iter().any(|effect| matches!(
            effect,
            Effect::SessionChanged {
                to: SessionState::Poisoned,
                ..
            }
        )));
        assert_eq!(engine.state(), SessionState::Poisoned);
        let terminal = engine.terminal_error().expect("stream poison verdict");
        let Error::StreamPoisoned { reason } = &terminal else {
            panic!("stream failure must retain a stream-poisoned terminal error: {terminal:?}");
        };
        assert_eq!(reason.as_ref(), expected_reason.as_str());
        assert!(terminal.requires_new_session());
        assert_eq!(engine.active_len(), 0);
        assert!(engine.entries.is_empty());
        assert!(engine.transmissions.is_empty());
        assert!(engine.sequences.is_empty());
        assert!(engine.lower_sequences.is_empty());
        assert_eq!(engine.next_wake(), None);

        let rejected = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(3),
                request: command(1, CancellationPolicy::Supported),
            },
            late_at,
        );
        assert!(rejected.iter().any(|effect| matches!(
            effect,
            Effect::AdmissionRejected {
                ticket: AdmissionTicket(3),
                error: Error::StreamPoisoned { reason },
            } if reason.as_ref() == expected_reason.as_str()
        )));
        engine.assert_invariants().unwrap();
    }
}

/// A cancellation recorded during an unwritten raw command schedules and owns
/// its ambiguity boundary; the same deferred-frame equality rule applies there.
#[test]
fn cancelled_raw_sending_latches_respect_ambiguity_boundary() {
    let start = Instant::now();

    // A deferred ACK at the exact ambiguity deadline is applied when its write
    // result arrives in the same input turn. The emitted cancel starts its own
    // later ambiguity window rather than being pre-empted by the old one.
    {
        let (mut engine, id, transmission) =
            raw_sending_with_reply_shape(ReplyShape::AckThenCompletion, RetryPolicy::NEVER, start);
        let ambiguity_deadline = cancel_raw_sending(&mut engine, id, start);
        let turn = engine.begin_input_turn(ambiguity_deadline);
        let mut effects = engine.handle_in_turn(
            &turn,
            frame(
                1,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
        );
        assert!(ignored_reasons(&effects).is_empty());
        assert!(engine
            .entry(id)
            .is_some_and(|entry| entry.deferred_ack.is_some()));
        effects.extend(engine.handle_in_turn(
            &turn,
            Input::TransmissionFinished {
                transmission,
                result: Ok(TransmissionMeta { sequence: None }),
            },
        ));
        assert!(matches!(
            phase_of(&engine, id),
            Some(Phase::Executing {
                socket: ViscaSocket::S1,
                ..
            })
        ));
        effects.extend(engine.finish_input_turn(turn));
        assert!(terminal_outcome(&effects, id).is_none());
        let extended_deadline = match engine.entry(id).map(Entry::cancellation) {
            Some(CancelState::Sending {
                ambiguity_deadline, ..
            }) => ambiguity_deadline,
            cancellation => panic!("expected emitted cancellation, got {cancellation:?}"),
        };
        assert_eq!(
            extended_deadline,
            ambiguity_deadline + Duration::from_millis(50)
        );
        assert!(engine
            .next_wake()
            .is_some_and(|next_wake| next_wake > ambiguity_deadline));
        engine.assert_invariants().unwrap();
    }

    // At +1ns the ACK cannot latch, and the ambiguity deadline now has a due
    // wake even though Sending has no protocol phase deadline.
    {
        let (mut engine, id, _) =
            raw_sending_with_reply_shape(ReplyShape::AckThenCompletion, RetryPolicy::NEVER, start);
        let ambiguity_deadline = cancel_raw_sending(&mut engine, id, start);
        let late = engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            ambiguity_deadline + Duration::from_nanos(1),
        );
        assert_eq!(ignored_reasons(&late), vec![IgnoreReason::UnmatchedFrame]);
        assert!(matches!(
            terminal_failure(&late, id),
            Some(Error::UnsequencedCommandUnconfirmed)
        ));
        engine.assert_invariants().unwrap();
    }

    // Completion-only carries the same rule through its separate deferred
    // completion latch. At equality the write result can still apply it.
    {
        let (mut engine, id, transmission) =
            raw_sending_with_reply_shape(ReplyShape::CompletionOnly, RetryPolicy::NEVER, start);
        let ambiguity_deadline = cancel_raw_sending(&mut engine, id, start);
        let turn = engine.begin_input_turn(ambiguity_deadline);
        let mut effects = engine.handle_in_turn(
            &turn,
            frame(1, None, DecodedResponse::Completion { socket: None }),
        );
        assert!(ignored_reasons(&effects).is_empty());
        assert!(engine
            .entry(id)
            .is_some_and(|entry| entry.deferred_completion.is_some()));
        effects.extend(engine.handle_in_turn(
            &turn,
            Input::TransmissionFinished {
                transmission,
                result: Ok(TransmissionMeta { sequence: None }),
            },
        ));
        effects.extend(engine.finish_input_turn(turn));
        assert!(matches!(
            terminal_outcome(&effects, id),
            Some(RuntimeOutcome::Applied)
        ));
        engine.assert_invariants().unwrap();
    }

    // A late completion is ignored before the cancellation ambiguity expires.
    {
        let (mut engine, id, _) =
            raw_sending_with_reply_shape(ReplyShape::CompletionOnly, RetryPolicy::NEVER, start);
        let ambiguity_deadline = cancel_raw_sending(&mut engine, id, start);
        let late = engine.handle(
            frame(1, None, DecodedResponse::Completion { socket: None }),
            ambiguity_deadline + Duration::from_nanos(1),
        );
        assert_eq!(ignored_reasons(&late), vec![IgnoreReason::UnmatchedFrame]);
        assert!(matches!(
            terminal_failure(&late, id),
            Some(Error::UnsequencedCommandUnconfirmed)
        ));
        engine.assert_invariants().unwrap();
    }
}

/// Frames delivered at the exact total-budget boundary remain input-first, but
/// when no frame arrives an equal phase deadline yields to the budget arm.
#[test]
fn initial_admission_budget_preserves_exact_boundary_precedence() {
    let start = Instant::now();

    // An exact input turn wins over the budget work due at the same instant.
    {
        let mut equal_engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let admission = equal_engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: inquiry_with_retry(
                    1,
                    POWER,
                    immediate_retry_budget(Duration::from_millis(10)),
                ),
            },
            start,
        );
        let id = admitted(&admission);
        let sequence = sony_sequence(id);
        send_ok(&mut equal_engine, &admission, Some(sequence), start);
        let budget_deadline = start + Duration::from_millis(10);
        assert_eq!(equal_engine.next_wake(), Some(budget_deadline));

        let reply = equal_engine.handle(
            frame(
                1,
                Some((sequence, SequenceWidth::Full32)),
                DecodedResponse::InquiryReply {
                    route: Some(POWER),
                    payload: smallvec![1],
                },
            ),
            budget_deadline,
        );
        assert!(matches!(
            terminal_outcome(&reply, id),
            Some(RuntimeOutcome::Reply { .. })
        ));
        assert!(deadline_expiries(&reply, id).is_empty());
        equal_engine.assert_invariants().unwrap();

        // One nanosecond later is no longer an equal input turn: reject the
        // stale reply, then let the already-due budget terminalize the request.
        let mut late_engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let late_admission = late_engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: inquiry_with_retry(
                    1,
                    POWER,
                    immediate_retry_budget(Duration::from_millis(10)),
                ),
            },
            start,
        );
        let late_id = admitted(&late_admission);
        let late_sequence = sony_sequence(late_id);
        send_ok(
            &mut late_engine,
            &late_admission,
            Some(late_sequence),
            start,
        );
        let late = late_engine.handle(
            frame(
                1,
                Some((late_sequence, SequenceWidth::Full32)),
                DecodedResponse::InquiryReply {
                    route: Some(POWER),
                    payload: smallvec![1],
                },
            ),
            budget_deadline + Duration::from_nanos(1),
        );
        assert_eq!(ignored_reasons(&late), vec![IgnoreReason::UnmatchedFrame]);
        assert!(matches!(
            terminal_failure(&late, late_id),
            Some(Error::Timeout)
        ));
        let ignored = position_of(&late, |effect| {
            matches!(effect, Effect::Ignored(IgnoreReason::UnmatchedFrame))
        })
        .expect("late reply is ignored");
        let terminal = position_of(
            &late,
            |effect| matches!(effect, Effect::Terminal { id, .. } if *id == late_id),
        )
        .expect("budget expiry follows the input");
        assert!(ignored < terminal);
        late_engine.assert_invariants().unwrap();
    }

    // With no input, the budget takes priority over an equal ACK deadline.
    {
        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let mut retry = retrying();
        retry.total_budget = Duration::from_millis(20);
        let admission = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command_with_retry(1, retry),
            },
            start,
        );
        let id = admitted(&admission);
        send_ok(&mut engine, &admission, Some(sony_sequence(id)), start);
        let deadline = start + Duration::from_millis(20);
        assert!(matches!(
            phase_of(&engine, id),
            Some(Phase::AwaitingAck {
                deadline: phase_deadline,
                ..
            }) if phase_deadline == deadline
        ));
        let expired = engine.advance(deadline);
        assert!(matches!(
            terminal_failure(&expired, id),
            Some(Error::Timeout)
        ));
        assert!(
            deadline_expiries(&expired, id).is_empty(),
            "the budget arm wins an equal ordinary deadline"
        );
        engine.assert_invariants().unwrap();
    }
}

/// A cancellation's ambiguity deadline is independent of a first attempt's
/// total budget.
#[test]
fn initial_budget_does_not_shorten_cancellation_ambiguity() {
    let start = Instant::now();
    let budget_deadline = start + Duration::from_millis(10);
    let mut request = command_with_retry(1, immediate_retry_budget(Duration::from_millis(10)));
    let RuntimeRequest::Command { context, .. } = &mut request else {
        unreachable!("test constructs a command");
    };
    context.timeout.completion = Duration::from_secs(1);

    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request,
        },
        start,
    );
    let id = admitted(&admission);
    let sequence = sony_sequence(id);
    send_ok(&mut engine, &admission, Some(sequence), start);
    engine.handle(
        frame(
            1,
            Some((sequence, SequenceWidth::Full32)),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );

    let cancellation = engine.handle(Input::Cancel { id }, start);
    assert!(cancel_transmit_optional(&cancellation).is_some());
    let ambiguity_deadline = match engine.entry(id).map(Entry::cancellation) {
        Some(CancelState::Sending {
            ambiguity_deadline, ..
        }) => ambiguity_deadline,
        cancellation => panic!("expected pending cancellation, got {cancellation:?}"),
    };
    assert_eq!(ambiguity_deadline, start + Duration::from_millis(50));
    assert_eq!(engine.next_wake(), Some(ambiguity_deadline));

    let at_budget = engine.advance(budget_deadline);
    assert!(terminal_outcome(&at_budget, id).is_none());
    assert!(engine.entry(id).is_some());
    assert_eq!(engine.next_wake(), Some(ambiguity_deadline));

    let resolved = engine.advance(ambiguity_deadline);
    assert!(matches!(
        terminal_failure(&resolved, id),
        Some(Error::CancellationUnconfirmed)
    ));
    engine.assert_invariants().unwrap();
}

/// A raw initial attempt caught by the budget starts its own unconfirmed hold;
/// the already-reached total budget must not collapse that ambiguity window.
#[test]
fn initial_budget_does_not_shorten_raw_unconfirmed_quarantine() {
    let start = Instant::now();
    let budget_deadline = start + Duration::from_millis(10);
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command_with_retry(1, immediate_retry_budget(Duration::from_millis(10))),
        },
        start,
    );
    let id = admitted(&admission);
    send_ok(&mut engine, &admission, None, start);
    assert!(matches!(
        phase_of(&engine, id),
        Some(Phase::AwaitingAck { .. })
    ));

    let expired = engine.advance(budget_deadline);
    assert!(terminal_outcome(&expired, id).is_none());
    let quarantine_deadline = match phase_of(&engine, id) {
        Some(Phase::AwaitingLateAck { deadline }) => deadline,
        phase => panic!("expected AwaitingLateAck, got {phase:?}"),
    };
    assert_eq!(quarantine_deadline, start + Duration::from_millis(60));
    assert_eq!(engine.next_wake(), Some(quarantine_deadline));

    let still_quarantined = engine.advance(start + Duration::from_millis(59));
    assert!(terminal_outcome(&still_quarantined, id).is_none());
    assert!(matches!(
        phase_of(&engine, id),
        Some(Phase::AwaitingLateAck { .. })
    ));

    let resolved = engine.advance(quarantine_deadline);
    assert!(matches!(
        terminal_failure(&resolved, id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    engine.assert_invariants().unwrap();
}

/// Once a Sony retry has been admitted, the total budget wins over an equal
/// ACK deadline and preserves the error that caused that retry.
#[test]
fn retry_budget_expires_while_awaiting_sony_ack() {
    let start = Instant::now();
    let retry = immediate_retry_budget(Duration::from_millis(21));
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command_with_retry(1, retry),
        },
        start,
    );
    let id = admitted(&admission);
    let sequence = sony_sequence(id);
    send_ok(&mut engine, &admission, Some(sequence), start);

    let retry_at = start + Duration::from_millis(1);
    let retried = engine.handle(
        frame(
            1,
            Some((sequence, SequenceWidth::Full32)),
            DecodedResponse::Error {
                socket: None,
                code: 0x03,
            },
        ),
        retry_at,
    );
    assert_eq!(
        retry_scheduled(&retried).map(|(_, attempt, _)| attempt),
        Some(1)
    );
    send_ok(&mut engine, &retried, Some(sequence), retry_at);
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::AwaitingAck { .. })
    ));

    let budget_deadline = start + Duration::from_millis(21);
    assert_eq!(engine.next_wake(), Some(budget_deadline));
    let expired = engine.advance(budget_deadline);
    assert!(matches!(
        terminal_failure(&expired, id),
        Some(Error::CommandBufferFull)
    ));
    assert_eq!(engine.state(), SessionState::Running);
    assert!(
        retry_scheduled(&expired).is_none(),
        "budget expiry must not schedule another retry"
    );
    engine.assert_invariants().unwrap();
}

/// The same admission-relative budget remains authoritative after a Sony ACK,
/// even though the completion deadline is later.
#[test]
fn retry_budget_expires_while_executing_sony_command() {
    let start = Instant::now();
    let retry = immediate_retry_budget(Duration::from_millis(21));
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command_with_retry(1, retry),
        },
        start,
    );
    let id = admitted(&admission);
    let sequence = sony_sequence(id);
    send_ok(&mut engine, &admission, Some(sequence), start);

    let retry_at = start + Duration::from_millis(1);
    let retried = engine.handle(
        frame(
            1,
            Some((sequence, SequenceWidth::Full32)),
            DecodedResponse::Error {
                socket: None,
                code: 0x03,
            },
        ),
        retry_at,
    );
    send_ok(&mut engine, &retried, Some(sequence), retry_at);
    let acknowledged = engine.handle(
        frame(
            1,
            Some((sequence, SequenceWidth::Full32)),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        retry_at,
    );
    assert!(acknowledged.iter().any(|effect| matches!(
        effect,
        Effect::Transition {
            to: Phase::Executing { .. },
            ..
        }
    )));

    let budget_deadline = start + Duration::from_millis(21);
    assert_eq!(engine.next_wake(), Some(budget_deadline));
    let expired = engine.advance(budget_deadline);
    assert!(matches!(
        terminal_failure(&expired, id),
        Some(Error::CommandBufferFull)
    ));
    assert_eq!(engine.state(), SessionState::Running);
    engine.assert_invariants().unwrap();
}

/// Inquiry retries use the same admission-relative deadline while awaiting a
/// reply and retain the camera error that triggered the retry.
#[test]
fn retry_budget_expires_while_awaiting_inquiry_reply() {
    let start = Instant::now();
    let retry = immediate_retry_budget(Duration::from_millis(21));
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: inquiry_with_retry(1, POWER, retry),
        },
        start,
    );
    let id = admitted(&admission);
    let sequence = sony_sequence(id);
    send_ok(&mut engine, &admission, Some(sequence), start);

    let retry_at = start + Duration::from_millis(1);
    let retried = engine.handle(
        frame(
            1,
            Some((sequence, SequenceWidth::Full32)),
            DecodedResponse::Error {
                socket: None,
                code: 0x03,
            },
        ),
        retry_at,
    );
    send_ok(&mut engine, &retried, Some(sequence), retry_at);
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::AwaitingReply { .. })
    ));

    let budget_deadline = start + Duration::from_millis(21);
    assert_eq!(engine.next_wake(), Some(budget_deadline));
    let expired = engine.advance(budget_deadline);
    assert!(matches!(
        terminal_failure(&expired, id),
        Some(Error::CommandBufferFull)
    ));
    assert_eq!(engine.state(), SessionState::Running);
    engine.assert_invariants().unwrap();
}

/// A raw retry that has returned to the ready queue can be expired safely;
/// only an emitted raw command whose result is physically ambiguous poisons.
#[test]
fn raw_ready_retry_budget_expiry_reports_last_error_without_poisoning() {
    let start = Instant::now();
    let mut retry = immediate_retry_budget(Duration::from_millis(21));
    retry.initial_backoff = Duration::from_millis(1);
    retry.maximum_backoff = Duration::from_millis(1);
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    engine.policy.command_spacing = Duration::from_secs(1);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command_with_retry(1, retry),
        },
        start,
    );
    let id = admitted(&admission);
    send_ok(&mut engine, &admission, None, start);

    let busy = engine.handle(
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
    let (_, _, ready_at) = retry_scheduled(&busy).expect("busy camera schedules raw retry");
    assert!(ready_at < start + Duration::from_millis(21));
    engine.advance(ready_at);
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::Ready { .. })
    ));

    let budget_deadline = start + Duration::from_millis(21);
    assert_eq!(engine.next_wake(), Some(budget_deadline));
    let expired = engine.advance(budget_deadline);
    assert!(matches!(
        terminal_failure(&expired, id),
        Some(Error::CommandBufferFull)
    ));
    assert_eq!(engine.state(), SessionState::Running);
    engine.assert_invariants().unwrap();
}

/// Issue #671: a raw retry budget spent while an attempt is still in flight
/// cannot replay the command (a delayed ACK could otherwise be misattributed),
/// but by default it fails only that request and quarantines its
/// unacknowledged-command slot until the ambiguity deadline. The session lives.
#[test]
fn raw_active_retry_budget_expiry_quarantines_and_fails_per_request() {
    let start = Instant::now();
    let retry = immediate_retry_budget(Duration::from_millis(21));
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command_with_retry(1, retry),
        },
        start,
    );
    let id = admitted(&admission);
    send_ok(&mut engine, &admission, None, start);

    let retried = engine.handle(
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
    assert!(retry_scheduled(&retried).is_some());
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::Sending { .. })
    ));

    let budget_deadline = start + Duration::from_millis(21);
    assert_eq!(engine.next_wake(), Some(budget_deadline));
    let expired = engine.advance(budget_deadline);
    // The in-flight attempt is quarantined, not terminated or poisoned.
    assert!(!expired
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { .. })));
    assert!(!expired
        .iter()
        .any(|effect| matches!(effect, Effect::SessionChanged { .. })));
    assert_eq!(engine.state(), SessionState::Running);
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::AwaitingLateAck { .. })
    ));

    // The command fails per-request at the ambiguity deadline (21ms + 50ms), and
    // the session is still running.
    let resolved = engine.advance(start + Duration::from_millis(71));
    assert!(matches!(
        terminal_failure(&resolved, id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert_eq!(engine.state(), SessionState::Running);
    engine.assert_invariants().unwrap();
}

/// Issue #671 strict opt-in: with `strict_unconfirmed_poison`, the same active
/// budget expiry restores the pre-fix whole-session poison, surfaced as
/// [`Error::StreamPoisoned`] so a poisoned session is still rebuildable.
#[test]
fn raw_active_retry_budget_expiry_poisons_under_strict_opt_in() {
    let start = Instant::now();
    let retry = immediate_retry_budget(Duration::from_millis(21));
    let mut engine = strict_poison_engine();
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command_with_retry(1, retry),
        },
        start,
    );
    let id = admitted(&admission);
    send_ok(&mut engine, &admission, None, start);

    engine.handle(
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
    let expired = engine.advance(start + Duration::from_millis(21));
    assert!(matches!(
        terminal_failure(&expired, id),
        Some(Error::StreamPoisoned { .. })
    ));
    assert_eq!(engine.state(), SessionState::Poisoned);
    assert!(
        expired.iter().any(|effect| matches!(
            effect,
            Effect::SessionChanged {
                to: SessionState::Poisoned,
                ..
            }
        )),
        "strict mode must poison the session"
    );
    engine.assert_invariants().unwrap();
}

/// Issue #671: a transient receive fault no longer poisons a raw session. A raw
/// command awaiting its ACK is left in place — never replayed, because a raw
/// command cannot be safely re-sent — to ride to its own ACK deadline, and the
/// session keeps running.
#[test]
fn raw_receive_fault_leaves_unacked_command_and_keeps_the_session() {
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
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::AwaitingAck { .. })
    ));

    let fault = engine.handle(
        Input::ReceiveFault {
            error: Error::Io(Arc::new(std::io::Error::from(
                std::io::ErrorKind::ConnectionRefused,
            ))),
        },
        start,
    );
    assert_eq!(engine.state(), SessionState::Running);
    assert!(!fault
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { .. })));
    assert!(!fault
        .iter()
        .any(|effect| matches!(effect, Effect::RetryScheduled { .. })));
    assert!(
        matches!(
            engine.entry(id).map(Entry::phase),
            Some(Phase::AwaitingAck { .. })
        ),
        "a raw command awaiting its ACK rides to its own deadline after a fault"
    );
    engine.assert_invariants().unwrap();
}

/// Issue #671 strict opt-in: `strict_unconfirmed_poison` restores the pre-fix
/// whole-session poison when a receive fault strikes an unacknowledged raw
/// command, surfaced as [`Error::StreamPoisoned`].
#[test]
fn raw_receive_fault_poisons_under_strict_opt_in() {
    let start = Instant::now();
    let mut engine = strict_poison_engine();
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let id = admitted(&admission);
    send_ok(&mut engine, &admission, None, start);

    let fault = engine.handle(
        Input::ReceiveFault {
            error: Error::Io(Arc::new(std::io::Error::from(
                std::io::ErrorKind::ConnectionRefused,
            ))),
        },
        start,
    );
    assert_eq!(engine.state(), SessionState::Poisoned);
    assert!(matches!(
        terminal_failure(&fault, id),
        Some(Error::StreamPoisoned { .. })
    ));
    engine.assert_invariants().unwrap();
}

/// Strict raw receive-fault poisoning applies only before cancellation intent
/// exists. Once cancellation is requested, a transient fault must leave the
/// request on the cancellation-driven late-ACK path: a late ACK can still
/// assign a socket and issue the cancel, and strict mode poisons only when that
/// resolution remains unconfirmed at its ambiguity deadline.
#[test]
fn strict_raw_receive_fault_after_cancel_uses_cancellation_resolution() {
    let start = Instant::now();
    let mut engine = strict_poison_engine();
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let id = admitted(&admission);
    send_ok(&mut engine, &admission, None, start);

    let cancel_at = start + Duration::from_millis(1);
    let cancelled = engine.handle(Input::Cancel { id }, cancel_at);
    assert!(cancelled
        .iter()
        .any(|effect| matches!(effect, Effect::CancellationRecorded { id: seen } if *seen == id)));
    assert!(matches!(
        engine.entry(id).map(Entry::cancellation),
        Some(CancelState::Requested { .. })
    ));

    let fault = engine.handle(
        Input::ReceiveFault {
            error: Error::Io(Arc::new(std::io::Error::from(
                std::io::ErrorKind::ConnectionRefused,
            ))),
        },
        cancel_at,
    );
    assert_eq!(engine.state(), SessionState::Running);
    assert!(!fault
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { .. })));
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::AwaitingAck { .. })
    ));
    assert!(
        engine.transmissions.is_empty(),
        "the request must not replay"
    );

    let acknowledged = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        cancel_at + Duration::from_millis(1),
    );
    let (_, cancel_request, _) = cancel_transmit(&acknowledged);
    assert_eq!(cancel_request, id);
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::Executing {
            socket: ViscaSocket::S1,
            ..
        })
    ));

    let ambiguity_deadline = match engine.entry(id).map(Entry::cancellation) {
        Some(CancelState::Sending {
            ambiguity_deadline, ..
        }) => ambiguity_deadline,
        cancellation => panic!("expected in-flight cancellation, got {cancellation:?}"),
    };
    let expired = engine.advance(ambiguity_deadline);
    assert_eq!(engine.state(), SessionState::Poisoned);
    assert_eq!(
        expired
            .iter()
            .filter(|effect| matches!(effect, Effect::Terminal { .. }))
            .count(),
        1,
        "strict terminalization must have exactly one terminal cause"
    );
    assert!(matches!(
        terminal_failure(&expired, id),
        Some(Error::StreamPoisoned { .. })
    ));
    assert!(!expired.iter().any(|effect| matches!(
        effect,
        Effect::RetryScheduled { .. } | Effect::AppliedState { .. }
    )));
    engine.assert_invariants().unwrap();
}

/// Issue #671 core safety (ACK path): a raw command whose ACK is lost fails only
/// itself and quarantines its unacknowledged slot for the ambiguity window. A
/// late ACK arriving during the quarantine is ignored — never applied, never
/// misattributed — the slot blocks a new command on that target until it
/// releases, and the session never poisons.
#[test]
fn raw_ack_timeout_quarantines_and_ignores_a_late_ack() {
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

    // ACK deadline (20ms): the command quarantines; the session stays live and
    // emits no terminal yet.
    let expired = engine.advance(start + Duration::from_millis(20));
    assert_eq!(engine.state(), SessionState::Running);
    assert!(!expired
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { .. })));
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::AwaitingLateAck { .. })
    ));

    // The slot is reserved: a new command on the same target queues rather than
    // dispatching while the quarantine holds.
    let queued = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1, CancellationPolicy::Supported),
        },
        start + Duration::from_millis(21),
    );
    let queued_id = admitted(&queued);
    assert!(request_transmit_optional(&queued).is_none());
    assert!(matches!(
        engine.entry(queued_id).map(Entry::phase),
        Some(Phase::Ready { .. })
    ));

    // A late ACK for the quarantined command is ignored, not applied: it must not
    // re-open the request nor bind to the queued command.
    let late = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(30),
    );
    assert_eq!(ignored_reasons(&late), vec![IgnoreReason::UnmatchedFrame]);
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::AwaitingLateAck { .. })
    ));
    assert!(matches!(
        engine.entry(queued_id).map(Entry::phase),
        Some(Phase::Ready { .. })
    ));

    // At the ambiguity deadline (20ms + 50ms) the command fails and the slot
    // releases, letting the queued command finally dispatch. Never poisoned.
    let resolved = engine.advance(start + Duration::from_millis(70));
    assert!(matches!(
        terminal_failure(&resolved, id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert_eq!(engine.state(), SessionState::Running);
    assert!(matches!(
        engine.entry(queued_id).map(Entry::phase),
        Some(Phase::Sending { .. })
    ));
    engine.assert_invariants().unwrap();
}

/// Issue #671 core safety (completion path): a raw command whose completion is
/// lost holds its already-owned socket quarantined (correlation is exact there).
/// A late completion for that socket is ignored, not applied, and the socket is
/// released only when the command fails at the ambiguity deadline. No poison.
#[test]
fn raw_completion_timeout_quarantines_socket_and_ignores_a_late_completion() {
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
    assert_eq!(socket_of(&engine, id), Some(ViscaSocket::S1));

    // Completion deadline (40ms): the owned socket is held quarantined; the
    // session stays live with no terminal yet.
    let expired = engine.advance(start + Duration::from_millis(40));
    assert_eq!(engine.state(), SessionState::Running);
    assert!(!expired
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { .. })));
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::AwaitingCancellationResolution { .. })
    ));
    assert_eq!(socket_of(&engine, id), Some(ViscaSocket::S1));

    // A late completion for the quarantined socket is ignored, not applied.
    let late = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(50),
    );
    assert_eq!(ignored_reasons(&late), vec![IgnoreReason::UnmatchedFrame]);
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::AwaitingCancellationResolution { .. })
    ));
    assert_eq!(engine.state(), SessionState::Running);

    // At the ambiguity deadline (40ms + 50ms) the command fails and the socket
    // releases.
    let resolved = engine.advance(start + Duration::from_millis(90));
    assert!(matches!(
        terminal_failure(&resolved, id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert_eq!(engine.state(), SessionState::Running);
    assert_eq!(socket_of(&engine, id), None);
    engine.assert_invariants().unwrap();
}

/// Issue #566: `0x41` (`CommandNotExecutable`) is retried for a movement or
/// preset request and is terminal for a standard one. Nothing pinned this.
#[test]
fn command_not_executable_retries_only_where_the_policy_allows_it() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);

    let mut movement = retrying();
    movement.movement_not_executable = true;
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command_with_retry(1, movement),
        },
        start,
    );
    let movement_id = admitted(&admission);
    send_ok(&mut engine, &admission, None, start);
    let refused = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x41,
            },
        ),
        start,
    );
    let (retried, attempt, _) =
        retry_scheduled(&refused).expect("a movement request retries a refused command");
    assert_eq!(retried, movement_id);
    assert_eq!(attempt, 1);
    assert!(terminal_outcome(&refused, movement_id).is_none());

    let mut standard = retrying();
    standard.movement_not_executable = false;
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command_with_retry(2, standard),
        },
        start,
    );
    let standard_id = admitted(&admission);
    send_ok(&mut engine, &admission, None, start);
    let refused = engine.handle(
        frame(
            2,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x41,
            },
        ),
        start,
    );
    assert!(retry_scheduled(&refused).is_none());
    assert!(
        matches!(
            terminal_failure(&refused, standard_id),
            Some(Error::CommandNotExecutable)
        ),
        "a standard request must surface the camera's refusal"
    );
    engine.assert_invariants().unwrap();
}

/// Issue #566: `0x05` (`NoSocket`) is a capacity answer like a full buffer, so
/// it is retried wherever a full buffer is, and is terminal where it is not.
#[test]
fn no_socket_is_retried_like_a_full_command_buffer() {
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
    let no_socket = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x05,
            },
        ),
        start,
    );
    assert_eq!(
        retry_scheduled(&no_socket).map(|scheduled| scheduled.0),
        Some(id)
    );

    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command_with_retry(2, RetryPolicy::NEVER),
        },
        start,
    );
    let never_id = admitted(&admission);
    send_ok(&mut engine, &admission, None, start);
    let no_socket = engine.handle(
        frame(
            2,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x05,
            },
        ),
        start,
    );
    assert!(retry_scheduled(&no_socket).is_none());
    assert!(matches!(
        terminal_failure(&no_socket, never_id),
        Some(Error::NoSocket)
    ));
    engine.assert_invariants().unwrap();
}

/// Issue #566: a post-ACK completion timeout retries. The rewrite hard-coded
/// this off, so a camera that ACKed and then went quiet failed on the first
/// deadline with no second attempt.
#[test]
fn a_post_ack_completion_timeout_retries_the_command() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1, CancellationPolicy::Supported),
        },
        start,
    );
    let id = admitted(&admission);
    let (_, _, first_wire) = request_transmit(&admission);
    send_ok(&mut engine, &admission, Some(0x1001), start);
    engine.handle(
        frame(
            1,
            Some((0x1001, SequenceWidth::Full32)),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    assert_eq!(socket_of(&engine, id), Some(ViscaSocket::S1));

    // The completion deadline is 40ms after the ACK.
    let lapsed = engine.handle(Input::Wake, start + Duration::from_millis(41));
    let (retried, attempt, ready_at) =
        retry_scheduled(&lapsed).expect("a completion timeout must retry");
    assert_eq!(retried, id);
    assert_eq!(attempt, 1);
    assert!(terminal_outcome(&lapsed, id).is_none());
    assert_eq!(
        socket_of(&engine, id),
        None,
        "the retry releases the socket it held"
    );

    let promoted = engine.advance(ready_at);
    let (transmission, sent, retry_wire) = request_transmit(&promoted);
    assert_eq!(sent, id);
    assert!(Arc::ptr_eq(&first_wire, &retry_wire));
    assert!(promoted.iter().any(|effect| matches!(
        effect,
        Effect::Transmit {
            kind: Transmission::Request {
                requested_sequence: Some(0x1001),
                ..
            },
            ..
        }
    )));
    engine.handle(
        Input::TransmissionFinished {
            transmission,
            result: Ok(TransmissionMeta {
                sequence: Some(0x1001),
            }),
        },
        ready_at,
    );
    engine.handle(
        frame(
            1,
            Some((0x1001, SequenceWidth::Full32)),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        ready_at,
    );
    let done = engine.handle(
        frame(
            1,
            Some((0x1001, SequenceWidth::Full32)),
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        ready_at,
    );
    assert!(
        matches!(terminal_outcome(&done, id), Some(RuntimeOutcome::Applied)),
        "the second attempt completes"
    );
    engine.assert_invariants().unwrap();
}

/// Issue #673's blocking drain is limited to a predecessor whose next accepted
/// frame can actually be an ACK. The broader raw unacknowledged predicate still
/// reserves the target for completion-only commands and #671 quarantines, but
/// neither of those states should make a blocking submit wait on a useless
/// receive. A cancellation-driven late-ACK state remains eligible because its
/// late ACK is still accepted and can establish the socket needed for cancel.
#[cfg(feature = "blocking")]
#[test]
fn blocking_preack_gate_requires_an_ack_capable_predecessor() {
    let start = Instant::now();

    // A normal raw command is ACK-capable before and after its write result.
    {
        let mut runtime = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (admitted_effects, id) = admit(
            &mut runtime,
            1,
            command(1, CancellationPolicy::Supported),
            start,
        );
        assert!(matches!(
            phase_of(&runtime, id),
            Some(Phase::Sending { .. })
        ));
        assert!(runtime.raw_preack_gate_frees_socket_on_ack(camera(1)));
        send_ok(&mut runtime, &admitted_effects, None, start);
        assert!(matches!(
            phase_of(&runtime, id),
            Some(Phase::AwaitingAck { .. })
        ));
        assert!(runtime.raw_preack_gate_frees_socket_on_ack(camera(1)));
    }

    // Completion-only has no ACK phase. It still holds the broad raw
    // unacknowledged/exclusivity slot, but the blocking owner must not pump.
    {
        let mut runtime = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (admitted_effects, id) = admit(
            &mut runtime,
            2,
            command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
            start,
        );
        send_ok(&mut runtime, &admitted_effects, None, start);
        assert!(matches!(
            phase_of(&runtime, id),
            Some(Phase::AwaitingCompletion { .. })
        ));
        assert!(!runtime.raw_preack_gate_frees_socket_on_ack(camera(1)));
    }

    // The default #671 late-ACK quarantine has no accepted ACK path. It must
    // retain the raw exclusivity slot without arming the blocking drain.
    {
        let mut runtime = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (admitted_effects, id) = admit(
            &mut runtime,
            3,
            command(1, CancellationPolicy::Supported),
            start,
        );
        send_ok(&mut runtime, &admitted_effects, None, start);
        runtime.advance(start + Duration::from_millis(25));
        assert!(matches!(
            phase_of(&runtime, id),
            Some(Phase::AwaitingLateAck { .. })
        ));
        assert_eq!(
            runtime.entry(id).map(Entry::cancellation),
            Some(CancelState::None)
        );
        assert!(!runtime.raw_preack_gate_frees_socket_on_ack(camera(1)));
    }

    // Cancellation requested before the ACK deadline deliberately keeps the
    // late-ACK path alive. The blocking drain remains reachable for this state
    // so it can receive the ACK and let the engine emit the socket cancel.
    {
        let mut runtime = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (admitted_effects, id) = admit(
            &mut runtime,
            4,
            command(1, CancellationPolicy::Supported),
            start,
        );
        send_ok(&mut runtime, &admitted_effects, None, start);
        runtime.handle(Input::Cancel { id }, start);
        runtime.advance(start + Duration::from_millis(25));
        assert!(matches!(
            phase_of(&runtime, id),
            Some(Phase::AwaitingLateAck { .. })
        ));
        assert!(!matches!(
            runtime.entry(id).map(Entry::cancellation),
            Some(CancelState::None) | None
        ));
        assert!(runtime.raw_preack_gate_frees_socket_on_ack(camera(1)));
    }
}

// ---- Issue #700: raw reply-shape axis (completion-only / no-reply) ---------

/// The raw single-candidate invariant must use the same phase predicate as the
/// dispatch gate. In particular, completion-only `AwaitingCompletion` owns no
/// socket and cannot coexist with another same-target positional candidate.
///
/// Design references: issue_542_design_review.md, “Decisions from this review”,
/// item 7; architecture_2_0.md, “Correlation before ACK is envelope-specific”
/// and “Operational invariants”.
#[test]
fn raw_single_candidate_invariant_includes_completion_only_awaiting_completion() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);

    let (first_admission, first_id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
        start,
    );
    send_ok(&mut engine, &first_admission, None, start);
    assert!(matches!(
        phase_of(&engine, first_id),
        Some(Phase::AwaitingCompletion { .. })
    ));
    engine
        .assert_invariants()
        .expect("one completion-only candidate is valid");

    // A candidate on another raw target is independently attributable.
    let (second_admission, second_id) = admit(
        &mut engine,
        2,
        command_with_reply_shape(2, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
        start,
    );
    send_ok(&mut engine, &second_admission, None, start);
    assert!(matches!(
        phase_of(&engine, second_id),
        Some(Phase::AwaitingCompletion { .. })
    ));
    engine
        .assert_invariants()
        .expect("one completion-only candidate per target is valid");

    // White-box corruption: normal dispatch cannot create this state, so move
    // the second otherwise-valid completion-only request onto target 1. There
    // are now two socketless candidates for one target and the audit must
    // reject them before any raw terminal could require a temporal guess.
    let same_target =
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly);
    engine
        .entries
        .get_mut(&second_id)
        .expect("second entry remains live")
        .request = same_target;
    assert_eq!(
        engine.assert_invariants().unwrap_err().as_ref(),
        "more than one raw command is unacknowledged on a target"
    );
}

/// #700: a completion-only raw command skips AwaitingAck entirely — it earns no
/// socket and awaits its completion under the completion deadline — then
/// terminates on the completion frame. Reverting the shape to the default would
/// send it to AwaitingAck, which the phase assertion here catches.
#[test]
fn completion_only_command_skips_ack_and_terminates_on_completion() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (admitted_effects, id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
        start,
    );
    let send = send_ok(&mut engine, &admitted_effects, None, start);
    assert!(
        matches!(
            phase_of(&engine, id),
            Some(Phase::AwaitingCompletion { .. })
        ),
        "a completion-only command awaits its completion, not an ACK: {:?}",
        phase_of(&engine, id),
    );
    assert!(
        !send.iter().any(|effect| matches!(
            effect,
            Effect::Transition {
                to: Phase::AwaitingAck { .. },
                ..
            }
        )),
        "a completion-only command must never enter AwaitingAck",
    );
    // A completion-only vendor frame typically answers with no socket nibble.
    let done = engine.handle(
        frame(1, None, DecodedResponse::Completion { socket: None }),
        start,
    );
    assert!(matches!(
        terminal_outcome(&done, id),
        Some(RuntimeOutcome::Applied)
    ));
    assert!(phase_of(&engine, id).is_none(), "the entry is finished");
    engine.assert_invariants().unwrap();
}

/// #700: an ACK is never correlation evidence for a completion-only command.
/// In particular, a timeout followed by cancellation must not turn the late-ACK
/// quarantine into a socket-owning cancellation path.
#[test]
fn completion_only_stray_acks_remain_inert_across_its_lifecycle() {
    let start = Instant::now();

    // An ACK racing the local request write must not become a deferred ACK.
    {
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (admitted_effects, id) = admit(
            &mut engine,
            1,
            command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
            start,
        );
        assert!(matches!(phase_of(&engine, id), Some(Phase::Sending { .. })));
        let stray_ack = engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start,
        );
        assert_eq!(
            ignored_reasons(&stray_ack),
            vec![IgnoreReason::UnmatchedFrame]
        );
        assert!(cancel_transmit_optional(&stray_ack).is_none());
        assert_eq!(engine.socket_owner(camera(1), ViscaSocket::S1), None);
        assert!(matches!(phase_of(&engine, id), Some(Phase::Sending { .. })));
        send_ok(&mut engine, &admitted_effects, None, start);
        engine.assert_invariants().unwrap();
    }

    // Once sent, a completion-only command still ignores a stray ACK rather
    // than acquiring its socket.
    {
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (admitted_effects, id) = admit(
            &mut engine,
            1,
            command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
            start,
        );
        send_ok(&mut engine, &admitted_effects, None, start);
        let stray_ack = engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start,
        );
        assert_eq!(
            ignored_reasons(&stray_ack),
            vec![IgnoreReason::UnmatchedFrame]
        );
        assert!(cancel_transmit_optional(&stray_ack).is_none());
        assert_eq!(engine.socket_owner(camera(1), ViscaSocket::S1), None);
        assert!(matches!(
            phase_of(&engine, id),
            Some(Phase::AwaitingCompletion { .. })
        ));
        engine.assert_invariants().unwrap();
    }

    // Its completion timeout creates the usual raw unconfirmed quarantine.
    // While cancellation remains absent, an ACK must remain inert there too.
    {
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (admitted_effects, id) = admit(
            &mut engine,
            1,
            command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
            start,
        );
        send_ok(&mut engine, &admitted_effects, None, start);
        engine.advance(start + Duration::from_millis(40));
        assert!(matches!(
            phase_of(&engine, id),
            Some(Phase::AwaitingLateAck { .. })
        ));
        assert_eq!(
            engine.entry(id).map(Entry::cancellation),
            Some(CancelState::None)
        );
        let stray_ack = engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start + Duration::from_millis(41),
        );
        assert_eq!(
            ignored_reasons(&stray_ack),
            vec![IgnoreReason::UnmatchedFrame]
        );
        assert!(cancel_transmit_optional(&stray_ack).is_none());
        assert_eq!(engine.socket_owner(camera(1), ViscaSocket::S1), None);
        assert!(matches!(
            phase_of(&engine, id),
            Some(Phase::AwaitingLateAck { .. })
        ));
        engine.assert_invariants().unwrap();
    }

    // This is the owner-reachable ordering: completion timeout, then the
    // caller's cancellation while the unconfirmed hold is live, then a stray
    // ACK. The ACK must neither acquire a socket nor emit a socket cancel.
    {
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (admitted_effects, id) = admit(
            &mut engine,
            1,
            command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
            start,
        );
        send_ok(&mut engine, &admitted_effects, None, start);
        engine.advance(start + Duration::from_millis(40));
        let cancelled = engine.handle(Input::Cancel { id }, start + Duration::from_millis(41));
        assert!(cancelled.iter().any(
            |effect| matches!(effect, Effect::CancellationRecorded { id: seen } if *seen == id)
        ));
        assert!(cancel_transmit_optional(&cancelled).is_none());
        assert!(matches!(
            engine.entry(id).map(Entry::cancellation),
            Some(CancelState::Requested { .. })
        ));

        let stray_ack = engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start + Duration::from_millis(42),
        );
        assert_eq!(
            ignored_reasons(&stray_ack),
            vec![IgnoreReason::UnmatchedFrame]
        );
        assert!(cancel_transmit_optional(&stray_ack).is_none());
        assert_eq!(engine.socket_owner(camera(1), ViscaSocket::S1), None);
        assert!(matches!(
            phase_of(&engine, id),
            Some(Phase::AwaitingLateAck { .. })
        ));
        assert!(matches!(
            engine.entry(id).map(Entry::cancellation),
            Some(CancelState::Requested { .. })
        ));
        engine.assert_invariants().unwrap();
    }
}

/// A completion-only terminal may carry a socket nibble even though this shape
/// never established socket ownership. The target was exclusive while it was
/// live, so the sole completion-only candidate is still exact evidence.
#[test]
fn completion_only_accepts_socket_bearing_terminal_completion() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (admitted_effects, id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
        start,
    );
    send_ok(&mut engine, &admitted_effects, None, start);

    let completed = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    assert!(matches!(
        terminal_outcome(&completed, id),
        Some(RuntimeOutcome::Applied)
    ));
    assert!(phase_of(&engine, id).is_none());
    assert_eq!(engine.next_wake(), Some(start + Duration::from_millis(50)));
    engine.assert_invariants().unwrap();
}

/// #700: a completion-only command has no ACK-timeout path, so passing the ACK
/// time neither fails nor — even in the strict opt-in mode — poisons it. Before
/// the fix it sat in AwaitingAck, where the strict mode poisons the whole
/// session at the ACK deadline; this pins that the ACK time is now inert and the
/// completion deadline is the one that governs.
#[test]
fn completion_only_command_has_no_ack_timeout_and_does_not_poison_strict() {
    let start = Instant::now();
    let mut engine = strict_poison_engine();
    let (admitted_effects, id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
        start,
    );
    send_ok(&mut engine, &admitted_effects, None, start);
    // Past the ACK time (20ms), before the completion deadline (40ms): still
    // awaiting completion, session healthy — the ACK deadline never existed.
    let past_ack = engine.advance(start + Duration::from_millis(25));
    assert!(
        past_ack
            .iter()
            .all(|effect| !matches!(effect, Effect::SessionChanged { .. })),
        "no ACK-timeout event fires for a completion-only command",
    );
    assert_eq!(engine.state(), SessionState::Running);
    assert!(matches!(
        phase_of(&engine, id),
        Some(Phase::AwaitingCompletion { .. })
    ));
    // The completion deadline (40ms) governs; strict mode poisons there — but
    // only then, not at the ACK time.
    let past_completion = engine.advance(start + Duration::from_millis(45));
    assert!(
        past_completion.iter().any(|effect| matches!(
            effect,
            Effect::SessionChanged {
                to: SessionState::Poisoned,
                ..
            }
        )),
        "the completion deadline is what governs a completion-only command",
    );
    engine.assert_invariants().unwrap();
}

/// #700 (default mode): with no completion, a completion-only command fails only
/// itself (UnsequencedCommandUnconfirmed) and the session keeps running — the
/// #671 per-request model, never a session-wide poison by default.
#[test]
fn completion_only_command_without_completion_fails_per_request_not_the_session() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (admitted_effects, id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
        start,
    );
    send_ok(&mut engine, &admitted_effects, None, start);
    // Completion deadline (40ms) → quarantine (AwaitingLateAck); session still up.
    engine.advance(start + Duration::from_millis(45));
    assert_eq!(engine.state(), SessionState::Running, "no session poison");
    assert!(matches!(
        phase_of(&engine, id),
        Some(Phase::AwaitingLateAck { .. })
    ));
    // Ambiguity window closes → the one request fails unconfirmed; session runs.
    let closed = engine.advance(start + Duration::from_millis(200));
    assert!(matches!(
        terminal_failure(&closed, id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert_eq!(engine.state(), SessionState::Running);
    engine.assert_invariants().unwrap();
}

/// #700: a no-reply command reaches its local-write terminal on a successful
/// transport write. It does not claim protocol application, and its bounded
/// target tombstone keeps any later command response inert until expiry.
#[test]
fn no_reply_command_terminates_on_send() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (admitted_effects, id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::NoReply),
        start,
    );
    let send = send_ok(&mut engine, &admitted_effects, None, start);
    assert!(
        matches!(terminal_outcome(&send, id), Some(RuntimeOutcome::Written)),
        "a no-reply command reports only a local write on send",
    );
    assert!(
        phase_of(&engine, id).is_none(),
        "the entry is finished on send",
    );
    let stray = engine.handle(
        frame(1, None, DecodedResponse::Completion { socket: None }),
        start,
    );
    assert!(terminal_outcome(&stray, id).is_none());
    assert!(stray
        .iter()
        .any(|effect| matches!(effect, Effect::Ignored(_))));
    engine.assert_invariants().unwrap();
}

/// A no-reply command has no response lifecycle at all. Raw frames that race
/// its write cannot latch an ACK, spend its retry budget, or otherwise change
/// its local-write-only terminal outcome.
#[test]
fn no_reply_ignores_every_raced_raw_terminal_frame_before_write_result() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (admitted_effects, id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::NoReply),
        start,
    );
    let (transmission, _, _) = request_transmit(&admitted_effects);
    for response in [
        DecodedResponse::Ack {
            socket: Some(ViscaSocket::S1),
        },
        DecodedResponse::Error {
            socket: None,
            code: 0x01,
        },
        DecodedResponse::Error {
            socket: Some(ViscaSocket::S1),
            code: 0x01,
        },
        DecodedResponse::Completion { socket: None },
        DecodedResponse::Completion {
            socket: Some(ViscaSocket::S1),
        },
    ] {
        let raced = engine.handle(frame(1, None, response), start);
        assert_eq!(ignored_reasons(&raced), vec![IgnoreReason::UnmatchedFrame]);
        assert!(terminal_outcome(&raced, id).is_none());
        assert!(matches!(phase_of(&engine, id), Some(Phase::Sending { .. })));
        assert!(engine.entry(id).is_some_and(|entry| {
            entry.deferred_ack.is_none() && entry.deferred_completion.is_none()
        }));
    }

    let written = engine.handle(
        Input::TransmissionFinished {
            transmission,
            result: Ok(TransmissionMeta { sequence: None }),
        },
        start,
    );
    assert!(matches!(
        terminal_outcome(&written, id),
        Some(RuntimeOutcome::Written)
    ));
    engine.assert_invariants().unwrap();
}

/// #700: after a completion-only command completes, a duplicate completion or a
/// stray ACK is ignored — there is no entry left to bind to, so nothing
/// misbinds.
#[test]
fn late_reply_to_completed_completion_only_is_ignored() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (admitted_effects, id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
        start,
    );
    send_ok(&mut engine, &admitted_effects, None, start);
    let done = engine.handle(
        frame(1, None, DecodedResponse::Completion { socket: None }),
        start,
    );
    assert!(matches!(
        terminal_outcome(&done, id),
        Some(RuntimeOutcome::Applied)
    ));
    for response in [
        DecodedResponse::Completion { socket: None },
        DecodedResponse::Completion {
            socket: Some(ViscaSocket::S1),
        },
        DecodedResponse::Ack {
            socket: Some(ViscaSocket::S1),
        },
    ] {
        let late = engine.handle(frame(1, None, response), start);
        assert!(terminal_outcome(&late, id).is_none());
        assert!(late
            .iter()
            .any(|effect| matches!(effect, Effect::Ignored(_))));
    }
    engine.assert_invariants().unwrap();
}

/// A successful raw no-reply command owns no response identity. Its bounded
/// target tombstone must therefore hold a same-target response-bearing
/// successor until expiry, including when stale ACK/completion/error frames are
/// delivered in the intervening turns. The tombstone itself has no entry, so
/// `next_wake` must keep the owner alive long enough to release queued work.
#[test]
fn no_reply_terminal_quarantine_blocks_successor_then_restores_liveness() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (first, first_id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::NoReply),
        start,
    );
    let (transmission, _, _) = request_transmit(&first);

    // Queue B while A's write is still in flight. Completing that write inside
    // an ordered owner turn must install A's target tombstone before
    // `finish_input_turn` considers B for dispatch.
    let (second, second_id) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        start,
    );
    assert!(request_transmit_optional(&second).is_none());
    let turn = engine.begin_input_turn(start);
    let sent = engine.handle_in_turn(
        &turn,
        Input::TransmissionFinished {
            transmission,
            result: Ok(TransmissionMeta { sequence: None }),
        },
    );
    assert!(matches!(
        terminal_outcome(&sent, first_id),
        Some(RuntimeOutcome::Written)
    ));
    assert!(
        request_transmit_optional(&engine.finish_input_turn(turn)).is_none(),
        "the target tombstone is installed before input-turn dispatch"
    );
    assert!(matches!(
        phase_of(&engine, second_id),
        Some(Phase::Ready { .. })
    ));

    let (other_target, other_target_id) = admit(
        &mut engine,
        3,
        command(2, CancellationPolicy::Supported),
        start,
    );
    assert_eq!(
        request_transmit(&other_target).1,
        other_target_id,
        "the target-local tombstone does not hold a different camera"
    );

    for response in [
        DecodedResponse::Ack {
            socket: Some(ViscaSocket::S1),
        },
        DecodedResponse::Completion { socket: None },
        DecodedResponse::Error {
            socket: None,
            code: 0x05,
        },
    ] {
        let late = engine.handle(frame(1, None, response), start);
        assert!(late
            .iter()
            .any(|effect| matches!(effect, Effect::Ignored(_))));
        assert!(
            terminal_outcome(&late, second_id).is_none(),
            "a late no-reply response must not mutate the queued successor"
        );
        assert!(matches!(
            phase_of(&engine, second_id),
            Some(Phase::Ready { .. })
        ));
    }

    let deadline = start + Duration::from_millis(50);
    assert_eq!(engine.next_wake(), Some(deadline));
    let released = engine.advance(deadline);
    assert_eq!(
        request_transmit(&released).1,
        second_id,
        "expiry releases the fixed target tombstone and dispatches queued work"
    );
    engine.assert_invariants().unwrap();
}

/// An input at the tombstone deadline wins over expiry in the same owner turn.
/// The stale terminal frame is still inert, then due work releases the fixed
/// slot and dispatches the queued successor without another wake.
#[test]
fn terminal_tombstone_ignores_frame_at_exact_expiry_before_releasing_successor() {
    let start = Instant::now();
    let deadline = start + Duration::from_millis(50);
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (first, first_id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::NoReply),
        start,
    );
    let written = send_ok(&mut engine, &first, None, start);
    assert!(matches!(
        terminal_outcome(&written, first_id),
        Some(RuntimeOutcome::Written)
    ));
    let (successor, successor_id) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        start,
    );
    assert!(request_transmit_optional(&successor).is_none());

    let at_deadline = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x01,
            },
        ),
        deadline,
    );
    assert_eq!(
        ignored_reasons(&at_deadline),
        vec![IgnoreReason::UnmatchedFrame]
    );
    assert!(terminal_outcome(&at_deadline, successor_id).is_none());
    let ignored = at_deadline
        .iter()
        .position(|effect| matches!(effect, Effect::Ignored(IgnoreReason::UnmatchedFrame)))
        .expect("stale frame is ignored before due work");
    let transmit = at_deadline
        .iter()
        .position(|effect| {
            matches!(
                effect,
                Effect::Transmit {
                    request,
                    kind: Transmission::Request { .. },
                    ..
                } if *request == successor_id
            )
        })
        .expect("expiry dispatches the queued successor in the same turn");
    assert!(ignored < transmit);
    assert!(matches!(
        phase_of(&engine, successor_id),
        Some(Phase::Sending { .. })
    ));
    engine.assert_invariants().unwrap();
}

/// A matched raw inquiry reply exhausts that request's possible response. It
/// therefore releases the single-flight inquiry lane immediately and installs
/// no target tombstone (#712).
#[test]
fn raw_single_flight_inquiry_success_releases_successor_without_tombstone() {
    let start = Instant::now();
    let mut engine = single_flight_raw_engine();

    let (first, first_id) = admit(&mut engine, 1, inquiry(1, POWER), start);
    send_ok(&mut engine, &first, None, start);
    let (successor, successor_id) = admit(&mut engine, 2, inquiry(1, ZOOM), start);
    assert!(request_transmit_optional(&successor).is_none());

    let first_reply = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: Some(POWER),
                payload: smallvec![0x0a],
            },
        ),
        start,
    );
    assert!(matches!(
        terminal_outcome(&first_reply, first_id),
        Some(RuntimeOutcome::Reply { payload, .. }) if payload.as_slice() == [0x0a]
    ));
    assert_eq!(
        request_transmit(&first_reply).1,
        successor_id,
        "a matched reply releases the same-target successor in the same turn"
    );
    assert_eq!(engine.raw_target_tombstones[1], None);
    let successor_sent = send_ok(&mut engine, &first_reply, None, start);
    assert!(request_transmit_optional(&successor_sent).is_none());
    let successor_reply = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: Some(ZOOM),
                payload: smallvec![0x0b],
            },
        ),
        start,
    );
    assert!(matches!(
        terminal_outcome(&successor_reply, successor_id),
        Some(RuntimeOutcome::Reply { payload, .. }) if payload.as_slice() == [0x0b]
    ));
    engine.assert_invariants().unwrap();
}

/// A successful inquiry does not start the urgent command-pacing clock. The
/// safety command may therefore write immediately, while its write does start
/// that clock for the next urgent command (#712).
#[test]
fn successful_inquiry_does_not_pace_urgent_but_urgent_commands_pace_each_other() {
    let start = Instant::now();
    let spacing = Duration::from_millis(100);
    let mut engine = single_flight_raw_engine();
    engine.policy.command_spacing = spacing;

    let (inquiry_send, inquiry_id) = admit(&mut engine, 1, inquiry(1, POWER), start);
    send_ok(&mut engine, &inquiry_send, None, start);
    let reply = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: Some(POWER),
                payload: smallvec![0x0a],
            },
        ),
        start,
    );
    assert!(terminal_outcome(&reply, inquiry_id).is_some());

    let (first_urgent, first_urgent_id) = admit(
        &mut engine,
        2,
        urgent_command(1, CancellationPolicy::Supported),
        start,
    );
    assert_eq!(request_transmit(&first_urgent).1, first_urgent_id);
    send_ok(&mut engine, &first_urgent, None, start);

    let (second_urgent, second_urgent_id) = admit(
        &mut engine,
        3,
        urgent_command(2, CancellationPolicy::Supported),
        start,
    );
    assert!(request_transmit_optional(&second_urgent).is_none());
    assert!(
        request_transmit_optional(&engine.advance(start + spacing - Duration::from_nanos(1)))
            .is_none()
    );
    assert_eq!(
        request_transmit(&engine.advance(start + spacing)).1,
        second_urgent_id
    );
    engine.assert_invariants().unwrap();
}

/// An inquiry's target-only reply hold must not steal complete evidence that
/// already belongs to a concurrently live raw command.  In particular, an ACK
/// still has its unique pre-ACK candidate and a socketless completion still has
/// its unique socket owner; neither can be a delayed inquiry reply.
#[test]
fn raw_inquiry_hold_preserves_live_preack_ack_and_sole_socketless_completion() {
    let start = Instant::now();
    let mut engine = single_flight_raw_engine();

    let (inquiry_send, inquiry_id) = admit(
        &mut engine,
        1,
        inquiry_with_retry(1, POWER, RetryPolicy::NEVER),
        start,
    );
    send_ok(&mut engine, &inquiry_send, None, start);
    let timeout_at = start + Duration::from_millis(30);
    let inquiry_timeout = engine.advance(timeout_at);
    assert!(matches!(
        terminal_failure(&inquiry_timeout, inquiry_id),
        Some(Error::Timeout)
    ));

    let (command_send, command_id) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        timeout_at,
    );
    send_ok(&mut engine, &command_send, None, timeout_at);
    assert!(matches!(
        phase_of(&engine, command_id),
        Some(Phase::AwaitingAck { .. })
    ));
    assert_eq!(
        engine.raw_target_tombstones[1],
        Some(RawTerminalTombstone::inquiry(
            timeout_at + Duration::from_millis(50)
        ))
    );

    // These are precisely the inquiry-shaped stale evidence the narrow hold
    // retains. They must leave the pre-ACK command live.
    for stale in [
        DecodedResponse::InquiryReply {
            route: Some(POWER),
            payload: smallvec![0x0a],
        },
        DecodedResponse::Error {
            socket: None,
            code: 0x01,
        },
        DecodedResponse::Error {
            socket: Some(ViscaSocket::S2),
            code: 0x01,
        },
    ] {
        let effects = engine.handle(frame(1, None, stale), timeout_at + Duration::from_nanos(1));
        assert_eq!(
            ignored_reasons(&effects),
            vec![IgnoreReason::UnmatchedFrame]
        );
        assert!(engine.entry(command_id).is_some());
    }

    // This ACK is attributable to the command that was already live before
    // the inquiry hold; it must not be broad-filtered as inquiry evidence.
    let ack = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        timeout_at + Duration::from_nanos(2),
    );
    assert!(ack.iter().any(|effect| matches!(
        effect,
        Effect::Transition {
            id,
            to: Phase::Executing {
                socket: ViscaSocket::S1,
                ..
            },
            ..
        } if *id == command_id
    )));

    // A named terminal without an owner remains inert under the hold; it may
    // not fall back to another socket or a successor.
    let unowned = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S2),
            },
        ),
        timeout_at + Duration::from_nanos(3),
    );
    assert_eq!(
        ignored_reasons(&unowned),
        vec![IgnoreReason::UnmatchedFrame]
    );
    assert!(engine.entry(command_id).is_some());

    // The socketless terminal is uniquely attributable to S1 and reaches the
    // live command even though the inquiry hold remains active.
    let completed = engine.handle(
        frame(1, None, DecodedResponse::Completion { socket: None }),
        timeout_at + Duration::from_nanos(4),
    );
    assert!(matches!(
        terminal_outcome(&completed, command_id),
        Some(RuntimeOutcome::Applied)
    ));
    engine.assert_invariants().unwrap();
}

/// A named completion/error has an exact raw socket key. An inquiry hold may
/// suppress stale data and socketless errors, but must never hide this key from
/// a still-live command on the same target.
#[test]
fn raw_inquiry_hold_preserves_live_named_socket_terminals() {
    let start = Instant::now();
    let setup = || {
        let mut engine = single_flight_raw_engine();
        let (inquiry_send, inquiry_id) = admit(
            &mut engine,
            1,
            inquiry_with_retry(1, POWER, RetryPolicy::NEVER),
            start,
        );
        send_ok(&mut engine, &inquiry_send, None, start);
        let timeout_at = start + Duration::from_millis(30);
        let timed_out = engine.advance(timeout_at);
        assert!(matches!(
            terminal_failure(&timed_out, inquiry_id),
            Some(Error::Timeout)
        ));
        let (command_send, command_id) = admit(
            &mut engine,
            2,
            command(1, CancellationPolicy::Supported),
            timeout_at,
        );
        send_ok(&mut engine, &command_send, None, timeout_at);
        engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            timeout_at,
        );
        (engine, command_id, timeout_at)
    };

    let (mut completion_engine, completion_id, timeout_at) = setup();
    let completion = completion_engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        timeout_at + Duration::from_nanos(1),
    );
    assert!(matches!(
        terminal_outcome(&completion, completion_id),
        Some(RuntimeOutcome::Applied)
    ));
    completion_engine.assert_invariants().unwrap();

    let (mut error_engine, error_id, timeout_at) = setup();
    let error = error_engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: Some(ViscaSocket::S1),
                code: 0x01,
            },
        ),
        timeout_at + Duration::from_nanos(1),
    );
    assert_eq!(terminal_id(&error), Some(error_id));
    error_engine.assert_invariants().unwrap();
}

/// A broad uncorrelatable-command hold remains intentionally conservative: no
/// raw response shape may escape it before the ambiguity deadline.
#[test]
fn raw_uncorrelatable_terminal_hold_still_filters_all_response_shapes() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (send, id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::NoReply),
        start,
    );
    let written = send_ok(&mut engine, &send, None, start);
    assert!(matches!(
        terminal_outcome(&written, id),
        Some(RuntimeOutcome::Written)
    ));
    for response in [
        DecodedResponse::Ack {
            socket: Some(ViscaSocket::S1),
        },
        DecodedResponse::Ack { socket: None },
        DecodedResponse::Completion {
            socket: Some(ViscaSocket::S1),
        },
        DecodedResponse::Completion { socket: None },
        DecodedResponse::InquiryReply {
            route: Some(POWER),
            payload: smallvec![0x0a],
        },
        DecodedResponse::Error {
            socket: Some(ViscaSocket::S1),
            code: 0x01,
        },
        DecodedResponse::Error {
            socket: None,
            code: 0x01,
        },
    ] {
        let effects = engine.handle(frame(1, None, response), start + Duration::from_nanos(1));
        assert_eq!(
            ignored_reasons(&effects),
            vec![IgnoreReason::UnmatchedFrame]
        );
    }
    engine.assert_invariants().unwrap();
}

/// The release projection carries every scope, rather than collapsing an S1,
/// S2, pre-ACK, and inquiry release to one target bool.  Its prefix decision
/// then preserves only evidence that remains exact for another live owner.
#[cfg(any(feature = "async", feature = "blocking"))]
#[test]
fn raw_typed_release_scopes_preserve_only_live_other_socket_evidence() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (send, command_id) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        start,
    );
    send_ok(&mut engine, &send, None, start);
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
    assert!(matches!(
        phase_of(&engine, command_id),
        Some(Phase::Executing {
            socket: ViscaSocket::S1,
            ..
        })
    ));

    let mut releases = RawCorrelationReleaseSet::default();
    let scope = releases.for_target_mut(camera(1));
    scope.release_exact_socket(ViscaSocket::S2);
    scope.release_pre_ack_unkeyed();
    scope.release_inquiry_unkeyed();
    assert!(scope.exact_socket(ViscaSocket::S2));
    assert!(scope.pre_ack_unkeyed());
    assert!(scope.inquiry_unkeyed());
    assert!(!scope.terminal_all());

    assert_eq!(
        engine.raw_prefix_disposition(
            releases,
            RawPrefixEvidence::Incomplete {
                target: camera(1),
                kind: RawIncompletePrefix::NamedCompletionOrError(ViscaSocket::S1),
            },
        ),
        RawPrefixDisposition::ReleasePreserving,
        "S1 remains exact for its live owner while S2/unkeyed scopes release"
    );
    assert_eq!(
        engine.raw_prefix_disposition(
            releases,
            RawPrefixEvidence::Incomplete {
                target: camera(1),
                kind: RawIncompletePrefix::NamedCompletionOrError(ViscaSocket::S2),
            },
        ),
        RawPrefixDisposition::Discard,
        "the releasing exact socket cannot leak into a successor"
    );
    assert_eq!(
        engine.raw_prefix_disposition(
            releases,
            RawPrefixEvidence::Incomplete {
                target: camera(1),
                kind: RawIncompletePrefix::Ack,
            },
        ),
        RawPrefixDisposition::Defer,
        "an ACK nibble is assignment preference, never ownership"
    );
    assert_eq!(
        engine.raw_prefix_disposition(
            releases,
            RawPrefixEvidence::Incomplete {
                target: camera(1),
                kind: RawIncompletePrefix::SourceOnly,
            },
        ),
        RawPrefixDisposition::Defer
    );
    assert_eq!(
        engine.raw_prefix_disposition(
            releases,
            RawPrefixEvidence::Incomplete {
                target: camera(1),
                kind: RawIncompletePrefix::SocketlessCompletion,
            },
        ),
        RawPrefixDisposition::Defer
    );
    assert_eq!(
        engine.raw_prefix_disposition(
            releases,
            RawPrefixEvidence::Incomplete {
                target: camera(1),
                kind: RawIncompletePrefix::SocketlessError,
            },
        ),
        RawPrefixDisposition::Defer
    );
    assert_eq!(
        engine.raw_prefix_disposition(
            releases,
            RawPrefixEvidence::Incomplete {
                target: camera(2),
                kind: RawIncompletePrefix::SourceOnly,
            },
        ),
        RawPrefixDisposition::ReleasePreserving,
        "a different target's retained prefix remains live"
    );
    assert_eq!(
        engine.raw_prefix_disposition(releases, RawPrefixEvidence::Complete),
        RawPrefixDisposition::Defer,
        "complete input must reach the engine before the due pass"
    );

    // A simultaneous broad terminal scope overrides every narrower exact
    // preservation rule; it deliberately retains the historical fail-closed
    // no-successor behavior.
    releases.for_target_mut(camera(1)).release_terminal_all();
    assert_eq!(
        engine.raw_prefix_disposition(
            releases,
            RawPrefixEvidence::Incomplete {
                target: camera(1),
                kind: RawIncompletePrefix::NamedCompletionOrError(ViscaSocket::S1),
            },
        ),
        RawPrefixDisposition::Discard
    );
    assert_eq!(
        engine.raw_prefix_disposition(
            releases,
            RawPrefixEvidence::Incomplete {
                target: camera(1),
                kind: RawIncompletePrefix::Noncorrelating,
            },
        ),
        RawPrefixDisposition::ReleasePreserving
    );
    engine.assert_invariants().unwrap();
}

/// Issue #713: ambiguous retained bytes are governed by elapsed time, not by
/// how many times an owner happens to poll an already-ready wake.
#[cfg(any(feature = "async", feature = "blocking"))]
#[test]
fn raw_release_gate_uses_one_time_budget_then_discards_the_orphan() {
    let start = Instant::now();
    let grace = Duration::from_millis(37);
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
    engine.policy.raw_release_grace = grace;
    engine.raw_target_tombstones[1] = Some(RawTerminalTombstone::inquiry(start));
    let evidence = RawPrefixEvidence::Incomplete {
        target: camera(1),
        kind: RawIncompletePrefix::SourceOnly,
    };

    assert_eq!(
        engine.resolve_raw_release_gate(start, Some(evidence)),
        RawReleaseGateAction::AwaitInputUntil(start + grace)
    );
    for _ in 0..128 {
        assert_eq!(
            engine.resolve_raw_release_gate(start, Some(evidence)),
            RawReleaseGateAction::AwaitInputUntil(start + grace),
            "re-polling without advancing time cannot consume the grace budget"
        );
    }
    assert_eq!(
        engine.resolve_raw_release_gate(start + grace - Duration::from_nanos(1), Some(evidence)),
        RawReleaseGateAction::AwaitInputUntil(start + grace)
    );
    assert_eq!(
        engine.resolve_raw_release_gate(start + grace, Some(evidence)),
        RawReleaseGateAction::DiscardFirst
    );
    assert_eq!(engine.state(), SessionState::Running);
}

#[cfg(any(feature = "async", feature = "blocking"))]
#[test]
fn raw_release_gate_advances_immediately_when_no_input_is_retained() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
    engine.raw_target_tombstones[1] = Some(RawTerminalTombstone::inquiry(start));

    assert_eq!(
        engine.resolve_raw_release_gate(start, None),
        RawReleaseGateAction::Advance
    );
}

/// A normal response deadline can create a future tombstone but does not itself
/// release correlation before input. Otherwise a split valid frame exactly at
/// its ordinary deadline would be discarded before the engine sees it.
#[cfg(any(feature = "async", feature = "blocking"))]
#[test]
fn raw_release_projection_excludes_ordinary_response_deadlines() {
    let start = Instant::now();
    let mut command_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (command_send, command_id) = admit(
        &mut command_engine,
        1,
        command(1, CancellationPolicy::Supported),
        start,
    );
    send_ok(&mut command_engine, &command_send, None, start);
    command_engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    let command_deadline = match phase_of(&command_engine, command_id) {
        Some(Phase::Executing { deadline, .. }) => deadline,
        phase => panic!("expected executing command, got {phase:?}"),
    };
    assert!(command_engine
        .raw_correlation_releases_due(command_deadline)
        .is_empty());

    let mut inquiry_engine = single_flight_raw_engine();
    let (inquiry_send, inquiry_id) = admit(&mut inquiry_engine, 1, inquiry(1, POWER), start);
    send_ok(&mut inquiry_engine, &inquiry_send, None, start);
    let inquiry_deadline = match phase_of(&inquiry_engine, inquiry_id) {
        Some(Phase::AwaitingReply { deadline, .. }) => deadline,
        phase => panic!("expected awaiting inquiry reply, got {phase:?}"),
    };
    assert!(inquiry_engine
        .raw_correlation_releases_due(inquiry_deadline)
        .is_empty());
}

/// A raw inquiry timeout releases the FIFO owner just as a reply does. A late
/// reply from the timed-out request is therefore inert until the target hold
/// expires, after which B's own reply remains attributable to B.
#[test]
fn raw_single_flight_inquiry_timeout_quarantines_late_reply_until_successor_release() {
    let start = Instant::now();
    let timeout_at = start + Duration::from_millis(30);
    let release_at = timeout_at + Duration::from_millis(50);
    let mut engine = single_flight_raw_engine();

    let (first, first_id) = admit(
        &mut engine,
        1,
        inquiry_with_retry(1, POWER, RetryPolicy::NEVER),
        start,
    );
    send_ok(&mut engine, &first, None, start);
    let (successor, successor_id) = admit(&mut engine, 2, inquiry(1, ZOOM), start);
    assert!(request_transmit_optional(&successor).is_none());

    let timed_out = engine.advance(timeout_at);
    assert!(matches!(
        terminal_failure(&timed_out, first_id),
        Some(Error::Timeout)
    ));
    assert!(request_transmit_optional(&timed_out).is_none());
    assert!(matches!(
        phase_of(&engine, successor_id),
        Some(Phase::Ready { .. })
    ));

    let late = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: Some(POWER),
                payload: smallvec![0x0a],
            },
        ),
        timeout_at + Duration::from_nanos(1),
    );
    assert_eq!(ignored_reasons(&late), vec![IgnoreReason::UnmatchedFrame]);
    assert!(terminal_outcome(&late, successor_id).is_none());

    let released = engine.advance(release_at);
    assert_eq!(request_transmit(&released).1, successor_id);
    let successor_sent = send_ok(&mut engine, &released, None, release_at);
    assert!(request_transmit_optional(&successor_sent).is_none());
    let successor_reply = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: Some(ZOOM),
                payload: smallvec![0x0b],
            },
        ),
        release_at,
    );
    assert!(matches!(
        terminal_outcome(&successor_reply, successor_id),
        Some(RuntimeOutcome::Reply { payload, .. }) if payload.as_slice() == [0x0b]
    ));
    engine.assert_invariants().unwrap();
}

/// The uncertainty left by a timed-out raw inquiry is narrow: another
/// same-target inquiry must wait, but an ACK-bearing safety command can write
/// immediately and own its ACK while that inquiry hold remains live (#712).
#[test]
fn raw_inquiry_timeout_hold_blocks_only_inquiries_and_never_urgent_commands() {
    let start = Instant::now();
    let timeout_at = start + Duration::from_millis(30);
    let release_at = timeout_at + Duration::from_millis(50);
    let mut engine = single_flight_raw_engine();

    let (first, first_id) = admit(
        &mut engine,
        1,
        inquiry_with_retry(1, POWER, RetryPolicy::NEVER),
        start,
    );
    send_ok(&mut engine, &first, None, start);
    let timed_out = engine.advance(timeout_at);
    assert!(matches!(
        terminal_failure(&timed_out, first_id),
        Some(Error::Timeout)
    ));

    let (inquiry_effects, successor_id) = admit(&mut engine, 2, inquiry(1, ZOOM), timeout_at);
    assert!(request_transmit_optional(&inquiry_effects).is_none());
    assert!(matches!(
        phase_of(&engine, successor_id),
        Some(Phase::Ready { .. })
    ));

    let (urgent_effects, urgent_id) = admit(
        &mut engine,
        3,
        urgent_command(1, CancellationPolicy::Supported),
        timeout_at,
    );
    assert_eq!(
        request_transmit(&urgent_effects).1,
        urgent_id,
        "the inquiry-only hold cannot delay an urgent ACK-bearing command"
    );
    send_ok(&mut engine, &urgent_effects, None, timeout_at);

    let ack = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        timeout_at + Duration::from_nanos(1),
    );
    assert!(ack.iter().any(|effect| matches!(
        effect,
        Effect::Transition {
            id,
            to: Phase::Executing { socket: ViscaSocket::S1, .. },
            ..
        } if *id == urgent_id
    )));
    assert!(matches!(
        phase_of(&engine, successor_id),
        Some(Phase::Ready { .. })
    ));

    let released = engine.advance(release_at);
    assert_eq!(request_transmit(&released).1, successor_id);
    engine.assert_invariants().unwrap();
}

/// A raw single-flight inquiry can run out of its admission budget while its
/// first write is still Sending. That release is physically uncertain just as
/// an `AwaitingReply` timeout is, so it must retain the target-only hold before
/// normal terminal cleanup frees the entry for unrelated work.
#[test]
fn raw_single_flight_sending_inquiry_budget_expiry_quarantines_before_successor_release() {
    let start = Instant::now();
    let budget_at = start + Duration::from_millis(10);
    let release_at = budget_at + Duration::from_millis(50);
    let mut engine = single_flight_raw_engine();

    let (first, first_id) = admit(
        &mut engine,
        1,
        inquiry_with_retry(1, POWER, immediate_retry_budget(Duration::from_millis(10))),
        start,
    );
    let (first_transmission, transmitted_id, _) = request_transmit(&first);
    assert_eq!(transmitted_id, first_id);
    assert!(matches!(
        phase_of(&engine, first_id),
        Some(Phase::Sending { .. })
    ));

    let (successor, successor_id) = admit(&mut engine, 2, inquiry(1, ZOOM), start);
    assert!(request_transmit_optional(&successor).is_none());
    assert_eq!(engine.next_wake(), Some(budget_at));

    let timed_out = engine.advance(budget_at);
    assert!(matches!(
        terminal_failure(&timed_out, first_id),
        Some(Error::Timeout)
    ));
    assert!(request_transmit_optional(&timed_out).is_none());
    assert!(engine.entry(first_id).is_none());
    assert!(
        !engine.transmissions.contains_key(&first_transmission),
        "normal finish removes the Sending write correlation"
    );
    assert!(
        engine.raw_inquiries[1].is_empty(),
        "a Sending inquiry never leaves a stale FIFO owner"
    );
    assert_eq!(
        engine.raw_target_tombstones[1],
        Some(RawTerminalTombstone::inquiry(release_at))
    );
    assert_eq!(engine.next_wake(), Some(release_at));
    assert!(matches!(
        phase_of(&engine, successor_id),
        Some(Phase::Ready { .. })
    ));

    // The hold is target-local: a different camera becomes eligible as soon
    // as the expired inquiry's normal finish releases global inquiry capacity.
    let (other_target, other_target_id) = admit(&mut engine, 3, inquiry(2, FOCUS), budget_at);
    assert_eq!(request_transmit(&other_target).1, other_target_id);
    let other_sent = send_ok(&mut engine, &other_target, None, budget_at);
    assert!(request_transmit_optional(&other_sent).is_none());
    let other_reply = engine.handle(
        frame(
            2,
            None,
            DecodedResponse::InquiryReply {
                route: Some(FOCUS),
                payload: smallvec![0x0c],
            },
        ),
        budget_at,
    );
    assert!(matches!(
        terminal_outcome(&other_reply, other_target_id),
        Some(RuntimeOutcome::Reply { payload, .. }) if payload.as_slice() == [0x0c]
    ));

    let stale = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: Some(POWER),
                payload: smallvec![0x0a],
            },
        ),
        budget_at + Duration::from_nanos(1),
    );
    assert_eq!(ignored_reasons(&stale), vec![IgnoreReason::UnmatchedFrame]);
    assert!(terminal_outcome(&stale, successor_id).is_none());
    assert!(matches!(
        phase_of(&engine, successor_id),
        Some(Phase::Ready { .. })
    ));

    // Input wins at the exact hold expiry: this last stale reply is inert,
    // then the due pass releases the lane and dispatches the same-target
    // successor in that owner turn.
    let at_expiry = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: Some(POWER),
                payload: smallvec![0x0a],
            },
        ),
        release_at,
    );
    assert_eq!(
        ignored_reasons(&at_expiry),
        vec![IgnoreReason::UnmatchedFrame]
    );
    let ignored = position_of(&at_expiry, |effect| {
        matches!(effect, Effect::Ignored(IgnoreReason::UnmatchedFrame))
    })
    .expect("stale inquiry reply is ignored before the due pass");
    let transmitted = position_of(&at_expiry, |effect| {
        matches!(
            effect,
            Effect::Transmit {
                request,
                kind: Transmission::Request { .. },
                ..
            } if *request == successor_id
        )
    })
    .expect("exact tombstone expiry dispatches the same-target successor");
    assert!(ignored < transmitted);

    let successor_sent = send_ok(&mut engine, &at_expiry, None, release_at);
    assert!(request_transmit_optional(&successor_sent).is_none());
    let successor_reply = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: Some(ZOOM),
                payload: smallvec![0x0b],
            },
        ),
        release_at + Duration::from_nanos(1),
    );
    assert!(matches!(
        terminal_outcome(&successor_reply, successor_id),
        Some(RuntimeOutcome::Reply { payload, .. }) if payload.as_slice() == [0x0b]
    ));
    engine.assert_invariants().unwrap();
}

/// The exact-first-dispatch seam belongs to the blocking submission owner,
/// which has not yet run its ordered input/due turn. It must therefore retain
/// a raw target tombstone even at its nominal deadline: buffered input gets
/// the boundary first, then dispatch-suppressed due work releases the hold or
/// terminalizes an earlier ready-budget expiry. A local tombstone expiration
/// here would write B after its budget or let a stale A reply bind to B.
#[cfg(feature = "blocking")]
#[test]
fn blocking_first_dispatch_waits_for_ordered_raw_tombstone_turn() {
    let start = Instant::now();
    let hold = Duration::from_millis(50);
    let timeout_at = start + Duration::from_millis(30);
    let release_at = timeout_at + hold;

    let setup = |budget| {
        let mut engine = single_flight_raw_engine();
        let (first, first_id) = admit(
            &mut engine,
            1,
            inquiry_with_retry(1, POWER, RetryPolicy::NEVER),
            start,
        );
        send_ok(&mut engine, &first, None, start);
        let first_reply = engine.advance(timeout_at);
        assert!(matches!(
            terminal_failure(&first_reply, first_id),
            Some(Error::Timeout)
        ));
        let (successor, successor_id) = admit(
            &mut engine,
            2,
            inquiry_with_retry(1, ZOOM, immediate_retry_budget(budget)),
            timeout_at,
        );
        assert!(request_transmit_optional(&successor).is_none());
        assert_eq!(
            engine.raw_target_tombstones[1],
            Some(RawTerminalTombstone::inquiry(release_at))
        );
        (engine, successor_id)
    };

    // A ready successor whose total budget is strictly before the target hold
    // must terminalize while unsent. `first_dispatch_without_due` itself does
    // not release the hold, and the owner-side no-dispatch turn services the
    // earlier global due deadline.
    {
        let (mut engine, successor_id) = setup(hold - Duration::from_nanos(1));
        assert!(matches!(
            engine.first_dispatch_without_due(successor_id, release_at),
            FirstDispatch::WaitUntil {
                deadline,
                reason: FirstDispatchWait::RawCorrelationTombstone,
            } if deadline == release_at
        ));
        assert_eq!(
            engine.raw_target_tombstones[1],
            Some(RawTerminalTombstone::inquiry(release_at))
        );
        let expired = engine.advance_without_dispatch(release_at - Duration::from_nanos(1));
        assert!(matches!(
            terminal_failure(&expired, successor_id),
            Some(Error::Timeout)
        ));
        assert!(request_transmit_optional(&expired).is_none());
        assert!(engine.entry(successor_id).is_none());
        engine.assert_invariants().unwrap();
    }

    // Equality is still input-first. The exact first-dispatch query remains a
    // wait until the ordered turn consumes any boundary frame; with no frame,
    // that same turn releases the tombstone and terminalizes B's equal budget
    // before ordinary dispatch is permitted.
    {
        let (mut engine, successor_id) = setup(hold);
        assert!(matches!(
            engine.first_dispatch_without_due(successor_id, release_at),
            FirstDispatch::WaitUntil {
                deadline,
                reason: FirstDispatchWait::RawCorrelationTombstone,
            } if deadline == release_at
        ));
        let expired = engine.advance_without_dispatch(release_at);
        assert!(matches!(
            terminal_failure(&expired, successor_id),
            Some(Error::Timeout)
        ));
        assert!(request_transmit_optional(&expired).is_none());
        assert!(engine.entry(successor_id).is_none());
        engine.assert_invariants().unwrap();
    }

    // One nanosecond beyond the hold/budget tie is a valid release: suppressed
    // due work frees only the tombstone, and the next exact dispatch may write
    // B. No peer was dispatched by the no-dispatch turn.
    let (mut engine, successor_id) = setup(hold + Duration::from_nanos(1));
    assert!(matches!(
        engine.first_dispatch_without_due(successor_id, release_at),
        FirstDispatch::WaitUntil {
            deadline,
            reason: FirstDispatchWait::RawCorrelationTombstone,
        } if deadline == release_at
    ));
    let released = engine.advance_without_dispatch(release_at);
    assert!(request_transmit_optional(&released).is_none());
    assert!(matches!(
        phase_of(&engine, successor_id),
        Some(Phase::Ready { .. })
    ));
    let dispatch = match engine.first_dispatch_without_due(successor_id, release_at) {
        FirstDispatch::Effects(effects) => effects,
        other => panic!("released successor did not win its first dispatch: {other:?}"),
    };
    assert_eq!(request_transmit(&dispatch).1, successor_id);
    engine.assert_invariants().unwrap();
}

/// Ordinary first-dispatch pacing is a clock-only wait. The blocking owner
/// must service a ready request's total budget through the no-dispatch seam at
/// this boundary, but it must not need an input turn (and therefore must not
/// consume an unrelated peer response) to do so.
#[cfg(feature = "blocking")]
#[test]
fn blocking_pacing_wait_services_total_budget_before_first_write() {
    let start = Instant::now();
    let spacing = Duration::from_millis(10);

    let setup = |budget| {
        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        engine.policy.command_spacing = spacing;

        let (first, first_id) = admit(
            &mut engine,
            1,
            command(1, CancellationPolicy::Supported),
            start,
        );
        assert_eq!(request_transmit(&first).1, first_id);
        let sent = send_ok(&mut engine, &first, Some(1), start);
        assert!(request_transmit_optional(&sent).is_none());

        let (successor, successor_id) = admit(
            &mut engine,
            2,
            command_with_reply_shape_and_retry(
                1,
                CancellationPolicy::Supported,
                ReplyShape::AckThenCompletion,
                immediate_retry_budget(budget),
            ),
            start,
        );
        assert!(request_transmit_optional(&successor).is_none());
        assert!(matches!(
            engine.first_dispatch_without_due(successor_id, start),
            FirstDispatch::WaitUntil {
                deadline,
                reason: FirstDispatchWait::Pacing,
            } if deadline == start + spacing
        ));
        (engine, successor_id)
    };

    // A total budget before the pacing release terminalizes the still-unsent
    // request. No receive/input turn is involved in this engine seam.
    let (mut earlier, earlier_id) = setup(spacing - Duration::from_nanos(1));
    let expired = earlier.advance_without_dispatch(start + spacing - Duration::from_nanos(1));
    assert!(matches!(
        terminal_failure(&expired, earlier_id),
        Some(Error::Timeout)
    ));
    assert!(request_transmit_optional(&expired).is_none());
    assert!(earlier.entry(earlier_id).is_none());

    // Equality is due-before-dispatch as well: the request cannot turn a
    // pacing wake into a local first-write budget bypass.
    let (mut equal, equal_id) = setup(spacing);
    let expired = equal.advance_without_dispatch(start + spacing);
    assert!(matches!(
        terminal_failure(&expired, equal_id),
        Some(Error::Timeout)
    ));
    assert!(request_transmit_optional(&expired).is_none());
    assert!(equal.entry(equal_id).is_none());

    // Once the budget is strictly later, the same no-dispatch wake leaves the
    // request ready; the exact first-dispatch query can then stage its write.
    let (mut later, later_id) = setup(spacing + Duration::from_nanos(1));
    let due = later.advance_without_dispatch(start + spacing);
    assert!(request_transmit_optional(&due).is_none());
    let dispatch = match later.first_dispatch_without_due(later_id, start + spacing) {
        FirstDispatch::Effects(effects) => effects,
        other => panic!("later paced request did not win first dispatch: {other:?}"),
    };
    assert_eq!(request_transmit(&dispatch).1, later_id);
    later.assert_invariants().unwrap();
}

/// The identified write-result ingress shares the Sending timeout policy. A
/// result at equality remains input-first and then reaches the due pass; a
/// result one nanosecond later must install the same target hold before it
/// terminalizes the raw inquiry.
#[test]
fn raw_single_flight_sending_inquiry_late_write_result_quarantines_before_successor_release() {
    let start = Instant::now();
    let budget_at = start + Duration::from_millis(10);
    let retry = immediate_retry_budget(Duration::from_millis(10));

    // Equality is a valid input turn. The write reaches AwaitingReply first,
    // then the due pass consumes its now-equal admission budget and retains
    // the same single-flight hold.
    {
        let mut engine = single_flight_raw_engine();
        let (first, first_id) = admit(&mut engine, 1, inquiry_with_retry(1, POWER, retry), start);
        let (transmission, transmitted_id, _) = request_transmit(&first);
        assert_eq!(transmitted_id, first_id);
        let (successor, successor_id) = admit(&mut engine, 2, inquiry(1, ZOOM), start);
        assert!(request_transmit_optional(&successor).is_none());

        let equal = engine.handle(
            Input::TransmissionFinished {
                transmission,
                result: Ok(TransmissionMeta { sequence: None }),
            },
            budget_at,
        );
        assert!(matches!(
            terminal_failure(&equal, first_id),
            Some(Error::Timeout)
        ));
        let awaiting_reply = position_of(&equal, |effect| {
            matches!(
                effect,
                Effect::Transition {
                    id,
                    to: Phase::AwaitingReply { .. },
                    ..
                } if *id == first_id
            )
        })
        .expect("equality write result transitions before due work");
        let terminal = position_of(
            &equal,
            |effect| matches!(effect, Effect::Terminal { id, .. } if *id == first_id),
        )
        .expect("due work terminalizes the equal-budget inquiry");
        assert!(awaiting_reply < terminal);
        assert_eq!(
            engine.raw_target_tombstones[1],
            Some(RawTerminalTombstone::inquiry(
                budget_at + Duration::from_millis(50)
            ))
        );
        assert!(matches!(
            phase_of(&engine, successor_id),
            Some(Phase::Ready { .. })
        ));
        engine.assert_invariants().unwrap();
    }

    let late_at = budget_at + Duration::from_nanos(1);
    let release_at = late_at + Duration::from_millis(50);
    let mut engine = single_flight_raw_engine();
    let (first, first_id) = admit(&mut engine, 1, inquiry_with_retry(1, POWER, retry), start);
    let (first_transmission, transmitted_id, _) = request_transmit(&first);
    assert_eq!(transmitted_id, first_id);
    let (successor, successor_id) = admit(&mut engine, 2, inquiry(1, ZOOM), start);
    assert!(request_transmit_optional(&successor).is_none());

    let late = engine.handle(
        Input::TransmissionFinished {
            transmission: first_transmission,
            result: Ok(TransmissionMeta { sequence: None }),
        },
        late_at,
    );
    assert!(matches!(
        terminal_failure(&late, first_id),
        Some(Error::Timeout)
    ));
    assert!(!late.iter().any(|effect| matches!(
        effect,
        Effect::Transition {
            id,
            to: Phase::AwaitingReply { .. },
            ..
        } if *id == first_id
    )));
    assert!(engine.entry(first_id).is_none());
    assert!(engine.transmissions.is_empty());
    assert!(engine.raw_inquiries[1].is_empty());
    assert!(engine.sequences.is_empty());
    assert!(engine.lower_sequences.is_empty());
    assert_eq!(
        engine.raw_target_tombstones[1],
        Some(RawTerminalTombstone::inquiry(release_at))
    );
    assert_eq!(engine.next_wake(), Some(release_at));
    assert!(matches!(
        phase_of(&engine, successor_id),
        Some(Phase::Ready { .. })
    ));

    // The same tombstone isolates the successor while a different target can
    // use the freed global single-flight capacity.
    let (other_target, other_target_id) = admit(&mut engine, 3, inquiry(2, FOCUS), late_at);
    assert_eq!(request_transmit(&other_target).1, other_target_id);
    let other_sent = send_ok(&mut engine, &other_target, None, late_at);
    assert!(request_transmit_optional(&other_sent).is_none());
    let other_reply = engine.handle(
        frame(
            2,
            None,
            DecodedResponse::InquiryReply {
                route: Some(FOCUS),
                payload: smallvec![0x0c],
            },
        ),
        late_at,
    );
    assert!(matches!(
        terminal_outcome(&other_reply, other_target_id),
        Some(RuntimeOutcome::Reply { payload, .. }) if payload.as_slice() == [0x0c]
    ));

    let stale = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: Some(POWER),
                payload: smallvec![0x0a],
            },
        ),
        late_at + Duration::from_nanos(1),
    );
    assert_eq!(ignored_reasons(&stale), vec![IgnoreReason::UnmatchedFrame]);
    assert!(terminal_outcome(&stale, successor_id).is_none());
    assert!(matches!(
        phase_of(&engine, successor_id),
        Some(Phase::Ready { .. })
    ));

    let at_expiry = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: Some(POWER),
                payload: smallvec![0x0a],
            },
        ),
        release_at,
    );
    assert_eq!(
        ignored_reasons(&at_expiry),
        vec![IgnoreReason::UnmatchedFrame]
    );
    let ignored = position_of(&at_expiry, |effect| {
        matches!(effect, Effect::Ignored(IgnoreReason::UnmatchedFrame))
    })
    .expect("stale inquiry reply is ignored before the due pass");
    let transmitted = position_of(&at_expiry, |effect| {
        matches!(
            effect,
            Effect::Transmit {
                request,
                kind: Transmission::Request { .. },
                ..
            } if *request == successor_id
        )
    })
    .expect("exact tombstone expiry dispatches the same-target successor");
    assert!(ignored < transmitted);

    let successor_sent = send_ok(&mut engine, &at_expiry, None, release_at);
    assert!(request_transmit_optional(&successor_sent).is_none());
    let successor_reply = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                route: Some(ZOOM),
                payload: smallvec![0x0b],
            },
        ),
        release_at + Duration::from_nanos(1),
    );
    assert!(matches!(
        terminal_outcome(&successor_reply, successor_id),
        Some(RuntimeOutcome::Reply { payload, .. }) if payload.as_slice() == [0x0b]
    ));
    engine.assert_invariants().unwrap();
}

/// A completion-only command likewise has no socket identity after it finishes.
/// Its duplicate terminal/error frames must remain inert until the bounded
/// target hold expires; a later ordinary command must never become its owner.
#[test]
fn completion_only_terminal_quarantine_blocks_successor_then_restores_liveness() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (first, first_id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
        start,
    );
    send_ok(&mut engine, &first, None, start);

    // B is already ready when A's uncorrelated terminal arrives. The target
    // tombstone must be present before `finish_input_turn` can dispatch it.
    let (second, second_id) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        start,
    );
    assert!(request_transmit_optional(&second).is_none());
    let turn = engine.begin_input_turn(start);
    let completed = engine.handle_in_turn(
        &turn,
        frame(1, None, DecodedResponse::Completion { socket: None }),
    );
    assert!(matches!(
        terminal_outcome(&completed, first_id),
        Some(RuntimeOutcome::Applied)
    ));
    assert!(request_transmit_optional(&engine.finish_input_turn(turn)).is_none());
    assert!(matches!(
        phase_of(&engine, second_id),
        Some(Phase::Ready { .. })
    ));

    for response in [
        DecodedResponse::Ack {
            socket: Some(ViscaSocket::S1),
        },
        DecodedResponse::Completion { socket: None },
        DecodedResponse::Error {
            socket: None,
            code: 0x01,
        },
    ] {
        let late = engine.handle(frame(1, None, response), start);
        assert!(late
            .iter()
            .any(|effect| matches!(effect, Effect::Ignored(_))));
        assert!(terminal_outcome(&late, second_id).is_none());
        assert!(matches!(
            phase_of(&engine, second_id),
            Some(Phase::Ready { .. })
        ));
    }

    let released = engine.advance(start + Duration::from_millis(50));
    assert_eq!(request_transmit(&released).1, second_id);
    engine.assert_invariants().unwrap();
}

/// A target tombstone also holds same-target inquiries. A raw socketless error
/// has no command/inquiry discriminator, so allowing an inquiry to start would
/// either bind a stale fire-and-forget error to it or force the owner's real
/// inquiry error to be silently discarded.
#[test]
fn no_reply_terminal_quarantine_holds_same_target_inquiry_until_expiry() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (first, first_id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::NoReply),
        start,
    );
    let written = send_ok(&mut engine, &first, None, start);
    assert!(matches!(
        terminal_outcome(&written, first_id),
        Some(RuntimeOutcome::Written)
    ));

    let (inquiry, inquiry_id) = admit(&mut engine, 2, inquiry(1, POWER), start);
    assert!(request_transmit_optional(&inquiry).is_none());
    assert!(matches!(
        phase_of(&engine, inquiry_id),
        Some(Phase::Ready { .. })
    ));

    let stale = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x01,
            },
        ),
        start,
    );
    assert_eq!(
        ignored_reasons(&stale),
        vec![IgnoreReason::UnmatchedFrame],
        "a delayed no-reply error cannot bind to the queued inquiry"
    );
    assert!(engine.entry(inquiry_id).is_some());

    let deadline = start + Duration::from_millis(50);
    let released = engine.advance(deadline);
    assert_eq!(request_transmit(&released).1, inquiry_id);
    send_ok(&mut engine, &released, None, deadline);
    let legitimate = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x01,
            },
        ),
        deadline,
    );
    assert!(
        matches!(
            terminal_failure(&legitimate, inquiry_id),
            Some(Error::MessageLengthError)
        ),
        "after expiry, an inquiry's own raw error remains attributable"
    );
    engine.assert_invariants().unwrap();
}

/// The no-reply write itself is uncorrelatable. It therefore may not begin
/// beside already live same-target command or inquiry work, and no inquiry may
/// slip in between its transmit effect and its successful write result.
#[test]
fn no_reply_raw_exclusivity_is_bidirectional_during_its_write() {
    let start = Instant::now();

    // An executing socket-owning command makes a no-reply write wait rather
    // than letting its later target tombstone swallow this command's terminal.
    let mut command_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (command_effects, command_id) = admit(
        &mut command_engine,
        1,
        command(1, CancellationPolicy::Supported),
        start,
    );
    send_ok(&mut command_engine, &command_effects, None, start);
    command_engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    assert!(matches!(
        phase_of(&command_engine, command_id),
        Some(Phase::Executing { .. })
    ));
    let (no_reply_after_command, _) = admit(
        &mut command_engine,
        2,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::NoReply),
        start,
    );
    assert!(request_transmit_optional(&no_reply_after_command).is_none());

    // An inquiry is equally unsafe: a socketless raw error has no
    // command/inquiry discriminator.
    let mut inquiry_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (inquiry_effects, inquiry_id) = admit(&mut inquiry_engine, 1, inquiry(1, POWER), start);
    send_ok(&mut inquiry_engine, &inquiry_effects, None, start);
    assert!(matches!(
        phase_of(&inquiry_engine, inquiry_id),
        Some(Phase::AwaitingReply { .. })
    ));
    let (no_reply_after_inquiry, _) = admit(
        &mut inquiry_engine,
        2,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::NoReply),
        start,
    );
    assert!(request_transmit_optional(&no_reply_after_inquiry).is_none());

    // Conversely, once the no-reply write is already Sending, an inquiry must
    // remain queued until that write turns into the target tombstone.
    let mut sending_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (no_reply_sending, no_reply_id) = admit(
        &mut sending_engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::NoReply),
        start,
    );
    assert_eq!(request_transmit(&no_reply_sending).1, no_reply_id);
    assert!(matches!(
        phase_of(&sending_engine, no_reply_id),
        Some(Phase::Sending { .. })
    ));
    let (raced_inquiry, raced_inquiry_id) = admit(&mut sending_engine, 2, inquiry(1, POWER), start);
    assert!(request_transmit_optional(&raced_inquiry).is_none());
    assert!(matches!(
        phase_of(&sending_engine, raced_inquiry_id),
        Some(Phase::Ready { .. })
    ));

    command_engine.assert_invariants().unwrap();
    inquiry_engine.assert_invariants().unwrap();
    sending_engine.assert_invariants().unwrap();
}

/// A terminal tombstone excludes response-bearing successors, not another
/// fire-and-forget write. A no-reply successor consumes no response identity,
/// so it can safely extend the same fixed target hold.
#[test]
fn uncorrelatable_terminal_allows_no_reply_successor_and_extends_hold() {
    let start = Instant::now();
    for reply_shape in [ReplyShape::NoReply, ReplyShape::CompletionOnly] {
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (first, first_id) = admit(
            &mut engine,
            1,
            command_with_reply_shape(1, CancellationPolicy::Supported, reply_shape),
            start,
        );
        let first_terminal = match reply_shape {
            ReplyShape::NoReply => send_ok(&mut engine, &first, None, start),
            ReplyShape::CompletionOnly => {
                send_ok(&mut engine, &first, None, start);
                engine.handle(
                    frame(1, None, DecodedResponse::Completion { socket: None }),
                    start,
                )
            }
            ReplyShape::AckThenCompletion => {
                unreachable!("loop contains only uncorrelatable shapes")
            }
        };
        assert!(terminal_outcome(&first_terminal, first_id).is_some());

        let successor_at = start + Duration::from_millis(1);
        let (second, second_id) = admit(
            &mut engine,
            2,
            command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::NoReply),
            successor_at,
        );
        assert_eq!(request_transmit(&second).1, second_id);
        let second_terminal = send_ok(&mut engine, &second, None, successor_at);
        assert!(matches!(
            terminal_outcome(&second_terminal, second_id),
            Some(RuntimeOutcome::Written)
        ));

        let extended_deadline = successor_at + Duration::from_millis(50);
        assert_eq!(engine.next_wake(), Some(extended_deadline));
        let still_held = engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Error {
                    socket: None,
                    code: 0x01,
                },
            ),
            start + Duration::from_millis(50),
        );
        assert_eq!(
            ignored_reasons(&still_held),
            vec![IgnoreReason::UnmatchedFrame]
        );
        assert!(engine.advance(extended_deadline).is_empty());
        engine.assert_invariants().unwrap();
    }
}

/// A normal raw command releases its camera socket at completion. Raw VISCA
/// carries no terminal frame identity, so this engine must not reserve that
/// socket in software and reject the camera's legitimate immediate reuse.
#[test]
fn completed_raw_command_allows_immediate_same_socket_reuse() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);

    let (first, first_id) = admit(
        &mut engine,
        1,
        command(1, CancellationPolicy::Supported),
        start,
    );
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
    let complete_first = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    assert!(matches!(
        terminal_outcome(&complete_first, first_id),
        Some(RuntimeOutcome::Applied)
    ));
    let (second, second_id) = admit(
        &mut engine,
        2,
        command(1, CancellationPolicy::Supported),
        start,
    );
    assert_eq!(request_transmit(&second).1, second_id);
    send_ok(&mut engine, &second, None, start);
    let ack = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    assert!(terminal_outcome(&ack, second_id).is_none());
    assert!(matches!(
        phase_of(&engine, second_id),
        Some(Phase::Executing { .. })
    ));
    let complete_second = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    assert!(matches!(
        terminal_outcome(&complete_second, second_id),
        Some(RuntimeOutcome::Applied)
    ));
    engine.assert_invariants().unwrap();
}

/// #700: a completion-only command owns the target's command channel for its
/// whole lifetime, because it can never be socket-correlated. Nothing else may
/// dispatch while it is in flight, and it may not start while anything else is —
/// even when a command socket is free.
#[test]
fn completion_only_command_holds_the_command_channel_exclusively() {
    let start = Instant::now();

    // Forward: a completion-only command in flight blocks a later ordinary one.
    {
        let mut runtime = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (first, _c1) = admit(
            &mut runtime,
            1,
            command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
            start,
        );
        send_ok(&mut runtime, &first, None, start);
        let second = admit(
            &mut runtime,
            2,
            command(1, CancellationPolicy::Supported),
            start,
        )
        .0;
        assert!(
            request_transmit_optional(&second).is_none(),
            "no command dispatches while a completion-only command awaits its completion",
        );
        runtime.assert_invariants().unwrap();
    }

    // Reverse: an ordinary Executing command leaves a free socket, yet a
    // completion-only command still may not start — it needs exclusivity.
    {
        let mut runtime = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (first, c1) = admit(
            &mut runtime,
            1,
            command(1, CancellationPolicy::Supported),
            start,
        );
        send_ok(&mut runtime, &first, None, start);
        runtime.handle(
            frame(
                1,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start,
        );
        assert!(matches!(
            phase_of(&runtime, c1),
            Some(Phase::Executing { .. })
        ));
        let second = admit(
            &mut runtime,
            2,
            command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
            start,
        )
        .0;
        assert!(
            request_transmit_optional(&second).is_none(),
            "a completion-only command waits for exclusive access even when a socket is free",
        );
        runtime.assert_invariants().unwrap();
    }
}

/// #700: completion-only ownership covers the target's inquiry channel too.
/// A same-target inquiry stays queued, so a socketless error remains
/// attributable to the uncorrelated command; requests for other targets still
/// dispatch normally.
#[test]
fn raw_completion_only_blocks_same_target_inquiry_and_routes_socketless_error() {
    let start = Instant::now();
    let mut runtime = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (completion_only, completion_id) = admit(
        &mut runtime,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
        start,
    );
    send_ok(&mut runtime, &completion_only, None, start);
    assert!(matches!(
        phase_of(&runtime, completion_id),
        Some(Phase::AwaitingCompletion { .. })
    ));

    let (same_target_inquiry, inquiry_id) = admit(&mut runtime, 2, inquiry(1, POWER), start);
    assert!(
        request_transmit_optional(&same_target_inquiry).is_none(),
        "a same-target inquiry must wait for the completion-only command",
    );
    assert!(matches!(
        phase_of(&runtime, inquiry_id),
        Some(Phase::Ready { .. })
    ));

    let (other_target_inquiry, other_target_id) = admit(&mut runtime, 3, inquiry(2, ZOOM), start);
    assert_eq!(
        request_transmit(&other_target_inquiry).1,
        other_target_id,
        "completion-only ownership is local to its target",
    );

    let error = runtime.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x01,
            },
        ),
        start,
    );
    assert!(matches!(
        terminal_failure(&error, completion_id),
        Some(Error::MessageLengthError)
    ));
    assert_eq!(
        request_transmit(&error).1,
        inquiry_id,
        "releasing completion-only ownership dispatches the queued same-target inquiry",
    );
    runtime.assert_invariants().unwrap();
}

/// #700: an already-live raw inquiry prevents a completion-only command from
/// starting until the inquiry's reply releases the target.
#[test]
fn raw_inquiry_blocks_same_target_completion_only_until_reply() {
    let start = Instant::now();
    let mut runtime = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (inquiry_effects, inquiry_id) = admit(&mut runtime, 1, inquiry(1, POWER), start);
    send_ok(&mut runtime, &inquiry_effects, None, start);
    assert!(matches!(
        phase_of(&runtime, inquiry_id),
        Some(Phase::AwaitingReply { .. })
    ));

    let (completion_only, completion_id) = admit(
        &mut runtime,
        2,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
        start,
    );
    assert!(
        request_transmit_optional(&completion_only).is_none(),
        "a completion-only command must wait for a same-target inquiry reply",
    );
    assert!(matches!(
        phase_of(&runtime, completion_id),
        Some(Phase::Ready { .. })
    ));

    let reply = runtime.handle(
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
    assert_eq!(terminal_id(&reply), Some(inquiry_id));
    assert_eq!(
        request_transmit(&reply).1,
        completion_id,
        "the completion-only command dispatches once the inquiry is terminal",
    );
    send_ok(&mut runtime, &reply, None, start);
    let completed = runtime.handle(
        frame(1, None, DecodedResponse::Completion { socket: None }),
        start,
    );
    assert!(matches!(
        terminal_outcome(&completed, completion_id),
        Some(RuntimeOutcome::Applied)
    ));
    runtime.assert_invariants().unwrap();
}

/// Completion-only exclusivity is a raw-VISCA rule. Sony's envelope sequence
/// still lets a same-target inquiry pipeline behind such a command.
#[test]
fn sony_completion_only_does_not_block_same_target_inquiry() {
    let start = Instant::now();
    let mut runtime = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (completion_only, completion_id) = admit(
        &mut runtime,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
        start,
    );
    send_ok(&mut runtime, &completion_only, Some(0x1001), start);
    assert!(matches!(
        phase_of(&runtime, completion_id),
        Some(Phase::AwaitingCompletion { .. })
    ));

    // Even exact Sony sequence correlation does not make an ACK meaningful for
    // this reply shape: it must not assign a socket or begin cancellation work.
    let stray_ack = runtime.handle(
        frame(
            1,
            Some((0x1001, SequenceWidth::Full32)),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    assert_eq!(
        ignored_reasons(&stray_ack),
        vec![IgnoreReason::UnmatchedFrame]
    );
    assert!(cancel_transmit_optional(&stray_ack).is_none());
    assert_eq!(socket_of(&runtime, completion_id), None);
    assert_eq!(runtime.socket_owner(camera(1), ViscaSocket::S1), None);
    assert!(matches!(
        phase_of(&runtime, completion_id),
        Some(Phase::AwaitingCompletion { .. })
    ));
    runtime.assert_invariants().unwrap();

    let (inquiry_effects, inquiry_id) = admit(&mut runtime, 2, inquiry(1, POWER), start);
    assert_eq!(
        request_transmit(&inquiry_effects).1,
        inquiry_id,
        "Sony sequence correlation must retain same-target concurrency",
    );
    send_ok(&mut runtime, &inquiry_effects, Some(0x1002), start);

    let completed = runtime.handle(
        frame(
            1,
            Some((0x1001, SequenceWidth::Full32)),
            DecodedResponse::Completion { socket: None },
        ),
        start,
    );
    assert!(matches!(
        terminal_outcome(&completed, completion_id),
        Some(RuntimeOutcome::Applied)
    ));
    let replied = runtime.handle(
        frame(
            1,
            Some((0x1002, SequenceWidth::Full32)),
            DecodedResponse::InquiryReply {
                route: Some(POWER),
                payload: smallvec![1],
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&replied), Some(inquiry_id));
    runtime.assert_invariants().unwrap();
}

/// #700 / #297: a completion that races ahead of the write result for a
/// completion-only command is latched while the write is Sending and applied the
/// instant the send is confirmed, so a fast camera's completion is never dropped.
#[test]
fn completion_only_completion_racing_the_write_result_is_latched() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (admitted_effects, id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
        start,
    );
    assert!(
        matches!(phase_of(&engine, id), Some(Phase::Sending { .. })),
        "the command is Sending until its write result arrives",
    );
    // The completion arrives before the write result; it is latched, not applied.
    let early = engine.handle(
        frame(1, None, DecodedResponse::Completion { socket: None }),
        start,
    );
    assert!(
        terminal_outcome(&early, id).is_none(),
        "the racing completion is latched, not applied yet",
    );
    // Confirming the send applies the latched completion immediately.
    let send = send_ok(&mut engine, &admitted_effects, None, start);
    assert!(
        matches!(terminal_outcome(&send, id), Some(RuntimeOutcome::Applied)),
        "the latched completion terminates the command on send confirmation",
    );
    engine.assert_invariants().unwrap();
}

/// #700: socketless capacity errors are terminal frames for a completion-only
/// command and reuse the ordinary camera-error retry policy. Both VISCA
/// capacity codes must schedule a retry instead of leaving the command to time
/// out in `AwaitingCompletion`.
#[test]
fn completion_only_socketless_capacity_errors_retry() {
    let start = Instant::now();
    for code in [0x03, 0x05] {
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (admitted_effects, id) = admit(
            &mut engine,
            u64::from(code),
            command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
            start,
        );
        send_ok(&mut engine, &admitted_effects, None, start);

        let refused = engine.handle(
            frame(1, None, DecodedResponse::Error { socket: None, code }),
            start,
        );
        assert!(
            ignored_reasons(&refused).is_empty(),
            "capacity error 0x{code:02X} must route to the completion-only command",
        );
        assert_eq!(
            retry_scheduled(&refused).map(|scheduled| scheduled.0),
            Some(id)
        );
        assert!(terminal_outcome(&refused, id).is_none());
        assert!(matches!(phase_of(&engine, id), Some(Phase::Backoff { .. })));
        engine.assert_invariants().unwrap();
    }
}

/// #700: a socketless nonretryable camera error terminates a completion-only
/// command with the camera's exact error, rather than falling through to the
/// completion deadline.
#[test]
fn completion_only_socketless_nonretryable_error_is_terminal() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (admitted_effects, id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
        start,
    );
    send_ok(&mut engine, &admitted_effects, None, start);

    let refused = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x01,
            },
        ),
        start,
    );
    assert!(ignored_reasons(&refused).is_empty());
    assert!(matches!(
        terminal_failure(&refused, id),
        Some(Error::MessageLengthError)
    ));
    assert!(engine.entry(id).is_none());
    engine.assert_invariants().unwrap();
}

/// #700: named raw error sockets remain authoritative. A completion-only
/// command owns no socket, so an error naming an unowned socket is ignored;
/// the socketless form is the compatible terminal frame for this shape.
#[test]
fn completion_only_named_unowned_error_does_not_fallback() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (admitted_effects, id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CancellationPolicy::Supported, ReplyShape::CompletionOnly),
        start,
    );
    send_ok(&mut engine, &admitted_effects, None, start);

    let named = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: Some(ViscaSocket::S1),
                code: 0x01,
            },
        ),
        start,
    );
    assert_eq!(ignored_reasons(&named), vec![IgnoreReason::UnmatchedFrame]);
    assert!(terminal_outcome(&named, id).is_none());
    assert!(matches!(
        phase_of(&engine, id),
        Some(Phase::AwaitingCompletion { .. })
    ));

    let socketless = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x01,
            },
        ),
        start,
    );
    assert!(matches!(
        terminal_failure(&socketless, id),
        Some(Error::MessageLengthError)
    ));
    engine.assert_invariants().unwrap();
}
