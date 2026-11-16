//! Camera-tagged inquiry sessions with skipped and delayed first answers.
//!
//! The oracle records only admissions, actual writes and camera answers. It
//! never consults an entry's chosen owner or the production shape classifier.
//! Streams preserve first-answer order among non-skipped answers per camera;
//! datagrams may reorder every answer. Same-shape ambiguity and Unknown retain
//! their documented residual, but no reply may bind outside its payload domain.

use std::collections::{BTreeMap, VecDeque};

use super::*;
use crate::PayloadShape;

const SHAPES: [PayloadShape; 11] = [
    PayloadShape::Bytes(1),
    PayloadShape::Nibbles(1),
    PayloadShape::Nibbles(2),
    PayloadShape::Nibbles(3),
    PayloadShape::Bytes(4),
    PayloadShape::Nibbles(4),
    PayloadShape::Bytes(7),
    PayloadShape::Nibbles(8),
    PayloadShape::Nibbles(9),
    PayloadShape::Nibbles(10),
    PayloadShape::Unknown,
];
const STEPS: usize = 1_200;
const SEEDS: u64 = 32;
const REPLY_MS: u64 = 12;

// The oracle deliberately does not call PayloadShape::admits/overlaps.
fn oracle_length(shape: PayloadShape) -> Option<usize> {
    match shape {
        PayloadShape::Unknown => None,
        PayloadShape::Bytes(length) | PayloadShape::Nibbles(length) => Some(length),
    }
}

fn oracle_admits(shape: PayloadShape, payload: &[u8]) -> bool {
    oracle_length(shape).is_none_or(|length| length == payload.len())
        && (!matches!(shape, PayloadShape::Nibbles(_))
            || payload.iter().all(|byte| byte & 0xf0 == 0))
}

fn oracle_overlaps(left: PayloadShape, right: PayloadShape) -> bool {
    match (oracle_length(left), oracle_length(right)) {
        (Some(left), Some(right)) => left == right,
        _ => true,
    }
}

#[derive(Debug, Clone, Copy)]
enum AnswerMode {
    Skip,
    After(u64),
}

#[derive(Debug)]
struct Record {
    target: u8,
    shape: PayloadShape,
    mode: AnswerMode,
    written: bool,
    terminal: bool,
    replied: bool,
}

#[derive(Debug)]
struct Answer {
    origin: RequestId,
    target: u8,
    declared: PayloadShape,
    payload: SmallVec<[u8; crate::raw::INLINE_BYTES]>,
    due: Instant,
    order: u64,
}

#[derive(Debug, Default)]
struct Coverage {
    submitted: u64,
    rejected: u64,
    written: u64,
    replies: u64,
    skipped: u64,
    delayed: u64,
    unbound_answers: u64,
    same_class_rebindings: u64,
    written_shapes: [u64; SHAPES.len()],
}

impl Coverage {
    fn merge(&mut self, other: Self) {
        self.submitted += other.submitted;
        self.rejected += other.rejected;
        self.written += other.written;
        self.replies += other.replies;
        self.skipped += other.skipped;
        self.delayed += other.delayed;
        self.unbound_answers += other.unbound_answers;
        self.same_class_rebindings += other.same_class_rebindings;
        for (total, count) in self.written_shapes.iter_mut().zip(other.written_shapes) {
            *total += count;
        }
    }
}

struct Session {
    engine: ProtocolEngine,
    transport: TransportKind,
    now: Instant,
    random: u64,
    seed: u64,
    ticket: u64,
    order: u64,
    records: BTreeMap<RequestId, Record>,
    answers: Vec<Answer>,
    stream_tail: [Instant; 2],
    coverage: Coverage,
}

impl Session {
    fn new(transport: TransportKind, capacity: usize, seed: u64) -> Self {
        let now = Instant::now();
        let mut configured = policy(EnvelopeKind::Raw, transport);
        configured.capacity = capacity;
        configured.inquiry_capacity = 8.min(capacity);
        configured.inquiry_spacing = Duration::from_millis(2);
        configured.raw_inquiry_release_hold = Duration::from_millis(5);
        configured.raw_release_grace = Duration::from_millis(8);
        let mut engine = ProtocolEngine::new(configured, now).unwrap();
        for target in [camera(1), camera(2)] {
            engine
                .register_target(target, TargetPolicy::test_default())
                .unwrap();
        }
        Self {
            engine,
            transport,
            now,
            random: seed,
            seed,
            ticket: 0,
            order: 0,
            records: BTreeMap::new(),
            answers: Vec::new(),
            stream_tail: [now; 2],
            coverage: Coverage::default(),
        }
    }

    fn random(&mut self) -> u64 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        self.random
    }

    fn submit(&mut self, target: u8, shape: PayloadShape, mode: AnswerMode) -> Option<RequestId> {
        self.ticket += 1;
        let mut request = inquiry_with_retry(target, RetryPolicy::NEVER);
        let RuntimeRequest::Inquiry {
            context,
            payload_shape,
            ..
        } = &mut request
        else {
            unreachable!("the inquiry fixture has the inquiry request class");
        };
        *payload_shape = shape;
        context.timeout.inquiry = NonZeroDuration::new(Duration::from_millis(REPLY_MS)).unwrap();
        context.timeout.ambiguity = NonZeroDuration::new(Duration::from_millis(40)).unwrap();
        context.dispatch_deadline = Some(self.now + Duration::from_millis(100));
        let effects = self.engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(self.ticket),
                request,
                slot: AdmissionSlot::Ordinary,
            },
            self.now,
        );
        let id = effects.iter().find_map(|effect| match effect {
            Effect::Admitted { id, .. } => Some(*id),
            _ => None,
        });
        if let Some(id) = id {
            self.coverage.submitted += 1;
            assert!(self
                .records
                .insert(
                    id,
                    Record {
                        target,
                        shape,
                        mode,
                        written: false,
                        terminal: false,
                        replied: false,
                    }
                )
                .is_none());
        }
        self.process(effects, None);
        id
    }

    fn payload(shape: PayloadShape, tag: u64) -> SmallVec<[u8; crate::raw::INLINE_BYTES]> {
        match shape {
            PayloadShape::Unknown => {
                Self::payload(SHAPES[(tag as usize) % (SHAPES.len() - 1)], tag)
            }
            PayloadShape::Bytes(length) => {
                std::iter::repeat_n(0x80 | ((tag % 127) as u8), length).collect()
            }
            PayloadShape::Nibbles(length) => {
                std::iter::repeat_n((tag % 15 + 1) as u8, length).collect()
            }
        }
    }

    fn process(&mut self, effects: Vec<Effect>, origin: Option<&Answer>) {
        let mut effects = VecDeque::from(effects);
        while let Some(effect) = effects.pop_front() {
            match effect {
                Effect::Transmit {
                    transmission,
                    request,
                    kind: Transmission::Request { target, .. },
                } => {
                    self.order += 1;
                    let record = self
                        .records
                        .get_mut(&request)
                        .expect("only admitted work writes");
                    assert_eq!(target, camera(record.target));
                    assert!(
                        !record.written,
                        "Never retry policy wrote {request:?} twice"
                    );
                    record.written = true;
                    self.coverage.written += 1;
                    if let Some(index) = SHAPES.iter().position(|shape| *shape == record.shape) {
                        self.coverage.written_shapes[index] += 1;
                    }
                    match record.mode {
                        AnswerMode::Skip => self.coverage.skipped += 1,
                        AnswerMode::After(delay) => {
                            let mut due = self.now + Duration::from_millis(delay);
                            if self.transport == TransportKind::Stream {
                                let tail = &mut self.stream_tail[usize::from(record.target - 1)];
                                due = due.max(*tail);
                                *tail = due;
                            }
                            self.coverage.delayed +=
                                u64::from(due > self.now + Duration::from_millis(REPLY_MS + 5));
                            self.answers.push(Answer {
                                origin: request,
                                target: record.target,
                                declared: record.shape,
                                payload: Self::payload(record.shape, request.get()),
                                due,
                                order: self.order,
                            });
                        }
                    }
                    let nested = self.engine.handle(
                        Input::TransmissionFinished {
                            request,
                            transmission,
                            result: WriteOutcome::Sent(TransmissionMeta::Raw),
                        },
                        self.now,
                    );
                    for effect in nested.into_iter().rev() {
                        effects.push_front(effect);
                    }
                }
                Effect::Terminal { id, outcome } => {
                    let record = self
                        .records
                        .get_mut(&id)
                        .expect("terminal has an admission");
                    assert!(!record.terminal, "duplicate terminal for {id:?}");
                    record.terminal = true;
                    if let RuntimeOutcome::Inquiry(InquiryOutcome::Reply { payload }) = outcome {
                        let answer = origin.expect("a reply terminal needs a camera-tagged answer");
                        assert!(record.written, "an unwritten inquiry received a reply");
                        assert_eq!(
                            record.target, answer.target,
                            "a reply crossed camera targets"
                        );
                        assert_eq!(
                            payload.as_slice(),
                            answer.payload.as_slice(),
                            "reply lost its camera tag"
                        );
                        assert!(
                            oracle_admits(record.shape, &payload),
                            "cross-domain bind: {:?} got {:?} from {:?}",
                            record.shape,
                            payload,
                            answer.declared
                        );
                        assert!(
                            oracle_overlaps(record.shape, answer.declared),
                            "cross-shape origin: {:?} got {:?}'s answer",
                            record.shape,
                            answer.declared
                        );
                        self.coverage.same_class_rebindings += u64::from(id != answer.origin);
                        self.coverage.replies += 1;
                        record.replied = true;
                    }
                }
                Effect::AdmissionRejected { .. } => self.coverage.rejected += 1,
                Effect::SessionEnded(end) => {
                    assert_eq!(
                        end.status.state(),
                        SessionState::Running,
                        "an inquiry workload ended its session"
                    );
                }
                Effect::Transmit {
                    kind: Transmission::Cancel { .. },
                    ..
                } => panic!("model admits no cancellations"),
                _ => {}
            }
        }
        for target in [camera(1), camera(2)] {
            assert!(
                self.engine.tracked_inquiry_shapes(target).unwrap().len()
                    <= MAX_RAW_INQUIRY_SHAPE_CLASSES_PER_TARGET,
                "derived declaration union exceeded its admission budget"
            );
        }
        self.engine
            .assert_invariants()
            .unwrap_or_else(|error| panic!("seed {} at {:?}: {error}", self.seed, self.now));
    }

    fn deliver(&mut self, answer: &Answer) {
        let replies = self.coverage.replies;
        let effects = self.engine.handle(
            frame(
                answer.target,
                None,
                DecodedResponse::InquiryReply {
                    payload: answer.payload.clone(),
                },
            ),
            self.now,
        );
        self.process(effects, Some(answer));
        self.coverage.unbound_answers += u64::from(self.coverage.replies == replies);
    }

    fn tick(&mut self, milliseconds: u64) {
        self.now += Duration::from_millis(milliseconds);
        self.answers
            .sort_unstable_by_key(|answer| (answer.due, answer.order));
        while self
            .answers
            .first()
            .is_some_and(|answer| answer.due <= self.now)
        {
            let answer = self.answers.remove(0);
            self.deliver(&answer);
        }
        let effects = self.engine.advance(self.now);
        self.process(effects, None);
    }

    fn run(transport: TransportKind, seed: u64) -> Coverage {
        let mut model = Self::new(transport, 32, seed);
        for _ in 0..STEPS {
            let live = model
                .records
                .values()
                .filter(|record| !record.terminal)
                .count();
            if live < 24 && !model.random().is_multiple_of(3) {
                let target = 1 + (model.random() % 2) as u8;
                let shape = SHAPES[model.random() as usize % SHAPES.len()];
                let choice = model.random() % 10;
                let mode = if choice < 2 && shape != PayloadShape::Unknown {
                    AnswerMode::Skip
                } else if choice < 3 {
                    AnswerMode::After(80 + model.random() % 61)
                } else {
                    AnswerMode::After(1 + model.random() % 5)
                };
                let _ = model.submit(target, shape, mode);
            }
            let delay = 1 + model.random() % 4;
            model.tick(delay);
        }
        for _ in 0..200 {
            model.tick(2);
        }
        assert!(model.answers.is_empty(), "the finite link delay must drain");
        model.coverage
    }
}

fn exercise(transport: TransportKind) {
    let mut coverage = Coverage::default();
    for seed in 1..=SEEDS {
        coverage.merge(Session::run(transport, seed));
    }
    assert!(
        coverage.written > 100 && coverage.replies > 50,
        "{coverage:?}"
    );
    assert!(
        coverage.skipped > 20 && coverage.delayed > 20,
        "{coverage:?}"
    );
    assert!(
        coverage.unbound_answers > 0,
        "late/skipped workloads must exercise discards: {coverage:?}"
    );
    assert!(
        coverage.written_shapes.iter().all(|count| *count > 0),
        "every declared shape must write: {coverage:?}"
    );
}

#[test]
fn randomized_stream_mixed_shapes_never_cross_bind() {
    exercise(TransportKind::Stream);
}

#[test]
fn randomized_datagram_mixed_shapes_never_cross_bind() {
    exercise(TransportKind::Datagram);
}

#[test]
fn queued_unknown_does_not_transitively_latch_disjoint_shapes() {
    let mut model = Session::new(TransportKind::Stream, 8, 1);
    let owed = model
        .submit(1, PayloadShape::Bytes(1), AnswerMode::Skip)
        .unwrap();
    assert!(model.records[&owed].written);
    model.tick(REPLY_MS + 1);
    assert!(model.records[&owed].terminal);
    let unknown = model
        .submit(1, PayloadShape::Unknown, AnswerMode::After(1))
        .unwrap();
    assert!(
        !model.records[&unknown].written,
        "Unknown overlaps the owed byte"
    );
    let position = model
        .submit(1, PayloadShape::Nibbles(4), AnswerMode::After(1))
        .unwrap();
    assert!(
        model.records[&position].written,
        "a queued Unknown is not an overlap equivalence class"
    );
    assert!(!model.records[&unknown].written);
    model.tick(1);
    assert!(
        model.records[&position].replied,
        "disjoint position must survive a skipped mode reply"
    );
    model.tick(10); // allow the configured skew and inquiry pacing to elapse
    assert!(
        model.records[&unknown].written,
        "write-order settlement must release the older debt"
    );
}

#[test]
fn distinct_shape_debts_stay_bounded_at_single_request_capacity() {
    let mut model = Session::new(TransportKind::Stream, 1, 1);
    let bound = stream_ledger::max_entries_per_target(1);
    let attempts = MAX_RAW_INQUIRY_SHAPE_CLASSES_PER_TARGET * 8 + 1;
    let mut first_written = None;
    for length in 1..=attempts {
        if let Some(id) = model.submit(1, PayloadShape::Bytes(length), AnswerMode::Skip) {
            if model.records[&id].written && first_written.is_none() {
                first_written = Some(id);
            }
        }
        model.tick(60);
        assert!(model.engine.correlation.ledger().len(camera(1)) <= bound);
    }
    assert_eq!(
        model.coverage.written as usize,
        MAX_RAW_INQUIRY_SHAPE_CLASSES_PER_TARGET
    );
    assert!(
        model.coverage.rejected > 0,
        "new declarations need explicit bounded backpressure"
    );
    assert_eq!(
        model
            .engine
            .tracked_inquiry_shapes(camera(1))
            .unwrap()
            .len(),
        MAX_RAW_INQUIRY_SHAPE_CLASSES_PER_TARGET
    );
    // Shape pressure must not consume the command lane. Leave this command
    // awaiting ACK while probing the oldest debt so no ACK can heal it first.
    model.ticket += 1;
    let effects = model.engine.handle(
        Input::Admit {
            ticket: AdmissionTicket(model.ticket),
            request: command(1, CancellationPolicy::Supported),
            slot: AdmissionSlot::Ordinary,
        },
        model.now,
    );
    let command = admitted(&effects);
    assert_eq!(request_transmit(&effects).1, command);
    let _ = send_ok(&mut model.engine, &effects, None, model.now);
    model.engine.assert_invariants().unwrap();
    let first = first_written.expect("the first shape is admitted");
    let debts = model.engine.correlation.ledger().debts(camera(1));
    assert!(
        debts > 0,
        "bounded pressure must retain the unanswered evidence"
    );
    let record = &model.records[&first];
    let answer = Answer {
        origin: first,
        target: 1,
        declared: record.shape,
        payload: Session::payload(record.shape, first.get()),
        due: model.now,
        order: 0,
    };
    model.deliver(&answer);
    assert!(
        model.engine.correlation.ledger().debts(camera(1)) < debts,
        "the oldest real answer must still find its retained debt after bounded pressure"
    );
    let _ = model.engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        model.now,
    );
    let effects = model.engine.handle(
        frame(
            1,
            None,
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        model.now,
    );
    assert!(effects.iter().any(|effect| matches!(effect, Effect::Terminal { id, outcome: RuntimeOutcome::Command(CommandOutcome::Applied) } if *id == command)));
    model.engine.assert_invariants().unwrap();
}

/// Distinct inquiry debts split otherwise mergeable ACK runs. Urgent STOPs
/// bypass the ordinary debt cap, so the shape reservation must bound both the
/// inquiry rows and the ACK-run separators, independently for every camera.
#[test]
fn alternating_inquiry_and_stop_debts_bound_the_ack_separator_charge() {
    const TAIL_STOPS: usize = 512;
    let mut now = Instant::now();
    let mut configured = policy(EnvelopeKind::Raw, TransportKind::Stream);
    configured.wire =
        WireProtocol::new(EnvelopeKind::Raw, crate::transport::AddressingMode::Serial);
    configured.capacity = 1;
    configured.inquiry_capacity = 1;
    configured.raw_inquiry_release_hold = Duration::from_millis(1);
    let mut engine = ProtocolEngine::new(configured, now).unwrap();
    for target in 1..=7 {
        engine
            .register_target(camera(target), TargetPolicy::test_default())
            .unwrap();
    }
    // No reserve is needed: each preceding request has already terminalized,
    // so A is exactly one while the urgent lane still bypasses the debt cap.
    let bound = stream_ledger::max_entries_per_target(1);
    let classes = MAX_RAW_INQUIRY_SHAPE_CLASSES_PER_TARGET;
    let mut ticket = 0;

    fn audit(engine: &ProtocolEngine, bound: usize) {
        assert_eq!(engine.state(), SessionState::Running);
        for target in 1..=7 {
            assert!(engine.correlation.ledger().len(camera(target)) <= bound);
            assert!(
                engine.tracked_inquiry_shapes(camera(target)).unwrap().len()
                    <= MAX_RAW_INQUIRY_SHAPE_CLASSES_PER_TARGET
            );
        }
        engine.assert_invariants().unwrap();
    }

    fn unanswered(
        engine: &mut ProtocolEngine,
        ticket: &mut u64,
        now: &mut Instant,
        target: u8,
        mut request: RuntimeRequest,
        bound: usize,
    ) {
        let context = request.context_mut();
        context.retry = RetryPolicy::NEVER;
        context.timeout.ack = NonZeroDuration::new(Duration::from_millis(1)).unwrap();
        context.timeout.inquiry = NonZeroDuration::new(Duration::from_millis(1)).unwrap();
        context.timeout.ambiguity = NonZeroDuration::new(Duration::from_millis(2)).unwrap();
        *ticket += 1;
        let (effects, id) = admit(engine, *ticket, request, *now);
        assert_eq!(
            request_transmit(&effects).1,
            id,
            "target {target}: the inquiry or urgent STOP must actually write"
        );
        audit(engine, bound);
        let _ = send_ok(engine, &effects, None, *now);
        audit(engine, bound);
        *now += Duration::from_millis(10);
        let effects = engine.advance(*now);
        assert!(
            terminal_failure(&effects, id).is_some(),
            "target {target}: the skipped first answer must leave a debt"
        );
        audit(engine, bound);
    }

    fn skipped_inquiry(target: u8, length: usize) -> RuntimeRequest {
        let mut request = inquiry_with_retry(target, RetryPolicy::NEVER);
        let RuntimeRequest::Inquiry { payload_shape, .. } = &mut request else {
            unreachable!("the inquiry fixture has the inquiry class");
        };
        *payload_shape = PayloadShape::Bytes(length);
        request
    }

    fn urgent_stop(target: u8) -> RuntimeRequest {
        let mut request = urgent_command(target, CancellationPolicy::Supported);
        request.context_mut().motion = Some(MotionEffect {
            axes: crate::AffectedAxes::PAN_TILT,
            stop: true,
        });
        request
    }

    // The targets use disjoint declaration sets so a global budget cannot
    // accidentally masquerade as seven independent per-camera budgets.
    for length in 1..=classes {
        for target in 1..=7 {
            unanswered(
                &mut engine,
                &mut ticket,
                &mut now,
                target,
                skipped_inquiry(target, usize::from(target - 1) * classes + length),
                bound,
            );
            unanswered(
                &mut engine,
                &mut ticket,
                &mut now,
                target,
                urgent_stop(target),
                bound,
            );
            assert_eq!(
                engine.correlation.ledger().len(camera(target)),
                2 * length,
                "each new inquiry must separate the surrounding ACK runs"
            );
        }
    }

    // Every camera gets its own full declaration budget. The next distinct
    // inquiry is refused before admission, while safety writes remain eligible.
    for target in 1..=7 {
        ticket += 1;
        let effects = engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(ticket),
                request: skipped_inquiry(target, usize::from(target - 1) * classes + classes + 1),
                slot: AdmissionSlot::Ordinary,
            },
            now,
        );
        assert!(effects.iter().any(|effect| matches!(effect,
            Effect::AdmissionRejected {
                error: Error::InquiryShapeCapacityExhausted { camera: seen, capacity }, ..
            } if *seen == camera(target).camera() && *capacity == classes)));
        assert!(!effects
            .iter()
            .any(|effect| matches!(effect, Effect::Admitted { .. } | Effect::Transmit { .. })));
        assert_eq!(
            engine.tracked_inquiry_shapes(camera(target)).unwrap().len(),
            classes
        );
        audit(&engine, bound);
    }

    for _ in 0..TAIL_STOPS {
        for target in 1..=7 {
            unanswered(
                &mut engine,
                &mut ticket,
                &mut now,
                target,
                urgent_stop(target),
                bound,
            );
            assert_eq!(
                engine.correlation.ledger().len(camera(target)),
                2 * classes,
                "a repeated STOP tail must merge into its existing ACK run"
            );
        }
    }

    // Paying the oldest real first answer must still find that evidence after
    // all the bounded pressure; no later ACK is sent first to heal it for us.
    for target in 1..=7 {
        let before = engine.correlation.ledger().debts(camera(target));
        let _ = engine.handle(
            frame(
                target,
                None,
                DecodedResponse::InquiryReply {
                    payload: std::iter::repeat_n(0x80, usize::from(target - 1) * classes + 1)
                        .collect(),
                },
            ),
            now,
        );
        assert_eq!(
            engine.correlation.ledger().debts(camera(target)) + 1,
            before,
            "target {target}: the oldest declared reply must not have been forgotten"
        );
        assert_eq!(
            engine.correlation.ledger().len(camera(target)),
            2 * classes - 1
        );
        audit(&engine, bound);
    }
}
