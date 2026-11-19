#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::{
    collections::{BTreeMap, VecDeque},
    num::NonZeroU64,
    sync::Arc,
    time::Duration,
};

use smallvec::{smallvec, SmallVec};

use super::*;
use crate::wire::WireValue;
use crate::FailureContext;

mod admission_verdict;
mod audit_rules;
mod camera_error_codes;
mod cancellation_holds;
mod due_actions;
mod effects_sink;
mod entry_table;
mod halt_order;
mod inquiry_shape_reservations;
mod inquiry_shapes;
mod late_evidence;
mod lifecycle_predicates;
mod lifecycle_transition_matrix;
mod occupancy_cost;
mod occupancy_oracle;
mod pacing_receipts;
mod pan_tilt_drive_cancellation;
mod prepared_retry;
mod raw_evidence;
mod raw_stream_model;
mod retry_history;
mod session_end;
mod socketless_ack_conflicts;
mod sony_socket_reuse;
mod speed_mode_order;
mod stop_quarantine;
mod write_outcomes;
mod zoom_drive_stop;

fn extend_fixture_hold(
    engine: &mut ProtocolEngine,
    target: TargetSlot,
    scope: StreamScope,
    until: Instant,
    owner: Option<RequestId>,
) {
    use super::holds::Scope;
    match &mut engine.correlation {
        CorrelationMode::RawDatagram { holds } => holds.extend(target, scope.raw(), until, owner),
        CorrelationMode::RawStream(stream) => stream.holds.extend(target, scope, until, owner),
        CorrelationMode::Sequenced => panic!("fixture requires a raw mode"),
    }
}
fn extend_fixture_pre_ack(
    engine: &mut ProtocolEngine,
    target: TargetSlot,
    until: Instant,
    owner: Option<RequestId>,
) {
    let CorrelationMode::RawDatagram { holds } = &mut engine.correlation else {
        panic!("fixture requires datagram holds")
    };
    holds.extend(target, RawHoldScope::PreAck, until, owner);
}

pub(super) fn assert_occupancy_agrees<E: EngineMethods + ?Sized>(engine: &E, now: Instant) {
    occupancy_oracle::assert_agrees(engine, now);
}

fn camera(value: u8) -> TargetSlot {
    TargetSlot::from(crate::IndividualCameraId::new(value).unwrap())
}

fn policy(envelope: EnvelopeKind, transport: TransportKind) -> ProtocolPolicy {
    ProtocolPolicy {
        wire: WireProtocol::new(envelope, crate::transport::AddressingMode::Ip),
        transport,
        ..ProtocolPolicy::test_default()
    }
}

/// A raw datagram policy that keeps the pre-fix whole-session poison behavior,
/// so a test can pin the strict opt-in mode explicitly.
fn strict_poison_policy() -> ProtocolPolicy {
    ProtocolPolicy {
        strict_unconfirmed_poison: true,
        ..ProtocolPolicy::test_default()
    }
}

/// A two-target raw datagram engine in the strict `strict_unconfirmed_poison`
/// opt-in mode, mirroring [`engine`] for the whole-session poison tests.
fn strict_poison_engine() -> ProtocolEngine {
    ProtocolEngine::new(
        strict_poison_policy(),
        target_policies(&[camera(1), camera(2)], TargetPolicy::test_default()),
        Instant::now(),
    )
    .unwrap()
}

fn retrying() -> RetryPolicy {
    RetryPolicy {
        max_retries: 3,
        initial_backoff: Duration::from_millis(10),
        maximum_backoff: Duration::from_millis(100),
        total_budget: Some(NonZeroDuration::new(Duration::from_secs(10)).unwrap()),
    }
}

fn context(target: u8) -> RequestContext {
    RequestContext {
        requires_extended_pan_tilt_speed: false,
        pan_tilt_drive: false,
        motion: None,
        submission_order: 0,
        dispatch_deadline: None,
        target: camera(target),
        timeout: TimeoutPolicy {
            ack: NonZeroDuration::new(Duration::from_millis(20)).unwrap(),
            completion: NonZeroDuration::new(Duration::from_millis(40)).unwrap(),
            inquiry: NonZeroDuration::new(Duration::from_millis(30)).unwrap(),
            cancellation: NonZeroDuration::new(Duration::from_millis(10)).unwrap(),
            ambiguity: NonZeroDuration::new(Duration::from_millis(50)).unwrap(),
        },
        retry: retrying(),
        control: ControlPolicy::default(),
    }
}

fn wire(address: u8) -> Arc<EncodedMessage> {
    Arc::new(EncodedMessage::new(&[address, 0x01, 0x04, 0x00, 0xff]).unwrap())
}

fn command(target: u8) -> RuntimeRequest {
    RuntimeRequest::Command {
        not_executable: true,
        reply_shape: CommandReply::AckThenCompletion,
        wire: wire(0x80 | target),
        context: context(target),
        applied_state: None,
    }
}

fn urgent_command(target: u8) -> RuntimeRequest {
    let mut request = command(target);
    let RuntimeRequest::Command { reply_shape, .. } = &mut request else {
        unreachable!("command helper always constructs a command");
    };
    *reply_shape = CommandReply::Stop;
    request
}

/// A raw command that declares a non-default reply shape (issue #700).
fn command_with_reply_shape(target: u8, reply_shape: CommandReply) -> RuntimeRequest {
    let context = context(target);

    RuntimeRequest::Command {
        not_executable: true,
        reply_shape,
        wire: wire(0x80 | target),
        context,
        applied_state: None,
    }
}

fn command_with_reply_shape_and_retry(
    target: u8,
    reply_shape: CommandReply,
    retry: RetryPolicy,
) -> RuntimeRequest {
    let mut request = command_with_reply_shape(target, reply_shape);
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

fn write_outcome(result: Result<TransmissionMeta, Error>) -> WriteOutcome {
    match result {
        Ok(meta) => WriteOutcome::Sent(meta),
        Err(error) => WriteOutcome::Failed(error),
    }
}

fn finish_write_input_only(
    engine: &mut ProtocolEngine,
    request: RequestId,
    transmission: TransmissionId,
    result: Result<TransmissionMeta, Error>,
    now: Instant,
) -> Vec<Effect> {
    engine.handle_turn(
        Input::TransmissionFinished {
            request,
            transmission,
            result: write_outcome(result),
        },
        now,
        EngineTurn::InputOnly,
    )
}

/// The authoritative phase of an admitted entry, for lifecycle assertions.
fn phase_of(engine: &ProtocolEngine, id: RequestId) -> Option<Phase> {
    engine.entries.get(&id).map(Entry::phase)
}

fn inquiry(target: u8) -> RuntimeRequest {
    inquiry_with_retry(target, retrying())
}

fn inquiry_with_retry(target: u8, retry: RetryPolicy) -> RuntimeRequest {
    let mut inquiry_context = context(target);
    inquiry_context.retry = retry;
    RuntimeRequest::Inquiry {
        syntax: true,
        payload_shape: PayloadShape::Unknown,
        wire: wire(0x80 | target),
        context: inquiry_context,
    }
}

fn inquiry_with_shape(target: u8, shape: PayloadShape) -> RuntimeRequest {
    RuntimeRequest::Inquiry {
        syntax: true,
        payload_shape: shape,
        wire: wire(0x80 | target),
        context: context(target),
    }
}

fn target_policies(
    targets: &[TargetSlot],
    policy: TargetPolicy,
) -> PerTarget<Option<TargetPolicy>> {
    PerTarget::from_fn(|target| targets.contains(&target).then_some(policy))
}

fn engine(envelope: EnvelopeKind, transport: TransportKind) -> ProtocolEngine {
    ProtocolEngine::new(
        policy(envelope, transport),
        target_policies(&[camera(1), camera(2)], TargetPolicy::test_default()),
        Instant::now(),
    )
    .unwrap()
}

/// Unknown raw inquiry declarations overlap. A released FIFO owner therefore
/// has a queued successor and needs the bounded target ambiguity hold.
fn single_flight_raw_engine() -> ProtocolEngine {
    let configured = policy(EnvelopeKind::Raw, TransportKind::Datagram);
    ProtocolEngine::new(
        configured,
        target_policies(&[camera(1), camera(2)], TargetPolicy::test_default()),
        Instant::now(),
    )
    .unwrap()
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
    let (transmission, id, _) = request_transmit(effects);
    engine.handle(
        Input::TransmissionFinished {
            request: id,
            transmission,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(sequence)),
        },
        now,
    )
}

fn frame(target: u8, sequence: Option<u32>, response: DecodedResponse) -> Input {
    Input::Frame(DecodedFrame {
        target: camera(target),
        sequence: sequence.map(FrameSequence::from_reply_header),
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
        not_executable: true,
        reply_shape: CommandReply::AckThenCompletion,
        wire: Arc::clone(&prepared),
        context: context(1),
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
            request: id,
            transmission: first_tx,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(Some(0x1001))),
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
    let mut target_two = command(2);
    if let RuntimeRequest::Command { wire, .. } = &mut target_two {
        *wire = wire_for_test(&[0x81, 0x01, 0xff]);
    }
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
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
fn duplicate_write_receipts_do_not_requeue_the_request() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
        },
        start,
    );
    let (tx, id, _) = request_transmit(&effects);
    engine.handle(
        Input::TransmissionFinished {
            request: id,
            transmission: tx,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
        },
        start,
    );
    let duplicate = engine.handle(
        Input::TransmissionFinished {
            request: id,
            transmission: tx,
            result: WriteOutcome::Failed(Error::io_timeout()),
        },
        start,
    );
    assert!(duplicate
        .iter()
        .any(|effect| matches!(effect, Effect::Ignored(IgnoreReason::StaleTransmission))));
    let effects = engine.advance(start);
    assert!(!effects
        .iter()
        .any(|effect| matches!(effect, Effect::Transmit { request, .. } if *request == id)));
    engine.assert_invariants().unwrap();
}

#[test]
fn request_and_transmission_allocators_stop_at_exhaustion() {
    let mut allocator = IdAllocator::seeded(u64::MAX);
    assert_eq!(allocator.candidate().map(NonZeroU64::get), Some(u64::MAX));
    assert!(allocator.candidate().is_none());

    let start = Instant::now();
    let mut request_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    request_engine.seed_allocators(u64::MAX, 1);
    let first = request_engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
        },
        start,
    );
    let first_id = admitted(&first);
    assert_eq!(first_id.get(), u64::MAX);
    send_ok(&mut request_engine, &first, None, start);
    let second = request_engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1),
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
    transmission_engine.seed_allocators(1, u64::MAX);
    let first = transmission_engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
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
            request: command(1),
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
            outcome: RuntimeOutcome::Command(CommandOutcome::Failed(Error::RuntimeIdentityExhausted)),
        } if *id == second_id
    )));
    assert!(transmission_engine.entry(first_id).is_some());
    request_engine.assert_invariants().unwrap();
    transmission_engine.assert_invariants().unwrap();
}

#[test]
fn sony_exact_and_unique_lower16_are_target_safe_and_owner_deduplicated() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let a = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: inquiry_with_retry(1, RetryPolicy::NEVER),
        },
        start,
    );
    let a_id = admitted(&a);
    send_ok(&mut engine, &a, Some(0x1111_beef), start);
    // Re-registering the same sequence for the same owner does not create a
    // false lower16 collision.
    engine
        .entries
        .transition(
            a_id,
            EntryChange::Written {
                receipt: TransmissionMeta::Allocated {
                    sequence: 0x1111_beef,
                },
                kind: CorrelationKind::Request,
                pan_tilt_drive_written_at: None,
            },
        )
        .unwrap();
    let unique = engine.handle(
        frame(
            1,
            Some(0xbeef),
            DecodedResponse::InquiryReply {
                payload: smallvec![1],
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&unique), Some(a_id));

    let b = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: inquiry(1),
        },
        start,
    );
    let b_id = admitted(&b);
    send_ok(&mut engine, &b, Some(0x2222_beef), start);
    let c = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: inquiry(1),
        },
        start,
    );
    let c_id = admitted(&c);
    send_ok(&mut engine, &c, Some(0x3333_beef), start);
    let collision = engine.handle(
        frame(
            1,
            Some(0xbeef),
            DecodedResponse::InquiryReply {
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
            Some(0x3333_beef),
            DecodedResponse::InquiryReply {
                payload: smallvec![3],
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&exact), Some(c_id));
    let now_unique = engine.handle(
        frame(
            1,
            Some(0xbeef),
            DecodedResponse::InquiryReply {
                payload: smallvec![2],
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&now_unique), Some(b_id));

    let wrong_target = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(4),
            request: inquiry_with_retry(1, RetryPolicy::NEVER),
        },
        start,
    );
    send_ok(&mut engine, &wrong_target, Some(0x1234_5678), start);
    let ignored = engine.handle(
        frame(
            2,
            Some(0x1234_5678),
            DecodedResponse::InquiryReply {
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
            request: command(1),
        },
        start,
    );
    let (request_tx, id, _) = request_transmit(&admitted_effects);
    engine.handle(
        Input::TransmissionFinished {
            request: id,
            transmission: request_tx,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(Some(0x1111_beef))),
        },
        start,
    );
    engine.handle(
        frame(
            1,
            Some(0x1111_beef),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    let cancellation = engine.handle(engine.cancel_input(id), start);
    let (cancel_tx, cancel_id, _) = cancel_transmit(&cancellation);
    assert_eq!(cancel_id, id);
    engine.handle(
        Input::TransmissionFinished {
            request: cancel_id,
            transmission: cancel_tx,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(
                // Deliberately collide with the original request identity.
                Some(0x1111_beef),
            )),
        },
        start,
    );

    let collision = engine.handle(
        frame(
            1,
            Some(0x1111_beef),
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
            request: command(1),
        },
        start,
    );
    let (request_tx, id, _) = request_transmit(&admitted_effects);
    engine.handle(
        Input::TransmissionFinished {
            request: id,
            transmission: request_tx,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(Some(0x1111_beef))),
        },
        start,
    );
    engine.handle(
        frame(
            1,
            Some(0x1111_beef),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    let cancellation = engine.handle(engine.cancel_input(id), start);
    let (cancel_tx, cancel_id, _) = cancel_transmit(&cancellation);
    assert_eq!(cancel_id, id);
    engine.handle(
        Input::TransmissionFinished {
            request: cancel_id,
            transmission: cancel_tx,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(
                // Distinct full identities still collide in the lower-16
                // fallback, so this reply must remain ambiguous.
                Some(0x2222_beef),
            )),
        },
        start,
    );

    let collision = engine.handle(
        frame(
            1,
            Some(0xbeef),
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
fn sony_reply_headers_preserve_width_specific_non_unique_verdicts() {
    for (second_sequence, reply_header, expected) in [
        (0x10042, 0x10042, IgnoreReason::UnmatchedSequencedFrame),
        (0x20042, 0x42, IgnoreReason::AmbiguousLower16Sequence),
    ] {
        let now = Instant::now();
        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        for (ticket, sequence) in [(1, 0x10042), (2, second_sequence)] {
            let (effects, _) = admit(
                &mut engine,
                ticket,
                inquiry_with_retry(1, RetryPolicy::NEVER),
                now,
            );
            send_ok(&mut engine, &effects, Some(sequence), now);
        }
        let effects = engine.handle(
            frame(
                1,
                Some(reply_header),
                DecodedResponse::InquiryReply {
                    payload: smallvec![1],
                },
            ),
            now,
        );
        assert!(effects.iter().any(|effect| matches!(effect,
            Effect::Ignored(reason) if *reason == expected)));
        assert!(!effects
            .iter()
            .any(|effect| matches!(effect, Effect::Terminal { .. })));
        engine.assert_invariants().unwrap();
    }
}

#[test]
fn sony_retry_reuses_first_successful_sequence_and_ignores_stale_result() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
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
                    framing,
                    ..
                },
                ..
            }
        ] if matches!(framing.sony_sequence(), Some(SonyRequestSequence::AllocateFresh))
    ));
    engine.handle(
        Input::TransmissionFinished {
            request: id,
            transmission: first_tx,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(Some(0x1020_3040))),
        },
        start,
    );
    assert_eq!(
        engine.entry(id).and_then(|entry| entry.current_sequence()),
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
            kind: Transmission::Request { framing, .. },
            ..
        } => Some(framing.sony_sequence()),
        _ => None,
    });
    assert_eq!(retry_id, id);
    assert_ne!(retry_tx, first_tx);
    assert!(Arc::ptr_eq(&first_wire, &retry_wire));
    assert_eq!(
        requested,
        Some(Some(SonyRequestSequence::Replay(0x1020_3040)))
    );

    // The old attempt was removed when the retry became authoritative. Its
    // late result must not overwrite the request's current sequence.
    let stale = finish_write_input_only(
        &mut engine,
        id,
        first_tx,
        Ok(TransmissionMeta::for_sequence(Some(0xdead_beef))),
        retry_ready,
    );
    assert!(stale
        .iter()
        .any(|effect| matches!(effect, Effect::Ignored(IgnoreReason::StaleTransmission))));
    assert_eq!(
        engine.entry(id).and_then(|entry| entry.current_sequence()),
        Some(0x1020_3040)
    );

    send_ok(&mut engine, &retry, Some(0x1020_3040), retry_ready);

    // A new logical request starts with no requested sequence after the prior
    // retry was written.
    let next = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1),
        },
        retry_ready,
    );
    let next_requested = next.iter().find_map(|effect| match effect {
        Effect::Transmit {
            kind: Transmission::Request { framing, .. },
            ..
        } => Some(framing.sony_sequence()),
        _ => None,
    });
    assert_eq!(
        next_requested,
        Some(Some(SonyRequestSequence::AllocateFresh))
    );
    engine.assert_invariants().unwrap();
}

#[test]
fn sony_stale_command_errors_do_not_spend_retry_during_backoff_or_ready() {
    let start = Instant::now();
    let mut engine = engine_with_target(
        EnvelopeKind::Sony,
        TransportKind::Datagram,
        CommandSocketCount::ONE,
        CancellationPolicy::Supported,
    );
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
        },
        start,
    );
    let (first_tx, id, _) = request_transmit(&first);
    let first_sequence = 0x1020_3040;
    engine.handle(
        Input::TransmissionFinished {
            request: id,
            transmission: first_tx,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(Some(first_sequence))),
        },
        start,
    );

    // The first busy response is current and schedules exactly one retry.
    let retry_error = engine.handle(
        frame(
            1,
            Some(first_sequence),
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
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
    assert_eq!(engine.entry(id).unwrap().retry_count(), 1);
    assert_eq!(engine.next_wake(start), Some(retry_ready));

    // The old attempt's duplicate is still routable by sequence, but must be
    // inert while the request is in backoff.
    let stale_backoff = engine.handle(
        frame(
            1,
            Some(first_sequence),
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
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
    assert_eq!(engine.entry(id).unwrap().retry_count(), 1);
    assert_eq!(engine.next_wake(start), Some(retry_ready));

    // Hold the sole command socket with another request so the retry reaches
    // Ready at its original deadline. This makes the second stale phase
    // observable instead of dispatching immediately.
    let blocker = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1),
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
            Some(blocker_sequence),
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
            from: Lifecycle::Backoff { ready_at, .. },
            to: Lifecycle::Ready { .. },
            ..
        } if *seen == id && *ready_at == retry_ready
    )));
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::Ready)
    ));
    assert!(request_transmit_optional(&promoted).is_none());

    // A duplicate retryable error and a stale permanent error are both inert
    // while the retry is queued. In particular, neither changes its attempt
    // budget nor terminalizes the request.
    let ready_before = (
        engine.entry(id).unwrap().retry_count(),
        engine.next_wake(start),
    );
    let stale_ready = engine.handle(
        frame(
            1,
            Some(first_sequence),
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
            },
        ),
        retry_ready + Duration::from_millis(1),
    );
    let stale_permanent = engine.handle(
        frame(
            1,
            Some(first_sequence),
            DecodedResponse::Error {
                socket: None,
                code: 0x01.into(),
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
        (
            engine.entry(id).unwrap().retry_count(),
            engine.next_wake(start)
        ),
        ready_before
    );

    // Once the blocker completes, the request dispatches once and reuses the
    // sequence from the original attempt; stale errors did not move its retry.
    let released = engine.handle(
        frame(
            1,
            Some(blocker_sequence),
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
            kind: Transmission::Request { framing, .. },
            ..
        } if *request == id => Some(framing.sony_sequence()),
        _ => None,
    });
    assert_eq!(
        requested_sequence,
        Some(Some(SonyRequestSequence::Replay(first_sequence)))
    );
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
    assert_eq!(engine.entry(id).unwrap().retry_count(), 1);
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::AwaitingAck { .. })
    ));
    assert!(engine.entry(blocker_id).is_none());
    engine.assert_invariants().unwrap();
}

/// Issue #803: the inquiry cooldown is pacing, encoded once in
/// `candidate_send_at`. A ready inquiry is neither written nor woken for
/// before the cooldown ends, and is written at exactly that instant.
#[test]
fn an_inquiry_is_not_dispatched_before_the_inquiry_cooldown_ends() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let cooldown_end = start + Duration::from_millis(25);
    engine.pacing.inquiry_cooldown_until = Some(cooldown_end);
    let (queued, id) = admit(&mut engine, 1, inquiry(1), start);
    assert!(request_transmit_optional(&queued).is_none());
    assert_eq!(engine.next_wake(start), Some(cooldown_end));
    let early = engine.advance(cooldown_end - Duration::from_millis(1));
    assert!(request_transmit_optional(&early).is_none());
    let due = engine.advance(cooldown_end);
    assert_eq!(request_transmit(&due).1, id);
    engine.assert_invariants().unwrap();
}

#[test]
fn sony_stale_inquiry_errors_do_not_spend_retry_during_backoff_or_ready() {
    let start = Instant::now();
    let mut configured = policy(EnvelopeKind::Sony, TransportKind::Datagram);
    configured.inquiry_cooldown = Duration::ZERO;
    configured.inquiry_spacing = Duration::from_millis(10);
    let mut engine = ProtocolEngine::new(
        configured,
        target_policies(&[camera(1)], TargetPolicy::test_default()),
        Instant::now(),
    )
    .unwrap();

    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: inquiry(1),
        },
        start,
    );
    let (_, id, _) = request_transmit(&first);
    let first_sequence = 0x3040_5060;
    send_ok(&mut engine, &first, Some(first_sequence), start);

    // Queue another inquiry before the first one retries. It will be the
    // pacing blocker that leaves the retried inquiry observable in Ready.
    let queued = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: inquiry(1),
        },
        start + Duration::from_millis(1),
    );
    assert!(request_transmit_optional(&queued).is_none());
    let retry_error = engine.handle(
        frame(
            1,
            Some(first_sequence),
            DecodedResponse::Error {
                socket: None,
                code: 0x02.into(),
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
            Some(first_sequence),
            DecodedResponse::Error {
                socket: None,
                code: 0x02.into(),
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
    assert_eq!(engine.entry(id).unwrap().retry_count(), 1);

    // The queued inquiry wins dispatch when the retry becomes ready, leaving
    // the old inquiry in Ready and preserving the original retry deadline.
    let promoted = engine.advance(retry_ready);
    let (_, queued_id, _) = request_transmit(&promoted);
    assert_eq!(queued_id, admitted(&queued));
    send_ok(&mut engine, &promoted, Some(0x4050_6070), retry_ready);
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::Ready)
    ));

    let ready_before = (
        engine.entry(id).unwrap().retry_count(),
        engine.next_wake(start),
    );
    let stale_ready = engine.handle(
        frame(
            1,
            Some(first_sequence),
            DecodedResponse::Error {
                socket: None,
                code: 0x02.into(),
            },
        ),
        retry_ready + Duration::from_millis(1),
    );
    let stale_permanent = engine.handle(
        frame(
            1,
            Some(first_sequence),
            DecodedResponse::Error {
                socket: None,
                code: 0x01.into(),
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
        (
            engine.entry(id).unwrap().retry_count(),
            engine.next_wake(start)
        ),
        ready_before
    );

    let released = engine.handle(
        frame(
            1,
            Some(0x4050_6070),
            DecodedResponse::InquiryReply {
                payload: smallvec![1],
            },
        ),
        retry_ready + Duration::from_millis(10),
    );
    let (retry_tx, dispatched, _) = request_transmit(&released);
    assert_eq!(dispatched, id);
    let requested_sequence = released.iter().find_map(|effect| match effect {
        Effect::Transmit {
            request,
            kind: Transmission::Request { framing, .. },
            ..
        } if *request == id => Some(framing.sony_sequence()),
        _ => None,
    });
    assert_eq!(
        requested_sequence,
        Some(Some(SonyRequestSequence::Replay(first_sequence)))
    );
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
            request: dispatched,
            transmission: retry_tx,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(Some(first_sequence))),
        },
        retry_ready + Duration::from_millis(10),
    );
    assert_eq!(engine.entry(id).unwrap().retry_count(), 1);
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::AwaitingReply { .. })
    ));
    assert!(engine.entry(admitted(&queued)).is_none());
    engine.assert_invariants().unwrap();
}

#[test]
fn raw_overlapping_inquiries_serialize_in_per_target_write_order() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let mut ids = Vec::new();
    for (ticket, target) in [(1, 1), (2, 1), (3, 1), (4, 2)] {
        let admitted_effects = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(ticket),
                request: inquiry(target),
            },
            start,
        );
        ids.push(admitted(&admitted_effects));
        if request_transmit_optional(&admitted_effects).is_some() {
            send_ok(&mut engine, &admitted_effects, None, start);
        } else {
            assert_eq!(target, 1);
            assert!(matches!(
                phase_of(&engine, *ids.last().unwrap()),
                Some(Phase::Ready)
            ));
        }
    }
    let first_target_one = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                payload: smallvec![9],
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&first_target_one), Some(ids[0]));
    assert_eq!(request_transmit(&first_target_one).1, ids[1]);
    send_ok(&mut engine, &first_target_one, None, start);
    let target_two_fifo = engine.handle(
        frame(
            2,
            None,
            DecodedResponse::InquiryReply {
                payload: smallvec![0],
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&target_two_fifo), Some(ids[3]));
    let second_target_one = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                payload: smallvec![1],
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&second_target_one), Some(ids[1]));
    assert_eq!(request_transmit(&second_target_one).1, ids[2]);
    send_ok(&mut engine, &second_target_one, None, start);
    let third_target_one = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                payload: smallvec![2],
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&third_target_one), Some(ids[2]));
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
                request: command(1),
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
                    code: 0x01.into(),
                },
            ),
            start,
        );
        assert_eq!(terminal_id(&resolved), Some(id));
        command_engine.assert_invariants().unwrap();
    }

    // Disjoint declarations may both be live, but a socketless error carries
    // no payload shape. It cannot choose either inquiry until data settles one.
    {
        let mut inquiry_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let first = inquiry_engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: inquiry_with_shape(1, PayloadShape::Bytes(std::num::NonZeroUsize::MIN)),
            },
            start,
        );
        let first_id = admitted(&first);
        send_ok(&mut inquiry_engine, &first, None, start);
        let second = inquiry_engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(2),
                request: inquiry_with_shape(
                    1,
                    PayloadShape::Bytes(std::num::NonZeroUsize::new(2).unwrap()),
                ),
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
                    code: 0x01.into(),
                },
            ),
            start,
        );
        assert_eq!(
            ignored_reasons(&first_error),
            [IgnoreReason::UnmatchedFrame]
        );
        assert!(terminal_outcome(&first_error, first_id).is_none());
        assert!(terminal_outcome(&first_error, second_id).is_none());
        let data = inquiry_engine.handle(
            frame(
                1,
                None,
                DecodedResponse::InquiryReply {
                    payload: smallvec![1],
                },
            ),
            start,
        );
        assert_eq!(terminal_id(&data), Some(first_id));
        let second_error = inquiry_engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Error {
                    socket: None,
                    code: 0x01.into(),
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
                request: inquiry(1),
            },
            start,
        );
        let inquiry_id = admitted(&inquiry_admission);
        send_ok(&mut collision_engine, &inquiry_admission, None, start);
        let command_admission = collision_engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(2),
                request: command(1),
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
                    code: 0x03.into(),
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
                to: Lifecycle::Executing { .. },
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

    // A named socket that no live request owns and no exact-socket hold
    // covers is the socket the camera allocated for the command it rejected
    // (the PTZOptics G2 answers `90 6y 41 FF` naming its next free socket).
    // Like an ACK naming a free socket, it is exact evidence for the target's
    // unique unacknowledged command when no inquiry is live. The ambiguous
    // forms stay inert: see `tests/raw_evidence.rs`.
    {
        let mut unowned_socket_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let admission = unowned_socket_engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: command(1),
            },
            start,
        );
        let id = admitted(&admission);
        send_ok(&mut unowned_socket_engine, &admission, None, start);
        let rejected = unowned_socket_engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Error {
                    socket: Some(ViscaSocket::S2),
                    code: 0x01.into(),
                },
            ),
            start,
        );
        assert!(matches!(
            terminal_failure(&rejected, id),
            Some(Error::MessageLengthError)
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
                request: command(1),
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
                    code: 0x01.into(),
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
                    code: 0x01.into(),
                },
            ),
            start,
        );
        assert_eq!(terminal_id(&owned), Some(id));
        executing_engine.assert_invariants().unwrap();
    }
}

#[test]
fn response_at_exact_deadline_wins_and_equal_deadlines_use_request_order() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
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
            request: inquiry_with_retry(1, RetryPolicy::NEVER),
        },
        start + Duration::from_millis(20),
    );
    let a_id = admitted(&a);
    send_ok(&mut engine, &a, None, start + Duration::from_millis(20));
    let b = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: inquiry_with_retry(2, RetryPolicy::NEVER),
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
        let sequence = (envelope == EnvelopeKind::Sony).then_some(0x1001);
        let transmission_sequence = sequence;

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
                        CommandReply::AckThenCompletion,
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
                    inquiry_with_retry(1, RetryPolicy::NEVER),
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
                    Some(RuntimeOutcome::Inquiry(InquiryOutcome::Reply { .. }))
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
                        CommandReply::AckThenCompletion,
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
                    Some(RuntimeOutcome::Command(CommandOutcome::Applied))
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
                        CommandReply::CompletionOnly,
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
                    Some(RuntimeOutcome::Command(CommandOutcome::Applied))
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
                    CommandReply::AckThenCompletion,
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
                    Some(sequence),
                    DecodedResponse::Completion { socket: None },
                ),
            )
        },
        DeadlineKind::Ack,
        |engine, id, effects| {
            assert!(engine.entry(id).is_none());
            assert!(matches!(
                terminal_outcome(effects, id),
                Some(RuntimeOutcome::Command(CommandOutcome::Applied))
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
        command_with_reply_shape_and_retry(1, CommandReply::AckThenCompletion, RetryPolicy::NEVER),
        start,
    );
    send_ok(&mut engine, &overdue_admission, None, start);

    let (reply_admission, reply_id) = admit(
        &mut engine,
        2,
        inquiry_with_retry(2, RetryPolicy::NEVER),
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
                payload: smallvec![1],
            },
        ),
        start + Duration::from_millis(20) + Duration::from_nanos(1),
    );
    assert!(ignored_reasons(&effects).is_empty());
    assert!(matches!(
        terminal_outcome(&effects, reply_id),
        Some(RuntimeOutcome::Inquiry(InquiryOutcome::Reply { .. }))
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
                    CommandReply::AckThenCompletion,
                    RetryPolicy::NEVER,
                ),
                start,
            );
            let request_sequence = (envelope == EnvelopeKind::Sony).then_some(0x3001);
            send_ok(&mut engine, &admission, request_sequence, start);
            engine.handle(
                frame(
                    1,
                    request_sequence,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                start,
            );
            let cancellation = engine.handle(engine.cancel_input(id), start);
            let (transmission, _, _) = cancel_transmit(&cancellation);
            let cancellation_sequence = (envelope == EnvelopeKind::Sony).then_some(0x3002);
            engine.handle(
                Input::TransmissionFinished {
                    request: id,
                    transmission,
                    result: WriteOutcome::Sent(TransmissionMeta::for_sequence(
                        cancellation_sequence,
                    )),
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
                    cancellation_sequence,
                    DecodedResponse::Error {
                        socket: Some(ViscaSocket::S1),
                        code: 0x04.into(),
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
            Some(RuntimeOutcome::Command(CommandOutcome::Cancelled))
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
        assert!(
            terminal < ignored,
            "retire the expired owner before accounting its late frame"
        );
        late_engine.assert_invariants().unwrap();
    }
}

#[test]
fn executing_response_outlives_cancellation_ambiguity_until_its_completion_deadline() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let mut request =
        command_with_reply_shape_and_retry(1, CommandReply::AckThenCompletion, RetryPolicy::NEVER);
    let RuntimeRequest::Command { context, .. } = &mut request else {
        unreachable!("test constructs a command");
    };
    context.timeout.completion = NonZeroDuration::new(Duration::from_secs(1)).unwrap();
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
    let cancellation = engine.handle(engine.cancel_input(id), start + Duration::from_millis(1));
    let (cancel_transmission, _, _) = cancel_transmit(&cancellation);
    let ambiguity_deadline = match engine.entry(id).map(Entry::cancellation) {
        Some(CancelState::Sending {
            ambiguity_deadline, ..
        }) => ambiguity_deadline,
        cancellation => panic!("expected a sent cancellation, got {cancellation:?}"),
    };
    engine.handle(
        Input::TransmissionFinished {
            request: id,
            transmission: cancel_transmission,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
        },
        start + Duration::from_millis(1),
    );
    assert!(
        engine
            .raw_correlation_releases_due(ambiguity_deadline)
            .is_empty(),
        "cancellation ambiguity cannot release a still-correlated response"
    );
    assert!(matches!(
        phase_of(&engine, id),
        Some(Phase::AwaitingCancellationResolution { deadline, .. })
            if deadline == start + Duration::from_secs(1)
    ));

    let ambiguity_passed = engine.advance(ambiguity_deadline + Duration::from_nanos(1));
    assert!(!ambiguity_passed
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { id: seen, .. } if *seen == id)));
    assert_eq!(engine.socket_owner(camera(1), ViscaSocket::S1), Some(id));

    let completed = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(60),
    );
    assert!(ignored_reasons(&completed).is_empty());
    assert!(matches!(
        terminal_outcome(&completed, id),
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
    ));
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
            request: command(1),
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

    // B's ACK deadline is the same instant. Its cancellation intent stays
    // pending through the rest of the read.
    let b_started = start + Duration::from_millis(20);
    let b = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1),
        },
        b_started,
    );
    let b_id = admitted(&b);
    send_ok(&mut engine, &b, None, b_started);
    let recorded = engine.handle(
        engine.cancel_input(b_id),
        deadline - Duration::from_millis(1),
    );
    assert!(recorded
        .iter()
        .any(|effect| matches!(effect, Effect::CancellationRecorded { id } if *id == b_id)));

    let turn = engine.begin_input_turn(deadline);
    let mut ordered_effects = Vec::new();

    // Wire frame 1: ACK(B). The socket is known but the write has to wait
    // until every available reply has been applied.
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
    assert!(cancel_transmit_optional(&ack_b).is_none());
    assert_eq!(
        engine
            .entry(b_id)
            .and_then(|entry| entry.lifecycle().pending_cancellation_socket()),
        Some(ViscaSocket::S2),
    );
    ordered_effects.extend(ack_b);
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
    let finished = engine.finish_input_turn(turn, EngineTurn::Complete);
    let (cancel_tx, cancel_id, cancel_socket) = cancel_transmit(&finished);
    assert_eq!(cancel_id, b_id);
    assert_eq!(cancel_socket, ViscaSocket::S2);
    ordered_effects.extend(finished);

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
                    outcome: RuntimeOutcome::Command(CommandOutcome::Applied),
                } if *id == a_id
            )
        })
        .expect("A applied terminal");
    assert!(applied_index < cancel_index);
    assert!(ordered_effects.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id,
            outcome: RuntimeOutcome::Command(CommandOutcome::Applied),
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
            outcome: RuntimeOutcome::Command(CommandOutcome::Failed(_)),
        } if *id == a_id
    )));
    assert!(engine.entry(a_id).is_none());

    engine.handle(
        Input::TransmissionFinished {
            request: b_id,
            transmission: cancel_tx,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
        },
        deadline,
    );
    assert!(matches!(
        engine.entry(b_id).map(Entry::cancellation),
        Some(CancelState::AwaitingTerminal { .. })
    ));
    engine.assert_invariants().unwrap();
}

#[test]
fn engine_turn_options_share_one_input_tail_without_implicit_dispatch() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);

    let admitted_only = engine.handle_turn(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
        },
        start,
        EngineTurn::InputOnly,
    );
    let id = admitted(&admitted_only);
    assert!(request_transmit_optional(&admitted_only).is_none());
    assert!(matches!(phase_of(&engine, id), Some(Phase::Ready)));

    let complete = engine.advance_turn(start, EngineTurn::Complete);
    assert_eq!(request_transmit(&complete).1, id);
    engine.assert_invariants().unwrap();
}

fn engine_with_target(
    envelope: EnvelopeKind,
    transport: TransportKind,
    sockets: CommandSocketCount,
    cancellation: CancellationPolicy,
) -> ProtocolEngine {
    ProtocolEngine::new(
        policy(envelope, transport),
        target_policies(
            &[camera(1)],
            TargetPolicy {
                command_sockets: sockets,
                physical_command_sockets: sockets,
                cancellation,
                ..TargetPolicy::test_default()
            },
        ),
        Instant::now(),
    )
    .unwrap()
}

#[test]
fn unsupported_target_cancels_queued_locally_but_sent_without_intent() {
    let start = Instant::now();
    let mut engine = engine_with_target(
        EnvelopeKind::Raw,
        TransportKind::Datagram,
        CommandSocketCount::ONE,
        CancellationPolicy::Unsupported,
    );
    let active = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
        },
        start,
    );
    let active_id = admitted(&active);
    send_ok(&mut engine, &active, None, start);
    let queued = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1),
        },
        start,
    );
    let queued_id = admitted(&queued);
    assert!(request_transmit_optional(&queued).is_none());
    let local = engine.handle(engine.cancel_input(queued_id), start);
    assert!(local
        .iter()
        .any(|effect| matches!(effect, Effect::CancellationRecorded { id } if *id == queued_id)));
    assert!(local.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id,
            outcome: RuntimeOutcome::Command(CommandOutcome::Cancelled)
        } if *id == queued_id
    )));
    assert!(!local.iter().any(|effect| matches!(
        effect,
        Effect::Transmit {
            kind: Transmission::Cancel { .. },
            ..
        }
    )));

    let unsupported = engine.handle(engine.cancel_input(active_id), start);
    assert!(unsupported.iter().any(|effect| matches!(
        effect,
        Effect::CancellationObservation {
            id,
            observation: CancellationObservation::AttemptFailed(Error::CancellationNotSupported)
        } if *id == active_id
    )));
    assert_eq!(
        engine.entry(active_id).unwrap().cancellation(),
        CancelState::None
    );
    // Issue #671/#723: the lost raw ACK fails this request immediately and
    // leaves only its positional correlation hold behind. The session remains
    // usable and a raw command is never replayed.
    let timeout = engine.advance(start + Duration::from_millis(20));
    assert_eq!(engine.state(), SessionState::Running);
    assert!(engine.entry(active_id).is_none());
    assert!(matches!(
        terminal_failure(&timeout, active_id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert!(!timeout
        .iter()
        .any(|effect| matches!(effect, Effect::RetryScheduled { .. })));
    let ambiguity_deadline = start + Duration::from_millis(70);
    assert_eq!(
        engine
            .raw_hold(camera(1), RawHoldScope::PreAck)
            .map(|hold| (hold.until, hold.owner)),
        Some((ambiguity_deadline, Some(active_id)))
    );
    assert_eq!(engine.next_wake(start), Some(ambiguity_deadline));
    let resolved = engine.advance(ambiguity_deadline);
    assert!(terminal_outcome(&resolved, active_id).is_none());
    assert!(engine.correlation.holds().is_empty());
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
fn pre_ack_cancellation_records_intent_then_emits_one_cancel_on_ack() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let mut request = command(1);
    if let RuntimeRequest::Command { applied_state, .. } = &mut request {
        *applied_state = Some(AppliedStateProjection::set(
            crate::KnownState::ExposureSpotlight(true),
        ));
    }
    let admitted_effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request,
        },
        start,
    );
    let (request_tx, id, _) = request_transmit(&admitted_effects);
    engine.handle(
        Input::TransmissionFinished {
            request: id,
            transmission: request_tx,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(Some(0x1001))),
        },
        start,
    );
    let recorded = engine.handle(engine.cancel_input(id), start);
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
    let ack = engine.handle(
        frame(
            1,
            Some(0x1001),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(1),
    );
    let (cancel_tx, cancel_id, socket) = cancel_transmit(&ack);
    assert_eq!(cancel_id, id);
    assert_eq!(socket, ViscaSocket::S1);
    engine.handle(
        Input::TransmissionFinished {
            request: cancel_id,
            transmission: cancel_tx,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(Some(0x2001))),
        },
        start + Duration::from_millis(2),
    );
    let duplicate = engine.handle(
        frame(
            1,
            Some(0x1001),
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
    let cancelled = engine.handle(
        frame(
            1,
            Some(0x2001),
            DecodedResponse::Error {
                socket: Some(ViscaSocket::S1),
                code: 0x04.into(),
            },
        ),
        start + Duration::from_millis(3),
    );
    assert!(cancelled.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id: seen,
            outcome: RuntimeOutcome::Command(CommandOutcome::Cancelled)
        } if *seen == id
    )));
    assert!(cancelled.iter().any(|effect| matches!(
        effect,
        Effect::AppliedState {
            effect: AppliedStateEffect {
                projection: AppliedStateProjection::Invalidate {
                    key: crate::StateKey::ExposureSpotlight
                },
                ..
            }
        }
    )));
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
            request: command(1),
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
    let cancellation = engine.handle(engine.cancel_input(id), before_spacing);
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
    assert_eq!(
        engine.next_wake(start),
        Some(start + Duration::from_millis(10))
    );
    engine.assert_invariants().unwrap();
}

#[test]
fn pending_cancellation_transmits_before_ordinary_work_and_advances_spacing() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
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
    let (second, second_id) = admit(&mut engine, 2, command(2), start);
    send_ok(&mut engine, &second, None, start);
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

    engine.policy.command_spacing = Duration::from_millis(10);
    let queued = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: command(1),
        },
        start + Duration::from_millis(1),
    );
    let queued_id = admitted(&queued);
    assert!(request_transmit_optional(&queued).is_none());
    for id in [second_id, first_id] {
        let requested = engine.handle(engine.cancel_input(id), start + Duration::from_millis(1));
        assert!(cancel_transmit_optional(&requested).is_none());
    }

    let first_eligible = start + Duration::from_millis(10);
    let cancellation = engine.advance(first_eligible);
    assert_eq!(cancel_transmit(&cancellation).1, first_id);
    assert!(request_transmit_optional(&cancellation).is_none());
    assert_eq!(
        cancellation
            .iter()
            .filter(|effect| matches!(effect, Effect::Transmit { .. }))
            .count(),
        1
    );
    assert_eq!(engine.pacing.last_request(), Some(start));
    assert!(matches!(
        engine.entry(queued_id).map(Entry::phase),
        Some(Phase::Ready)
    ));
    let (transmission, request, _) = cancel_transmit(&cancellation);
    let first_written = first_eligible + Duration::from_millis(4);
    let first_finished = engine.handle(
        Input::TransmissionFinished {
            request,
            transmission,
            result: WriteOutcome::Sent(TransmissionMeta::Known),
        },
        first_written,
    );
    assert!(cancel_transmit_optional(&first_finished).is_none());
    assert!(request_transmit_optional(&first_finished).is_none());
    assert_eq!(engine.pacing.last_request(), Some(first_written));

    let second_eligible = first_written + Duration::from_millis(10);
    let early = engine.advance(second_eligible - Duration::from_nanos(1));
    assert!(cancel_transmit_optional(&early).is_none());
    assert!(request_transmit_optional(&early).is_none());
    let second_cancel = engine.advance(second_eligible);
    let (transmission, request, _) = cancel_transmit(&second_cancel);
    assert_eq!(request, second_id);
    assert!(request_transmit_optional(&second_cancel).is_none());
    let second_written = second_eligible + Duration::from_millis(2);
    let second_finished = engine.handle(
        Input::TransmissionFinished {
            request,
            transmission,
            result: WriteOutcome::Failed(Error::io_timeout()),
        },
        second_written,
    );
    assert!(request_transmit_optional(&second_finished).is_none());
    assert_eq!(engine.pacing.last_request(), Some(second_written));

    let ordinary = engine.advance(second_written + Duration::from_millis(10));
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
fn zero_spacing_drains_pending_cancellations_in_request_order() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
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
            request: command(1),
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
            request: command(2),
        },
        start + Duration::from_millis(1),
    );
    let ordinary_id = admitted(&ordinary);
    assert!(request_transmit_optional(&ordinary).is_none());
    assert!(cancel_transmit_optional(&engine.handle(
        engine.cancel_input(first_id),
        start + Duration::from_millis(1),
    ))
    .is_none());
    assert!(cancel_transmit_optional(&engine.handle(
        engine.cancel_input(second_id),
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
    assert!(request_transmit_optional(&drained).is_none());
    assert!(engine.assert_input_boundary().is_err());
    let mut ordinary = None;
    for (index, effect) in drained
        .into_iter()
        .filter(|effect| matches!(effect, Effect::Transmit { .. }))
        .enumerate()
    {
        let Effect::Transmit {
            transmission,
            request,
            ..
        } = effect
        else {
            unreachable!("filtered transmission")
        };
        let finished = engine.handle(
            Input::TransmissionFinished {
                request,
                transmission,
                result: WriteOutcome::Sent(TransmissionMeta::Known),
            },
            start + Duration::from_millis(1),
        );
        if index == 0 {
            assert!(finished
                .iter()
                .all(|effect| !matches!(effect, Effect::Transmit { .. })));
            assert!(
                engine.assert_input_boundary().is_err(),
                "second cancellation still awaits its receipt"
            );
        } else {
            assert_eq!(request_transmit(&finished).1, ordinary_id);
            ordinary = Some(finished);
        }
    }
    send_ok(
        &mut engine,
        &ordinary.expect("dispatch after the final cancellation receipt"),
        None,
        start + Duration::from_millis(1),
    );
    assert!(engine.assert_input_boundary().is_ok());
    engine.assert_invariants().unwrap();
}

/// #1021: a cancellation recorded as a retry matures keeps wire priority.
/// Once staged, its write receipt must arrive before dispatching the retry.
#[test]
fn zero_spacing_cancel_with_a_matured_retry_preserves_the_write_boundary() {
    for (transport, finish_turn) in [TransportKind::Stream, TransportKind::Datagram]
        .into_iter()
        .flat_map(|transport| {
            [EngineTurn::Complete, EngineTurn::InputOnly].map(|turn| (transport, turn))
        })
    {
        let start = Instant::now();
        let mut engine = engine(EnvelopeKind::Raw, transport);
        let (first, executing) = admit(&mut engine, 1, command(1), start);
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
        let (second, retry) = admit(&mut engine, 2, command(1), start);
        send_ok(&mut engine, &second, None, start);
        let rejected = engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Error {
                    socket: None,
                    code: 0x03.into(),
                },
            ),
            start,
        );
        let ready_at = rejected
            .iter()
            .find_map(|effect| match effect {
                Effect::RetryScheduled { id, ready_at, .. } if *id == retry => Some(*ready_at),
                _ => None,
            })
            .expect("buffer-full retry");
        let cancelled = engine.handle(engine.cancel_input(executing), ready_at);
        let (transmission, request, _) = cancel_transmit(&cancelled);
        assert_eq!(request, executing);
        assert_eq!(
            cancelled
                .iter()
                .filter(|effect| matches!(effect, Effect::Transmit { .. }))
                .count(),
            1
        );
        assert!(
            engine.assert_input_boundary().is_err(),
            "a cancellation write is a staged write"
        );
        assert!(
            engine.advance(ready_at).is_empty(),
            "a wake cannot run due work across the write boundary"
        );
        assert!(matches!(
            engine.entry(retry).unwrap().lifecycle(),
            Lifecycle::Ready { .. }
        ));
        let mut finished = engine.handle_turn(
            Input::TransmissionFinished {
                request,
                transmission,
                result: WriteOutcome::Sent(TransmissionMeta::Known),
            },
            ready_at,
            finish_turn,
        );
        if matches!(finish_turn, EngineTurn::InputOnly) {
            assert!(finished
                .iter()
                .all(|effect| !matches!(effect, Effect::Transmit { .. })));
            assert!(engine.assert_input_boundary().is_ok());
            assert!(engine.next_wake(ready_at).is_some_and(|at| at <= ready_at));
            finished.extend(engine.advance(ready_at));
        }
        assert_eq!(request_transmit(&finished).1, retry);
        assert_eq!(
            finished
                .iter()
                .filter(|effect| matches!(effect, Effect::Transmit { .. }))
                .count(),
            1
        );
        send_ok(&mut engine, &finished, None, ready_at);
        assert!(engine.assert_input_boundary().is_ok());
        engine.assert_invariants().unwrap();
    }
}

#[test]
fn executing_cancel_waits_for_pacing_without_expiring_at_ambiguity() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    engine.policy.command_spacing = Duration::from_millis(100);
    let mut request_context = context(1);
    request_context.timeout.completion = NonZeroDuration::new(Duration::from_secs(1)).unwrap();
    let effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: RuntimeRequest::Command {
                not_executable: true,
                reply_shape: CommandReply::AckThenCompletion,
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
        request_transmit_optional(&engine.handle(engine.cancel_input(id), requested_at,)).is_none()
    );
    let ambiguity_deadline = start + Duration::from_millis(51);
    let pacing_deadline = start + Duration::from_millis(100);
    assert_eq!(engine.next_wake(start), Some(pacing_deadline));

    let ambiguity_passed = engine.advance(ambiguity_deadline);
    assert!(!ambiguity_passed.iter().any(|effect| matches!(
        effect,
        Effect::Transmit {
            kind: Transmission::Cancel { .. },
            ..
        }
    )));
    assert!(!ambiguity_passed
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { id: seen, .. } if *seen == id)));
    assert!(matches!(
        phase_of(&engine, id),
        Some(Phase::Executing { .. })
    ));

    let paced = engine.advance(pacing_deadline);
    assert_eq!(cancel_transmit(&paced).1, id);
    assert!(!paced
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { id: seen, .. } if *seen == id)));
    assert_eq!(engine.state(), SessionState::Running);
    engine.assert_invariants().unwrap();
}

#[test]
fn cancel_after_raw_ack_timeout_cannot_resurrect_the_terminal_owner() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let request = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
        },
        start,
    );
    let id = admitted(&request);
    let cancel = engine.cancel_input(id);
    send_ok(&mut engine, &request, None, start);

    // The ordinary raw ACK timeout terminates the request and retains only a
    // bounded, inert positional hold.
    let original_deadline = start + Duration::from_millis(70);
    let timed_out = engine.advance(start + Duration::from_millis(20));
    assert!(matches!(
        terminal_failure(&timed_out, id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert!(phase_of(&engine, id).is_none());
    assert_eq!(
        engine
            .raw_hold(camera(1), RawHoldScope::PreAck)
            .map(|hold| (hold.until, hold.owner)),
        Some((original_deadline, Some(id)))
    );

    // Cancellation cannot address or extend a request which has already
    // received its terminal outcome.
    let cancelled = engine.handle(cancel, start + Duration::from_millis(30));
    assert_eq!(
        ignored_reasons(&cancelled),
        vec![IgnoreReason::UnknownRequest]
    );
    assert_eq!(engine.next_wake(start), Some(original_deadline));

    // A late ACK during the hold is inert and cannot cause a cancellation write
    // or recreate socket ownership.
    let late_ack = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(60),
    );
    assert_eq!(
        ignored_reasons(&late_ack),
        vec![IgnoreReason::UnmatchedFrame]
    );
    assert!(cancel_transmit_optional(&late_ack).is_none());
    assert!(engine.socket_owner(camera(1), ViscaSocket::S1).is_none());
    engine.advance(original_deadline);
    assert!(engine.correlation.holds().is_empty());
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
            request: command(1),
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
        request_transmit_optional(&engine.handle(engine.cancel_input(id), requested_at,)).is_none()
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
            outcome: RuntimeOutcome::Command(CommandOutcome::Applied),
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
        CommandSocketCount::ONE,
        CancellationPolicy::Supported,
    );
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
        },
        start,
    );
    let (tx, id, _) = request_transmit(&first);
    engine.handle(
        Input::TransmissionFinished {
            request: id,
            transmission: tx,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(Some(0x1010))),
        },
        start,
    );
    engine.handle(engine.cancel_input(id), start);
    let ack_timeout = engine.advance(start + Duration::from_millis(20));
    assert!(terminal_outcome(&ack_timeout, id).is_none());
    assert!(deadline_expiries(&ack_timeout, id).is_empty());
    // The original ACK deadline stays in its one lifecycle field. The
    // effective cancellation window requires no intermediate re-arm.
    assert_eq!(
        engine.next_wake(start),
        Some(start + Duration::from_millis(50))
    );
    let queued = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1),
        },
        start + Duration::from_millis(21),
    );
    assert!(request_transmit_optional(&queued).is_none());
    let completed_at_boundary = engine.handle(
        frame(
            1,
            Some(0x1010),
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
            outcome: RuntimeOutcome::Command(CommandOutcome::Applied)
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
        CommandSocketCount::ONE,
        CancellationPolicy::Supported,
    );
    let effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
        },
        start,
    );
    let (tx, id, _) = request_transmit(&effects);
    engine.handle(
        Input::TransmissionFinished {
            request: id,
            transmission: tx,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(Some(0xabcd))),
        },
        start,
    );
    engine.handle(engine.cancel_input(id), start);
    engine.advance(start + Duration::from_millis(20));
    assert_eq!(engine.active_len(), 1);
    let expired = engine.advance(start + Duration::from_millis(50));
    assert!(expired.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id: seen,
            outcome: RuntimeOutcome::Command(CommandOutcome::Failed(Error::CancellationUnconfirmed))
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
            request: command(1),
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
    let cancellation = engine.handle(engine.cancel_input(id), start + Duration::from_millis(2));
    let (cancel_tx, _, _) = cancel_transmit(&cancellation);
    let failed = engine.handle(
        Input::TransmissionFinished {
            request: id,
            transmission: cancel_tx,
            result: WriteOutcome::Failed(Error::transport_error(std::io::Error::other(
                "cancel write failed",
            ))),
        },
        start + Duration::from_millis(2),
    );
    assert!(failed.iter().any(|effect| matches!(
        effect,
        Effect::CancellationObservation {
            id: seen,
            observation: CancellationObservation::AttemptFailed(Error::TransportError { .. })
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
            outcome: RuntimeOutcome::Command(CommandOutcome::Applied)
        } if *seen == id
    )));
    assert!(!completion
        .iter()
        .any(|effect| matches!(effect, Effect::CancellationObservation { .. })));
    engine.assert_invariants().unwrap();
}

#[test]
fn stream_cancel_failure_poisons_and_terminalizes_in_request_order() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
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
            request: command(2),
        },
        start,
    );
    let second_id = admitted(&second);
    send_ok(&mut engine, &second, None, start);
    let cancellation = engine.handle(engine.cancel_input(first_id), start);
    let (cancel_tx, _, _) = cancel_transmit(&cancellation);
    let poison = engine.handle(
        Input::TransmissionFinished {
            request: first_id,
            transmission: cancel_tx,
            result: WriteOutcome::Failed(Error::io_timeout()),
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
                    outcome: RuntimeOutcome::Command(CommandOutcome::Failed(
                        Error::StreamPoisoned { .. }
                    )),
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
    let second = datagram_engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: inquiry(2),
        },
        start,
    );
    let second_id = admitted(&second);
    send_ok(&mut datagram_engine, &second, None, start);
    let first = datagram_engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
        },
        start,
    );
    let first_id = admitted(&first);
    let (first_tx, _, _) = request_transmit(&first);
    let isolated = datagram_engine.handle(
        Input::TransmissionFinished {
            request: first_id,
            transmission: first_tx,
            result: WriteOutcome::Failed(Error::transport_error(std::io::Error::other(
                "datagram write",
            ))),
        },
        start,
    );
    assert_eq!(terminal_id(&isolated), Some(first_id));
    assert!(datagram_engine.entry(second_id).is_some());
    let closed = datagram_engine.handle(
        Input::Shutdown(ShutdownReason::TransportClosed {
            cause: Some(Arc::new(Error::from(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "peer closed",
            )))),
        }),
        start,
    );
    assert_eq!(datagram_engine.state(), SessionState::Closed);
    assert!(closed.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id,
            outcome: RuntimeOutcome::Inquiry(InquiryOutcome::Failed(Error::InquiryFailed { source }))
        } if *id == second_id && matches!(source.as_ref(), Error::ConnectionClosed { .. })
    )));

    let mut fresh = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let pending = fresh.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: inquiry(1),
        },
        start,
    );
    let pending_id = admitted(&pending);
    send_ok(&mut fresh, &pending, None, start);
    let shutdown = fresh.handle(Input::Shutdown(ShutdownReason::Explicit), start);
    assert_eq!(fresh.state(), SessionState::Shutdown);
    assert!(shutdown.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id,
            outcome: RuntimeOutcome::Inquiry(InquiryOutcome::Failed(Error::InquiryFailed { source }))
        } if *id == pending_id && matches!(source.as_ref(), Error::RuntimeShutdown)
    )));
    fresh.assert_invariants().unwrap();
}

#[test]
fn applied_state_projection_is_target_qualified_and_emitted_only_on_applied() {
    let start = Instant::now();
    let projection = AppliedStateProjection::set(crate::KnownState::PanTiltLimit(
        crate::PanTiltLimitUpdate::set(
            crate::command::PanTiltLimitCorner::UpRight,
            crate::types::PanTiltPosition::from_degrees(1.0, 2.0).unwrap(),
        ),
    ));
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let request = RuntimeRequest::Command {
        not_executable: true,
        reply_shape: CommandReply::AckThenCompletion,
        wire: wire(0x82),
        context: context(2),
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
fn applied_state_actions_are_closed_and_uncertain_failure_invalidates() {
    use crate::StateKey;

    let start = Instant::now();
    let projections = [
        AppliedStateProjection::set(crate::KnownState::ExposureSpotlight(true)),
        AppliedStateProjection::set(crate::KnownState::PanTiltLimit(
            crate::PanTiltLimitUpdate::clear(crate::command::PanTiltLimitCorner::UpRight),
        )),
        AppliedStateProjection::invalidate(StateKey::TallyMode),
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
                    not_executable: true,
                    reply_shape: CommandReply::AckThenCompletion,
                    wire: wire(0x81),
                    context: context(1),
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

    let mut context = context(1);
    context.retry = RetryPolicy::NEVER;
    let failed = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(9),
            request: RuntimeRequest::Command {
                not_executable: true,
                reply_shape: CommandReply::AckThenCompletion,
                wire: wire(0x81),
                context,
                applied_state: Some(AppliedStateProjection::set(
                    crate::KnownState::ExposureSpotlight(false),
                )),
            },
        },
        start + Duration::from_millis(153),
    );
    let (transmission, id, _) = request_transmit(&failed);
    let failed = engine.handle(
        Input::TransmissionFinished {
            request: id,
            transmission,
            result: WriteOutcome::Failed(Error::transport_error(std::io::Error::other(
                "request write",
            ))),
        },
        start + Duration::from_millis(153),
    );
    assert!(failed.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id: terminal,
            outcome: RuntimeOutcome::Command(CommandOutcome::Failed(_)),
        } if *terminal == id
    )));
    assert!(failed.iter().any(|effect| matches!(
        effect,
        Effect::AppliedState {
            effect: AppliedStateEffect {
                projection: AppliedStateProjection::Invalidate {
                    key: StateKey::ExposureSpotlight
                },
                ..
            }
        }
    )));
    engine.assert_invariants().unwrap();
}

#[test]
fn dispatch_is_priority_fifo_in_request_order_across_lanes() {
    let start = Instant::now();
    let mut configured = policy(EnvelopeKind::Raw, TransportKind::Datagram);
    configured.command_spacing = Duration::from_millis(10);
    let mut engine = ProtocolEngine::new(
        configured,
        target_policies(&[camera(1), camera(2)], TargetPolicy::test_default()),
        Instant::now(),
    )
    .unwrap();
    let seed = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
        },
        start,
    );
    send_ok(&mut engine, &seed, None, start);
    let mut background_command = command(2);
    let RuntimeRequest::Command {
        context: command_context,
        ..
    } = &mut background_command
    else {
        unreachable!("command helper always constructs a command");
    };
    command_context.control.class = ControlClass::Background;
    let queued_command = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: background_command,
        },
        start,
    );
    let command_id = admitted(&queued_command);
    let mut user_inquiry = inquiry(2);
    let RuntimeRequest::Inquiry {
        context: inquiry_context,
        ..
    } = &mut user_inquiry
    else {
        unreachable!("inquiry helper always constructs an inquiry");
    };
    inquiry_context.control.class = ControlClass::User;
    let inquiry = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: user_inquiry,
        },
        start,
    );
    let inquiry_id = admitted(&inquiry);
    assert!(request_transmit_optional(&queued_command).is_none());
    assert!(request_transmit_optional(&inquiry).is_none());
    let selected = engine.advance(start + Duration::from_millis(10));
    assert_eq!(request_transmit(&selected).1, inquiry_id);
    assert_ne!(request_transmit(&selected).1, command_id);

    // FIFO among commands of the same explicit default priority.
    let mut fifo = engine_with_target(
        EnvelopeKind::Raw,
        TransportKind::Datagram,
        CommandSocketCount::ONE,
        CancellationPolicy::Supported,
    );
    let occupying = fifo.handle(
        Input::Admit {
            ticket: AdmissionTicket(10),
            request: command(1),
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
            request: command(1),
        },
        start,
    );
    let first_id = admitted(&first_queued);
    let second_queued = fifo.handle(
        Input::Admit {
            ticket: AdmissionTicket(12),
            request: command(1),
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
    let mut engine = ProtocolEngine::new(
        configured,
        target_policies(&[camera(1), camera(2)], TargetPolicy::test_default()),
        Instant::now(),
    )
    .unwrap();

    let seed = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
        },
        start,
    );
    send_ok(&mut engine, &seed, None, start);
    let queued_command = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(2),
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
                request: inquiry(2),
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
            Some(Phase::Ready)
        ));
    }
    engine.assert_invariants().unwrap();
}

#[test]
fn retune_to_one_socket_allows_preexisting_second_command_to_drain_via_named_reuse() {
    let start = Instant::now();
    let mut engine = engine_with_target(
        EnvelopeKind::Raw,
        TransportKind::Datagram,
        CommandSocketCount::TWO,
        CancellationPolicy::Supported,
    );
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
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
            request: command(1),
        },
        start,
    );
    let second_id = admitted(&second);
    send_ok(&mut engine, &second, None, start);
    assert_eq!(
        engine
            .entry(second_id)
            .unwrap()
            .dispatched_socket_capacity(),
        Some(CommandSocketCount::TWO)
    );

    let mut retuned_sockets = PerTarget::default();
    retuned_sockets[camera(1)] = Some(CommandSocketCount::ONE);
    engine
        .retune(
            Duration::ZERO,
            Duration::ZERO,
            retuned_sockets,
            Instant::now(),
        )
        .unwrap();
    assert_eq!(engine.command_sockets(camera(1)), CommandSocketCount::ONE);

    // New work observes the reduced concurrency limit immediately.
    let third = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: command(1),
        },
        start + Duration::from_millis(1),
    );
    let third_id = admitted(&third);
    assert!(request_transmit_optional(&third).is_none());

    // The camera reuses the busy first socket in its ACK. Even though the
    // second attempt was dispatched under the old two-socket capacity, raw
    // ownership follows that named assignment and quarantines the stale first
    // owner (#721).
    let reused = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(2),
    );
    assert!(!reused
        .iter()
        .any(|effect| matches!(effect, Effect::Ignored(IgnoreReason::SocketConflict))));
    assert!(matches!(
        phase_of(&engine, second_id),
        Some(Phase::Executing {
            socket: ViscaSocket::S1,
            ..
        })
    ));
    assert_eq!(
        engine.socket_owner(camera(1), ViscaSocket::S1),
        Some(second_id)
    );
    assert!(matches!(
        terminal_failure(&reused, first_id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert!(phase_of(&engine, first_id).is_none());
    let first_quarantine_deadline = engine
        .raw_hold(camera(1), RawHoldScope::PreAck)
        .expect("displaced owner hold")
        .until;

    // The named completion belongs to the second request. The lower retuned
    // limit and the displaced predecessor's unkeyed hold keep new work queued
    // until that hold expires.
    let second_complete = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(3),
    );
    assert_eq!(terminal_id(&second_complete), Some(second_id));
    assert!(request_transmit_optional(&second_complete).is_none());
    assert!(matches!(phase_of(&engine, third_id), Some(Phase::Ready)));

    let released = engine.advance(first_quarantine_deadline);
    assert!(terminal_outcome(&released, first_id).is_none());
    assert_eq!(
        request_transmit(&released).1,
        third_id,
        "retuned one-socket work dispatches after the stale hold releases"
    );
    assert_eq!(
        engine.entry(third_id).unwrap().dispatched_socket_capacity(),
        Some(CommandSocketCount::ONE)
    );
    engine.assert_invariants().unwrap();
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "engine invariant violated")]
fn retune_runs_the_production_debug_invariant_hook() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (admission, id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CommandReply::CompletionOnly),
        start,
    );
    send_ok(&mut engine, &admission, None, start);
    assert!(engine.entry(id).is_some());
    admit(&mut engine, 2, inquiry(1), start);
    engine.policy.capacity = std::num::NonZeroUsize::MIN;

    engine
        .retune(
            Duration::ZERO,
            Duration::ZERO,
            PerTarget::default(),
            Instant::now(),
        )
        .unwrap();
}

#[test]
fn raw_inquiry_retry_releases_fifo_and_requeues_at_tail_with_same_wire() {
    let start = Instant::now();
    let configured = policy(EnvelopeKind::Raw, TransportKind::Datagram);
    let mut engine = ProtocolEngine::new(
        configured,
        target_policies(&[camera(1)], TargetPolicy::test_default()),
        Instant::now(),
    )
    .unwrap();
    let first_wire = wire(0x81);
    let first_request = RuntimeRequest::Inquiry {
        syntax: true,
        payload_shape: PayloadShape::Unknown,
        wire: Arc::clone(&first_wire),
        context: context(1),
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
            request: inquiry(1),
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
                code: 0x02.into(),
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
                payload: smallvec![1],
            },
        ),
        start + Duration::from_millis(50),
    );
    assert!(matches!(
        terminal_outcome(&second_reply, second_id),
        Some(RuntimeOutcome::Inquiry(InquiryOutcome::Reply { payload, .. })) if payload.as_slice() == [1]
    ));
    // A matched reply cannot produce a late duplicate, so B's success releases
    // the FIFO-tail retry immediately without adding another hold (#712).
    let (retry_tx, retried_id, retried_wire) = request_transmit(&second_reply);
    assert_eq!(retried_id, first_id);
    assert!(Arc::ptr_eq(&first_wire, &retried_wire));
    engine.handle(
        Input::TransmissionFinished {
            request: retried_id,
            transmission: retry_tx,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
        },
        start + Duration::from_millis(50),
    );
    assert_eq!(
        engine.raw_inquiry_matching(camera(1), &[0x0b]),
        Some(first_id)
    );

    let legitimate_retry = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                payload: smallvec![0x0b],
            },
        ),
        start + Duration::from_millis(50),
    );
    assert!(matches!(
        terminal_outcome(&legitimate_retry, first_id),
        Some(RuntimeOutcome::Inquiry(InquiryOutcome::Reply { payload, .. })) if payload.as_slice() == [0x0b]
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
                request: command(1),
            },
            start,
        );
        output.extend(admitted_effects.iter().map(|effect| format!("{effect:?}")));
        let (tx, id, _) = request_transmit(&admitted_effects);
        let sent = engine.handle(
            Input::TransmissionFinished {
                request: id,
                transmission: tx,
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(Some(0x1234))),
            },
            start,
        );
        output.extend(sent.iter().map(|effect| format!("{effect:?}")));
        let ack = engine.handle(
            frame(
                1,
                Some(0x1234),
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
                Some(0x1234),
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
        include_str!("../../../testdata/issue_542/protocol/correlation-routing.trace");

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
                    let created = ProtocolEngine::new(
                        policy(record_envelope, TransportKind::Datagram),
                        target_policies(
                            &[camera(1), camera(2), camera(3)],
                            TargetPolicy::test_default(),
                        ),
                        Instant::now(),
                    )
                    .unwrap();
                    engine = Some(created);
                    envelope = Some(record_envelope);
                }
                assert_eq!(envelope, Some(record_envelope));
                ticket += 1;
                let target = fixture_required(&fields, "target").parse::<u8>().unwrap();
                let mut fixture_context = context(target);
                fixture_context.retry = RetryPolicy::NEVER;
                let request = match fixture_required(&fields, "kind") {
                    "command" => RuntimeRequest::Command {
                        not_executable: true,
                        reply_shape: CommandReply::AckThenCompletion,
                        wire: wire(0x80 | target),
                        context: fixture_context,
                        applied_state: None,
                    },
                    "inquiry" => RuntimeRequest::Inquiry {
                        syntax: true,
                        payload_shape: fixture_shape(&fields),
                        wire: wire(0x80 | target),
                        context: fixture_context,
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
                        request: id,
                        transmission,
                        result: WriteOutcome::Sent(TransmissionMeta::for_sequence(sequence)),
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
                let sequence = fields.get("sequence").map(|value| {
                    let number = fixture_u32(value);
                    FrameSequence::from_reply_header(
                        if fields.get("sequence-width").map(String::as_str) == Some("lower16") {
                            number & 0xFFFF
                        } else {
                            number
                        },
                    )
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
                        payload: SmallVec::from_slice(
                            fixture_required(&fields, "payload").as_bytes(),
                        ),
                    },
                    "error" => DecodedResponse::Error {
                        socket: fields.get("socket").map(|_| fixture_socket(&fields)),
                        code: (fixture_u32(fixture_required(&fields, "code")) as u8).into(),
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

fn fixture_shape(fields: &BTreeMap<String, String>) -> PayloadShape {
    match fixture_required(fields, "shape") {
        "unknown" => PayloadShape::Unknown,
        declaration => {
            let (kind, length) = declaration.split_once(':').expect("declared payload shape");
            let length = length
                .parse::<std::num::NonZeroUsize>()
                .expect("positive payload length");
            match kind {
                "bytes" => PayloadShape::Bytes(length),
                "nibbles" => PayloadShape::Nibbles(length),
                other => panic!("unknown payload shape {other}"),
            }
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
    ViscaSocket::from_wire(fixture_required(fields, "socket").parse::<u8>().unwrap()).unwrap()
}

/// Recovers the VISCA error code the engine's terminal error stands for.
///
/// This is the inverse of [`Error::from_code`], so an engine that collapses
/// distinct camera error codes onto one error renders the wrong code and the
/// fixture's expectation column rejects it.
fn fixture_error_code(error: &Error) -> String {
    let code = match error {
        Error::InquiryFailed { source } => return fixture_error_code(source),
        Error::MessageLengthError => 0x01,
        Error::SyntaxError => 0x02,
        Error::CommandBufferFull => 0x03,
        Error::CommandCancelled => 0x04,
        Error::NoSocket => 0x05,
        Error::CommandNotExecutable => 0x41,
        Error::Unknown { code } => *code,
        other => panic!("a camera error frame produced a non-camera error: {other:?}"),
    };
    format!("0x{code:02X}")
}

/// Every column below is read out of the engine's own effects. Nothing is
/// rebuilt from the fixture's input record, so corrupting a reply payload,
/// an attributed socket or an error code fails the expectation.
fn fixture_observation(
    engine: &ProtocolEngine,
    effects: &[Effect],
    names: &BTreeMap<RequestId, String>,
    response: &str,
) -> String {
    if let Some((id, socket)) = effects.iter().find_map(|effect| match effect {
        Effect::Transition {
            id,
            to: Lifecycle::Executing { socket, .. },
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
            target.camera().value(),
            socket.to_wire()
        );
    }
    if let Some((id, outcome)) = effects.iter().find_map(|effect| match effect {
        Effect::Terminal { id, outcome } => Some((*id, outcome)),
        _ => None,
    }) {
        let name = names.get(&id).unwrap();
        let outcome = match outcome {
            RuntimeOutcome::Command(CommandOutcome::Written) => "written".to_owned(),
            RuntimeOutcome::Command(CommandOutcome::Applied) => "applied".to_owned(),
            RuntimeOutcome::Inquiry(InquiryOutcome::Reply { payload }) => {
                format!("reply:{}", String::from_utf8_lossy(payload))
            }
            // #795: an error after the request's ACK keeps the camera's
            // exact code but is reported as unconfirmed.
            RuntimeOutcome::Command(CommandOutcome::Failed(Error::CommandFailedAfterAck {
                source,
                ..
            })) => {
                format!("error-after-ack:{}", fixture_error_code(source))
            }
            RuntimeOutcome::Command(CommandOutcome::Failed(error))
            | RuntimeOutcome::Inquiry(InquiryOutcome::Failed(error)) => {
                format!("error:{}", fixture_error_code(error))
            }
            RuntimeOutcome::Command(CommandOutcome::Cancelled) => "cancelled".to_owned(),
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

/// Every materially distinct session shape used by the invariant generators.
///
/// Both envelopes and transports, plus strict raw-command poisoning. Raw
/// inquiry serialization follows payload overlap, independently of admission.
#[derive(Clone, Copy, Debug)]
struct FuzzConfiguration {
    envelope: EnvelopeKind,
    transport: TransportKind,
    strict_unconfirmed_poison: bool,
}

const FUZZ_CONFIGURATIONS: [FuzzConfiguration; 5] = [
    FuzzConfiguration {
        envelope: EnvelopeKind::Raw,
        transport: TransportKind::Datagram,
        strict_unconfirmed_poison: false,
    },
    FuzzConfiguration {
        envelope: EnvelopeKind::Raw,
        transport: TransportKind::Stream,
        strict_unconfirmed_poison: false,
    },
    FuzzConfiguration {
        envelope: EnvelopeKind::Sony,
        transport: TransportKind::Datagram,
        strict_unconfirmed_poison: false,
    },
    FuzzConfiguration {
        envelope: EnvelopeKind::Sony,
        transport: TransportKind::Stream,
        strict_unconfirmed_poison: false,
    },
    FuzzConfiguration {
        envelope: EnvelopeKind::Raw,
        transport: TransportKind::Datagram,
        strict_unconfirmed_poison: true,
    },
];

fn active_transmissions(
    engine: &ProtocolEngine,
) -> impl Iterator<Item = (TransmissionId, TransmissionOwner)> {
    let mut owners: Vec<_> = engine
        .entries
        .active_entries()
        .filter_map(|(id, entry)| {
            let transmission = match entry.lifecycle() {
                Lifecycle::Sending { transmission, .. } => transmission,
                lifecycle => lifecycle.sending_cancellation()?.transmission,
            };
            engine
                .transmission_owner(*id, transmission)
                .map(|owner| (transmission, owner))
        })
        .collect();
    owners.sort_unstable_by_key(|(transmission, _)| *transmission);
    owners.into_iter()
}

fn due_effects(engine: &mut ProtocolEngine, now: Instant) -> Vec<Effect> {
    let mut sink = Vec::new();
    let mut decision = Decision::new(&mut sink);
    engine.run_due(now, &mut decision);
    engine.finish_decision(now, decision);
    sink
}

fn fuzz_engine(configuration: FuzzConfiguration) -> ProtocolEngine {
    let mut configured = policy(configuration.envelope, configuration.transport);
    configured.strict_unconfirmed_poison = configuration.strict_unconfirmed_poison;
    ProtocolEngine::new(
        configured,
        target_policies(&[camera(1), camera(2)], TargetPolicy::test_default()),
        Instant::now(),
    )
    .unwrap()
}

/// A receive-side fault the owner has already classified as transient.
fn fuzz_receive_fault(action: u64) -> Error {
    match action % 3 {
        0 => Error::io_timeout(),
        1 => Error::from(std::io::Error::from(std::io::ErrorKind::WouldBlock)),
        _ => Error::transport_error(std::io::Error::other("fuzz receive fault")),
    }
}

/// A frame the engine can actually attribute, drawn from one live entry.
///
/// Purely arbitrary frames almost never match a request's current phase, so a
/// generator built only from them audits the inert paths and very little else.
/// This branch keeps the same determinism — the entry is chosen by index into
/// the ordered entry map, the socket and sequence are read back out of that
/// entry — while driving real request lifecycles through ACK, completion,
/// camera error and inquiry reply. The write result always precedes another
/// input, as it does in both production owners (#860).
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
            !matches!(entry.phase(), Phase::Ready | Phase::Backoff { .. })
                && (engine.policy.wire.kind() == EnvelopeKind::Raw
                    || entry.current_sequence().is_some())
        })?;
    let other = if action & 0x8000 == 0 {
        ViscaSocket::S1
    } else {
        ViscaSocket::S2
    };
    let response = match entry.phase() {
        Phase::Sending { .. } | Phase::AwaitingAck { .. } => DecodedResponse::Ack {
            socket: (action & 0x200 == 0).then_some(other),
        },
        Phase::Executing { socket, .. } | Phase::AwaitingCancellationResolution { socket, .. } => {
            if action & 0x4000 == 0 {
                DecodedResponse::Completion {
                    socket: (action & 0x200 == 0).then_some(socket),
                }
            } else {
                DecodedResponse::Error {
                    socket: Some(socket),
                    code: [0x01, 0x02, 0x03, 0x04, 0x41][(action as usize) % 5].into(),
                }
            }
        }
        Phase::ExecutingUnassigned { .. }
        | Phase::ExecutingDisplaced { .. }
        | Phase::AwaitingCompletion { .. } => DecodedResponse::Completion { socket: None },
        Phase::AwaitingReply { .. } => DecodedResponse::InquiryReply {
            payload: smallvec![action as u8],
        },
        Phase::Ready | Phase::Backoff { .. } => return None,
    };
    Some(Input::Frame(DecodedFrame {
        target: entry.request.context().target,
        // Sony correlation is by envelope sequence, so a directed frame quotes
        // the sequence the request actually registered — at either width.
        sequence: entry.sequences.records().next_back().map(|record| {
            FrameSequence::from_reply_header(if action & 0x400 == 0 {
                record.sequence
            } else {
                record.sequence & 0xFFFF
            })
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
    let previous_write = engine.pacing.last_request();
    let target = ((action >> 8) & 1) as u8 + 1;
    let socket = if action & 0x100 == 0 {
        ViscaSocket::S1
    } else {
        ViscaSocket::S2
    };
    // The socket nibble is genuinely optional on the wire: a camera may answer
    // `90 40 FF` / `90 50 FF` with no socket at all.
    let named_socket = (action & 0x200 == 0).then_some(socket);
    // Full-width generated headers have a nonzero upper half, as on the wire;
    // the other headers exercise production's potentially truncated path.
    let sequence_value =
        ((action >> 24) & 0xffff) as u32 | if action & 0x400 == 0 { 0x10000 } else { 0 };
    let sequence = (envelope == EnvelopeKind::Sony).then_some(sequence_value);
    let write_sequence = (envelope == EnvelopeKind::Sony).then_some(sequence_value);
    let effects = match action % 12 {
        0 => {
            *ticket = ticket.wrapping_add(1);
            // An urgent STOP is never blocked by a latched raw stream lane
            // (#795), so the generator keeps writing even then.
            let request = match (action >> 16) & 0x3 {
                _ if (action >> 40) & 0x7 == 0 => urgent_command(target),
                0 => command(target),
                1 => inquiry(target),
                2 => command_with_reply_shape(target, CommandReply::CompletionOnly),
                _ => command_with_reply_shape(target, CommandReply::NoReply),
            };
            if engine.active_len() >= engine.policy.capacity.get() {
                engine.advance(*now)
            } else {
                engine.handle(
                    Input::Admit {
                        ticket: AdmissionTicket(*ticket),
                        request,
                    },
                    *now,
                )
            }
        }
        1 => {
            let Some((transmission, owner)) = active_transmissions(engine).next() else {
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
                Err(Error::io_timeout())
            } else {
                Ok(TransmissionMeta::for_sequence(write_sequence))
            };
            engine.handle(
                Input::TransmissionFinished {
                    request: owner.request,
                    transmission,
                    result: write_outcome(result),
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
                    code: [0x01, 0x02, 0x03, 0x04, 0x41][(action as usize) % 5].into(),
                },
                _ => DecodedResponse::InquiryReply {
                    payload: smallvec![action as u8],
                },
            };
            engine.handle(frame(target, sequence, response), *now)
        }
        3 => {
            let Some(id) = engine
                .entries
                .iter()
                .find(|(_, entry)| entry.request.lane() == Lane::Command)
                .map(|(id, _)| *id)
            else {
                return Vec::new();
            };
            engine.handle(engine.cancel_input(id), *now)
        }
        4 => engine.advance(*now),
        5 => {
            let stale = TransmissionId::from_nonzero(
                NonZeroU64::new(action | 1).expect("an odd value is never zero"),
            );
            engine.handle(
                Input::TransmissionFinished {
                    request: RequestId::from_nonzero(NonZeroU64::new(action | 1).unwrap()),
                    transmission: stale,
                    result: WriteOutcome::Sent(TransmissionMeta::for_sequence(write_sequence)),
                },
                *now,
            )
        }
        6 => {
            let Some(wake) = engine.next_wake(*now) else {
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
            let mismatched = sequence.is_none().then_some(sequence_value);
            engine.handle(frame(target, mismatched, DecodedResponse::Unknown), *now)
        }
    };
    let mut pending: VecDeque<_> = effects.into();
    let mut recorded = Vec::new();
    while let Some(effect) = pending.pop_front() {
        if let Effect::Transmit {
            transmission,
            request,
            ..
        } = &effect
        {
            let sequence = engine
                .transmission_owner(*request, *transmission)
                .and_then(|owner| {
                    engine
                        .entries
                        .get(request)
                        .and_then(|entry| entry.sequences.get(owner.write.kind()))
                })
                .or(write_sequence);
            let fails = match transport {
                TransportKind::Datagram => action & 0x2_0000 == 0,
                TransportKind::Stream => terminal_faults && (action >> 33).is_multiple_of(8),
            };
            let result = if fails {
                Err(Error::io_timeout())
            } else {
                Ok(TransmissionMeta::for_sequence(sequence))
            };
            let finished = engine.handle_turn(
                Input::TransmissionFinished {
                    request: *request,
                    transmission: *transmission,
                    result: write_outcome(result),
                },
                *now,
                EngineTurn::InputOnly,
            );
            for produced in finished.into_iter().rev() {
                pending.push_front(produced);
            }
        }
        recorded.push(effect);
    }
    let latest_write = if recorded
        .iter()
        .any(|effect| matches!(effect, Effect::Transmit { .. }))
    {
        Some(*now)
    } else {
        previous_write
    };
    assert_eq!(engine.pacing.last_request(), latest_write);
    recorded
}

/// What a fuzz run actually reached, so the generators cannot quietly decay
/// into a stream of inert frames while still "passing".
#[derive(Debug, Default)]
struct FuzzCoverage {
    acknowledged: usize,
    applied: usize,
    replied: usize,
    written: usize,
    retried: usize,
    cancelled: usize,
    ignored: usize,
    completion_only: usize,
    quarantined: usize,
    tombstoned: usize,
    single_flight_blocked: usize,
    strict_poisoned: usize,
    /// The directed socket-reuse seed reached the strict displacement path.
    strict_displacements: usize,
    /// Stream writes that failed: the step that poisoned a default-mode
    /// stream session, which nothing else the generator does can poison.
    failed_stream_writes: usize,
}

impl FuzzCoverage {
    fn record(&mut self, engine: &ProtocolEngine, effects: &[Effect]) {
        for effect in effects {
            match effect {
                Effect::Transition {
                    to: Lifecycle::Executing { .. },
                    ..
                } => self.acknowledged += 1,
                Effect::Transition {
                    to: Lifecycle::AwaitingCompletion { .. },
                    ..
                } => self.completion_only += 1,
                Effect::Terminal {
                    outcome:
                        RuntimeOutcome::Command(CommandOutcome::Failed(
                            Error::UnsequencedCommandUnconfirmed,
                        )),
                    ..
                } => self.quarantined += 1,
                Effect::Terminal {
                    outcome: RuntimeOutcome::Command(CommandOutcome::Applied),
                    ..
                } => self.applied += 1,
                Effect::Terminal {
                    outcome: RuntimeOutcome::Inquiry(InquiryOutcome::Reply { .. }),
                    ..
                } => self.replied += 1,
                Effect::Terminal {
                    outcome: RuntimeOutcome::Command(CommandOutcome::Cancelled),
                    ..
                } => self.cancelled += 1,
                Effect::Terminal {
                    outcome: RuntimeOutcome::Command(CommandOutcome::Written),
                    ..
                } => self.written += 1,
                Effect::RetryScheduled { .. } => self.retried += 1,
                Effect::Ignored(_) => self.ignored += 1,
                Effect::SessionEnded(SessionEnd {
                    status: EndStatus::Poisoned,
                    ..
                }) if engine.policy.strict_unconfirmed_poison => self.strict_poisoned += 1,
                _ => {}
            }
        }
        if engine.policy.strict_unconfirmed_poison && engine.mode().is_raw() {
            assert_eq!(
                self.quarantined, 0,
                "a strict raw session emitted a survivable unconfirmed terminal"
            );
        }
        if !engine.correlation.holds().is_empty() {
            self.tombstoned += 1;
        }
        if engine.mode().is_raw()
            && engine.entries.values().any(|entry| {
                entry.request.is_inquiry()
                    && matches!(entry.phase(), Phase::Ready)
                    && engine.inquiries_inflight_for(entry.request.context().target) == 1
            })
        {
            self.single_flight_blocked += 1;
        }
    }
}

fn fuzz_coverage_floor_met(configuration: FuzzConfiguration, coverage: &FuzzCoverage) -> bool {
    coverage.acknowledged > 0
        && coverage.applied > 0
        && coverage.replied > 0
        && coverage.written > 0
        && coverage.retried > 0
        && coverage.cancelled > 0
        && coverage.ignored > 0
        && coverage.completion_only > 0
        && (configuration.envelope != EnvelopeKind::Raw || coverage.tombstoned > 0)
        && (configuration.envelope != EnvelopeKind::Raw
            || configuration.strict_unconfirmed_poison
            || coverage.quarantined > 0)
        && (configuration.envelope != EnvelopeKind::Raw || coverage.single_flight_blocked > 0)
        && (!configuration.strict_unconfirmed_poison
            || (coverage.strict_poisoned > 0 && coverage.strict_displacements > 0))
}

/// Seeds every generated session with real traffic through each required state
/// family before the stale/reordered stream begins. Raw retries use a conclusive
/// camera rejection, while Sony retries carry and reuse an envelope sequence.
/// Reply-shape, quarantine, tombstone, single-flight, and strict-policy seeds
/// prevent generator drift from silently deleting the newer #700/#712/#671
/// state space (#724).
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
    coverage.record(engine, &effects);
    effects
}

fn seed_fuzz_coverage(
    engine: &mut ProtocolEngine,
    configuration: FuzzConfiguration,
    ticket: &mut u64,
    now: &mut Instant,
    coverage: &mut FuzzCoverage,
) {
    let envelope = configuration.envelope;
    let sequence = |value| (envelope == EnvelopeKind::Sony).then_some(value);
    let write_sequence = |value| (envelope == EnvelopeKind::Sony).then_some(value);

    // A normal command reaches Applied, exercising ACK and completion
    // attribution for both envelopes.
    let admitted_effects = seed_admit(engine, ticket, coverage, command(1), *now);
    let (transmission, id, _) = request_transmit(&admitted_effects);
    let sent = engine.handle(
        Input::TransmissionFinished {
            request: id,
            transmission,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(write_sequence(0x5101))),
        },
        *now,
    );
    coverage.record(engine, &sent);
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
    coverage.record(engine, &acknowledged);
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
    coverage.record(engine, &completed);
    assert!(completed.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id: terminal,
            outcome: RuntimeOutcome::Command(CommandOutcome::Applied),
        } if *terminal == id
    )));
    *now += Duration::from_micros(2);

    // An inquiry reaches a reply without an ACK phase.
    let admitted_effects = seed_admit(
        engine,
        ticket,
        coverage,
        inquiry(1),
        *now + Duration::from_micros(1),
    );
    let (transmission, id, _) = request_transmit(&admitted_effects);
    let sent = engine.handle(
        Input::TransmissionFinished {
            request: id,
            transmission,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(write_sequence(0x5102))),
        },
        *now + Duration::from_micros(1),
    );
    coverage.record(engine, &sent);
    let replied = engine.handle(
        frame(
            1,
            sequence(0x5102),
            DecodedResponse::InquiryReply {
                payload: smallvec![1],
            },
        ),
        *now + Duration::from_micros(2),
    );
    coverage.record(engine, &replied);
    assert!(replied.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id: terminal,
            outcome: RuntimeOutcome::Inquiry(InquiryOutcome::Reply { .. }),
        } if *terminal == id
    )));
    *now += Duration::from_micros(2);

    // A conclusive buffer-full rejection is retryable even for raw VISCA;
    // only an ambiguous timeout after a successful send is unsequenced.
    let admitted_effects = seed_admit(engine, ticket, coverage, command(2), *now);
    let (transmission, retry_id, _) = request_transmit(&admitted_effects);
    let sent = engine.handle(
        Input::TransmissionFinished {
            request: retry_id,
            transmission,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(write_sequence(0x5103))),
        },
        *now,
    );
    coverage.record(engine, &sent);
    let rejection = engine.handle(
        frame(
            2,
            sequence(0x5103),
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
            },
        ),
        *now + Duration::from_micros(1),
    );
    coverage.record(engine, &rejection);
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
    coverage.record(engine, &retry);
    let (retry_transmission, retried, _) = request_transmit(&retry);
    assert_eq!(retried, retry_id);
    let sent = engine.handle(
        Input::TransmissionFinished {
            request: retried,
            transmission: retry_transmission,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(write_sequence(0x5103))),
        },
        retry_ready,
    );
    coverage.record(engine, &sent);
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
    coverage.record(engine, &acknowledged);
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
    coverage.record(engine, &completed);
    *now = retry_ready + Duration::from_micros(2);

    // Cancellation is a separate wire operation. A successful response keeps
    // this seed non-terminal for both the raw and Sony correlation paths.
    let admitted_effects = seed_admit(engine, ticket, coverage, command(1), *now);
    let (transmission, cancel_id, _) = request_transmit(&admitted_effects);
    let sent = engine.handle(
        Input::TransmissionFinished {
            request: cancel_id,
            transmission,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(write_sequence(0x5104))),
        },
        *now,
    );
    coverage.record(engine, &sent);
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
    coverage.record(engine, &acknowledged);
    let cancellation = engine.handle(
        engine.cancel_input(cancel_id),
        *now + Duration::from_micros(2),
    );
    coverage.record(engine, &cancellation);
    let (cancel_transmission, _, _) = cancel_transmit(&cancellation);
    let sent = engine.handle(
        Input::TransmissionFinished {
            request: cancel_id,
            transmission: cancel_transmission,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(write_sequence(0x5105))),
        },
        *now + Duration::from_micros(2),
    );
    coverage.record(engine, &sent);
    let cancellation_response = if envelope == EnvelopeKind::Raw {
        DecodedResponse::Error {
            socket: Some(ViscaSocket::S2),
            code: 0x04.into(),
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
    coverage.record(engine, &cancelled);
    assert!(
        cancelled.iter().any(|effect| matches!(
            effect,
            Effect::Terminal {
                id,
                outcome: RuntimeOutcome::Command(CommandOutcome::Cancelled),
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
    coverage.record(engine, &stale);
    assert!(stale
        .iter()
        .any(|effect| matches!(effect, Effect::Ignored(_))));

    // Completion-only reaches its socketless response phase, then completes.
    let completion_only = seed_admit(
        engine,
        ticket,
        coverage,
        command_with_reply_shape(1, CommandReply::CompletionOnly),
        *now + Duration::from_micros(1),
    );
    let (transmission, completion_only_id, _) = request_transmit(&completion_only);
    let sent = engine.handle(
        Input::TransmissionFinished {
            request: completion_only_id,
            transmission,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(write_sequence(0x5106))),
        },
        *now + Duration::from_micros(1),
    );
    coverage.record(engine, &sent);
    let completed = engine.handle(
        frame(
            1,
            sequence(0x5106),
            DecodedResponse::Completion { socket: None },
        ),
        *now + Duration::from_micros(2),
    );
    coverage.record(engine, &completed);
    assert!(matches!(
        terminal_outcome(&completed, completion_only_id),
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
    ));
    *now += Duration::from_micros(2);

    // No-reply reaches its distinct local-write terminal. On raw framing both
    // this and the preceding completion-only terminal leave a bounded target
    // tombstone; observe it before releasing it for subsequent seeds.
    let no_reply = seed_admit(
        engine,
        ticket,
        coverage,
        command_with_reply_shape(1, CommandReply::NoReply),
        *now + Duration::from_micros(1),
    );
    let (transmission, no_reply_id, _) = request_transmit(&no_reply);
    let written = engine.handle(
        Input::TransmissionFinished {
            request: no_reply_id,
            transmission,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(write_sequence(0x5107))),
        },
        *now + Duration::from_micros(1),
    );
    coverage.record(engine, &written);
    assert!(matches!(
        terminal_outcome(&written, no_reply_id),
        Some(RuntimeOutcome::Command(CommandOutcome::Written))
    ));
    *now += Duration::from_micros(1);
    if envelope == EnvelopeKind::Raw {
        let released = engine.advance(*now + Duration::from_millis(50));
        coverage.record(engine, &released);
        *now += Duration::from_millis(50);
    }

    // Unknown raw inquiries overlap: hold a second inquiry behind the first,
    // then retain its target-only tombstone after timeout.
    if envelope == EnvelopeKind::Raw {
        let first = seed_admit(
            engine,
            ticket,
            coverage,
            inquiry_with_retry(1, RetryPolicy::NEVER),
            *now,
        );
        let (first_transmission, first_id, _) = request_transmit(&first);
        let first_sent = engine.handle(
            Input::TransmissionFinished {
                request: first_id,
                transmission: first_transmission,
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
            },
            *now,
        );
        coverage.record(engine, &first_sent);
        let second = seed_admit(
            engine,
            ticket,
            coverage,
            inquiry_with_retry(1, RetryPolicy::NEVER),
            *now,
        );
        let second_id = admitted(&second);
        assert!(request_transmit_optional(&second).is_none());

        let timed_out = engine.advance(*now + Duration::from_millis(30));
        coverage.record(engine, &timed_out);
        assert!(matches!(
            terminal_cause(&timed_out, first_id),
            Some(Error::Timeout { .. })
        ));
        let mut release_at = *now + Duration::from_millis(80);
        if configuration.transport == TransportKind::Stream {
            // A stream still owes the first inquiry's reply. Where the
            // datagram skew hold would release the second inquiry (+80 ms), the
            // owed hold keeps it queued; the late reply, delivered at that
            // instant, is absorbed rather than bound to it, and only then does
            // the ordinary skew start.
            let owed = engine.handle(
                frame(
                    1,
                    None,
                    DecodedResponse::InquiryReply {
                        payload: smallvec![1],
                    },
                ),
                release_at,
            );
            coverage.record(engine, &owed);
            assert!(request_transmit_optional(&owed).is_none());
            assert!(matches!(
                engine.entry(second_id).map(Entry::phase),
                Some(Phase::Ready)
            ));
            release_at += Duration::from_millis(50);
        }
        let released = engine.advance(release_at);
        coverage.record(engine, &released);
        let (second_transmission, transmitted_id, _) = request_transmit(&released);
        assert_eq!(transmitted_id, second_id);
        let second_sent = engine.handle(
            Input::TransmissionFinished {
                request: transmitted_id,
                transmission: second_transmission,
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
            },
            release_at,
        );
        coverage.record(engine, &second_sent);
        let replied = engine.handle(
            frame(
                1,
                None,
                DecodedResponse::InquiryReply {
                    payload: smallvec![2],
                },
            ),
            release_at + Duration::from_micros(1),
        );
        coverage.record(engine, &replied);
        assert!(matches!(
            terminal_outcome(&replied, second_id),
            Some(RuntimeOutcome::Inquiry(InquiryOutcome::Reply { payload, .. })) if payload.as_slice() == [2]
        ));
        *now = release_at + Duration::from_micros(1);
    }

    // #951 directed seed: the first command's completion is lost and the
    // second command's ACK reuses its socket. No deadline or receive fault
    // precedes the evidence, so this can poison only through displacement.
    // Keep this last: a strict session is terminal after the second ACK.
    if envelope == EnvelopeKind::Raw && configuration.strict_unconfirmed_poison {
        let first = seed_admit(engine, ticket, coverage, command(2), *now);
        let sent = send_ok(engine, &first, None, *now);
        coverage.record(engine, &sent);
        let first_id = admitted(&first);
        let acked = engine.handle(
            frame(
                2,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            *now,
        );
        coverage.record(engine, &acked);
        let second = seed_admit(engine, ticket, coverage, command(2), *now);
        let sent = send_ok(engine, &second, None, *now);
        coverage.record(engine, &sent);
        let second_id = admitted(&second);
        let displaced = engine.handle(
            frame(
                2,
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            *now,
        );
        coverage.record(engine, &displaced);
        for id in [first_id, second_id] {
            assert!(matches!(terminal_failure(&displaced, id),
                Some(Error::StreamPoisoned { source })
                    if matches!(source.as_ref(), Error::UnsequencedCommandUnconfirmed)));
        }
        assert_eq!(engine.state(), SessionState::Poisoned);
        coverage.strict_displacements += 1;
        engine.assert_invariants().unwrap();
        return;
    }

    // Raw ACK loss reaches either the default immediate per-request failure plus
    // keyed hold or the strict whole-session poison branch. Keep this last for
    // strict mode, since poisoning makes all later generated inputs inert.
    if envelope == EnvelopeKind::Raw {
        let command = seed_admit(
            engine,
            ticket,
            coverage,
            command_with_reply_shape_and_retry(
                2,
                CommandReply::AckThenCompletion,
                RetryPolicy::NEVER,
            ),
            *now,
        );
        let (transmission, id, _) = request_transmit(&command);
        let sent = engine.handle(
            Input::TransmissionFinished {
                request: id,
                transmission,
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
            },
            *now,
        );
        coverage.record(engine, &sent);
        let ack_timeout = engine.advance(*now + Duration::from_millis(20));
        coverage.record(engine, &ack_timeout);
        if configuration.strict_unconfirmed_poison {
            assert_eq!(engine.state(), SessionState::Poisoned);
        } else {
            assert!(phase_of(engine, id).is_none());
            // A byte stream owes the lost ACK through its ledger (#795); a
            // datagram keeps the time-bounded `PreAck` hold.
            if configuration.transport == TransportKind::Stream {
                assert!(engine
                    .correlation
                    .ledger()
                    .entries()
                    .any(|(target, entry)| target == camera(2) && entry.is_debt()));
            } else {
                assert!(engine.raw_hold(camera(2), RawHoldScope::PreAck).is_some());
            }
            let late_ack = engine.handle(
                frame(
                    2,
                    None,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                *now + Duration::from_millis(21),
            );
            coverage.record(engine, &late_ack);
            assert_eq!(
                ignored_reasons(&late_ack),
                vec![IgnoreReason::UnmatchedFrame]
            );
            let completed = engine.handle(
                frame(
                    2,
                    None,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                *now + Duration::from_millis(22),
            );
            coverage.record(engine, &completed);
            assert_eq!(
                ignored_reasons(&completed),
                vec![IgnoreReason::UnmatchedFrame]
            );
        }
        *now += Duration::from_millis(22);
    }
    engine.assert_invariants().unwrap();
}

#[test]
fn arbitrary_stale_and_reordered_inputs_preserve_invariants() {
    for (index, configuration) in FUZZ_CONFIGURATIONS.into_iter().enumerate() {
        let envelope = configuration.envelope;
        let transport = configuration.transport;
        let start = Instant::now();
        let mut engine = fuzz_engine(configuration);
        // Seeded per configuration so the runs are not the same stream.
        let mut random = 0x5425_49ab_cdef_0123_u64 ^ (index as u64).wrapping_mul(0x9E37_79B9);
        let mut ticket = 0_u64;
        let mut now = start;
        let mut coverage = FuzzCoverage::default();
        seed_fuzz_coverage(
            &mut engine,
            configuration,
            &mut ticket,
            &mut now,
            &mut coverage,
        );
        const STEPS: u64 = 2_000;
        for step in 0..STEPS {
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            now += Duration::from_millis(1);
            let running = engine.state() == SessionState::Running;
            let effects = fuzz_step(
                &mut engine,
                envelope,
                transport,
                random,
                step * 4 >= STEPS * 3,
                &mut ticket,
                &mut now,
            );
            if transport == TransportKind::Stream
                && running
                && engine.state() == SessionState::Poisoned
            {
                coverage.failed_stream_writes += 1;
            }
            coverage.record(&engine, &effects);
        }
        assert!(
            fuzz_coverage_floor_met(configuration, &coverage),
            "{configuration:?} reached too little of the engine: {coverage:?}"
        );
        // The poisoned tail must be reached: a generator change that stops
        // failing stream writes would otherwise drop it silently.
        assert!(
            transport != TransportKind::Stream || coverage.failed_stream_writes > 0,
            "{configuration:?} never failed a stream write: {coverage:?}"
        );
        // A failed stream write poisons the session (its byte-stream position is
        // unknowable). A raw datagram's unconfirmed outcome no longer does:
        // issue #671 fails only that command and quarantines its correlation, so
        // the session survives. Datagram sessions therefore stay live in default
        // mode regardless of envelope; streams and the strict raw configuration
        // end poisoned here.
        assert_eq!(
            engine.state() == SessionState::Poisoned,
            transport == TransportKind::Stream || configuration.strict_unconfirmed_poison,
            "{configuration:?} ended in {:?}",
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
        /// frames are all in the search space (#636). The seeded coverage floor
        /// additionally makes reply-shape, quarantine/tombstone, single-flight,
        /// and strict-policy drift a property failure (#724).
        #[test]
        fn arbitrary_ordered_and_stale_inputs_preserve_invariants_property(
            configuration in 0_usize..FUZZ_CONFIGURATIONS.len(),
            actions in prop::collection::vec(any::<u64>(), 1..256)
        ) {
            let configuration = FUZZ_CONFIGURATIONS[configuration];
            let envelope = configuration.envelope;
            let transport = configuration.transport;
            let start = Instant::now();
            let mut engine = fuzz_engine(configuration);
            let mut ticket = 0_u64;
            let mut now = start;
            let mut coverage = FuzzCoverage::default();
            seed_fuzz_coverage(
                &mut engine,
                configuration,
                &mut ticket,
                &mut now,
                &mut coverage,
            );

            let steps = actions.len();
            for (step, action) in actions.into_iter().enumerate() {
                now += Duration::from_micros((action & 0x3f).saturating_add(1));
                let effects = fuzz_step(
                    &mut engine,
                    envelope,
                    transport,
                    action,
                    step * 4 >= steps * 3,
                    &mut ticket,
                    &mut now,
                );
                coverage.record(&engine, &effects);
                prop_assert!(lifecycle_transition_matrix::all_entries_have_admissible_pairs(&engine));
            }
            prop_assert!(
                fuzz_coverage_floor_met(configuration, &coverage),
                "{configuration:?} reached too little of the engine: {coverage:?}"
            );
        }
    }
}

#[test]
fn raw_late_ack_at_ambiguity_boundary_is_routed_and_cancel_remains_active() {
    let start = Instant::now();
    let mut engine = engine_with_target(
        EnvelopeKind::Raw,
        TransportKind::Datagram,
        CommandSocketCount::ONE,
        CancellationPolicy::Supported,
    );
    let admitted_effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
        },
        start,
    );
    let id = admitted(&admitted_effects);
    send_ok(&mut engine, &admitted_effects, None, start);
    engine.handle(engine.cancel_input(id), start);
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
            request: inquiry_with_retry(1, RetryPolicy::NEVER),
        },
        start,
    );
    let id = admitted(&effects);
    send_ok(&mut engine, &effects, Some(0x1111_beef), start);
    let miss = engine.handle(
        frame(
            1,
            Some(0x2222_beef),
            DecodedResponse::InquiryReply {
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
            request: command(1),
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
    let cancellation = engine.handle(engine.cancel_input(id), start + Duration::from_millis(2));
    let (cancel_tx, _, _) = cancel_transmit(&cancellation);
    engine.handle(
        Input::TransmissionFinished {
            request: id,
            transmission: cancel_tx,
            result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
        },
        start + Duration::from_millis(2),
    );
    let observer_timeout = engine.advance(start + Duration::from_millis(12));
    assert!(observer_timeout.iter().any(|effect| matches!(
        effect,
        Effect::CancellationObservation {
            id: seen,
            observation: CancellationObservation::AttemptFailed(Error::Timeout { .. })
        } if *seen == id
    )));
    assert!(engine.entry(id).is_some());
    assert_eq!(engine.socket_owner(camera(1), ViscaSocket::S1), Some(id));
    let quarantine = engine.advance(start + Duration::from_millis(52));
    assert!(quarantine.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id: seen,
            outcome: RuntimeOutcome::Command(CommandOutcome::Failed(Error::UnsequencedCommandUnconfirmed))
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
    let mut request_context = context(1);
    request_context.timeout.completion = NonZeroDuration::new(Duration::from_millis(5)).unwrap();
    request_context.timeout.ambiguity = NonZeroDuration::new(Duration::from_millis(50)).unwrap();
    let effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: RuntimeRequest::Command {
                not_executable: true,
                reply_shape: CommandReply::AckThenCompletion,
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
    let cancellation = engine.handle(engine.cancel_input(id), start + Duration::from_millis(2));
    let (cancel_tx, _, _) = cancel_transmit(&cancellation);
    engine.handle(
        Input::TransmissionFinished {
            request: id,
            transmission: cancel_tx,
            result: WriteOutcome::Failed(Error::transport_error(std::io::Error::other(
                "cancel write",
            ))),
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
fn failed_cancel_write_does_not_shorten_an_executing_commands_completion_window() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let mut request_context = context(1);
    request_context.timeout.completion = NonZeroDuration::new(Duration::from_millis(100)).unwrap();
    request_context.timeout.ambiguity = NonZeroDuration::new(Duration::from_millis(50)).unwrap();
    let (admission, id) = admit(
        &mut engine,
        1,
        RuntimeRequest::Command {
            not_executable: true,
            reply_shape: CommandReply::AckThenCompletion,
            wire: wire(0x81),
            context: request_context,
            applied_state: None,
        },
        start,
    );
    send_ok(&mut engine, &admission, None, start);
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
    let cancellation = engine.handle(engine.cancel_input(id), start + Duration::from_millis(2));
    let (cancel_tx, _, _) = cancel_transmit(&cancellation);
    let failed = engine.handle(
        Input::TransmissionFinished {
            request: id,
            transmission: cancel_tx,
            result: WriteOutcome::Failed(Error::transport_error(std::io::Error::other(
                "cancel write",
            ))),
        },
        start + Duration::from_millis(2),
    );
    assert!(failed.iter().any(|effect| matches!(
        effect,
        Effect::CancellationObservation {
            id: seen,
            observation: CancellationObservation::AttemptFailed(Error::TransportError { .. }),
        } if *seen == id
    )));

    let former_ambiguity_deadline = start + Duration::from_millis(52);
    assert_eq!(
        engine.next_wake(start),
        Some(start + Duration::from_millis(101)),
        "exact socket ownership leaves the command completion deadline in charge"
    );
    let ambiguity_passed = engine.advance(former_ambiguity_deadline);
    assert!(!ambiguity_passed
        .iter()
        .any(|effect| matches!(effect, Effect::Terminal { id: seen, .. } if *seen == id)));
    assert!(matches!(
        phase_of(&engine, id),
        Some(Phase::Executing { .. })
    ));
    assert_eq!(engine.socket_owner(camera(1), ViscaSocket::S1), Some(id));

    let completed = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(60),
    );
    assert!(matches!(
        terminal_outcome(&completed, id),
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
    ));
    assert_eq!(engine.state(), SessionState::Running);
    engine.assert_invariants().unwrap();
}

#[test]
fn retryable_rejection_after_cancel_intent_is_cancelled_without_retry() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
        },
        start,
    );
    let id = admitted(&effects);
    send_ok(&mut engine, &effects, None, start);
    engine.handle(engine.cancel_input(id), start);
    let rejected = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
            },
        ),
        start + Duration::from_millis(1),
    );
    assert!(rejected.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id: seen,
            outcome: RuntimeOutcome::Command(CommandOutcome::Cancelled)
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

    let mut custom = inquiry(1);
    let RuntimeRequest::Inquiry { syntax, .. } = &mut custom else {
        unreachable!("an inquiry fixture");
    };
    *syntax = false;
    let mut custom_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let custom_admission = custom_engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: custom,
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
                code: 0x02.into(),
            },
        ),
        start,
    );
    assert!(custom_error.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id,
            outcome: RuntimeOutcome::Inquiry(InquiryOutcome::Failed(Error::InquiryFailed { source })),
        } if *id == custom_id && matches!(source.as_ref(), Error::SyntaxError)
    )));
    assert!(!custom_error
        .iter()
        .any(|effect| matches!(effect, Effect::RetryScheduled { .. })));

    let mut builtin_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let builtin_admission = builtin_engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: inquiry_with_retry(1, retrying()),
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
                code: 0x02.into(),
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
    configured.capacity = std::num::NonZeroUsize::new(4).unwrap();
    let mut engine = ProtocolEngine::new(
        configured,
        target_policies(
            &[camera(1), camera(2)],
            TargetPolicy {
                command_sockets: CommandSocketCount::ONE,
                ..TargetPolicy::test_default()
            },
        ),
        Instant::now(),
    )
    .unwrap();
    let occupying = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
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
            request: command(1),
        },
        start,
    );
    assert!(request_transmit_optional(&blocked).is_none());
    for ticket in 3..20 {
        let queued = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(ticket),
                request: command(1),
            },
            start,
        );
        let id = admitted(&queued);
        engine.handle(engine.cancel_input(id), start);
    }
    assert_eq!(
        (0..4)
            .map(|priority| engine.entries.ready(priority).len())
            .sum::<usize>(),
        1
    );
    let other_target = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(20),
            request: command(2),
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
    retry.total_budget = Some(NonZeroDuration::new(Duration::from_millis(15)).unwrap());
    // Backoff is jittered into `[ceiling / 2, ceiling)`, so the *floor* of the
    // first attempt's band has to overshoot the budget for this test to be
    // about the budget rather than about the draw: 30ms/2 = 15ms, and the
    // error below arrives 5ms in.
    retry.initial_backoff = Duration::from_millis(30);
    let mut request_context = context(1);
    request_context.retry = retry;
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let effects = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: RuntimeRequest::Command {
                not_executable: true,
                reply_shape: CommandReply::AckThenCompletion,
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
                code: 0x03.into(),
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
// Issue #565: transport fault tolerance.
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
        Phase::Executing { socket, .. } => Some(socket),
        _ => None,
    }
}

/// A transient receive fault retries every sequenced Sony command still
/// awaiting an ACK and leaves the session running. Raw commands instead poison
/// the session because their outcomes have no sequence key, so replay is unsafe.
#[test]
fn transient_receive_fault_retries_awaiting_ack_work_and_keeps_the_session() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
        },
        start,
    );
    let first_id = admitted(&first);
    send_ok(&mut engine, &first, Some(0x2001), start);
    let second = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(2),
        },
        start,
    );
    let second_id = admitted(&second);
    send_ok(&mut engine, &second, Some(0x2002), start);

    let fault = engine.handle(
        Input::ReceiveFault {
            error: Error::Io {
                source: Arc::new(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
            },
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

/// An inquiry awaiting its reply is untouched, because the fault scan examines
/// commands only, and a request that cannot retry fails with the transport error
/// without ending the session.
#[test]
fn receive_fault_fails_only_unretryable_work_and_never_the_session() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let mut once = context(1);
    once.retry = RetryPolicy::NEVER;
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: RuntimeRequest::Command {
                not_executable: true,
                reply_shape: CommandReply::AckThenCompletion,
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
            request: inquiry(2),
        },
        start,
    );
    let inquiry_id = admitted(&inquiry_admission);
    send_ok(&mut engine, &inquiry_admission, Some(0x3002), start);

    let fault = engine.handle(
        Input::ReceiveFault {
            error: Error::transport_error(std::io::Error::other("ICMP port unreachable")),
        },
        start,
    );

    assert_eq!(engine.state(), SessionState::Running);
    assert!(fault.iter().any(|effect| matches!(
        effect,
        Effect::Terminal {
            id,
            outcome: RuntimeOutcome::Command(CommandOutcome::Failed(Error::RequestUnconfirmed { source, .. })),
        } if *id == command_id && matches!(source.as_ref(), Error::TransportError { .. })
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
    engine.handle(
        Input::Shutdown(ShutdownReason::TransportClosed { cause: None }),
        start,
    );
    let fault = engine.handle(
        Input::ReceiveFault {
            error: Error::io_timeout(),
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
fn stream_write_failure_poisons_with_the_typed_transport_cause() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
        },
        start,
    );
    let first_id = admitted(&first);
    send_ok(&mut engine, &first, None, start);
    let second = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(2),
        },
        start,
    );
    let second_id = admitted(&second);
    let (second_tx, _, _) = request_transmit(&second);

    let failure = engine.handle(
        Input::TransmissionFinished {
            request: second_id,
            transmission: second_tx,
            result: WriteOutcome::Failed(Error::transport_error(std::io::Error::other(
                "short write",
            ))),
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
        let RuntimeOutcome::Command(CommandOutcome::Failed(Error::StreamPoisoned { source })) =
            outcome
        else {
            panic!("every request on a poisoned stream reports the session verdict: {outcome:?}");
        };
        assert!(matches!(source.as_ref(), Error::TransportError { source }
            if matches!(source.as_ref(), Error::Io { source: io } if io.kind() == std::io::ErrorKind::Other)));
        let RuntimeOutcome::Command(CommandOutcome::Failed(error)) = outcome else {
            unreachable!()
        };
        assert!(std::error::Error::source(error).is_some());
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
            request: command(1),
        },
        start,
    );
    let first_id = admitted(&first);
    send_ok(&mut engine, &first, None, start);
    let second = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(2),
        },
        start,
    );
    let second_id = admitted(&second);
    let (second_tx, _, _) = request_transmit(&second);
    let failure = engine.handle(
        Input::TransmissionFinished {
            request: second_id,
            transmission: second_tx,
            result: WriteOutcome::Failed(Error::transport_error(std::io::Error::other(
                "datagram refused",
            ))),
        },
        start,
    );
    assert_eq!(terminal_id(&failure), Some(second_id));
    assert_eq!(engine.state(), SessionState::Running);
    assert!(engine.entry(first_id).is_some());
    engine.assert_invariants().unwrap();
}

/// `90 40 FF` carries no socket nibble, so the ACK binds to the first free
/// socket, the one the camera assigns next.
#[test]
fn socketless_ack_assigns_the_first_free_socket() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
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
            request: command(1),
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
            request: command(1),
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

/// A raw named-socket collision needs mutable reconciliation by the ACK path;
/// the pure assignment helper never invents the other socket. Socketless ACKs
/// retain first-free compatibility, and a fully occupied target remains inert.
#[test]
fn raw_socket_assignment_never_falls_back_from_an_occupied_named_socket() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
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
            request: command(1),
        },
        start,
    );
    let second_id = admitted(&second);
    send_ok(&mut engine, &second, None, start);

    // Socket one is taken by another request. A raw named ACK cannot silently
    // become socket two; the authoritative ACK path must displace the stale
    // owner first (#721). A socketless ACK may still select the free socket.
    assert_eq!(
        engine
            .assign_socket(camera(1), Some(ViscaSocket::S1), second_id)
            .map(SocketClaim::socket),
        None
    );
    assert_eq!(
        engine
            .assign_socket(camera(1), None, second_id)
            .map(SocketClaim::socket),
        Some(ViscaSocket::S2)
    );
    // A request keeps the socket it already owns.
    assert_eq!(
        engine
            .assign_socket(camera(1), Some(ViscaSocket::S1), first_id)
            .map(SocketClaim::socket),
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
    assert_eq!(
        engine
            .assign_socket(camera(1), None, stranger)
            .map(SocketClaim::socket),
        None
    );
    assert_eq!(
        engine
            .assign_socket(camera(1), Some(ViscaSocket::S1), stranger)
            .map(SocketClaim::socket),
        None
    );
    engine.assert_invariants().unwrap();

    // A one-socket target has no other socket to fall back to.
    let mut single = engine_with_target(
        EnvelopeKind::Raw,
        TransportKind::Datagram,
        CommandSocketCount::ONE,
        CancellationPolicy::Supported,
    );
    let only = single.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
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
    assert_eq!(
        single
            .assign_socket(camera(1), None, stranger)
            .map(SocketClaim::socket),
        None
    );
    assert_eq!(
        single
            .assign_socket(camera(1), Some(ViscaSocket::S1), stranger)
            .map(SocketClaim::socket),
        None
    );
    single.assert_invariants().unwrap();
}

/// A socketless Sony ACK cannot invent capacity when both sockets remain
/// held. Its sequence proves acceptance but names no owner to displace.
#[test]
fn sequenced_socketless_ack_with_two_live_socket_owners_fails_after_ack_without_retry() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    for (ticket, sequence, socket) in [(1, 0x1001, ViscaSocket::S1), (2, 0x1002, ViscaSocket::S2)] {
        let admitted_effects = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(ticket),
                request: command(1),
            },
            start,
        );
        send_ok(&mut engine, &admitted_effects, Some(sequence), start);
        engine.handle(
            frame(
                1,
                Some(sequence),
                DecodedResponse::Ack {
                    socket: Some(socket),
                },
            ),
            start,
        );
    }

    // Capacity correctly leaves C queued. Make its phase the narrow ACK-path
    // seam so the test can prove assignment itself still reports a conflict
    // when both physical owners are genuinely live.
    let queued = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: command(1),
        },
        start,
    );
    let third_id = admitted(&queued);
    engine
        .entries
        .transition(
            third_id,
            EntryChange::Lifecycle(UnsocketedLifecycle::AwaitingAck {
                sent_at: start,
                deadline: start + Duration::from_millis(20),
                attempt: AttemptFacts {
                    socket_capacity: Some(CommandSocketCount::TWO),
                    stream_order: None,
                },
                cancellation: None,
            }),
        )
        .unwrap();
    let mut sink = Vec::new();
    let mut decision = Decision::new(&mut sink);
    engine.ack(third_id, None, start, &mut decision);
    engine.finish_decision(start, decision);
    let effects = sink;
    assert_eq!(
        ignored_reasons(&effects),
        vec![IgnoreReason::SocketConflict]
    );
    assert!(matches!(
        terminal_failure(&effects, third_id),
        Some(Error::CommandFailedAfterAck { .. })
    ));
    assert!(engine.entry(third_id).is_none());
    engine.assert_invariants().unwrap();
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

/// Issue #721: when a raw camera reuses a socket after the older completion was
/// lost, its named ACK is authoritative. The stale local owner moves to an
/// unkeyed ambiguity quarantine; the new request owns the named socket, uses
/// it for cancellation, and receives its named completion there.
#[test]
fn raw_ack_reusing_an_occupied_socket_displaces_the_stale_owner() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
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
            request: command(1),
        },
        start,
    );
    let second_id = admitted(&second);
    send_ok(&mut engine, &second, None, start);

    // The camera names socket one for the second command after releasing and
    // reusing it. Local ownership must follow that evidence.
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
    assert!(matches!(
        terminal_failure(&reused, first_id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert!(engine.entry(first_id).is_none());
    let first_quarantine_deadline = engine
        .raw_hold(camera(1), RawHoldScope::PreAck)
        .expect("stale owner hold")
        .until;
    assert_eq!(socket_of(&engine, first_id), None);
    assert_eq!(socket_of(&engine, second_id), Some(ViscaSocket::S1));
    assert!(matches!(
        engine.entry(second_id).map(Entry::phase),
        Some(Phase::Executing { .. })
    ));

    let cancellation = engine.handle(engine.cancel_input(second_id), start);
    let (_, cancelled_id, cancel_socket) = cancel_transmit(&cancellation);
    assert_eq!(
        (cancelled_id, cancel_socket),
        (second_id, ViscaSocket::S1),
        "cancellation must name the camera-assigned socket"
    );

    let (transmission, request, _) = cancel_transmit(&cancellation);
    engine.handle(
        Input::TransmissionFinished {
            request,
            transmission,
            result: WriteOutcome::NotSent(Error::io_timeout()),
        },
        start,
    );

    // A completion on S1 now belongs to the new owner, never the displaced
    // predecessor.
    let done = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&done), Some(second_id));
    assert_eq!(engine.state(), SessionState::Running);

    let expired = engine.advance(first_quarantine_deadline);
    assert!(terminal_outcome(&expired, first_id).is_none());
    assert!(engine.correlation.holds().is_empty());
    engine.assert_invariants().unwrap();
}

/// Issue #750: an exact socket tombstone is stale local evidence just like a
/// live owner that the camera has reused. A uniquely resolved camera ACK must
/// bind its named socket, while the predecessor's original ambiguity deadline
/// remains as an unkeyed hold.
#[test]
fn raw_ack_reusing_a_quarantined_socket_downgrades_the_exact_hold() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let first = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
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

    // A's lost completion makes S1 an inert exact-socket quarantine. The
    // two-socket target still admits B, but B's camera ACK names S1 rather
    // than the locally free S2.
    let timed_out = engine.advance(start + Duration::from_millis(40));
    assert!(matches!(
        terminal_failure(&timed_out, first_id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    let hold_deadline = engine
        .raw_hold(camera(1), RawHoldScope::Socket(ViscaSocket::S1))
        .expect("timed-out first command's S1 hold")
        .until;
    assert_eq!(hold_deadline, start + Duration::from_millis(90));

    let second = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1),
        },
        start + Duration::from_millis(41),
    );
    let second_id = admitted(&second);
    send_ok(
        &mut engine,
        &second,
        None,
        start + Duration::from_millis(41),
    );
    let reused = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(42),
    );

    assert!(!reused
        .iter()
        .any(|effect| matches!(effect, Effect::Ignored(IgnoreReason::SocketConflict))));
    assert_eq!(socket_of(&engine, second_id), Some(ViscaSocket::S1));
    assert_eq!(
        engine.raw_hold(camera(1), RawHoldScope::Socket(ViscaSocket::S1)),
        None,
        "the camera-named socket must no longer be quarantined"
    );
    assert_eq!(
        engine.raw_hold(camera(1), RawHoldScope::PreAck),
        Some(RawHold {
            until: hold_deadline,
            owner: Some(first_id),
        }),
        "the predecessor's own deadline remains as unkeyed correlation protection"
    );

    // B's terminal follows the camera-assigned socket and cannot be stolen by
    // the inert predecessor. The session keeps running after both outcomes.
    let completed = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(43),
    );
    assert_eq!(terminal_id(&completed), Some(second_id));
    assert_eq!(engine.state(), SessionState::Running);
    engine.advance(hold_deadline);
    assert!(
        engine.correlation.holds().is_empty(),
        "the original hold expires on schedule"
    );
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
            request: command(1),
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
            to: Lifecycle::Executing {
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
    let now = Instant::now();
    for envelope in [EnvelopeKind::Raw, EnvelopeKind::Sony] {
        let mut engine = engine(envelope, TransportKind::Datagram);
        let (first, first_id) = admit(&mut engine, 1, command(1), now);
        let sequence = (envelope == EnvelopeKind::Sony).then_some(0x1001);
        send_ok(&mut engine, &first, sequence, now);
        let (second, second_id) = admit(&mut engine, 2, command(1), now);
        if envelope == EnvelopeKind::Sony {
            assert_eq!(request_transmit(&second).1, second_id);
            send_ok(&mut engine, &second, Some(0x1002), now);
            assert!(matches!(
                phase_of(&engine, first_id),
                Some(Phase::AwaitingAck { .. })
            ));
            assert!(matches!(
                phase_of(&engine, second_id),
                Some(Phase::AwaitingAck { .. })
            ));
        } else {
            assert!(request_transmit_optional(&second).is_none());
            let accepted = engine.handle(
                frame(
                    1,
                    None,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            );
            assert_eq!(request_transmit(&accepted).1, second_id);
            send_ok(&mut engine, &accepted, None, now);
            assert!(matches!(
                phase_of(&engine, first_id),
                Some(Phase::Executing { .. })
            ));
            assert!(matches!(
                phase_of(&engine, second_id),
                Some(Phase::AwaitingAck { .. })
            ));
        }
        engine.assert_invariants().unwrap();
    }
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
            request: command(1),
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
            request: command(1),
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
            request: command(1),
        },
        start,
    );
    let id = admitted(&admission);
    send_ok(&mut engine, &admission, Some(7), start);
    engine.handle(
        frame(1, Some(7), DecodedResponse::Ack { socket: None }),
        start,
    );
    assert_eq!(socket_of(&engine, id), Some(ViscaSocket::S1));
    let done = engine.handle(
        frame(1, Some(7), DecodedResponse::Completion { socket: None }),
        start,
    );
    assert_eq!(terminal_id(&done), Some(id));
    engine.assert_invariants().unwrap();
}

fn without_not_executable_retry(mut request: RuntimeRequest) -> RuntimeRequest {
    let RuntimeRequest::Command { not_executable, .. } = &mut request else {
        unreachable!("the fixture has the command class");
    };
    *not_executable = false;
    request
}

fn command_with_retry(target: u8, retry: RetryPolicy) -> RuntimeRequest {
    let mut command_context = context(target);
    command_context.retry = retry;
    RuntimeRequest::Command {
        not_executable: true,
        reply_shape: CommandReply::AckThenCompletion,
        wire: wire(0x80 | target),
        context: command_context,
        applied_state: None,
    }
}

fn sony_sequence(id: RequestId) -> u32 {
    0x1000 + u32::try_from(id.get()).unwrap()
}

pub(super) fn deadline_expiries(effects: &[Effect], id: RequestId) -> Vec<(DeadlineKind, bool)> {
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
        expired < ignored,
        "the due transition retires the owner before accounting the late frame"
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
            request: command(1),
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
    // Issue #671/#723: the deadline emits the per-request terminal immediately;
    // an inert keyed hold, not a live phase, retains positional correlation.
    assert!(matches!(
        terminal_failure(&expired, id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert!(engine.entry(id).is_none());
    assert_eq!(
        engine
            .raw_hold(camera(1), RawHoldScope::PreAck)
            .map(|hold| hold.until),
        Some(start + Duration::from_millis(70))
    );
    assert_eq!(engine.state(), SessionState::Running);
    let resolved = engine.advance(start + Duration::from_millis(70));
    assert!(terminal_outcome(&resolved, id).is_none());
    assert!(engine.correlation.holds().is_empty());
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
            request: command(1),
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
    // Issue #671/#723: the completion deadline reports and fails this request
    // immediately, while an inert exact-socket hold protects the successor.
    assert!(!expired
        .iter()
        .any(|effect| matches!(effect, Effect::SessionEnded(_))));
    assert!(matches!(
        terminal_failure(&expired, id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert_eq!(engine.state(), SessionState::Running);
    assert!(engine.entry(id).is_none());
    assert_eq!(
        engine
            .raw_hold(camera(1), RawHoldScope::Socket(ViscaSocket::S1))
            .map(|hold| hold.until),
        Some(start + Duration::from_millis(90))
    );
    let resolved = engine.advance(start + Duration::from_millis(90));
    assert!(terminal_outcome(&resolved, id).is_none());
    assert!(engine.correlation.holds().is_empty());
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
            request: inquiry(1),
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
/// the policy that motivated it. The policy still permits ACK-deadline
/// retries on the second expiry below; the attempt budget is what refuses it.
#[test]
fn deadline_expiry_reports_no_retry_once_the_attempt_budget_is_spent() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let policy = RetryPolicy {
        max_retries: 1,
        ..retrying()
    };
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
        total_budget: Some(NonZeroDuration::new(Duration::from_secs(600)).unwrap()),
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
    terminal_outcome(effects, id).and_then(|outcome| outcome.failure().cloned())
}

/// The original protocol/transport cause, independent of inquiry terminal facts.
fn terminal_cause(effects: &[Effect], id: RequestId) -> Option<Error> {
    terminal_failure(effects, id).map(|error| match error {
        Error::InquiryFailed { source } => source.as_ref().clone(),
        error => error,
    })
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

/// Issue #566: the backoff is the exponential ceiling with an equal-jitter band
/// under it, and the whole sequence is a pure function of the engine's
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
                    code: 0x03.into(),
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
                request: sent,
                transmission,
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
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
                request: command(target),
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
            request: command(1),
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

/// Issue #566: the ACK backoff exponent is capped at five and every other
/// retry trigger is uncapped. Only the ACK path stops doubling.
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
                request: id,
                transmission,
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(Some(sequence))),
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
    // whose retry is triggered by a pre-ACK camera rejection must have its
    // exponent left uncapped by the engine's own trigger selection. (A
    // completion timeout used to be this trigger; an acknowledged command is
    // never rewritten since #795.)
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
        // Reject each attempt inside its ACK deadline, so the only retry
        // trigger is the camera's buffer-full answer.
        let rejected = uncapped_engine.handle(
            frame(
                1,
                Some(uncapped_sequence),
                DecodedResponse::Error {
                    socket: None,
                    code: 0x03.into(),
                },
            ),
            now,
        );
        let (retried, attempt, ready_at) =
            retry_scheduled(&rejected).expect("buffer-full rejection retry");
        assert_eq!(retried, uncapped_id);
        uncapped_waits.push(ready_at - now);
        assert_eq!(attempt, u32::try_from(uncapped_waits.len()).unwrap());
        let promoted = uncapped_engine.advance(ready_at);
        let (transmission, _, _) = request_transmit(&promoted);
        uncapped_engine.handle(
            Input::TransmissionFinished {
                request: id,
                transmission,
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(Some(uncapped_sequence))),
            },
            ready_at,
        );
        now = ready_at;
    }

    // Attempts 7 and 8 use exponents 6 and 7, so their ceilings are 64ms and
    // 128ms and every wait sits in the upper half-open band `[ceiling/2,
    // ceiling)`. Both therefore clear the capped ceiling the ACK trigger is
    // held to, which is exactly what selecting `Backoff::AckCapped` for a
    // camera rejection would destroy.
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
                request: id,
                transmission,
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(Some(sequence))),
            },
            ready_at,
        );
        now = ready_at;
    }

    let exhausted = engine.handle(Input::Wake, now + Duration::from_millis(20));
    assert!(retry_scheduled(&exhausted).is_none(), "the budget is spent");
    // A sent command that never acknowledged may still have reached the
    // camera: its terminal timeout is unconfirmed (D20, #783).
    assert_eq!(
        terminal_failure(&exhausted, id)
            .as_ref()
            .and_then(Error::failure_context),
        Some(FailureContext::new(
            FailureStage::Terminal,
            Certainty::Unconfirmed
        ))
    );
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
    retry.total_budget = Some(NonZeroDuration::new(Duration::from_millis(12)).unwrap());
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
                code: 0x03.into(),
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
        total_budget: Some(NonZeroDuration::new(total_budget).unwrap()),
        ..retrying()
    }
}

fn raw_sending_with_reply_shape(
    reply_shape: CommandReply,
    retry: RetryPolicy,
    start: Instant,
) -> (ProtocolEngine, RequestId, TransmissionId) {
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command_with_reply_shape_and_retry(1, reply_shape, retry),
        },
        start,
    );
    let (transmission, id, _) = request_transmit(&admission);
    (engine, id, transmission)
}

fn assert_initial_attempt_budget_expiry(
    engine: &mut ProtocolEngine,
    id: RequestId,
    budget_deadline: Instant,
) {
    assert_eq!(engine.entry(id).map(|entry| entry.retry_count()), Some(0));
    assert_eq!(
        engine.next_wake(engine.entry(id).unwrap().submitted_at),
        Some(budget_deadline)
    );
    let expired = engine.advance(budget_deadline);
    assert!(matches!(
        terminal_cause(&expired, id),
        Some(Error::Timeout { .. })
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
            request: command(1),
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
    assert!(matches!(phase_of(&engine, queued_id), Some(Phase::Ready)));

    let budget_deadline = start + Duration::from_millis(10);
    assert_eq!(engine.next_wake(start), Some(budget_deadline));
    let expired = engine.advance(budget_deadline);
    // Never written, so never accepted: resubmitting is safe (D20, #783).
    assert_eq!(
        terminal_failure(&expired, queued_id)
            .as_ref()
            .and_then(Error::failure_context),
        Some(FailureContext::new(
            FailureStage::Terminal,
            Certainty::NotAccepted
        ))
    );
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
                Some(sequence),
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
                    CommandReply::CompletionOnly,
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
                request: inquiry_with_retry(1, immediate_retry_budget(Duration::from_millis(10))),
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
            raw_sending_with_reply_shape(CommandReply::NoReply, retry, start);
        let equal = engine.handle(
            Input::TransmissionFinished {
                request: id,
                transmission,
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
            },
            budget_deadline,
        );
        assert!(matches!(
            terminal_outcome(&equal, id),
            Some(RuntimeOutcome::Command(CommandOutcome::Written))
        ));
        assert!(!equal.iter().any(|effect| matches!(
            effect,
            Effect::RetryScheduled { id: retried, .. } if *retried == id
        )));
        assert!(engine.entry(id).is_none());
        assert_eq!(
            engine.next_wake(start),
            Some(budget_deadline + Duration::from_millis(50)),
            "the successful no-reply terminal retains its ordinary raw tombstone"
        );
        engine.assert_invariants().unwrap();
    }

    // Strictly after the budget, a raw write result — even a successful one —
    // is physically ambiguous. It must clear the active transmission and hold
    // an inert all-response hold rather than declaring `Written` or retrying.
    {
        let (mut engine, id, transmission) =
            raw_sending_with_reply_shape(CommandReply::NoReply, retry, start);
        let late_at = budget_deadline + Duration::from_nanos(1);
        let late = engine.handle(
            Input::TransmissionFinished {
                request: id,
                transmission,
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
            },
            late_at,
        );
        assert!(matches!(
            terminal_failure(&late, id),
            Some(Error::UnsequencedCommandUnconfirmed)
        ));
        assert!(!late.iter().any(|effect| matches!(
            effect,
            Effect::RetryScheduled { id: retried, .. } if *retried == id
        )));
        let quarantine_deadline = late_at + Duration::from_millis(50);
        assert!(engine.entry(id).is_none());
        assert_eq!(
            engine
                .raw_hold(camera(1), RawHoldScope::AllResponses)
                .map(|hold| (hold.until, hold.owner)),
            Some((quarantine_deadline, Some(id)))
        );
        assert!(active_transmissions(&engine).next().is_none());
        assert_eq!(engine.next_wake(start), Some(quarantine_deadline));

        let resolved = engine.advance(quarantine_deadline);
        assert!(terminal_outcome(&resolved, id).is_none());
        assert!(engine.correlation.holds().is_empty());
        assert_eq!(engine.state(), SessionState::Running);
        engine.assert_invariants().unwrap();
    }

    // The same boundary wins over a late local write error. The result is too
    // late to prove that raw bytes did not reach the camera, so it follows the
    // unconfirmed quarantine rather than terminalizing with that transport
    // error.
    {
        let (mut engine, id, transmission) =
            raw_sending_with_reply_shape(CommandReply::NoReply, retry, start);
        let late_at = budget_deadline + Duration::from_nanos(1);
        let late = engine.handle(
            Input::TransmissionFinished {
                request: id,
                transmission,
                result: WriteOutcome::Failed(Error::transport_error(std::io::Error::other(
                    "late raw write result",
                ))),
            },
            late_at,
        );
        assert!(matches!(
            terminal_failure(&late, id),
            Some(Error::UnsequencedCommandUnconfirmed)
        ));
        assert!(engine.entry(id).is_none());
        assert!(engine
            .raw_hold(camera(1), RawHoldScope::AllResponses)
            .is_some());
        assert!(active_transmissions(&engine).next().is_none());
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
        assert!(terminal_outcome(&resolved, id).is_none());
        assert!(engine.correlation.holds().is_empty());
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
                request: command_with_reply_shape_and_retry(1, CommandReply::NoReply, retry),
            },
            start,
        );
        let (transmission, id, _) = request_transmit(&admission);
        let late = engine.handle(
            Input::TransmissionFinished {
                request: id,
                transmission,
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(None)),
            },
            budget_deadline + Duration::from_nanos(1),
        );
        assert!(matches!(
            terminal_failure(&late, id),
            Some(Error::StreamPoisoned { .. })
        ));
        assert_eq!(engine.state(), SessionState::Poisoned);
        assert!(engine.entry(id).is_none());
        assert!(active_transmissions(&engine).next().is_none());
        assert_eq!(engine.next_wake(start), None);
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
                request: id,
                transmission,
                result: WriteOutcome::Sent(TransmissionMeta::for_sequence(Some(sequence))),
            },
        );
        assert!(terminal_outcome(&equal, id).is_none());
        assert!(matches!(
            phase_of(&engine, id),
            Some(Phase::AwaitingAck { .. })
        ));
        assert_eq!(
            engine.entry(id).and_then(|entry| entry.current_sequence()),
            Some(sequence)
        );
        assert!(!engine.entries.exact_owners(sequence).is_empty());
        assert!(!engine.entries.lower_owners(sequence as u16).is_empty());

        let expired = engine.finish_input_turn(turn, EngineTurn::Complete);
        assert!(matches!(
            terminal_failure(&expired, id),
            Some(Error::Timeout { .. })
        ));

        engine.assert_invariants().unwrap();
    }

    // The owner shell's no-due write-result turn cannot bypass the same
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
        let late = finish_write_input_only(
            &mut engine,
            id,
            transmission,
            Ok(TransmissionMeta::for_sequence(Some(0xdead_beef))),
            budget_deadline + Duration::from_nanos(1),
        );
        assert!(matches!(
            terminal_failure(&late, id),
            Some(Error::Timeout { .. })
        ));
        assert!(!late.iter().any(|effect| matches!(
            effect,
            Effect::Transition {
                id: transitioned,
                to: Lifecycle::AwaitingAck { .. },
                ..
            } if *transitioned == id
        )));
        assert!(!late.iter().any(|effect| matches!(
            effect,
            Effect::RetryScheduled { id: retried, .. } if *retried == id
        )));
        assert!(engine.entry(id).is_none());
        assert!(active_transmissions(&engine).next().is_none());

        assert_eq!(engine.next_wake(start), None);
        engine.assert_invariants().unwrap();
    }

    // A failed write has its own certainty-bearing terminal even after the
    // budget; its actual transport cause is retained and no retry is scheduled.
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
                Some(first_sequence),
                DecodedResponse::Error {
                    socket: None,
                    code: 0x03.into(),
                },
            ),
            start + Duration::from_nanos(1),
        );
        let (transmission, retried_id, _) = request_transmit(&retried);
        assert_eq!(retried_id, id);
        assert!(matches!(
            engine
                .entry(id)
                .and_then(|entry| entry.retry_cause().cloned()),
            Some(Error::CommandBufferFull)
        ));
        assert!(matches!(phase_of(&engine, id), Some(Phase::Sending { .. })));

        let late = finish_write_input_only(
            &mut engine,
            id,
            transmission,
            Err(Error::transport_error(std::io::Error::other(
                "late Sony write result",
            ))),
            budget_deadline + Duration::from_nanos(1),
        );
        assert!(matches!(
            terminal_failure(&late, id),
            Some(Error::RequestUnconfirmed { source, .. }) if matches!(source.as_ref(), Error::TransportError { .. })
        ));
        assert!(!late.iter().any(|effect| matches!(
            effect,
            Effect::RetryScheduled { id: retried, .. } if *retried == id
        )));
        assert!(engine.entry(id).is_none());
        assert!(active_transmissions(&engine).next().is_none());

        assert_eq!(
            engine.state(),
            SessionState::Running,
            "a late datagram write error must not poison the session"
        );
        assert_eq!(engine.next_wake(start), None);
        engine.assert_invariants().unwrap();
    }
}

/// A compatible stream write failure is an authoritative session verdict even
/// if it arrives strictly after the request's total budget. The Raw case uses
/// the ordinary input ingress; the Sony case exercises the owner shell's
/// no-due write-result ingress, which shares that same engine path.
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
        let result = Err(Error::transport_error(std::io::Error::other(cause)));
        let poisoned = if use_no_due_ingress {
            finish_write_input_only(&mut engine, failed_id, transmission, result, late_at)
        } else {
            engine.handle(
                Input::TransmissionFinished {
                    request: failed_id,
                    transmission,
                    result: write_outcome(result),
                },
                late_at,
            )
        };

        let expected_source = Error::transport_error(std::io::Error::other(cause));
        let terminal_ids: Vec<_> = poisoned
            .iter()
            .filter_map(|effect| match effect {
                Effect::Terminal { id, .. } => Some(*id),
                _ => None,
            })
            .collect();
        assert_eq!(terminal_ids, vec![peer_id, failed_id]);
        for id in [peer_id, failed_id] {
            let Some(Error::StreamPoisoned { source }) = terminal_failure(&poisoned, id) else {
                panic!("{envelope:?} stream failure must terminalize {id:?} with poison");
            };
            assert_eq!(source.to_string(), expected_source.to_string());
            assert!(
                matches!(source.as_ref(), Error::TransportError { source } if matches!(source.as_ref(), Error::Io { source: io } if io.kind() == std::io::ErrorKind::Other))
            );
        }
        assert!(poisoned.iter().any(|effect| matches!(
            effect,
            Effect::SessionEnded(SessionEnd {
                status: EndStatus::Poisoned,
                ..
            })
        )));
        assert_eq!(engine.state(), SessionState::Poisoned);
        let terminal = engine.terminal_error().expect("stream poison verdict");
        let Error::StreamPoisoned { source } = &terminal else {
            panic!("stream failure must retain a stream-poisoned terminal error: {terminal:?}");
        };
        assert_eq!(source.to_string(), expected_source.to_string());
        assert!(
            matches!(source.as_ref(), Error::TransportError { source } if matches!(source.as_ref(), Error::Io { source: io } if io.kind() == std::io::ErrorKind::Other))
        );
        assert!(terminal.requires_new_session());
        assert_eq!(engine.active_len(), 0);
        assert!(engine.entries.is_empty());
        assert!(active_transmissions(&engine).next().is_none());

        assert_eq!(engine.next_wake(start), None);

        let rejected = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(3),
                request: command(1),
            },
            late_at,
        );
        assert!(rejected.iter().any(|effect| matches!(
            effect,
            Effect::AdmissionRejected {
                ticket: AdmissionTicket(3),
                error: Error::StreamPoisoned { source },
            } if source.to_string() == expected_source.to_string()
        )));
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
                request: inquiry_with_retry(1, immediate_retry_budget(Duration::from_millis(10))),
            },
            start,
        );
        let id = admitted(&admission);
        let sequence = sony_sequence(id);
        send_ok(&mut equal_engine, &admission, Some(sequence), start);
        let budget_deadline = start + Duration::from_millis(10);
        assert_eq!(equal_engine.next_wake(start), Some(budget_deadline));

        let reply = equal_engine.handle(
            frame(
                1,
                Some(sequence),
                DecodedResponse::InquiryReply {
                    payload: smallvec![1],
                },
            ),
            budget_deadline,
        );
        assert!(matches!(
            terminal_outcome(&reply, id),
            Some(RuntimeOutcome::Inquiry(InquiryOutcome::Reply { .. }))
        ));
        assert!(deadline_expiries(&reply, id).is_empty());
        equal_engine.assert_invariants().unwrap();

        // One nanosecond later is no longer an equal input turn: reject the
        // stale reply, then let the already-due budget terminalize the request.
        let mut late_engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let late_admission = late_engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(1),
                request: inquiry_with_retry(1, immediate_retry_budget(Duration::from_millis(10))),
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
                Some(late_sequence),
                DecodedResponse::InquiryReply {
                    payload: smallvec![1],
                },
            ),
            budget_deadline + Duration::from_nanos(1),
        );
        assert_eq!(ignored_reasons(&late), vec![IgnoreReason::UnmatchedFrame]);
        assert!(matches!(
            terminal_cause(&late, late_id),
            Some(Error::Timeout { .. })
        ));
        let ignored = position_of(&late, |effect| {
            matches!(effect, Effect::Ignored(IgnoreReason::UnmatchedFrame))
        })
        .expect("late reply is ignored");
        let terminal = position_of(
            &late,
            |effect| matches!(effect, Effect::Terminal { id, .. } if *id == late_id),
        )
        .expect("budget expiry retires the owner");
        assert!(terminal < ignored);
        late_engine.assert_invariants().unwrap();
    }

    // With no input, the budget takes priority over an equal ACK deadline.
    {
        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let mut retry = retrying();
        retry.total_budget = Some(NonZeroDuration::new(Duration::from_millis(20)).unwrap());
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
            terminal_cause(&expired, id),
            Some(Error::Timeout { .. })
        ));
        assert!(
            deadline_expiries(&expired, id).is_empty(),
            "the budget arm wins an equal ordinary deadline"
        );
        engine.assert_invariants().unwrap();
    }
}

/// Once ACK establishes exact socket correlation, neither the admission budget
/// nor cancellation ambiguity shortens the command's completion window.
#[test]
fn initial_budget_and_cancel_ambiguity_do_not_shorten_executing_completion() {
    let start = Instant::now();
    let budget_deadline = start + Duration::from_millis(10);
    let mut request = command_with_retry(1, immediate_retry_budget(Duration::from_millis(10)));
    let RuntimeRequest::Command { context, .. } = &mut request else {
        unreachable!("test constructs a command");
    };
    context.timeout.completion = NonZeroDuration::new(Duration::from_secs(1)).unwrap();

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
            Some(sequence),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );

    let cancellation = engine.handle(engine.cancel_input(id), start);
    assert!(cancel_transmit_optional(&cancellation).is_some());
    let ambiguity_deadline = match engine.entry(id).map(Entry::cancellation) {
        Some(CancelState::Sending {
            ambiguity_deadline, ..
        }) => ambiguity_deadline,
        cancellation => panic!("expected pending cancellation, got {cancellation:?}"),
    };
    assert_eq!(ambiguity_deadline, start + Duration::from_millis(50));
    let completion_deadline = start + Duration::from_secs(1);
    assert_eq!(engine.next_wake(start), Some(completion_deadline));

    let (transmission, request, _) = cancel_transmit(&cancellation);
    engine.handle(
        Input::TransmissionFinished {
            request,
            transmission,
            result: WriteOutcome::NotSent(Error::io_timeout()),
        },
        start,
    );

    let at_budget = engine.advance(budget_deadline);
    assert!(terminal_outcome(&at_budget, id).is_none());
    assert!(engine.entry(id).is_some());
    assert_eq!(engine.next_wake(start), Some(completion_deadline));

    let at_ambiguity = engine.advance(ambiguity_deadline);
    assert!(terminal_outcome(&at_ambiguity, id).is_none());
    assert!(matches!(
        phase_of(&engine, id),
        Some(Phase::Executing { .. })
    ));

    let resolved = engine.handle(
        frame(
            1,
            Some(sequence),
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start + Duration::from_millis(60),
    );
    assert!(matches!(
        terminal_outcome(&resolved, id),
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
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
    assert!(matches!(
        terminal_failure(&expired, id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert!(phase_of(&engine, id).is_none());
    let quarantine_deadline = engine
        .raw_hold(camera(1), RawHoldScope::PreAck)
        .expect("pre-ACK hold")
        .until;
    assert_eq!(quarantine_deadline, start + Duration::from_millis(60));
    assert_eq!(engine.next_wake(start), Some(quarantine_deadline));

    let still_quarantined = engine.advance(start + Duration::from_millis(59));
    assert!(terminal_outcome(&still_quarantined, id).is_none());
    assert!(engine.raw_hold(camera(1), RawHoldScope::PreAck).is_some());

    let resolved = engine.advance(quarantine_deadline);
    assert!(terminal_outcome(&resolved, id).is_none());
    assert!(engine.correlation.holds().is_empty());
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
            Some(sequence),
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
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
    assert_eq!(engine.next_wake(start), Some(budget_deadline));
    let expired = engine.advance(budget_deadline);
    assert!(matches!(
        terminal_failure(&expired, id),
        Some(Error::RequestUnconfirmed { source, .. }) if matches!(source.as_ref(), Error::CommandBufferFull)
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
            Some(sequence),
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
            },
        ),
        retry_at,
    );
    send_ok(&mut engine, &retried, Some(sequence), retry_at);
    let acknowledged = engine.handle(
        frame(
            1,
            Some(sequence),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        retry_at,
    );
    assert!(acknowledged.iter().any(|effect| matches!(
        effect,
        Effect::Transition {
            to: Lifecycle::Executing { .. },
            ..
        }
    )));

    let budget_deadline = start + Duration::from_millis(21);
    assert_eq!(engine.next_wake(start), Some(budget_deadline));
    let expired = engine.advance(budget_deadline);
    assert!(matches!(
        terminal_failure(&expired, id),
        Some(Error::RequestUnconfirmed { source, .. }) if matches!(source.as_ref(), Error::CommandBufferFull)
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
            request: inquiry_with_retry(1, retry),
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
            Some(sequence),
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
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
    assert_eq!(engine.next_wake(start), Some(budget_deadline));
    let expired = engine.advance(budget_deadline);
    assert!(matches!(
        terminal_cause(&expired, id),
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
                code: 0x03.into(),
            },
        ),
        start + Duration::from_millis(1),
    );
    let (_, _, ready_at) = retry_scheduled(&busy).expect("busy camera schedules raw retry");
    assert!(ready_at < start + Duration::from_millis(21));
    engine.advance(ready_at);
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::Ready)
    ));

    let budget_deadline = start + Duration::from_millis(21);
    assert_eq!(engine.next_wake(start), Some(budget_deadline));
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
                code: 0x03.into(),
            },
        ),
        start + Duration::from_millis(1),
    );
    assert!(retry_scheduled(&retried).is_some());
    assert!(matches!(
        engine.entry(id).map(Entry::phase),
        Some(Phase::Sending { .. })
    ));

    send_ok(
        &mut engine,
        &retried,
        None,
        start + Duration::from_millis(1),
    );
    let budget_deadline = start + Duration::from_millis(21);
    assert_eq!(engine.next_wake(start), Some(budget_deadline));
    let expired = engine.advance(budget_deadline);
    // The in-flight attempt terminates immediately without poisoning, and only
    // its keyed correlation hold remains active.
    assert!(matches!(
        terminal_failure(&expired, id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert!(!expired
        .iter()
        .any(|effect| matches!(effect, Effect::SessionEnded(_))));
    assert_eq!(engine.state(), SessionState::Running);
    assert!(engine.entry(id).is_none());
    assert!(engine.raw_hold(camera(1), RawHoldScope::PreAck).is_some());

    // The ambiguity deadline only releases the inert hold; it emits no second
    // terminal and the session remains running.
    let resolved = engine.advance(start + Duration::from_millis(71));
    assert!(terminal_outcome(&resolved, id).is_none());
    assert!(engine.correlation.holds().is_empty());
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

    let retried = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
            },
        ),
        start + Duration::from_millis(1),
    );
    send_ok(
        &mut engine,
        &retried,
        None,
        start + Duration::from_millis(1),
    );
    let expired = engine.advance(start + Duration::from_millis(21));
    assert!(matches!(
        terminal_failure(&expired, id),
        Some(Error::StreamPoisoned { source }) if matches!(source.as_ref(), Error::UnsequencedCommandUnconfirmed)
    ));
    assert_eq!(engine.state(), SessionState::Poisoned);
    assert!(
        expired.iter().any(|effect| matches!(
            effect,
            Effect::SessionEnded(SessionEnd {
                status: EndStatus::Poisoned,
                ..
            })
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
            request: command(1),
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
            error: Error::Io {
                source: Arc::new(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
            },
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
            request: command(1),
        },
        start,
    );
    let id = admitted(&admission);
    send_ok(&mut engine, &admission, None, start);

    let fault = engine.handle(
        Input::ReceiveFault {
            error: Error::Io {
                source: Arc::new(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
            },
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
/// request on its live pre-ACK cancellation path: an ACK can still assign a
/// socket and issue the cancel, and strict mode poisons only when that
/// resolution remains unconfirmed at its ambiguity deadline.
#[test]
fn strict_raw_receive_fault_after_cancel_uses_cancellation_resolution() {
    let start = Instant::now();
    let mut engine = strict_poison_engine();
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
        },
        start,
    );
    let id = admitted(&admission);
    send_ok(&mut engine, &admission, None, start);

    let cancel_at = start + Duration::from_millis(1);
    let cancelled = engine.handle(engine.cancel_input(id), cancel_at);
    assert!(cancelled
        .iter()
        .any(|effect| matches!(effect, Effect::CancellationRecorded { id: seen } if *seen == id)));
    assert!(matches!(
        engine.entry(id).map(Entry::cancellation),
        Some(CancelState::Requested { .. })
    ));

    let fault = engine.handle(
        Input::ReceiveFault {
            error: Error::Io {
                source: Arc::new(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
            },
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
        active_transmissions(&engine).next().is_none(),
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
    let (transmission, request, _) = cancel_transmit(&acknowledged);
    engine.handle(
        Input::TransmissionFinished {
            request,
            transmission,
            result: WriteOutcome::Sent(TransmissionMeta::Known),
        },
        cancel_at + Duration::from_millis(1),
    );
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

/// Issue #723: an ACK timeout terminalizes the request immediately and leaves a
/// bounded positional hold. A late ACK in that window is inert and cannot bind
/// either the terminal predecessor or its queued successor.
#[test]
fn raw_ack_timeout_hold_rejects_late_ack_and_protects_successor() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
        },
        start,
    );
    let id = admitted(&admission);
    send_ok(&mut engine, &admission, None, start);

    // ACK deadline (20ms): the command fails, the session stays live, and the
    // PreAck hold remains until 70ms.
    let expired = engine.advance(start + Duration::from_millis(20));
    assert_eq!(engine.state(), SessionState::Running);
    assert!(matches!(
        terminal_failure(&expired, id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert!(engine.entry(id).is_none());
    let hold_deadline = engine
        .raw_hold(camera(1), RawHoldScope::PreAck)
        .expect("pre-ACK hold")
        .until;
    assert_eq!(hold_deadline, start + Duration::from_millis(70));

    // The slot is reserved: a new command on the same target queues rather than
    // dispatching while the quarantine holds.
    let queued = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: command(1),
        },
        start + Duration::from_millis(21),
    );
    let queued_id = admitted(&queued);
    assert!(request_transmit_optional(&queued).is_none());
    assert!(matches!(
        engine.entry(queued_id).map(Entry::phase),
        Some(Phase::Ready)
    ));

    // The hold has no live owner: the late ACK is ignored and the successor
    // remains queued.
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
        engine.entry(queued_id).map(Entry::phase),
        Some(Phase::Ready)
    ));
    assert!(request_transmit_optional(&late).is_none());

    let released = engine.advance(hold_deadline);
    assert_eq!(request_transmit(&released).1, queued_id);
    assert!(terminal_outcome(&released, id).is_none());
    assert_eq!(engine.state(), SessionState::Running);
    engine.assert_invariants().unwrap();
}

/// An owned socket becomes an inert exact-socket hold after timeout, so a late
/// completion cannot resurrect the already-terminal request.
#[test]
fn raw_completion_timeout_hold_rejects_exact_late_completion() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
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

    // Completion deadline (40ms): the request fails and S1 remains held until
    // the 90ms ambiguity boundary.
    let expired = engine.advance(start + Duration::from_millis(40));
    assert_eq!(engine.state(), SessionState::Running);
    assert!(matches!(
        terminal_failure(&expired, id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert!(engine.entry(id).is_none());
    let hold_deadline = engine
        .raw_hold(camera(1), RawHoldScope::Socket(ViscaSocket::S1))
        .expect("S1 hold")
        .until;
    assert_eq!(hold_deadline, start + Duration::from_millis(90));

    // The late exact completion is stale while the hold remains active.
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
    assert!(terminal_outcome(&late, id).is_none());
    assert_eq!(engine.state(), SessionState::Running);
    assert_eq!(socket_of(&engine, id), None);
    engine.advance(hold_deadline);
    assert!(engine.correlation.holds().is_empty());
    engine.assert_invariants().unwrap();
}

/// Issue #566: `0x41` (`CommandNotExecutable`) is retried for a movement or
/// preset request and is terminal for a standard one. Nothing pinned this.
#[test]
fn command_not_executable_retries_only_where_the_policy_allows_it() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);

    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
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
                code: 0x41.into(),
            },
        ),
        start,
    );
    let (retried, attempt, _) =
        retry_scheduled(&refused).expect("a movement request retries a refused command");
    assert_eq!(retried, movement_id);
    assert_eq!(attempt, 1);
    assert!(terminal_outcome(&refused, movement_id).is_none());

    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(2),
            request: without_not_executable_retry(command(2)),
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
                code: 0x41.into(),
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
            request: command(1),
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
                code: 0x05.into(),
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
                code: 0x05.into(),
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

/// Issue #566 retried a Sony post-ACK completion timeout with the same
/// sequence. #795 reverses that: an ACK proves the camera accepted the
/// command, and the cited Sony same-sequence retransmission
/// (docs/visca_reference.md §5.3) recovers a lost message, not an accepted
/// one. The completion deadline now ends the request unconfirmed without a
/// second write.
#[test]
fn a_post_ack_completion_timeout_never_rewrites_the_command() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let admission = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command(1),
        },
        start,
    );
    let id = admitted(&admission);
    send_ok(&mut engine, &admission, Some(0x1001), start);
    engine.handle(
        frame(
            1,
            Some(0x1001),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );
    assert_eq!(socket_of(&engine, id), Some(ViscaSocket::S1));

    // The completion deadline is 40ms after the ACK.
    let lapsed = engine.handle(Input::Wake, start + Duration::from_millis(41));
    assert!(retry_scheduled(&lapsed).is_none());
    assert!(request_transmit_optional(&lapsed).is_none());
    let error = terminal_failure(&lapsed, id).expect("the completion deadline is terminal");
    assert_eq!(
        error.failure_context(),
        Some(FailureContext::new(
            FailureStage::Terminal,
            Certainty::Unconfirmed
        ))
    );
    assert_eq!(
        deadline_expiries(&lapsed, id),
        vec![(DeadlineKind::Completion, false)]
    );
    engine.assert_invariants().unwrap();
}

// ---- Issue #700: raw reply-shape axis (completion-only / no-reply) ---------

/// The raw single-candidate invariant must use the same phase predicate as the
/// dispatch gate. In particular, completion-only `AwaitingCompletion` owns no
/// socket and cannot coexist with another same-target positional candidate.
///
/// Design references: GitHub issue #542, “Decisions from this review”,
/// item 7; architecture_2_0.md, “Correlation before ACK is envelope-specific”
/// and “Operational invariants”.
#[test]
fn raw_single_candidate_invariant_includes_completion_only_awaiting_completion() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);

    let (first_admission, first_id) = admit(
        &mut engine,
        1,
        command_with_reply_shape(1, CommandReply::CompletionOnly),
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
        command_with_reply_shape(2, CommandReply::CompletionOnly),
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
    let same_target = command_with_reply_shape(1, CommandReply::CompletionOnly);
    engine
        .entries
        .replace_request_for_audit(second_id, same_target);
    assert_eq!(
        engine.assert_invariants().unwrap_err().as_ref(),
        "more than one raw command is unacknowledged on a target"
    );
}

#[test]
fn raw_uncorrelatable_command_invariant_rejects_the_urgent_lane_exception() {
    let start = Instant::now();
    let mut runtime = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (completion, _) = admit(
        &mut runtime,
        1,
        command_with_reply_shape(1, CommandReply::CompletionOnly),
        start,
    );
    send_ok(&mut runtime, &completion, None, start);

    // A real urgent request cannot cross the #700 exclusive lane.
    let (blocked, blocked_id) = admit(&mut runtime, 2, urgent_command(1), start);
    assert!(request_transmit_optional(&blocked).is_none());
    assert!(matches!(phase_of(&runtime, blocked_id), Some(Phase::Ready)));

    // White-box corruption gives the urgent request an in-flight phase while
    // preserving its queue/index consistency by using a separately valid
    // engine. The ordinary two-candidate audit permits an urgent exception;
    // the #700 exclusivity audit must still reject this mixed state.
    let mut corrupted = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (completion, _) = admit(
        &mut corrupted,
        1,
        command_with_reply_shape(1, CommandReply::CompletionOnly),
        start,
    );
    send_ok(&mut corrupted, &completion, None, start);
    let (urgent, urgent_id) = admit(&mut corrupted, 2, urgent_command(2), start);
    let mut same_target = urgent_command(1);
    same_target.context_mut().submission_order = corrupted
        .entry(urgent_id)
        .unwrap()
        .request
        .context()
        .submission_order;
    corrupted
        .entries
        .replace_request_for_audit(urgent_id, same_target);
    assert!(request_transmit_optional(&urgent).is_some());
    assert_eq!(
        corrupted.assert_invariants().unwrap_err().as_ref(),
        "uncorrelatable raw command does not own its target exclusively"
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
        command_with_reply_shape(1, CommandReply::CompletionOnly),
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
                to: Lifecycle::AwaitingAck { .. },
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
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
    ));
    assert!(phase_of(&engine, id).is_none(), "the entry is finished");
    engine.assert_invariants().unwrap();
}

/// #700: an ACK is never correlation evidence for a completion-only command.
/// In particular, cancellation must not turn its socketless completion window
/// into a socket-owning path.
#[test]
fn completion_only_stray_acks_remain_inert_across_its_lifecycle() {
    let start = Instant::now();

    // Once sent, a completion-only command still ignores a stray ACK rather
    // than acquiring its socket.
    {
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (admitted_effects, id) = admit(
            &mut engine,
            1,
            command_with_reply_shape(1, CommandReply::CompletionOnly),
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

    // Its completion timeout emits the raw unconfirmed terminal and leaves an
    // all-response hold. An ACK remains inert there too.
    {
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (admitted_effects, id) = admit(
            &mut engine,
            1,
            command_with_reply_shape(1, CommandReply::CompletionOnly),
            start,
        );
        send_ok(&mut engine, &admitted_effects, None, start);
        let expired = engine.advance(start + Duration::from_millis(40));
        assert!(matches!(
            terminal_failure(&expired, id),
            Some(Error::UnsequencedCommandUnconfirmed)
        ));
        assert!(phase_of(&engine, id).is_none());
        assert!(engine
            .raw_hold(camera(1), RawHoldScope::AllResponses)
            .is_some());
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
        assert!(phase_of(&engine, id).is_none());
        engine.assert_invariants().unwrap();
    }

    // Once the completion timeout terminal has been delivered, a caller's
    // cancellation cannot resurrect that request. A subsequent ACK still must
    // neither acquire a socket nor emit a socket cancel.
    {
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (admitted_effects, id) = admit(
            &mut engine,
            1,
            command_with_reply_shape(1, CommandReply::CompletionOnly),
            start,
        );
        let cancel = engine.cancel_input(id);
        send_ok(&mut engine, &admitted_effects, None, start);
        let expired = engine.advance(start + Duration::from_millis(40));
        assert!(matches!(
            terminal_failure(&expired, id),
            Some(Error::UnsequencedCommandUnconfirmed)
        ));
        let cancelled = engine.handle(cancel, start + Duration::from_millis(41));
        assert_eq!(
            ignored_reasons(&cancelled),
            vec![IgnoreReason::UnknownRequest]
        );
        assert!(cancel_transmit_optional(&cancelled).is_none());

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
        assert!(phase_of(&engine, id).is_none());
        assert!(engine
            .raw_hold(camera(1), RawHoldScope::AllResponses)
            .is_some());
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
        command_with_reply_shape(1, CommandReply::CompletionOnly),
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
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
    ));
    assert!(phase_of(&engine, id).is_none());
    assert_eq!(
        engine.next_wake(start),
        Some(start + Duration::from_millis(50))
    );
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
        command_with_reply_shape(1, CommandReply::CompletionOnly),
        start,
    );
    send_ok(&mut engine, &admitted_effects, None, start);
    // Past the ACK time (20ms), before the completion deadline (40ms): still
    // awaiting completion, session healthy — the ACK deadline never existed.
    let past_ack = engine.advance(start + Duration::from_millis(25));
    assert!(
        past_ack
            .iter()
            .all(|effect| !matches!(effect, Effect::SessionEnded(_))),
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
            Effect::SessionEnded(SessionEnd {
                status: EndStatus::Poisoned,
                ..
            })
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
        command_with_reply_shape(1, CommandReply::CompletionOnly),
        start,
    );
    send_ok(&mut engine, &admitted_effects, None, start);
    // Completion deadline emits the per-request terminal immediately and leaves
    // its all-response hold; the session remains usable.
    let expired = engine.advance(start + Duration::from_millis(45));
    assert_eq!(engine.state(), SessionState::Running, "no session poison");
    assert!(matches!(
        terminal_failure(&expired, id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    assert!(phase_of(&engine, id).is_none());
    assert!(engine
        .raw_hold(camera(1), RawHoldScope::AllResponses)
        .is_some());
    // Ambiguity expiry removes the hold without emitting another terminal.
    let closed = engine.advance(start + Duration::from_millis(200));
    assert!(terminal_outcome(&closed, id).is_none());
    assert!(engine.correlation.holds().is_empty());
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
        command_with_reply_shape(1, CommandReply::NoReply),
        start,
    );
    let send = send_ok(&mut engine, &admitted_effects, None, start);
    assert!(
        matches!(
            terminal_outcome(&send, id),
            Some(RuntimeOutcome::Command(CommandOutcome::Written))
        ),
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
        command_with_reply_shape(1, CommandReply::CompletionOnly),
        start,
    );
    send_ok(&mut engine, &admitted_effects, None, start);
    let done = engine.handle(
        frame(1, None, DecodedResponse::Completion { socket: None }),
        start,
    );
    assert!(matches!(
        terminal_outcome(&done, id),
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
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
        command_with_reply_shape(1, CommandReply::NoReply),
        start,
    );
    let sent = send_ok(&mut engine, &first, None, start);
    assert!(matches!(
        terminal_outcome(&sent, first_id),
        Some(RuntimeOutcome::Command(CommandOutcome::Written))
    ));
    // The owner applies this admission only after the immediate write result
    // installed the predecessor's target tombstone.
    let (second, second_id) = admit(&mut engine, 2, command(1), start);
    assert!(request_transmit_optional(&second).is_none());
    assert!(matches!(phase_of(&engine, second_id), Some(Phase::Ready)));

    let (other_target, other_target_id) = admit(&mut engine, 3, command(2), start);
    assert_eq!(
        request_transmit(&other_target).1,
        other_target_id,
        "the target-local tombstone does not hold a different camera"
    );

    send_ok(&mut engine, &other_target, None, start);
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
    engine.handle(
        frame(
            2,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        start,
    );

    for response in [
        DecodedResponse::Ack {
            socket: Some(ViscaSocket::S1),
        },
        DecodedResponse::Completion { socket: None },
        DecodedResponse::Error {
            socket: None,
            code: 0x05.into(),
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
        assert!(matches!(phase_of(&engine, second_id), Some(Phase::Ready)));
    }

    let deadline = start + Duration::from_millis(50);
    assert_eq!(engine.next_wake(start), Some(deadline));
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
        command_with_reply_shape(1, CommandReply::NoReply),
        start,
    );
    let written = send_ok(&mut engine, &first, None, start);
    assert!(matches!(
        terminal_outcome(&written, first_id),
        Some(RuntimeOutcome::Command(CommandOutcome::Written))
    ));
    let (successor, successor_id) = admit(&mut engine, 2, command(1), start);
    assert!(request_transmit_optional(&successor).is_none());

    let at_deadline = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x01.into(),
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
/// therefore releases the overlapping inquiry class immediately and installs
/// no target tombstone (#712).
#[test]
fn raw_single_flight_inquiry_success_releases_successor_without_tombstone() {
    let start = Instant::now();
    let mut engine = single_flight_raw_engine();

    let (first, first_id) = admit(&mut engine, 1, inquiry(1), start);
    send_ok(&mut engine, &first, None, start);
    let (successor, successor_id) = admit(&mut engine, 2, inquiry(1), start);
    assert!(request_transmit_optional(&successor).is_none());

    let first_reply = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                payload: smallvec![0x0a],
            },
        ),
        start,
    );
    assert!(matches!(
        terminal_outcome(&first_reply, first_id),
        Some(RuntimeOutcome::Inquiry(InquiryOutcome::Reply { payload, .. })) if payload.as_slice() == [0x0a]
    ));
    assert_eq!(
        request_transmit(&first_reply).1,
        successor_id,
        "a matched reply releases the same-target successor in the same turn"
    );
    assert_eq!(
        engine.raw_hold(camera(1), RawHoldScope::Inquiry(PayloadShape::Unknown)),
        None
    );
    let successor_sent = send_ok(&mut engine, &first_reply, None, start);
    assert!(request_transmit_optional(&successor_sent).is_none());
    let successor_reply = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                payload: smallvec![0x0b],
            },
        ),
        start,
    );
    assert!(matches!(
        terminal_outcome(&successor_reply, successor_id),
        Some(RuntimeOutcome::Inquiry(InquiryOutcome::Reply { payload, .. })) if payload.as_slice() == [0x0b]
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

    let (inquiry_send, inquiry_id) = admit(&mut engine, 1, inquiry(1), start);
    send_ok(&mut engine, &inquiry_send, None, start);
    let reply = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                payload: smallvec![0x0a],
            },
        ),
        start,
    );
    assert!(terminal_outcome(&reply, inquiry_id).is_some());

    let (first_urgent, first_urgent_id) = admit(&mut engine, 2, urgent_command(1), start);
    assert_eq!(request_transmit(&first_urgent).1, first_urgent_id);
    send_ok(&mut engine, &first_urgent, None, start);

    let (second_urgent, second_urgent_id) = admit(&mut engine, 3, urgent_command(2), start);
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
        inquiry_with_retry(1, RetryPolicy::NEVER),
        start,
    );
    send_ok(&mut engine, &inquiry_send, None, start);
    let timeout_at = start + Duration::from_millis(30);
    let inquiry_timeout = engine.advance(timeout_at);
    assert!(matches!(
        terminal_cause(&inquiry_timeout, inquiry_id),
        Some(Error::Timeout { .. })
    ));

    let (command_send, command_id) = admit(&mut engine, 2, command(1), timeout_at);
    send_ok(&mut engine, &command_send, None, timeout_at);
    assert!(matches!(
        phase_of(&engine, command_id),
        Some(Phase::AwaitingAck { .. })
    ));
    assert_eq!(
        engine.raw_hold(camera(1), RawHoldScope::Inquiry(PayloadShape::Unknown)),
        Some(RawHold {
            until: timeout_at + Duration::from_millis(50),
            owner: Some(inquiry_id),
        })
    );

    // These are precisely the inquiry-shaped stale evidence the narrow hold
    // retains. They must leave the pre-ACK command live.
    for stale in [
        DecodedResponse::InquiryReply {
            payload: smallvec![0x0a],
        },
        DecodedResponse::Error {
            socket: None,
            code: 0x01.into(),
        },
        DecodedResponse::Error {
            socket: Some(ViscaSocket::S2),
            code: 0x01.into(),
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
            to: Lifecycle::Executing {
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
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
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
            inquiry_with_retry(1, RetryPolicy::NEVER),
            start,
        );
        send_ok(&mut engine, &inquiry_send, None, start);
        let timeout_at = start + Duration::from_millis(30);
        let timed_out = engine.advance(timeout_at);
        assert!(matches!(
            terminal_cause(&timed_out, inquiry_id),
            Some(Error::Timeout { .. })
        ));
        let (command_send, command_id) = admit(&mut engine, 2, command(1), timeout_at);
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
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
    ));
    completion_engine.assert_invariants().unwrap();

    let (mut error_engine, error_id, timeout_at) = setup();
    let error = error_engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: Some(ViscaSocket::S1),
                code: 0x01.into(),
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
        command_with_reply_shape(1, CommandReply::NoReply),
        start,
    );
    let written = send_ok(&mut engine, &send, None, start);
    assert!(matches!(
        terminal_outcome(&written, id),
        Some(RuntimeOutcome::Command(CommandOutcome::Written))
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
            payload: smallvec![0x0a],
        },
        DecodedResponse::Error {
            socket: Some(ViscaSocket::S1),
            code: 0x01.into(),
        },
        DecodedResponse::Error {
            socket: None,
            code: 0x01.into(),
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

/// The authoritative table keeps every target/scope pair independent, extends
/// deadlines monotonically, and erases owner identity when different requests
/// overlap on the same correlation slot.
#[test]
fn raw_keyed_holds_merge_by_scope_and_expire_independently() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let first = RequestId::from_nonzero(NonZeroU64::new(101).unwrap());
    let second = RequestId::from_nonzero(NonZeroU64::new(202).unwrap());

    extend_fixture_hold(
        &mut engine,
        camera(1),
        StreamScope::Socket(ViscaSocket::S1),
        start + Duration::from_millis(10),
        Some(first),
    );
    extend_fixture_hold(
        &mut engine,
        camera(1),
        StreamScope::Socket(ViscaSocket::S2),
        start + Duration::from_millis(20),
        Some(first),
    );
    extend_fixture_pre_ack(
        &mut engine,
        camera(1),
        start + Duration::from_millis(30),
        Some(first),
    );
    extend_fixture_hold(
        &mut engine,
        camera(1),
        StreamScope::Inquiry(PayloadShape::Unknown),
        start + Duration::from_millis(40),
        Some(first),
    );
    extend_fixture_hold(
        &mut engine,
        camera(1),
        StreamScope::AllResponses,
        start + Duration::from_millis(50),
        Some(first),
    );
    extend_fixture_pre_ack(
        &mut engine,
        camera(2),
        start + Duration::from_millis(15),
        Some(second),
    );
    assert_eq!(engine.correlation.holds().len(), 6);

    // An earlier contribution from the same request cannot shorten the hold.
    extend_fixture_pre_ack(
        &mut engine,
        camera(1),
        start + Duration::from_millis(25),
        Some(first),
    );
    assert_eq!(
        engine.raw_hold(camera(1), RawHoldScope::PreAck),
        Some(RawHold {
            until: start + Duration::from_millis(30),
            owner: Some(first),
        })
    );

    // A different contributor extends the same slot without guessing which
    // request owns the overlapping evidence. Once erased, identity stays
    // erased even if a later contribution happens to name one request again.
    extend_fixture_pre_ack(
        &mut engine,
        camera(1),
        start + Duration::from_millis(35),
        Some(second),
    );
    extend_fixture_pre_ack(
        &mut engine,
        camera(1),
        start + Duration::from_millis(34),
        Some(first),
    );
    assert_eq!(
        engine.raw_hold(camera(1), RawHoldScope::PreAck),
        Some(RawHold {
            until: start + Duration::from_millis(35),
            owner: None,
        })
    );

    let first_release = engine.raw_correlation_releases_due(start + Duration::from_millis(15));
    assert!(first_release
        .for_target(camera(1))
        .exact_socket(ViscaSocket::S1));
    assert!(!first_release
        .for_target(camera(1))
        .exact_socket(ViscaSocket::S2));
    assert!(first_release.for_target(camera(2)).pre_ack_unkeyed());
    engine.advance(start + Duration::from_millis(15));
    assert!(engine
        .raw_hold(camera(1), RawHoldScope::Socket(ViscaSocket::S1))
        .is_none());
    assert!(engine.raw_hold(camera(2), RawHoldScope::PreAck).is_none());
    assert_eq!(engine.correlation.holds().len(), 4);

    let pre_ack_release = engine.raw_correlation_releases_due(start + Duration::from_millis(35));
    assert!(pre_ack_release.for_target(camera(1)).pre_ack_unkeyed());
    assert!(!pre_ack_release.for_target(camera(1)).inquiry_unkeyed());
    engine.advance(start + Duration::from_millis(35));
    assert_eq!(
        engine.next_wake(start),
        Some(start + Duration::from_millis(40))
    );

    let final_release = engine.raw_correlation_releases_due(start + Duration::from_millis(50));
    let target = final_release.for_target(camera(1));
    assert!(target.inquiry_unkeyed());
    assert!(target.terminal_all());
    engine.advance(start + Duration::from_millis(50));
    assert!(engine.correlation.holds().is_empty());
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
    let (send, command_id) = admit(&mut engine, 1, command(1), start);
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
    extend_fixture_hold(
        &mut engine,
        camera(1),
        StreamScope::Inquiry(PayloadShape::Unknown),
        start,
        None,
    );
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
    extend_fixture_hold(
        &mut engine,
        camera(1),
        StreamScope::Inquiry(PayloadShape::Unknown),
        start,
        None,
    );

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
    let (command_send, command_id) = admit(&mut command_engine, 1, command(1), start);
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

    let mut cancelled_preack = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (cancelled_send, cancelled_id) = admit(&mut cancelled_preack, 1, command(1), start);
    send_ok(&mut cancelled_preack, &cancelled_send, None, start);
    let ack_deadline = match phase_of(&cancelled_preack, cancelled_id) {
        Some(Phase::AwaitingAck { deadline, .. }) => deadline,
        phase => panic!("expected awaiting ACK, got {phase:?}"),
    };
    cancelled_preack.handle(cancelled_preack.cancel_input(cancelled_id), start);
    let ambiguity_deadline = match cancelled_preack
        .entry(cancelled_id)
        .map(Entry::cancellation)
    {
        Some(CancelState::Requested { ambiguity_deadline }) => ambiguity_deadline,
        cancellation => panic!("expected cancellation intent, got {cancellation:?}"),
    };
    assert!(cancelled_preack
        .raw_correlation_releases_due(ack_deadline)
        .is_empty());
    assert!(cancelled_preack
        .raw_correlation_releases_due(ambiguity_deadline)
        .for_target(camera(1))
        .pre_ack_unkeyed());

    let mut inquiry_engine = single_flight_raw_engine();
    let (inquiry_send, inquiry_id) = admit(&mut inquiry_engine, 1, inquiry(1), start);
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
        inquiry_with_retry(1, RetryPolicy::NEVER),
        start,
    );
    send_ok(&mut engine, &first, None, start);
    let (successor, successor_id) = admit(&mut engine, 2, inquiry(1), start);
    assert!(request_transmit_optional(&successor).is_none());

    let timed_out = engine.advance(timeout_at);
    // An inquiry has no effect, so its timeout is conclusive (D20, #783).
    assert_eq!(
        terminal_failure(&timed_out, first_id)
            .as_ref()
            .and_then(Error::failure_context),
        Some(FailureContext::new(
            FailureStage::Terminal,
            Certainty::FailedConclusively
        ))
    );
    assert!(request_transmit_optional(&timed_out).is_none());
    assert!(matches!(
        phase_of(&engine, successor_id),
        Some(Phase::Ready)
    ));

    let late = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
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
                payload: smallvec![0x0b],
            },
        ),
        release_at,
    );
    assert!(matches!(
        terminal_outcome(&successor_reply, successor_id),
        Some(RuntimeOutcome::Inquiry(InquiryOutcome::Reply { payload, .. })) if payload.as_slice() == [0x0b]
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
        inquiry_with_retry(1, RetryPolicy::NEVER),
        start,
    );
    send_ok(&mut engine, &first, None, start);
    let timed_out = engine.advance(timeout_at);
    assert!(matches!(
        terminal_cause(&timed_out, first_id),
        Some(Error::Timeout { .. })
    ));

    let (inquiry_effects, successor_id) = admit(&mut engine, 2, inquiry(1), timeout_at);
    assert!(request_transmit_optional(&inquiry_effects).is_none());
    assert!(matches!(
        phase_of(&engine, successor_id),
        Some(Phase::Ready)
    ));

    let (urgent_effects, urgent_id) = admit(&mut engine, 3, urgent_command(1), timeout_at);
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
            to: Lifecycle::Executing { socket: ViscaSocket::S1, .. },
            ..
        } if *id == urgent_id
    )));
    assert!(matches!(
        phase_of(&engine, successor_id),
        Some(Phase::Ready)
    ));

    let released = engine.advance(release_at);
    assert_eq!(request_transmit(&released).1, successor_id);
    engine.assert_invariants().unwrap();
}

/// #744: a lost predecessor ACK leaves a `PreAck` tombstone, but an Urgent
/// stop admitted through that hold is the sole live raw candidate. Its ACK is
/// therefore attributable to the stop, not inert tombstone traffic.
#[test]
fn urgent_ack_binds_when_its_preack_hold_has_no_live_predecessor() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);

    let (predecessor_effects, predecessor_id) = admit(&mut engine, 1, command(1), start);
    send_ok(&mut engine, &predecessor_effects, None, start);
    let ack_deadline = start + Duration::from_millis(20);
    let expired = engine.advance(ack_deadline);
    assert!(matches!(
        terminal_failure(&expired, predecessor_id),
        Some(Error::UnsequencedCommandUnconfirmed)
    ));
    let original_hold = engine
        .raw_hold(camera(1), RawHoldScope::PreAck)
        .expect("lost predecessor retains its pre-ACK hold");

    let (urgent_admission, urgent_id) = admit(&mut engine, 2, urgent_command(1), ack_deadline);
    assert_eq!(request_transmit(&urgent_admission).1, urgent_id);
    send_ok(&mut engine, &urgent_admission, None, ack_deadline);

    let acknowledged = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        ack_deadline + Duration::from_millis(1),
    );
    assert!(matches!(
        phase_of(&engine, urgent_id),
        Some(Phase::Executing {
            socket: ViscaSocket::S1,
            ..
        })
    ));
    assert!(
        !acknowledged
            .iter()
            .any(|effect| matches!(effect, Effect::Ignored(IgnoreReason::UnmatchedFrame))),
        "the unique urgent candidate owns its ACK"
    );
    assert_eq!(
        engine.raw_hold(camera(1), RawHoldScope::PreAck),
        Some(original_hold),
        "the original hold remains bounded; the stop installs no second hold"
    );

    let completed = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        ack_deadline + Duration::from_millis(2),
    );
    assert!(matches!(
        terminal_outcome(&completed, urgent_id),
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
    ));
    assert_eq!(
        engine.raw_hold(camera(1), RawHoldScope::PreAck),
        Some(original_hold),
        "the stop completion cannot extend the predecessor's hold"
    );
    engine.assert_invariants().unwrap();
}

/// The datagram urgent exception admits two positional candidates, so an
/// unsequenced ACK proves neither request owns the named socket.
#[test]
fn urgent_raw_command_bypasses_preack_gate_and_ambiguous_ack_binds_neither() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (ordinary, ordinary_id) = admit(&mut engine, 1, command(1), now);
    send_ok(&mut engine, &ordinary, None, now);
    let (urgent, urgent_id) = admit(&mut engine, 2, urgent_command(1), now);
    assert_eq!(request_transmit(&urgent).1, urgent_id);
    send_ok(&mut engine, &urgent, None, now);
    let answer = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        now,
    );
    assert!(answer
        .iter()
        .any(|effect| matches!(effect, Effect::Ignored(IgnoreReason::UnmatchedFrame))));
    for id in [ordinary_id, urgent_id] {
        assert!(matches!(
            phase_of(&engine, id),
            Some(Phase::AwaitingAck { .. })
        ));
        assert_eq!(socket_of(&engine, id), None);
    }
    engine.assert_invariants().unwrap();
}

/// The identified write-result ingress shares the Sending timeout policy. A
/// result at equality remains input-first and then reaches the due pass; a
/// result one nanosecond later must install the same target hold before it
/// terminalizes the raw inquiry.
#[test]
fn raw_single_flight_sending_inquiry_late_write_result_quarantines_before_successor_release() {
    let start = Instant::now();
    for offset in [Duration::ZERO, Duration::from_nanos(1)] {
        let mut engine = single_flight_raw_engine();
        let (first, first_id) = admit(
            &mut engine,
            1,
            inquiry_with_retry(1, immediate_retry_budget(Duration::from_millis(10))),
            start,
        );
        let at = start + Duration::from_millis(10) + offset;
        let finished = send_ok(&mut engine, &first, None, at);
        assert!(matches!(
            terminal_cause(&finished, first_id),
            Some(Error::Timeout { .. })
        ));
        let entered_reply = finished.iter().any(|effect| {
            matches!(
                effect,
                Effect::Transition {
                    to: Lifecycle::AwaitingReply { .. },
                    ..
                }
            )
        });
        assert_eq!(
            entered_reply,
            offset.is_zero(),
            "only a result at equality can enter reply waiting"
        );
        let (queued, successor) = admit(&mut engine, 2, inquiry(1), at);
        assert!(request_transmit_optional(&queued).is_none());
        let release_at = at + Duration::from_millis(50);
        assert_eq!(engine.next_wake(start), Some(release_at));
        let stale = engine.handle(unkeyed_reply(1, 0x0a), release_at);
        assert_eq!(ignored_reasons(&stale), [IgnoreReason::UnmatchedFrame]);
        assert_eq!(request_transmit(&stale).1, successor);
        send_ok(&mut engine, &stale, None, release_at);
        let own_reply = engine.handle(unkeyed_reply(1, 0x0b), release_at);
        assert!(
            matches!(terminal_outcome(&own_reply, successor), Some(RuntimeOutcome::Inquiry(InquiryOutcome::Reply { payload })) if payload.as_slice() == [0x0b])
        );
        engine.assert_invariants().unwrap();
    }
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
        command_with_reply_shape(1, CommandReply::CompletionOnly),
        start,
    );
    send_ok(&mut engine, &first, None, start);

    // B is already ready when A's uncorrelated terminal arrives. The target
    // tombstone must be present before `finish_input_turn` can dispatch it.
    let (second, second_id) = admit(&mut engine, 2, command(1), start);
    assert!(request_transmit_optional(&second).is_none());
    let turn = engine.begin_input_turn(start);
    let completed = engine.handle_in_turn(
        &turn,
        frame(1, None, DecodedResponse::Completion { socket: None }),
    );
    assert!(matches!(
        terminal_outcome(&completed, first_id),
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
    ));
    assert!(
        request_transmit_optional(&engine.finish_input_turn(turn, EngineTurn::Complete)).is_none()
    );
    assert!(matches!(phase_of(&engine, second_id), Some(Phase::Ready)));

    for response in [
        DecodedResponse::Ack {
            socket: Some(ViscaSocket::S1),
        },
        DecodedResponse::Completion { socket: None },
        DecodedResponse::Error {
            socket: None,
            code: 0x01.into(),
        },
    ] {
        let late = engine.handle(frame(1, None, response), start);
        assert!(late
            .iter()
            .any(|effect| matches!(effect, Effect::Ignored(_))));
        assert!(terminal_outcome(&late, second_id).is_none());
        assert!(matches!(phase_of(&engine, second_id), Some(Phase::Ready)));
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
        command_with_reply_shape(1, CommandReply::NoReply),
        start,
    );
    let written = send_ok(&mut engine, &first, None, start);
    assert!(matches!(
        terminal_outcome(&written, first_id),
        Some(RuntimeOutcome::Command(CommandOutcome::Written))
    ));

    let (inquiry, inquiry_id) = admit(&mut engine, 2, inquiry(1), start);
    assert!(request_transmit_optional(&inquiry).is_none());
    assert!(matches!(phase_of(&engine, inquiry_id), Some(Phase::Ready)));

    let stale = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x01.into(),
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
                code: 0x01.into(),
            },
        ),
        deadline,
    );
    assert!(
        matches!(
            terminal_cause(&legitimate, inquiry_id),
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
fn no_reply_waits_for_same_target_response_owners() {
    let start = Instant::now();

    // An executing socket-owning command makes a no-reply write wait rather
    // than letting its later target tombstone swallow this command's terminal.
    let mut command_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (command_effects, command_id) = admit(&mut command_engine, 1, command(1), start);
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
        command_with_reply_shape(1, CommandReply::NoReply),
        start,
    );
    assert!(request_transmit_optional(&no_reply_after_command).is_none());

    // An inquiry is equally unsafe: a socketless raw error has no
    // command/inquiry discriminator.
    let mut inquiry_engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (inquiry_effects, inquiry_id) = admit(&mut inquiry_engine, 1, inquiry(1), start);
    send_ok(&mut inquiry_engine, &inquiry_effects, None, start);
    assert!(matches!(
        phase_of(&inquiry_engine, inquiry_id),
        Some(Phase::AwaitingReply { .. })
    ));
    let (no_reply_after_inquiry, _) = admit(
        &mut inquiry_engine,
        2,
        command_with_reply_shape(1, CommandReply::NoReply),
        start,
    );
    assert!(request_transmit_optional(&no_reply_after_inquiry).is_none());

    command_engine.assert_invariants().unwrap();
    inquiry_engine.assert_invariants().unwrap();
}

/// A terminal tombstone excludes response-bearing successors, not another
/// fire-and-forget write. A no-reply successor consumes no response identity,
/// so it can safely extend the same fixed target hold.
#[test]
fn uncorrelatable_terminal_allows_no_reply_successor_and_extends_hold() {
    let start = Instant::now();
    for reply_shape in [CommandReply::NoReply, CommandReply::CompletionOnly] {
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
        let (first, first_id) = admit(
            &mut engine,
            1,
            command_with_reply_shape(1, reply_shape),
            start,
        );
        let first_terminal = match reply_shape {
            CommandReply::NoReply => send_ok(&mut engine, &first, None, start),
            CommandReply::CompletionOnly => {
                send_ok(&mut engine, &first, None, start);
                engine.handle(
                    frame(1, None, DecodedResponse::Completion { socket: None }),
                    start,
                )
            }
            CommandReply::Stop | CommandReply::AckThenCompletion => {
                unreachable!("loop contains only uncorrelatable shapes")
            }
        };
        assert!(terminal_outcome(&first_terminal, first_id).is_some());

        let successor_at = start + Duration::from_millis(1);
        let (second, second_id) = admit(
            &mut engine,
            2,
            command_with_reply_shape(1, CommandReply::NoReply),
            successor_at,
        );
        assert_eq!(request_transmit(&second).1, second_id);
        let second_terminal = send_ok(&mut engine, &second, None, successor_at);
        assert!(matches!(
            terminal_outcome(&second_terminal, second_id),
            Some(RuntimeOutcome::Command(CommandOutcome::Written))
        ));

        let extended_deadline = successor_at + Duration::from_millis(50);
        assert_eq!(engine.next_wake(start), Some(extended_deadline));
        let still_held = engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Error {
                    socket: None,
                    code: 0x01.into(),
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

    let (first, first_id) = admit(&mut engine, 1, command(1), start);
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
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
    ));
    let (second, second_id) = admit(&mut engine, 2, command(1), start);
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
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
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
            command_with_reply_shape(1, CommandReply::CompletionOnly),
            start,
        );
        send_ok(&mut runtime, &first, None, start);
        let second = admit(&mut runtime, 2, command(1), start).0;
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
        let (first, c1) = admit(&mut runtime, 1, command(1), start);
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
            command_with_reply_shape(1, CommandReply::CompletionOnly),
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
        command_with_reply_shape(1, CommandReply::CompletionOnly),
        start,
    );
    send_ok(&mut runtime, &completion_only, None, start);
    assert!(matches!(
        phase_of(&runtime, completion_id),
        Some(Phase::AwaitingCompletion { .. })
    ));

    let (same_target_inquiry, inquiry_id) = admit(&mut runtime, 2, inquiry(1), start);
    assert!(
        request_transmit_optional(&same_target_inquiry).is_none(),
        "a same-target inquiry must wait for the completion-only command",
    );
    assert!(matches!(phase_of(&runtime, inquiry_id), Some(Phase::Ready)));

    let (other_target_inquiry, other_target_id) = admit(&mut runtime, 3, inquiry(2), start);
    assert_eq!(
        request_transmit(&other_target_inquiry).1,
        other_target_id,
        "completion-only ownership is local to its target",
    );

    send_ok(&mut runtime, &other_target_inquiry, None, start);
    let error = runtime.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x01.into(),
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
    let (inquiry_effects, inquiry_id) = admit(&mut runtime, 1, inquiry(1), start);
    send_ok(&mut runtime, &inquiry_effects, None, start);
    assert!(matches!(
        phase_of(&runtime, inquiry_id),
        Some(Phase::AwaitingReply { .. })
    ));

    let (completion_only, completion_id) = admit(
        &mut runtime,
        2,
        command_with_reply_shape(1, CommandReply::CompletionOnly),
        start,
    );
    assert!(
        request_transmit_optional(&completion_only).is_none(),
        "a completion-only command must wait for a same-target inquiry reply",
    );
    assert!(matches!(
        phase_of(&runtime, completion_id),
        Some(Phase::Ready)
    ));

    let reply = runtime.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
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
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
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
        command_with_reply_shape(1, CommandReply::CompletionOnly),
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
            Some(0x1001),
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

    let (inquiry_effects, inquiry_id) = admit(&mut runtime, 2, inquiry(1), start);
    assert_eq!(
        request_transmit(&inquiry_effects).1,
        inquiry_id,
        "Sony sequence correlation must retain same-target concurrency",
    );
    send_ok(&mut runtime, &inquiry_effects, Some(0x1002), start);

    let completed = runtime.handle(
        frame(
            1,
            Some(0x1001),
            DecodedResponse::Completion { socket: None },
        ),
        start,
    );
    assert!(matches!(
        terminal_outcome(&completed, completion_id),
        Some(RuntimeOutcome::Command(CommandOutcome::Applied))
    ));
    let replied = runtime.handle(
        frame(
            1,
            Some(0x1002),
            DecodedResponse::InquiryReply {
                payload: smallvec![1],
            },
        ),
        start,
    );
    assert_eq!(terminal_id(&replied), Some(inquiry_id));
    runtime.assert_invariants().unwrap();
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
            command_with_reply_shape(1, CommandReply::CompletionOnly),
            start,
        );
        send_ok(&mut engine, &admitted_effects, None, start);

        let refused = engine.handle(
            frame(
                1,
                None,
                DecodedResponse::Error {
                    socket: None,
                    code: code.into(),
                },
            ),
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
        command_with_reply_shape(1, CommandReply::CompletionOnly),
        start,
    );
    send_ok(&mut engine, &admitted_effects, None, start);

    let refused = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x01.into(),
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
        command_with_reply_shape(1, CommandReply::CompletionOnly),
        start,
    );
    send_ok(&mut engine, &admitted_effects, None, start);

    let named = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: Some(ViscaSocket::S1),
                code: 0x01.into(),
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
                code: 0x01.into(),
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

/// A cancellation requested before ACK uses the later of the original ACK
/// deadline and the cancellation ambiguity deadline. Input is processed before
/// due work, so an ACK inside that effective window must still correlate and
/// start the socket-addressed cancellation.
#[test]
fn pre_ack_cancellation_extends_correlation_before_due_work() {
    let start = Instant::now();
    for envelope in [EnvelopeKind::Raw, EnvelopeKind::Sony] {
        let mut engine = engine(envelope, TransportKind::Datagram);
        let (admission, id) = admit(&mut engine, 1, command(1), start);
        let sequence = (envelope == EnvelopeKind::Sony).then(|| sony_sequence(id));
        send_ok(&mut engine, &admission, sequence, start);

        let cancel_at = start + Duration::from_millis(1);
        let requested = engine.handle(engine.cancel_input(id), cancel_at);
        assert!(cancel_transmit_optional(&requested).is_none());

        let ack = engine.handle(
            frame(
                1,
                sequence,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start + Duration::from_millis(20) + Duration::from_nanos(1),
        );
        assert!(
            ignored_reasons(&ack).is_empty(),
            "{envelope:?} ACK inside cancellation ambiguity was rejected: {ack:?}"
        );
        assert_eq!(cancel_transmit(&ack).1, id);
        assert!(terminal_outcome(&ack, id).is_none());
        engine.assert_invariants().unwrap();
    }
}

/// A completion-only command has no socket to cancel (#835). Refusing its
/// cancellation leaves its ordinary completion deadline authoritative.
#[test]
fn completion_only_refused_cancel_does_not_extend_correlation() {
    let start = Instant::now();
    for envelope in [EnvelopeKind::Raw, EnvelopeKind::Sony] {
        let mut engine = engine(envelope, TransportKind::Datagram);
        let (admission, id) = admit(
            &mut engine,
            1,
            command_with_reply_shape(1, CommandReply::CompletionOnly),
            start,
        );
        let sequence = (envelope == EnvelopeKind::Sony).then(|| sony_sequence(id));
        send_ok(&mut engine, &admission, sequence, start);

        let requested = engine.handle(engine.cancel_input(id), start + Duration::from_millis(1));
        assert!(cancel_transmit_optional(&requested).is_none());
        assert!(requested.iter().any(|effect| matches!(
            effect,
            Effect::CancellationObservation {
                observation: CancellationObservation::AttemptFailed(
                    Error::CancellationNotSupported
                ),
                ..
            }
        )));
        assert_eq!(engine.entry(id).unwrap().cancellation(), CancelState::None);

        let completion = engine.handle(
            frame(1, sequence, DecodedResponse::Completion { socket: None }),
            start + Duration::from_millis(40) + Duration::from_nanos(1),
        );
        if envelope == EnvelopeKind::Sony {
            assert!(matches!(
                terminal_outcome(&completion, id),
                Some(RuntimeOutcome::Command(CommandOutcome::Applied))
            ));
            assert_eq!(
                deadline_expiries(&completion, id),
                [(DeadlineKind::Completion, true)]
            );
        } else {
            assert!(ignored_reasons(&completion).contains(&IgnoreReason::UnmatchedFrame));
            assert!(!matches!(
                terminal_outcome(&completion, id),
                Some(RuntimeOutcome::Command(CommandOutcome::Applied))
            ));
        }
        engine.assert_invariants().unwrap();
    }
}

/// Overlap exclusion serializes Unknown inquiries per response source,
/// without blocking an independent target on the same serial bus.
#[test]
fn overlapping_raw_inquiries_are_serialized_per_target() {
    let start = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);

    let (first, first_id) = admit(&mut engine, 1, inquiry(1), start);
    assert_eq!(request_transmit(&first).1, first_id);
    send_ok(&mut engine, &first, None, start);
    let (second, second_id) = admit(&mut engine, 2, inquiry(2), start);
    assert_eq!(request_transmit(&second).1, second_id);
    send_ok(&mut engine, &second, None, start);
    let (third, third_id) = admit(&mut engine, 3, inquiry(1), start);
    assert!(request_transmit_optional(&third).is_none());

    let first_reply = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::InquiryReply {
                payload: smallvec![1],
            },
        ),
        start,
    );
    assert!(matches!(
        terminal_outcome(&first_reply, first_id),
        Some(RuntimeOutcome::Inquiry(InquiryOutcome::Reply { .. }))
    ));
    assert_eq!(request_transmit(&first_reply).1, third_id);
    assert!(engine.entry(second_id).is_some());
    engine.assert_invariants().unwrap();
}

fn declared_motion(target: u8, axes: crate::AffectedAxes, order: u64) -> RuntimeRequest {
    let mut request = command(target);
    request.context_mut().motion = Some(MotionEffect { axes });
    request.context_mut().submission_order = order;
    request
}

#[test]
fn halt_fence_suppresses_queued_declared_motion_only_and_preserves_new_work() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let halt_axes = crate::AffectedAxes::PAN_TILT | crate::AffectedAxes::ZOOM;
    let mut ids = Vec::new();
    for (ticket, request) in [
        declared_motion(1, halt_axes, 1),
        declared_motion(1, crate::AffectedAxes::ZOOM, 2),
        declared_motion(1, crate::AffectedAxes::IRIS, 3),
        declared_motion(2, crate::AffectedAxes::PAN_TILT, 3),
        command(1),
        declared_motion(1, crate::AffectedAxes::PAN_TILT, 6),
    ]
    .into_iter()
    .enumerate()
    {
        let effects = engine.handle_turn(
            Input::Admit {
                ticket: AdmissionTicket(ticket as u64 + 1),
                request,
            },
            now,
            EngineTurn::InputOnly,
        );
        ids.push(admitted(&effects));
    }
    let halted = engine.halt_in_turn(&engine.begin_input_turn(now), camera(1), halt_axes, 5);
    for (id, expected) in [(ids[0], halt_axes), (ids[1], crate::AffectedAxes::ZOOM)] {
        assert!(matches!(
            terminal_failure(&halted, id),
            Some(Error::MotionSuperseded { axes, context })
                if axes == expected
                    && context == FailureContext::new(FailureStage::Terminal, Certainty::NotAccepted)
        ));
    }
    for id in &ids[2..] {
        assert!(phase_of(&engine, *id).is_some());
    }
    // Submissions already in the ingress channel when the owner accepted
    // halt remain fenced even when admission runs after the halt lane.
    for (ticket, declared, expected) in [
        (20, crate::AffectedAxes::ZOOM, crate::AffectedAxes::ZOOM),
        (
            21,
            crate::AffectedAxes::PAN_TILT | crate::AffectedAxes::IRIS,
            crate::AffectedAxes::PAN_TILT,
        ),
    ] {
        let old_ingress = engine.handle_turn(
            Input::Admit {
                ticket: AdmissionTicket(ticket),
                request: declared_motion(1, declared, 4),
            },
            now,
            EngineTurn::InputOnly,
        );
        assert!(old_ingress.iter().any(|effect| matches!(
            effect,
            Effect::AdmissionRejected {
                error: Error::MotionSuperseded { axes, context },
                ..
            } if *axes == expected
                && *context == FailureContext::new(FailureStage::PreAdmission, Certainty::NotAccepted)
        )));
    }
    engine.assert_invariants().unwrap();
}

#[test]
fn halt_during_retry_backoff_suppresses_rewrite_without_claiming_no_prior_effect() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (effects, id) = admit(
        &mut engine,
        1,
        declared_motion(
            1,
            crate::AffectedAxes::PAN_TILT | crate::AffectedAxes::IRIS,
            1,
        ),
        now,
    );
    let (_, _, retry_at) = ack_timeout_retry(&mut engine, &effects, now);
    let halted = engine.halt_in_turn(
        &engine.begin_input_turn(now),
        camera(1),
        crate::AffectedAxes::PAN_TILT,
        2,
    );
    let error = terminal_failure(&halted, id).expect("retry terminalized by halt");
    assert!(
        matches!(error, Error::MotionSuperseded { axes, .. } if axes == crate::AffectedAxes::PAN_TILT)
    );
    assert_eq!(
        error.failure_context(),
        Some(FailureContext::new(
            FailureStage::Terminal,
            Certainty::Unconfirmed
        ))
    );
    assert!(request_transmit_optional(&engine.advance(retry_at)).is_none());
    engine.assert_invariants().unwrap();
}

#[test]
fn halt_of_written_motion_preserves_correlation_but_disables_future_retry() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
    let (effects, id) = admit(
        &mut engine,
        1,
        declared_motion(
            1,
            crate::AffectedAxes::PAN_TILT | crate::AffectedAxes::IRIS,
            1,
        ),
        now,
    );
    send_ok(&mut engine, &effects, Some(sony_sequence(id)), now);
    assert!(engine
        .halt_in_turn(
            &engine.begin_input_turn(now),
            camera(1),
            crate::AffectedAxes::PAN_TILT,
            2
        )
        .is_empty());
    assert!(phase_of(&engine, id).is_some());
    let timeout = engine.advance(now + Duration::from_millis(20));
    assert!(matches!(
        terminal_failure(&timeout, id),
        Some(Error::MotionSuperseded { axes, context })
            if axes == crate::AffectedAxes::PAN_TILT
                && context == FailureContext::new(FailureStage::Terminal, Certainty::Unconfirmed)
    ));
    assert!(retry_scheduled(&timeout).is_none());
    engine.assert_invariants().unwrap();
}

#[cfg(any(feature = "blocking", feature = "test-utils"))]
#[test]
fn halting_a_zero_retry_movement_preserves_its_own_ack_timeout() {
    use crate::request::CommandRetryClass;
    use crate::testing::manual_clock::ManualClock;

    for retry_class in [CommandRetryClass::Movement, CommandRetryClass::Never] {
        let clock = ManualClock::new();
        let start = clock.now();
        let registration = crate::TargetRegistration::new(
            crate::IndividualCameraId::CAMERA_1,
            crate::profiles::ProfileId::SonyFr7.spec().unwrap(),
        );
        let tuning = crate::OperationalTuning::new()
            .with_retry_limit(crate::tuning::RetryLimit::new(0).unwrap());
        let request = crate::raw::PlainRequest::new(
            [0x81, 0x01, 0x04, 0x00, 0x02, 0xff],
            crate::CommandTimeoutClass::Movement,
            retry_class,
            crate::SubmissionClass::Normal,
        )
        .unwrap()
        .with_motion_axes(crate::AffectedAxes::ZOOM);
        let mut request = crate::prepared::test_support::prepare_command(
            &request,
            &registration,
            tuning,
            crate::prepared::ClassSelection::Request,
        )
        .unwrap()
        .admit_with(|request, _| request.into_inner());
        request.context_mut().submission_order = 1;
        let ack_window = request.context().timeout.ack.get();
        assert_eq!(request.context().retry.max_retries, 0);

        let mut engine = engine(EnvelopeKind::Sony, TransportKind::Datagram);
        let (effects, id) = admit(&mut engine, 1, request, start);
        send_ok(&mut engine, &effects, Some(sony_sequence(id)), start);
        let turn = engine.begin_input_turn(start);
        assert!(engine
            .halt_in_turn(&turn, camera(1), crate::AffectedAxes::ZOOM, 1)
            .is_empty());

        clock.advance(ack_window);
        let timeout = engine.advance(clock.now());
        assert_eq!(
            deadline_expiries(&timeout, id),
            [(DeadlineKind::Ack, false)]
        );
        assert!(
            matches!(
                terminal_failure(&timeout, id),
                Some(Error::Timeout { context })
                    if context == FailureContext::new(FailureStage::Terminal, Certainty::Unconfirmed)
            ),
            "{retry_class:?}: {timeout:?}"
        );
    }
}

#[test]
fn halt_stop_write_cutoff_is_strict_and_stale_staged_effect_cannot_write() {
    for offset in [-1_i64, 0, 1] {
        let now = Instant::now();
        let deadline = now + Duration::from_millis(10);
        let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
        let mut stop = urgent_command(1);
        stop.context_mut().dispatch_deadline = Some(deadline);
        stop.context_mut().retry.total_budget =
            Some(NonZeroDuration::new(Duration::from_millis(10)).unwrap());
        let (effects, id) = admit(&mut engine, 1, stop, now);
        let (transmission, _, _) = request_transmit(&effects);
        let observed_at = if offset < 0 {
            deadline - Duration::from_nanos(1)
        } else {
            deadline + Duration::from_nanos(offset as u64)
        };
        let cutoff = engine.expire_unwritten_halt(id, transmission, observed_at);
        if offset < 0 {
            assert!(cutoff.is_none());
        } else {
            assert!(terminal_failure(&cutoff.expect("expired before driver write"), id).is_some());
            assert!(matches!(
                engine
                    .expire_unwritten_halt(id, transmission, observed_at)
                    .as_deref(),
                Some([Effect::Ignored(IgnoreReason::StaleTransmission)])
            ));
        }
        assert_eq!(engine.state(), SessionState::Running);
        engine.assert_invariants().unwrap();
    }
}

#[test]
fn halt_raw_siblings_wait_for_ack_not_completion_and_still_bypass_ordinary_motion() {
    let now = Instant::now();
    let mut stop = urgent_command(1);
    stop.context_mut().dispatch_deadline = Some(now + Duration::from_secs(1));
    stop.context_mut().submission_order = 2;
    let mut owner = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (first, first_id) = admit(&mut owner, 1, stop.clone(), now);
    send_ok(&mut owner, &first, None, now);
    let (second, second_id) = admit(&mut owner, 2, stop.clone(), now);
    assert!(
        request_transmit_optional(&second).is_none(),
        "sibling ACKs must remain attributable"
    );
    let ack = owner.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        now,
    );
    assert_eq!(
        request_transmit(&ack).1,
        second_id,
        "second STOP starts without first completion"
    );
    assert!(matches!(
        phase_of(&owner, first_id),
        Some(Phase::Executing { .. })
    ));
    owner.assert_invariants().unwrap();
    let mut owner = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let (ordinary, _) = admit(&mut owner, 1, command(1), now);
    send_ok(&mut owner, &ordinary, None, now);
    let (urgent, urgent_id) = admit(&mut owner, 2, stop, now);
    assert_eq!(
        request_transmit(&urgent).1,
        urgent_id,
        "#714 ordinary pre-ACK bypass remains available"
    );
    owner.assert_invariants().unwrap();
}

/// A raw stream engine whose Unknown inquiry declarations overlap per target.
fn single_flight_raw_stream_engine() -> ProtocolEngine {
    let configured = policy(EnvelopeKind::Raw, TransportKind::Stream);
    ProtocolEngine::new(
        configured,
        target_policies(&[camera(1), camera(2)], TargetPolicy::test_default()),
        Instant::now(),
    )
    .unwrap()
}

fn unkeyed_reply(target: u8, value: u8) -> Input {
    frame(
        target,
        None,
        DecodedResponse::InquiryReply {
            payload: smallvec![value],
        },
    )
}

/// The inquiry class's owed-reply state while the target's inquiry hold exists.
fn owed_state(engine: &ProtocolEngine, target: u8) -> Option<LaneState> {
    engine
        .raw_hold(camera(target), RawHoldScope::Inquiry(PayloadShape::Unknown))
        .map(|_| {
            engine
                .correlation
                .ledger()
                .lane_state(camera(target), Lane::Inquiry)
        })
}

/// Times out one written raw stream inquiry at `start`'s reply deadline and
/// returns its id and the deadline.
fn time_out_stream_inquiry(engine: &mut ProtocolEngine, start: Instant) -> (RequestId, Instant) {
    let (send, id) = admit(engine, 1, inquiry(1), start);
    send_ok(engine, &send, None, start);
    let timeout_at = start + Duration::from_millis(30);
    let timed_out = engine.advance(timeout_at);
    assert_eq!(
        terminal_failure(&timed_out, id).and_then(|error| error.failure_context()),
        Some(FailureContext::new(
            FailureStage::Terminal,
            Certainty::FailedConclusively
        )),
        "the reply timeout fails this inquiry at once, with no stream resend"
    );
    assert!(retry_scheduled(&timed_out).is_none());
    assert!(engine.entry(id).is_none());
    (id, timeout_at)
}

/// #795: on a stream a timed-out raw inquiry's reply is late,
/// not lost. It is owed: the inquiry class stays closed until that one
/// unkeyed reply is absorbed, so it can never complete a later inquiry.
#[test]
fn raw_stream_inquiry_timeout_owes_its_reply_and_never_binds_it_to_a_successor() {
    let start = Instant::now();
    let mut engine = single_flight_raw_stream_engine();
    let (id, timeout_at) = time_out_stream_inquiry(&mut engine, start);
    let window_end = timeout_at + Duration::from_millis(50);
    assert_eq!(
        engine.raw_hold(camera(1), RawHoldScope::Inquiry(PayloadShape::Unknown)),
        Some(RawHold {
            until: window_end,
            owner: Some(id),
        })
    );
    assert_eq!(owed_state(&engine, 1), Some(LaneState::Window));

    let (_, successor) = admit(&mut engine, 2, inquiry(1), timeout_at);
    assert!(matches!(phase_of(&engine, successor), Some(Phase::Ready)));

    let late = timeout_at + Duration::from_millis(40);
    let stale = engine.handle(unkeyed_reply(1, 0x02), late);
    assert_eq!(ignored_reasons(&stale), vec![IgnoreReason::UnmatchedFrame]);
    assert!(matches!(phase_of(&engine, successor), Some(Phase::Ready)));
    assert_eq!(
        engine.raw_hold(camera(1), RawHoldScope::Inquiry(PayloadShape::Unknown)),
        Some(RawHold {
            until: late + Duration::from_millis(50),
            owner: Some(id),
        }),
        "a settled debt falls back to the ordinary reply skew"
    );

    // At the original window end the settled hold no longer latches; after
    // the skew the successor is written and only its own reply completes it.
    engine.advance(window_end);
    assert!(matches!(phase_of(&engine, successor), Some(Phase::Ready)));
    let released = engine.advance(late + Duration::from_millis(50));
    let (_, written, _) = request_transmit(&released);
    assert_eq!(written, successor);
    send_ok(
        &mut engine,
        &released,
        None,
        late + Duration::from_millis(50),
    );
    let reply = engine.handle(unkeyed_reply(1, 0x03), late + Duration::from_millis(51));
    assert!(matches!(
        terminal_outcome(&reply, successor),
        Some(RuntimeOutcome::Inquiry(InquiryOutcome::Reply { payload, .. })) if payload.as_slice() == [0x03]
    ));
    engine.assert_invariants().unwrap();
}

/// An owed reply is settled by any complete input applied before the due pass
/// that would latch it — here exactly at the window end — including an
/// attributable socketless error.
#[test]
fn owed_raw_stream_inquiry_reply_settles_by_socketless_error_before_the_due_pass() {
    let start = Instant::now();
    let mut engine = single_flight_raw_stream_engine();
    let (_, timeout_at) = time_out_stream_inquiry(&mut engine, start);
    let window_end = timeout_at + Duration::from_millis(50);
    let error = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x02.into(),
            },
        ),
        window_end,
    );
    assert_eq!(ignored_reasons(&error), vec![IgnoreReason::UnmatchedFrame]);
    assert_eq!(owed_state(&engine, 1), Some(LaneState::Clear));
    engine.assert_invariants().unwrap();
}

/// A socketless error is a rejection, the first answer of the oldest
/// outstanding request: an executing command's error names its socket. So on
/// a byte stream it settles the owed reply even while a command executes
/// (#795); leaving it unpaid would make the next answer pay it and shift
/// every later answer onto the wrong request.
#[test]
fn socketless_error_settles_the_owed_reply_while_a_command_executes() {
    let start = Instant::now();
    let mut engine = single_flight_raw_stream_engine();
    let (_, timeout_at) = time_out_stream_inquiry(&mut engine, start);

    let (command_send, command_id) = admit(&mut engine, 2, command(1), timeout_at);
    send_ok(&mut engine, &command_send, None, timeout_at);
    engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        timeout_at + Duration::from_millis(1),
    );
    assert!(matches!(
        phase_of(&engine, command_id),
        Some(Phase::Executing { .. })
    ));
    let error = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x41.into(),
            },
        ),
        timeout_at + Duration::from_millis(2),
    );
    assert_eq!(ignored_reasons(&error), vec![IgnoreReason::UnmatchedFrame]);
    assert_eq!(owed_state(&engine, 1), Some(LaneState::Clear));
    assert!(matches!(
        phase_of(&engine, command_id),
        Some(Phase::Executing { .. })
    ));
    engine.assert_invariants().unwrap();
}

/// A reply still owed at the window end latches only that target's inquiry
/// lane: queued and new inquiries to it fail promptly, unwritten and
/// `NotAccepted`, while the session, commands to the same target, and
/// inquiries to another target keep working. The latch never wakes the
/// owner. A reply arriving after the window still settles it and reopens the
/// lane with correct data.
#[test]
fn unsettled_owed_reply_latches_only_that_targets_inquiries_until_it_arrives() {
    let start = Instant::now();
    let mut engine = single_flight_raw_stream_engine();
    let (_, timeout_at) = time_out_stream_inquiry(&mut engine, start);
    let window_end = timeout_at + Duration::from_millis(50);
    let (_, queued) = admit(&mut engine, 2, inquiry(1), timeout_at);

    let still_owed = engine.advance(window_end - Duration::from_millis(1));
    assert!(terminal_outcome(&still_owed, queued).is_none());
    let latched = engine.advance(window_end);
    assert_eq!(engine.state(), SessionState::Running);
    assert_eq!(owed_state(&engine, 1), Some(LaneState::Latched));
    let failure = terminal_failure(&latched, queued).expect("queued inquiry fails at the latch");
    assert!(matches!(
        &failure,
        Error::InquiryFailed { source } if matches!(source.as_ref(), Error::InquiryCorrelationLost { camera: lost } if *lost == camera(1).individual_camera())
    ));
    assert_eq!(
        failure.failure_context(),
        Some(FailureContext::new(
            FailureStage::Terminal,
            Certainty::FailedConclusively
        ))
    );
    assert!(!failure.requires_new_session());
    assert!(!matches!(
        failure.recovery(),
        crate::Recovery::Resubmit { .. }
    ));
    assert!(request_transmit_optional(&latched).is_none());
    assert!(
        engine.next_wake(start).is_none_or(|wake| wake > window_end),
        "a latched lane must not wake the owner"
    );

    let rejected = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: inquiry(1),
        },
        window_end + Duration::from_millis(1),
    );
    assert!(rejected.iter().any(|effect| matches!(
        effect,
        Effect::AdmissionRejected {
            error: Error::InquiryCorrelationLost { camera: lost },
            ..
        } if *lost == camera(1).individual_camera()
    )));

    // A command to the latched target and an inquiry to another target still
    // dispatch.
    let now = window_end + Duration::from_millis(2);
    let (command_send, command_id) = admit(&mut engine, 4, command(1), now);
    assert_eq!(request_transmit(&command_send).1, command_id);
    send_ok(&mut engine, &command_send, None, now);
    let (other_send, other_id) = admit(&mut engine, 5, inquiry(2), now);
    assert_eq!(request_transmit(&other_send).1, other_id);
    send_ok(&mut engine, &other_send, None, now);
    let other_reply = engine.handle(unkeyed_reply(2, 0x03), now);
    assert!(matches!(
        terminal_outcome(&other_reply, other_id),
        Some(RuntimeOutcome::Inquiry(InquiryOutcome::Reply { payload, .. })) if payload.as_slice() == [0x03]
    ));

    // The owed reply finally arrives: it is discarded and the lane reopens
    // after the ordinary skew.
    let late = window_end + Duration::from_secs(5);
    let settled = engine.handle(unkeyed_reply(1, 0x02), late);
    assert_eq!(
        ignored_reasons(&settled),
        vec![IgnoreReason::UnmatchedFrame]
    );
    assert_eq!(owed_state(&engine, 1), Some(LaneState::Clear));
    let reopened = late + Duration::from_millis(50);
    engine.advance(reopened);
    let (send, id) = admit(&mut engine, 6, inquiry(1), reopened);
    send_ok(&mut engine, &send, None, reopened);
    let reply = engine.handle(unkeyed_reply(1, 0x03), reopened);
    assert!(matches!(
        terminal_outcome(&reply, id),
        Some(RuntimeOutcome::Inquiry(InquiryOutcome::Reply { payload, .. })) if payload.as_slice() == [0x03]
    ));
    assert_eq!(engine.state(), SessionState::Running);
    engine.assert_invariants().unwrap();
}

fn short_budget() -> RetryPolicy {
    RetryPolicy {
        total_budget: Some(NonZeroDuration::new(Duration::from_millis(20)).unwrap()),
        ..retrying()
    }
}

/// A successful stream write sampled after the total budget proves the bytes
/// entered the stream, so the inquiry's reply is owed.
#[test]
fn stream_inquiry_write_succeeding_after_its_budget_owes_its_reply() {
    let start = Instant::now();
    let mut engine = single_flight_raw_stream_engine();
    let (send, id) = admit(&mut engine, 1, inquiry_with_retry(1, short_budget()), start);
    let (transmission, _, _) = request_transmit(&send);
    let late = start + Duration::from_millis(21);
    let finished = finish_write_input_only(
        &mut engine,
        id,
        transmission,
        Ok(TransmissionMeta::for_sequence(None)),
        late,
    );
    assert!(matches!(
        terminal_cause(&finished, id),
        Some(Error::Timeout { .. })
    ));
    assert_eq!(
        engine.raw_hold(camera(1), RawHoldScope::Inquiry(PayloadShape::Unknown)),
        Some(RawHold {
            until: late + Duration::from_millis(50),
            owner: Some(id),
        })
    );
    assert_eq!(owed_state(&engine, 1), Some(LaneState::Window));
    engine.assert_invariants().unwrap();
}

/// Datagram behaviour is unchanged: a reply timeout keeps the short skew hold
/// with no owed reply and retries under policy.
#[test]
fn raw_datagram_inquiry_timeout_keeps_skew_hold_and_retry() {
    let start = Instant::now();
    let mut engine = single_flight_raw_engine();
    let (send, id) = admit(&mut engine, 1, inquiry(1), start);
    send_ok(&mut engine, &send, None, start);
    let timeout_at = start + Duration::from_millis(30);
    let timed_out = engine.advance(timeout_at);
    assert!(retry_scheduled(&timed_out).is_some_and(|(retried, ..)| retried == id));
    assert_eq!(
        engine.raw_hold(camera(1), RawHoldScope::Inquiry(PayloadShape::Unknown)),
        Some(RawHold {
            until: timeout_at + Duration::from_millis(50),
            owner: Some(id),
        })
    );
}

/// Latch the owed reply on camera 1 of a fresh stream engine and return the
/// latch instant.
fn latch_camera_one(engine: &mut ProtocolEngine, start: Instant) -> Instant {
    let (_, timeout_at) = time_out_stream_inquiry(engine, start);
    let window_end = timeout_at + Duration::from_millis(50);
    engine.advance(window_end);
    assert_eq!(owed_state(engine, 1), Some(LaneState::Latched));
    window_end
}

/// A latched owed reply was written before any later command, so on a byte
/// stream a socketless error is that reply's late answer: stream order
/// settles the inquiry class (#795), and the command's own rejection, which
/// follows it, still fails the command.
#[test]
fn latched_target_settles_its_owed_reply_before_a_commands_socketless_rejection() {
    let start = Instant::now();
    let mut engine = single_flight_raw_stream_engine();
    let latched_at = latch_camera_one(&mut engine, start);

    let (send, id) = admit(
        &mut engine,
        2,
        command_with_reply_shape_and_retry(1, CommandReply::AckThenCompletion, RetryPolicy::NEVER),
        latched_at,
    );
    assert_eq!(request_transmit(&send).1, id, "commands still dispatch");
    send_ok(&mut engine, &send, None, latched_at);
    let busy = || {
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
            },
        )
    };
    let owed = engine.handle(busy(), latched_at + Duration::from_millis(1));
    assert_eq!(ignored_reasons(&owed), vec![IgnoreReason::UnmatchedFrame]);
    assert_eq!(owed_state(&engine, 1), Some(LaneState::Clear));
    assert!(engine.entry(id).is_some());
    let rejected = engine.handle(busy(), latched_at + Duration::from_millis(2));
    assert!(matches!(
        terminal_failure(&rejected, id),
        Some(Error::CommandBufferFull)
    ));
    assert_eq!(engine.state(), SessionState::Running);
    engine.assert_invariants().unwrap();
}

/// A user `CompletionOnly` command shares the unkeyed correlation that the
/// latch keeps filtering. Rather than wait — with no deadline at all when its
/// retry budget is unbounded — it fails promptly, unwritten, both when queued
/// at the latch and when admitted afterwards.
#[test]
fn latched_target_fails_completion_only_commands_promptly() {
    let start = Instant::now();
    let mut engine = single_flight_raw_stream_engine();
    let (_, timeout_at) = time_out_stream_inquiry(&mut engine, start);
    let unbounded = RetryPolicy {
        total_budget: None,
        ..RetryPolicy::NEVER
    };
    let completion_only =
        || command_with_reply_shape_and_retry(1, CommandReply::CompletionOnly, unbounded);
    let (queued_send, queued) = admit(&mut engine, 2, completion_only(), timeout_at);
    assert!(request_transmit_optional(&queued_send).is_none());

    let window_end = timeout_at + Duration::from_millis(50);
    let latched = engine.advance(window_end);
    assert!(matches!(
        terminal_failure(&latched, queued),
        Some(Error::InquiryCorrelationLost { .. })
    ));
    let rejected = engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(3),
            request: completion_only(),
        },
        window_end,
    );
    assert!(rejected.iter().any(|effect| matches!(
        effect,
        Effect::AdmissionRejected {
            error: Error::InquiryCorrelationLost { .. },
            ..
        }
    )));
    engine.assert_invariants().unwrap();
}

#[test]
fn halt_uses_its_turn_instant() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
    let (write, id) = admit(
        &mut engine,
        1,
        declared_motion(1, crate::AffectedAxes::PAN_TILT, 1),
        now,
    );
    send_ok(&mut engine, &write, None, now);
    let rejected = engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
            },
        ),
        now,
    );
    assert!(retry_scheduled(&rejected).is_some());
    assert!(matches!(phase_of(&engine, id), Some(Phase::Backoff { .. })));

    let halt_at = now + Duration::from_secs(30);
    let turn = engine.begin_input_turn(halt_at);
    let halted = engine.halt_in_turn(&turn, camera(1), crate::AffectedAxes::PAN_TILT, 2);
    assert!(matches!(
        terminal_failure(&halted, id),
        Some(Error::MotionSuperseded { .. })
    ));
    engine.assert_invariants().unwrap();
}

#[test]
fn next_wake_ignores_a_stream_socket_hold_that_expired_after_the_last_turn() {
    let now = Instant::now();
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Stream);
    let hold_until = now + Duration::from_millis(50);
    extend_fixture_hold(
        &mut engine,
        camera(1),
        StreamScope::Socket(ViscaSocket::S1),
        hold_until,
        None,
    );
    let admitted = engine.handle_turn(
        Input::Admit {
            ticket: AdmissionTicket(1),
            request: command_with_reply_shape_and_retry(
                1,
                CommandReply::CompletionOnly,
                retrying(),
            ),
        },
        now,
        EngineTurn::InputOnly,
    );
    let id = admitted
        .iter()
        .find_map(|effect| match effect {
            Effect::Admitted { id, .. } => Some(*id),
            _ => None,
        })
        .unwrap();
    assert_eq!(engine.next_wake(now), Some(hold_until));

    let sampled_at = hold_until + Duration::from_millis(1);
    // The deadline opens the dispatch gate even without another engine turn.
    // The waiting command supplies its pacing wake, so filtering the expired
    // hold cannot hide work from the owner's planner.
    assert_eq!(engine.next_wake(sampled_at), Some(now));
    assert!(!engine.dispatch_blocked(id, engine.entry(id).unwrap(), sampled_at));
    engine.assert_invariants().unwrap();
}
