//! Randomized raw byte-stream sessions against an in-order camera model
//! (#795).
//!
//! One stream carries two cameras. Each camera reads the requests written to
//! it in order (unless the link is stalled, sometimes past the ambiguity
//! interval), answers each at once, runs accepted commands on the socket it
//! allocates, and later completes them — sometimes after the engine's
//! completion deadline, sometimes with an execution error. It answers socket
//! cancellations with `0x04` or `0x05` (cancelling whatever runs there),
//! rejects with several codes (named and socketless), rejects inquiries, and
//! runs `CompletionOnly` and `NoReply` commands, rejecting some of either.
//! STOPs come alone and in halts of three siblings, and are written inside a
//! `NoReply` command's hold. Every frame it sends is tagged with the request
//! that caused it.
//!
//! Camera kinds vary what the documented assumptions allow: whether
//! completions name their sockets (always, never, or sometimes; ACKs and
//! errors always name theirs), and whether a `CompletionOnly` command's
//! completion is sometimes never sent. A *dropping* camera also leaves some
//! requests unanswered, outside the assumptions.
//!
//! Every step checks every engine invariant (including the ledger bound),
//! that no acknowledged command is written again, and that every STOP is
//! written within its wait bound (one ACK plus one ambiguity interval, at
//! most half its budget; the G2-like link uses the production deadlines and
//! halt budget) and so within its dispatch deadline. For a camera within the
//! documented assumptions every frame binds only to its originator (a
//! dropping camera's once its target has healed; a camera naming sockets in
//! only some completions has its misbindings counted). The only other
//! request a frame may touch is an executing one whose socket a camera ACK
//! proves stale, or that a cancellation written for an ended command reached
//! in its socket; either ends unconfirmed. When the link recovers, every debt
//! settles except a `CompletionOnly` completion that never came — the
//! documented command-lane latch.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::*;

const ACTIVE_STEPS: u32 = 4_000;
const DRAIN_STEPS: u32 = 3_000;
/// The longest settling phase: it ends once nothing is live and every
/// camera owes at most a `CompletionOnly` completion.
const SETTLE_STEPS: u32 = 20_000;

/// The link and the engine's deadlines.
#[derive(Debug, Clone, Copy)]
struct Link {
    /// Chance per millisecond, in parts per million, that the link stalls.
    stall_ppm: u64,
    /// Longest stall, in milliseconds.
    stall_max: u64,
    /// Chance per millisecond, in parts per million, of a long stall: one
    /// that outlasts the ambiguity interval, up to `long_stall_max`
    /// milliseconds.
    long_stall_ppm: u64,
    long_stall_max: u64,
    timeout: TimeoutPolicy,
    /// A halt's dispatch budget (production: the movement completion
    /// timeout).
    stop_budget: Duration,
    /// Whether stalls routinely outlast the deadlines, so that every
    /// timeout path must be exercised.
    hostile: bool,
    /// Of every thousand requests admitted, how many are `NoReply` commands.
    /// Each successful one holds its camera's ordinary work, but never a
    /// STOP, for an ambiguity interval (#700).
    no_reply_per_mille: u64,
}

/// A deliberately hostile link: stalls hold it about 45% of the time, and
/// deadlines are short (20 ms ACK, 40 ms completion). Most stalls already
/// outlast its 50 ms ambiguity interval.
const HOSTILE: Link = Link {
    stall_ppm: 3_000,
    stall_max: 300,
    long_stall_ppm: 0,
    long_stall_max: 0,
    timeout: TimeoutPolicy {
        ack: Duration::from_millis(20),
        completion: Duration::from_millis(40),
        inquiry: Duration::from_millis(30),
        cancellation: Duration::from_millis(10),
        ambiguity: Duration::from_millis(50),
    },
    stop_budget: Duration::from_millis(250),
    hostile: true,
    no_reply_per_mille: 100,
};

/// A PTZOptics-G2-like link: stalls hold it under 5% of the time, and the
/// deadlines are the built-in profiles' (500 ms ACK, 1 s ambiguity), as is
/// the STOP budget (30 s); now and then a stall outlasts the ambiguity
/// interval and a STOP's whole wait bound. A `NoReply`
/// command comes about every three seconds per camera, so its one-second hold
/// lapses between them while the STOPs written inside it are many.
const REALISTIC: Link = Link {
    stall_ppm: 300,
    stall_max: 300,
    long_stall_ppm: 150,
    long_stall_max: 3_000,
    timeout: TimeoutPolicy {
        ack: Duration::from_millis(500),
        completion: Duration::from_millis(2_000),
        inquiry: Duration::from_millis(1_000),
        cancellation: Duration::from_millis(500),
        ambiguity: Duration::from_millis(1_000),
    },
    // Production: a halt's budget is at least the movement completion
    // timeout, 30 s in every built-in profile.
    stop_budget: Duration::from_secs(30),
    hostile: false,
    no_reply_per_mille: 5,
};

/// What a camera does within (or, for `dropping`, outside) the documented
/// assumptions, over which link.
#[derive(Debug, Clone, Copy)]
struct Kind {
    name: &'static str,
    seeds: u64,
    link: Link,
    /// Percent of a socketed command's completions that name its socket.
    named_completions: u64,
    /// Percent of `CompletionOnly` commands whose completion is never sent.
    lost_completion_only: u64,
    /// Percent of requests left unanswered (outside the assumptions).
    dropped: u64,
}

impl Kind {
    /// A camera that keeps the documented assumptions: completions name
    /// their sockets always or never, and every request is answered (only a
    /// `CompletionOnly` completion may be missing).
    const fn within_assumptions(&self) -> bool {
        (self.named_completions == 0 || self.named_completions == 100) && self.dropped == 0
    }

    /// A camera that keeps every assumption and names its sockets in
    /// completions, like the PTZOptics G2 (`90 51 FF`, `90 52 FF`).
    const fn names_sockets(&self) -> bool {
        self.named_completions == 100 && self.lost_completion_only == 0 && self.dropped == 0
    }
}

const fn kind(name: &'static str, seeds: u64, link: Link) -> Kind {
    Kind {
        name,
        seeds,
        link,
        named_completions: 100,
        lost_completion_only: 0,
        dropped: 0,
    }
}

const KINDS: [Kind; 6] = [
    kind("names sockets, hostile link", 96, HOSTILE),
    kind("names sockets, G2-like link", 96, REALISTIC),
    Kind {
        named_completions: 0,
        ..kind("omits socket in completions", 48, HOSTILE)
    },
    Kind {
        named_completions: 50,
        ..kind("names socket in half its completions", 48, HOSTILE)
    },
    Kind {
        lost_completion_only: 10,
        ..kind("loses CompletionOnly completions", 48, HOSTILE)
    },
    Kind {
        dropped: 3,
        ..kind("drops requests", 48, HOSTILE)
    },
];

#[test]
fn randomized_stream_sessions_bind_every_answer_to_its_originator() {
    for kind in KINDS {
        let mut totals = Coverage::default();
        for seed in 1..=kind.seeds {
            totals.add(&Session::run(kind, seed));
        }
        eprintln!(
            "{}: {} seeds x {ACTIVE_STEPS}+{DRAIN_STEPS}(+settling) steps: {}",
            kind.name,
            kind.seeds,
            totals.summary()
        );
        assert!(
            totals.covers_every_path(kind.link.hostile),
            "{}: {totals:?}",
            kind.name
        );
        if kind.within_assumptions() {
            assert_eq!(totals.misbindings, 0, "{}", kind.name);
        }
        if kind.dropped > 0 {
            assert!(totals.dropped > 0 && totals.healed > 0, "{totals:?}");
        }
        if kind.names_sockets() {
            // On a camera within the assumptions, a STOP ends unconfirmed only
            // when its own answer did not arrive in time or a dispute could
            // not assign it, never by a cascade; no lane is latched at the end.
            assert_eq!(
                totals.stop_unconfirmed[cause(Cause::Cascade)],
                0,
                "{}",
                kind.name
            );
            assert_eq!(
                totals.stop_unconfirmed[cause(Cause::StaleSocket)],
                0,
                "{}",
                kind.name
            );
            assert_eq!(
                totals.stop_unconfirmed[cause(Cause::Other)],
                0,
                "{}",
                kind.name
            );
            assert_eq!(totals.latched_at_end, 0, "{}", kind.name);
        }
    }
}

fn cause(cause: Cause) -> usize {
    CAUSES.iter().position(|known| *known == cause).unwrap_or(4)
}

/// What a run exercised and how it ended, so a regression in the generator
/// cannot silently hollow the test out.
#[derive(Debug, Default, Clone)]
struct Coverage {
    written: u64,
    acked: u64,
    stops: u64,
    cancels: u64,
    cancel_replies: u64,
    named_rejections: u64,
    socketless_rejections: u64,
    inquiry_rejections: u64,
    execution_errors: u64,
    executing_timeouts: u64,
    completion_only: u64,
    no_reply: u64,
    /// STOPs first written while a `NoReply` command's hold held the
    /// camera's ordinary work.
    stops_in_hold: u64,
    /// `NoReply` commands the camera rejected.
    no_reply_rejections: u64,
    /// Stalls that outlasted the ambiguity interval (G2-like link).
    long_stalls: u64,
    /// Halts (three sibling STOPs), and those admitted while a
    /// `CompletionOnly` or `NoReply` answer that may be a rejection was owed.
    halts: u64,
    halts_unproven: u64,
    /// Longest wait from admission to first write, in milliseconds: of any
    /// STOP, of one admitted while such an answer was owed, of a halt's, and
    /// of an unproven halt's.
    max_stop_wait: u64,
    max_unproven_stop_wait: u64,
    max_halt_wait: u64,
    max_unproven_halt_wait: u64,
    /// Frames that bound to no request while their originator was live.
    discarded_live: u64,
    /// Socketless errors the ledger disputed.
    disputes: u64,
    /// Executing requests whose socket a camera ACK proved stale.
    displaced: u64,
    /// Executing requests a cancellation written for an ended command
    /// reached.
    cancel_victims: u64,
    /// Requests that ended `UnsequencedCommandUnconfirmed`.
    unconfirmed: u64,
    /// Targets whose command lane was latched when the run ended.
    latched_at_end: u64,
    peak_debts: usize,
    peak_entries: usize,
    dropped: u64,
    healed: u64,
    /// STOPs and other commands that ended unconfirmed, by cause.
    stop_unconfirmed: [u64; 5],
    command_unconfirmed: [u64; 5],
    /// End-latched command lanes by cause: the `CompletionOnly` command was
    /// rejected, its completion was lost by the camera, its completion was
    /// discarded, other.
    latched_by_cause: [u64; 4],
    misbindings: u64,
    /// Disputed frames assigned to their requests once proven.
    assigned: u64,
}

impl Coverage {
    fn add(&mut self, run: &Self) {
        self.written += run.written;
        self.acked += run.acked;
        self.stops += run.stops;
        self.cancels += run.cancels;
        self.cancel_replies += run.cancel_replies;
        self.named_rejections += run.named_rejections;
        self.socketless_rejections += run.socketless_rejections;
        self.inquiry_rejections += run.inquiry_rejections;
        self.execution_errors += run.execution_errors;
        self.executing_timeouts += run.executing_timeouts;
        self.completion_only += run.completion_only;
        self.no_reply += run.no_reply;
        self.stops_in_hold += run.stops_in_hold;
        self.no_reply_rejections += run.no_reply_rejections;
        self.long_stalls += run.long_stalls;
        self.halts += run.halts;
        self.halts_unproven += run.halts_unproven;
        self.max_stop_wait = self.max_stop_wait.max(run.max_stop_wait);
        self.max_unproven_stop_wait = self.max_unproven_stop_wait.max(run.max_unproven_stop_wait);
        self.max_halt_wait = self.max_halt_wait.max(run.max_halt_wait);
        self.max_unproven_halt_wait = self.max_unproven_halt_wait.max(run.max_unproven_halt_wait);
        self.discarded_live += run.discarded_live;
        self.disputes += run.disputes;
        self.displaced += run.displaced;
        self.cancel_victims += run.cancel_victims;
        self.unconfirmed += run.unconfirmed;
        self.latched_at_end += run.latched_at_end;
        self.peak_debts = self.peak_debts.max(run.peak_debts);
        self.peak_entries = self.peak_entries.max(run.peak_entries);
        self.dropped += run.dropped;
        self.healed += run.healed;
        for index in 0..5 {
            self.stop_unconfirmed[index] += run.stop_unconfirmed[index];
            self.command_unconfirmed[index] += run.command_unconfirmed[index];
        }
        for index in 0..4 {
            self.latched_by_cause[index] += run.latched_by_cause[index];
        }
        self.misbindings += run.misbindings;
        self.assigned += run.assigned;
    }

    /// The report columns: STOPs, unconfirmed STOPs and other commands by
    /// cause (never arrived / disputed / cascade / stale socket / other),
    /// lanes latched at the end by cause (rejected / lost / discarded /
    /// other), misbindings.
    fn summary(&self) -> String {
        format!(
            "stops {} ({} in a NoReply hold; {} halts, {} unproven) | max STOP wait {} ms ({} unproven; halt {}, {} unproven) | stop unconfirmed {:?} | command unconfirmed {:?} | latched at end {:?} | misbindings {} | disputes {} | peak entries {} | discarded live {} | dropped {} healed {}",
            self.stops,
            self.stops_in_hold,
            self.halts,
            self.halts_unproven,
            self.max_stop_wait,
            self.max_unproven_stop_wait,
            self.max_halt_wait,
            self.max_unproven_halt_wait,
            self.stop_unconfirmed,
            self.command_unconfirmed,
            self.latched_by_cause,
            self.misbindings,
            self.disputes,
            self.peak_entries,
            self.discarded_live,
            self.dropped,
            self.healed
        )
    }

    /// Whether the run exercised every path the link allows (a G2-like link
    /// rarely outlasts a deadline).
    fn covers_every_path(&self, hostile: bool) -> bool {
        let always = [
            self.acked,
            self.stops,
            self.cancels,
            self.cancel_replies,
            self.named_rejections,
            self.socketless_rejections,
            self.inquiry_rejections,
            self.execution_errors,
            self.completion_only,
            self.no_reply,
            self.no_reply_rejections,
            self.stops_in_hold,
            self.halts,
            self.halts_unproven,
        ];
        let hostile_only = [self.executing_timeouts];
        always.iter().all(|count| *count > 0)
            && (!hostile || hostile_only.iter().all(|count| *count > 0))
    }
}

/// Why a request ended `UnsequencedCommandUnconfirmed` (diagnosis).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cause {
    /// Its own answer was not delivered before its deadline: a stall, a slow
    /// completion, or a camera that never sent it (a cancellation that
    /// reached the command the camera had just put in that socket ends it
    /// unseen, and a later ACK there displaces it).
    NeverArrived,
    /// Its own answer arrived in time but a dispute bound it to nothing, or
    /// it paid a disputed debt (whose late answer it could have been).
    Disputed,
    /// Its own answer arrived in time but paid an older, undisputed debt.
    Cascade,
    /// Its own completion or error was discarded (a held socket, or socket
    /// ambiguity on a camera that omits the socket in completions).
    StaleSocket,
    /// Its own answer arrived in time and was discarded for another reason.
    Other,
}

const CAUSES: [Cause; 5] = [
    Cause::NeverArrived,
    Cause::Disputed,
    Cause::Cascade,
    Cause::StaleSocket,
    Cause::Other,
];

#[derive(Debug, Clone, Copy)]
enum Job {
    Command { id: RequestId, shape: ReplyShape },
    Inquiry(RequestId),
    Cancel { id: RequestId, socket: ViscaSocket },
}

/// How an accepted command running on a socket ends.
#[derive(Debug, Clone, Copy)]
struct Running {
    id: RequestId,
    done: Instant,
    error: bool,
}

#[derive(Default)]
struct Camera {
    sockets: [Option<Running>; 2],
    rotation: usize,
    /// A running `CompletionOnly` command and when it completes (`None`: it
    /// never sends its completion).
    completion_only: Option<(RequestId, Option<Instant>)>,
}

struct Session {
    kind: Kind,
    seed: u64,
    misbehaving: bool,
    random: u64,
    now: Instant,
    engine: ProtocolEngine,
    cameras: [Camera; 2],
    /// Requests written and not yet read by their camera, in write order.
    unread: VecDeque<(u8, Job)>,
    /// Frames sent by the cameras and not yet delivered, with their cause.
    outbound: VecDeque<(u8, DecodedResponse, RequestId)>,
    stalled_until: Instant,
    ticket: u64,
    acknowledged: BTreeSet<RequestId>,
    ended: BTreeSet<RequestId>,
    /// STOPs admitted, with their dispatch deadline and first write.
    stops: BTreeMap<RequestId, (Instant, Option<Instant>)>,
    /// When each STOP was admitted, and whether a `CompletionOnly` or
    /// `NoReply` command's answer that may be a rejection was owed then.
    stop_admitted: BTreeMap<RequestId, (Instant, bool)>,
    /// STOPs admitted by a halt, and whether such an answer was owed then.
    halt_stops: BTreeMap<RequestId, bool>,
    /// Cancellations that reached another command than their own (the
    /// camera had put it in the socket since): their answer ends it.
    cancel_victims: BTreeSet<(RequestId, RequestId)>,
    /// Per target, for a dropping camera: frames caused by requests written
    /// after this write count are held to the exact rules. `None` from a
    /// dropped request until the target has healed.
    certain_from: [Option<u64>; 2],
    written_order: BTreeMap<RequestId, u64>,
    writes: u64,
    coverage: Coverage,
    /// Why each request's own answer was discarded, if it was.
    discard_cause: BTreeMap<RequestId, Cause>,
    co_rejected: BTreeSet<RequestId>,
    co_lost: BTreeSet<RequestId>,
    co_completion_discarded: BTreeSet<RequestId>,
}

impl Session {
    fn run(kind: Kind, seed: u64) -> Coverage {
        let start = Instant::now();
        let mut configured = policy(EnvelopeKind::Raw, TransportKind::Stream);
        configured.inquiry_capacity = 1;
        let mut engine = ProtocolEngine::new(configured).unwrap();
        for target in [camera(1), camera(2)] {
            engine
                .register_target(
                    target,
                    TargetPolicy {
                        control_reserve: 2,
                        ..TargetPolicy::test_default()
                    },
                )
                .unwrap();
        }
        let mut session = Self {
            kind,
            seed,
            misbehaving: kind.dropped > 0 || kind.lost_completion_only > 0,
            random: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
            now: start,
            engine,
            cameras: [Camera::default(), Camera::default()],
            unread: VecDeque::new(),
            outbound: VecDeque::new(),
            stalled_until: start,
            ticket: 0,
            acknowledged: BTreeSet::new(),
            ended: BTreeSet::new(),
            stops: BTreeMap::new(),
            stop_admitted: BTreeMap::new(),
            halt_stops: BTreeMap::new(),
            cancel_victims: BTreeSet::new(),
            certain_from: [Some(0); 2],
            written_order: BTreeMap::new(),
            writes: 0,
            coverage: Coverage::default(),
            discard_cause: BTreeMap::new(),
            co_rejected: BTreeSet::new(),
            co_lost: BTreeSet::new(),
            co_completion_discarded: BTreeSet::new(),
        };
        for _ in 0..ACTIVE_STEPS {
            session.step(true);
        }
        // Let the link recover: no new stalls, drops or lost completions,
        // and only periodic inquiries and STOPs, whose answers settle
        // whatever is still owed.
        session.stalled_until = session.now;
        session.misbehaving = false;
        for step in 0..DRAIN_STEPS {
            if step % 50 == 0 {
                for target in [1, 2] {
                    let request = inquiry(target, InquiryRoute::UNKNOWN);
                    session.admit(request, AdmissionSlot::Ordinary);
                    if step % 200 == 0 {
                        let now = session.now;
                        session.admit(
                            stop(
                                target,
                                now,
                                session.kind.link.stop_budget,
                                session.ticket + 1,
                            ),
                            AdmissionSlot::ControlReserve,
                        );
                    }
                }
            }
            session.step(false);
        }
        // Keep probing any camera that still owes more than a completion:
        // every dispute must resolve, given traffic, or collapse into the
        // documented command-lane latch.
        for step in 0..SETTLE_STEPS {
            let settled = [camera(1), camera(2)]
                .into_iter()
                .all(|target| session.owes_only_completions(target))
                && session.engine.entries.is_empty();
            if settled {
                break;
            }
            if step % 50 == 0 {
                for target in [camera(1), camera(2)] {
                    if !session.owes_only_completions(target) {
                        let now = session.now;
                        session.admit(
                            inquiry(target.id(), InquiryRoute::UNKNOWN),
                            AdmissionSlot::Ordinary,
                        );
                        session.admit(
                            stop(
                                target.id(),
                                now,
                                session.kind.link.stop_budget,
                                session.ticket + 1,
                            ),
                            AdmissionSlot::ControlReserve,
                        );
                    }
                }
            }
            session.step(false);
        }
        session.finish();
        session.coverage
    }

    fn roll(&mut self, range: u64) -> u64 {
        next_random(&mut self.random) % range
    }

    fn step(&mut self, active: bool) {
        self.now += Duration::from_millis(1);
        if active {
            if self.roll(1_000_000) < self.kind.link.stall_ppm {
                let stall = self.roll(self.kind.link.stall_max);
                self.stalled_until = self.now + Duration::from_millis(stall);
            }
            if self.roll(1_000_000) < self.kind.link.long_stall_ppm {
                let ambiguity =
                    u64::try_from(self.kind.link.timeout.ambiguity.as_millis()).unwrap();
                let stall = ambiguity + self.roll(self.kind.link.long_stall_max - ambiguity);
                self.stalled_until = self
                    .stalled_until
                    .max(self.now + Duration::from_millis(stall));
                self.coverage.long_stalls += 1;
            }
            let roll = self.roll(1_000);
            if roll < 130 {
                let target = 1 + u8::try_from(self.roll(2)).unwrap();
                self.admit_random(target);
            } else if roll < 150 {
                self.cancel_random();
            } else if roll < 152 {
                let target = 1 + u8::try_from(self.roll(2)).unwrap();
                self.halt(target);
            }
        }
        if self.now >= self.stalled_until {
            self.read_all();
        }
        self.complete_due();
        if self.now >= self.stalled_until {
            self.deliver_all();
        }
        let effects = self.engine.advance(self.now);
        self.apply(effects);
        self.check();
    }

    /// An owner halt: three sibling STOPs (pan/tilt, zoom, focus) admitted
    /// back to back under one deadline, through the control reserve while it
    /// lasts.
    fn halt(&mut self, target: u8) {
        let order = self.ticket + 1;
        let unproven = self.unproven(camera(target));
        self.coverage.halts += 1;
        if unproven {
            self.coverage.halts_unproven += 1;
        }
        for _ in 0..3 {
            for slot in [AdmissionSlot::ControlReserve, AdmissionSlot::Ordinary] {
                let request = stop(target, self.now, self.kind.link.stop_budget, order);
                if let Some(id) = self.admit(request, slot) {
                    self.halt_stops.insert(id, unproven);
                    break;
                }
            }
        }
    }

    /// Whether `target` owes a `CompletionOnly` or `NoReply` command's answer
    /// that may still be a rejection.
    fn unproven(&self, target: CameraId) -> bool {
        self.engine.ledger.entries().any(|(owner, entry)| {
            owner == target && matches!(entry.owes, Owes::Completion | Owes::Rejection)
        })
    }

    fn admit_random(&mut self, target: u8) {
        if self.roll(1_000) < self.kind.link.no_reply_per_mille {
            let request = command_with_reply_shape(
                target,
                CancellationPolicy::Supported,
                ReplyShape::NoReply,
            );
            self.admit(request, AdmissionSlot::Ordinary);
            return;
        }
        let (request, slot) = match self.roll(20) {
            0..=2 => (
                stop(
                    target,
                    self.now,
                    self.kind.link.stop_budget,
                    self.ticket + 1,
                ),
                AdmissionSlot::ControlReserve,
            ),
            3..=8 => (
                inquiry(target, InquiryRoute::UNKNOWN),
                AdmissionSlot::Ordinary,
            ),
            9..=15 => (
                command(target, CancellationPolicy::Supported),
                AdmissionSlot::Ordinary,
            ),
            16..=17 => (
                command_with_reply_shape(
                    target,
                    CancellationPolicy::Supported,
                    ReplyShape::CompletionOnly,
                ),
                AdmissionSlot::Ordinary,
            ),
            _ => (
                command(target, CancellationPolicy::Supported),
                AdmissionSlot::Ordinary,
            ),
        };
        self.admit(request, slot);
    }

    fn admit(&mut self, mut request: RuntimeRequest, slot: AdmissionSlot) -> Option<RequestId> {
        self.ticket += 1;
        request.context_mut().timeout = self.kind.link.timeout;
        let deadline = request.context().dispatch_deadline;
        let target = request.context().target;
        let effects = self.engine.handle(
            Input::Admit {
                ticket: AdmissionTicket(self.ticket),
                request,
                slot,
            },
            self.now,
        );
        let admitted = effects.iter().find_map(|effect| match effect {
            Effect::Admitted { id, .. } => Some(*id),
            _ => None,
        });
        if let (Some(deadline), Some(id)) = (deadline, admitted) {
            self.stops.insert(id, (deadline, None));
            self.stop_admitted
                .insert(id, (self.now, self.unproven(target)));
            self.coverage.stops += 1;
        }
        self.apply(effects);
        admitted
    }

    /// Cancels a random executing command.
    fn cancel_random(&mut self) {
        let live: Vec<RequestId> = self
            .acknowledged
            .iter()
            .filter(|id| !self.ended.contains(id))
            .copied()
            .collect();
        if live.is_empty() {
            return;
        }
        let index = usize::try_from(self.roll(live.len() as u64)).unwrap();
        let effects = self
            .engine
            .handle(Input::Cancel { id: live[index] }, self.now);
        self.apply(effects);
    }

    /// Applies engine effects: performs every write (which then completes),
    /// and records acknowledgements and terminals.
    fn apply(&mut self, effects: Vec<Effect>) {
        let mut pending = effects;
        while !pending.is_empty() {
            let mut next = Vec::new();
            for effect in pending {
                match effect {
                    Effect::Transmit {
                        transmission,
                        request,
                        kind,
                    } => {
                        let entry = self.engine.entry(request).expect("written request");
                        let target = entry.request.context().target.id();
                        let job = match kind {
                            Transmission::Request { .. } => {
                                assert!(
                                    !self.acknowledged.contains(&request),
                                    "seed {}: acknowledged {request:?} written again",
                                    self.seed
                                );
                                if entry.request.is_inquiry() {
                                    Job::Inquiry(request)
                                } else {
                                    Job::Command {
                                        id: request,
                                        shape: entry.request.context().reply_shape,
                                    }
                                }
                            }
                            Transmission::Cancel { socket, .. } => {
                                self.coverage.cancels += 1;
                                Job::Cancel {
                                    id: request,
                                    socket,
                                }
                            }
                        };
                        if let Some((deadline, first)) = self.stops.get_mut(&request) {
                            if first.is_none() {
                                assert!(
                                    self.now <= *deadline,
                                    "seed {}: STOP {request:?} written after its deadline",
                                    self.seed
                                );
                                *first = Some(self.now);
                                let (admitted, unproven) = self.stop_admitted[&request];
                                let wait = self.now - admitted;
                                // Written within its wait bound: one ACK plus
                                // one ambiguity interval, at most half its
                                // budget (the step is one millisecond).
                                let timeout = self.kind.link.timeout;
                                let bound = (timeout.ack + timeout.ambiguity)
                                    .min((*deadline - admitted) / 2);
                                assert!(
                                    wait <= bound + Duration::from_millis(1),
                                    "seed {}: STOP {request:?} waited {wait:?}, past its bound {bound:?}",
                                    self.seed
                                );
                                let wait = u64::try_from(wait.as_millis()).unwrap();
                                let coverage = &mut self.coverage;
                                coverage.max_stop_wait = coverage.max_stop_wait.max(wait);
                                if unproven {
                                    coverage.max_unproven_stop_wait =
                                        coverage.max_unproven_stop_wait.max(wait);
                                }
                                if let Some(halt_unproven) = self.halt_stops.get(&request) {
                                    coverage.max_halt_wait = coverage.max_halt_wait.max(wait);
                                    if *halt_unproven {
                                        coverage.max_unproven_halt_wait =
                                            coverage.max_unproven_halt_wait.max(wait);
                                    }
                                }
                                if self
                                    .engine
                                    .raw_hold(camera(target), RawHoldScope::AllResponses)
                                    .is_some()
                                {
                                    self.coverage.stops_in_hold += 1;
                                }
                            }
                        }
                        self.writes += 1;
                        self.written_order.entry(request).or_insert(self.writes);
                        self.coverage.written += 1;
                        self.unread.push_back((target, job));
                        next.extend(self.engine.handle(
                            Input::TransmissionFinished {
                                transmission,
                                result: Ok(TransmissionMeta { sequence: None }),
                            },
                            self.now,
                        ));
                    }
                    Effect::Transition {
                        id,
                        to: Phase::Executing { .. },
                        ..
                    } => {
                        self.acknowledged.insert(id);
                        self.coverage.acked += 1;
                    }
                    Effect::Terminal { id, outcome } => {
                        if matches!(
                            outcome,
                            RuntimeOutcome::Failed(Error::UnsequencedCommandUnconfirmed)
                        ) {
                            let cause = self
                                .discard_cause
                                .get(&id)
                                .copied()
                                .unwrap_or(Cause::NeverArrived);
                            let index = CAUSES.iter().position(|c| *c == cause).unwrap_or(4);
                            if self.stops.contains_key(&id) {
                                self.coverage.stop_unconfirmed[index] += 1;
                            } else {
                                self.coverage.command_unconfirmed[index] += 1;
                            }
                            self.coverage.unconfirmed += 1;
                            if self.acknowledged.contains(&id) {
                                self.coverage.executing_timeouts += 1;
                            }
                        }
                        self.ended.insert(id);
                    }
                    _ => {}
                }
            }
            pending = next;
        }
    }

    /// The camera reads every written request, in order.
    fn read_all(&mut self) {
        while let Some((target, job)) = self.unread.pop_front() {
            let dropped = self.misbehaving
                && !matches!(
                    job,
                    Job::Command {
                        shape: ReplyShape::CompletionOnly,
                        ..
                    }
                )
                && self.roll(100) < self.kind.dropped;
            if dropped {
                self.coverage.dropped += 1;
                self.certain_from[usize::from(target - 1)] = None;
                continue;
            }
            self.read(target, job);
        }
    }

    fn read(&mut self, target: u8, job: Job) {
        let now = self.now;
        match job {
            Job::Inquiry(id) => {
                let response = if self.roll(10) == 0 {
                    self.coverage.inquiry_rejections += 1;
                    let code = [0x02, 0x41][usize::try_from(self.roll(2)).unwrap()];
                    DecodedResponse::Error { socket: None, code }
                } else {
                    DecodedResponse::InquiryReply {
                        route: None,
                        payload: SmallVec::from_slice(&id.get().to_le_bytes()),
                    }
                };
                self.outbound.push_back((target, response, id));
            }
            Job::Command {
                id,
                shape: ReplyShape::NoReply,
            } => {
                self.coverage.no_reply += 1;
                if self.roll(6) == 0 {
                    self.coverage.no_reply_rejections += 1;
                    self.outbound.push_back((
                        target,
                        DecodedResponse::Error {
                            socket: None,
                            code: 0x41,
                        },
                        id,
                    ));
                }
            }
            Job::Command {
                id,
                shape: ReplyShape::CompletionOnly,
            } => {
                self.coverage.completion_only += 1;
                if self.roll(6) == 0 {
                    self.coverage.socketless_rejections += 1;
                    self.co_rejected.insert(id);
                    self.outbound.push_back((
                        target,
                        DecodedResponse::Error {
                            socket: None,
                            code: 0x02,
                        },
                        id,
                    ));
                } else {
                    let lost = self.misbehaving && self.roll(100) < self.kind.lost_completion_only;
                    if lost {
                        self.co_lost.insert(id);
                    }
                    let latency = self.latency();
                    self.cameras[usize::from(target - 1)].completion_only =
                        Some((id, (!lost).then_some(now + latency)));
                }
            }
            Job::Command {
                id,
                shape: ReplyShape::AckThenCompletion,
            } => {
                let roll = self.roll(100);
                let latency = self.latency();
                let error = self.roll(20) == 0;
                let code = [0x02, 0x03, 0x41][usize::try_from(self.roll(3)).unwrap()];
                let camera = &mut self.cameras[usize::from(target - 1)];
                let free = (0..2)
                    .map(|offset| (camera.rotation + offset) % 2)
                    .find(|index| camera.sockets[*index].is_none());
                let response = match free {
                    None => {
                        self.coverage.socketless_rejections += 1;
                        DecodedResponse::Error {
                            socket: None,
                            code: 0x03,
                        }
                    }
                    Some(index) if roll < 8 => {
                        // The G2 rejects in the socket it allocated, and
                        // skips it in its rotation.
                        camera.rotation = (index + 1) % 2;
                        self.coverage.named_rejections += 1;
                        DecodedResponse::Error {
                            socket: Some(socket_at(index)),
                            code: 0x41,
                        }
                    }
                    Some(_) if roll < 14 => {
                        self.coverage.socketless_rejections += 1;
                        DecodedResponse::Error { socket: None, code }
                    }
                    Some(index) => {
                        camera.rotation = (index + 1) % 2;
                        camera.sockets[index] = Some(Running {
                            id,
                            done: now + latency,
                            error,
                        });
                        DecodedResponse::Ack {
                            socket: Some(socket_at(index)),
                        }
                    }
                };
                self.outbound.push_back((target, response, id));
            }
            Job::Cancel { id, socket } => {
                // A camera cancels whatever runs in the socket: when the
                // command it was written for has ended, another command the
                // camera put there since.
                let slot = &mut self.cameras[usize::from(target - 1)].sockets[socket_index(socket)];
                let code = match slot.take() {
                    Some(running) => {
                        if running.id != id {
                            self.cancel_victims.insert((id, running.id));
                        }
                        0x04
                    }
                    None => 0x05,
                };
                self.coverage.cancel_replies += 1;
                self.outbound.push_back((
                    target,
                    DecodedResponse::Error {
                        socket: Some(socket),
                        code,
                    },
                    id,
                ));
            }
        }
    }

    /// Mostly prompt; sometimes past the engine's 40 ms completion deadline.
    fn latency(&mut self) -> Duration {
        let millis = match self.roll(20) {
            0 => 60 + self.roll(400),
            1 | 2 => 25 + self.roll(30),
            _ => self.roll(25),
        };
        Duration::from_millis(millis)
    }

    /// Commands that finished running send their completion (or error).
    fn complete_due(&mut self) {
        let now = self.now;
        let named_percent = self.kind.named_completions;
        for index in 0..self.cameras.len() {
            let named = next_random(&mut self.random) % 100 < named_percent;
            let camera = &mut self.cameras[index];
            let target = u8::try_from(index + 1).unwrap();
            for (socket, slot) in camera.sockets.iter_mut().enumerate() {
                if let Some(running) = slot.filter(|running| running.done <= now) {
                    *slot = None;
                    let response = if running.error {
                        // An execution error names its socket (assumption 2).
                        self.coverage.execution_errors += 1;
                        DecodedResponse::Error {
                            socket: Some(socket_at(socket)),
                            code: 0x41,
                        }
                    } else {
                        DecodedResponse::Completion {
                            socket: named.then(|| socket_at(socket)),
                        }
                    };
                    self.outbound.push_back((target, response, running.id));
                }
            }
            match camera.completion_only {
                Some((id, Some(done))) if done <= now => {
                    camera.completion_only = None;
                    self.outbound.push_back((
                        target,
                        DecodedResponse::Completion { socket: None },
                        id,
                    ));
                }
                // A lost completion: the command runs on, silently, until the
                // camera reads its next `CompletionOnly` command.
                _ => {}
            }
        }
    }

    /// Delivers every frame the cameras sent, in order, each in its own
    /// input-only turn, and checks what it bound to.
    fn deliver_all(&mut self) {
        while let Some((target, response, originator)) = self.outbound.pop_front() {
            let camera_id = camera(target);
            let exact = self.certain(target, originator);
            let accepted_before = self.accepted_completions(camera_id);
            let disputes_before = self.engine.ledger.disputes_seen;
            let debts_before = self.engine.ledger.debts(camera_id);
            let late = self
                .engine
                .entry(originator)
                .and_then(correlated_response_deadline)
                .is_some_and(|deadline| deadline < self.now);
            let effects = self.engine.handle_turn(
                frame(target, None, response.clone()),
                self.now,
                EngineTurn::INPUT_ONLY,
            );
            let accepted_after = self.accepted_completions(camera_id);
            let mut bound = false;
            for effect in &effects {
                let id = match effect {
                    Effect::Transition { id, .. }
                    | Effect::Terminal { id, .. }
                    | Effect::RetryScheduled { id, .. }
                    | Effect::CancellationObservation { id, .. } => *id,
                    _ => continue,
                };
                if id == originator {
                    bound = true;
                    continue;
                }
                // A camera ACK naming a socket the engine still indexes to an
                // executing command proves that claim stale (#721): that
                // command ends unconfirmed. The ACK itself binds only to its
                // originator.
                let displaced = matches!(response, DecodedResponse::Ack { .. })
                    && self.acknowledged.contains(&id)
                    && matches!(
                        effect,
                        Effect::Terminal {
                            outcome: RuntimeOutcome::Failed(Error::UnsequencedCommandUnconfirmed),
                            ..
                        } | Effect::Transition { .. }
                            | Effect::CancellationObservation { .. }
                    );
                if displaced {
                    if matches!(effect, Effect::Terminal { .. }) {
                        self.coverage.displaced += 1;
                    }
                    continue;
                }
                // A cancellation's `0x04` ends the command it reached.
                if self.cancel_victims.contains(&(originator, id))
                    && matches!(
                        effect,
                        Effect::Terminal {
                            outcome: RuntimeOutcome::Failed(Error::UnsequencedCommandUnconfirmed),
                            ..
                        }
                    )
                {
                    self.coverage.cancel_victims += 1;
                    continue;
                }
                // A disputed frame of `id`'s own, assigned to it now that the
                // dispute is proven.
                if self.discard_cause.get(&id) == Some(&Cause::Disputed) {
                    self.coverage.assigned += 1;
                    continue;
                }
                if exact {
                    self.coverage.misbindings += 1;
                    assert!(
                        !self.kind.within_assumptions(),
                        "seed {} ({}): {response:?} from {originator:?} bound to {id:?}: {effect:?}",
                        self.seed,
                        self.kind.name
                    );
                }
            }
            for effect in &effects {
                if let Effect::Terminal {
                    id,
                    outcome: RuntimeOutcome::Reply { payload, .. },
                } = effect
                {
                    assert!(
                        !exact || payload.as_slice() == id.get().to_le_bytes(),
                        "seed {}: {id:?} returned another inquiry's reply",
                        self.seed
                    );
                }
            }
            if !bound
                && accepted_after > accepted_before
                && matches!(response, DecodedResponse::Error { socket: None, .. })
            {
                self.coverage.disputes += 1;
            }
            if !bound && !self.ended.contains(&originator) {
                self.coverage.discarded_live += 1;
                let cause = if late {
                    Cause::NeverArrived
                } else if self.engine.ledger.disputes_seen > disputes_before {
                    Cause::Disputed
                } else if self.engine.ledger.debts(camera_id) < debts_before {
                    Cause::Cascade
                } else if matches!(
                    response,
                    DecodedResponse::Completion { .. }
                        | DecodedResponse::Error {
                            socket: Some(_),
                            ..
                        }
                ) {
                    Cause::StaleSocket
                } else {
                    Cause::Other
                };
                self.discard_cause.entry(originator).or_insert(cause);
                if matches!(response, DecodedResponse::Completion { socket: None }) {
                    self.co_completion_discarded.insert(originator);
                }
            }
            self.apply(effects);
        }
    }

    fn accepted_completions(&self, target: CameraId) -> usize {
        self.engine
            .ledger
            .entries()
            .filter(|(owner, entry)| *owner == target && entry.owes == Owes::AcceptedCompletion)
            .count()
    }

    /// Whether `originator`'s frames are held to the exact rules: its target
    /// is certain, and it was written after the target became so.
    fn certain(&self, target: u8, originator: RequestId) -> bool {
        match self.certain_from[usize::from(target - 1)] {
            Some(from) => self
                .written_order
                .get(&originator)
                .is_some_and(|order| *order > from),
            None => false,
        }
    }

    fn check(&mut self) {
        self.engine
            .assert_invariants()
            .unwrap_or_else(|violation| panic!("seed {}: {violation}", self.seed));
        assert_eq!(
            self.engine.state(),
            SessionState::Running,
            "seed {}",
            self.seed
        );
        for (index, target) in [camera(1), camera(2)].into_iter().enumerate() {
            let debts = usize::try_from(self.engine.ledger.debts(target)).unwrap();
            self.coverage.peak_debts = self.coverage.peak_debts.max(debts);
            self.coverage.peak_entries = self
                .coverage
                .peak_entries
                .max(self.engine.ledger.len(target));
            // A dropping camera's target has healed once it behaves again,
            // nothing is owed but a `CompletionOnly` completion, and nothing
            // from before is live or held.
            if self.certain_from[index].is_none()
                && !self.misbehaving
                && self.owes_only_completions(target)
                && self
                    .engine
                    .entries
                    .values()
                    .all(|entry| entry.request.context().target != target)
                && self.engine.holds.keys().all(|key| key.target != target)
            {
                self.certain_from[index] = Some(self.writes);
                self.coverage.healed += 1;
            }
        }
        for (stop, (deadline, first)) in &self.stops {
            assert!(
                first.is_some() || self.now <= *deadline,
                "seed {}: STOP {stop:?} not written by its deadline",
                self.seed
            );
        }
    }

    /// Whether `target` owes nothing but `CompletionOnly` completions: the
    /// documented endgame of one that never comes is a latched command lane.
    fn owes_only_completions(&self, target: CameraId) -> bool {
        self.engine.ledger.entries().all(|(owner, entry)| {
            owner != target
                || (entry.is_debt()
                    && matches!(entry.owes, Owes::Completion | Owes::AcceptedCompletion))
        })
    }

    fn finish(&mut self) {
        for target in [camera(1), camera(2)] {
            assert!(
                self.owes_only_completions(target),
                "seed {} ({}): {target:?} still owes answers after the link recovered: {:?}",
                self.seed,
                self.kind.name,
                self.engine
                    .ledger
                    .entries()
                    .filter(|(owner, _)| *owner == target)
                    .collect::<Vec<_>>(),
            );
            if self.engine.ledger.lane_state(target, Lane::Command) == LaneState::Latched {
                self.coverage.latched_at_end += 1;
                let owner = self
                    .engine
                    .ledger
                    .entries()
                    .find(|(owner, entry)| {
                        *owner == target
                            && matches!(entry.owes, Owes::Completion | Owes::AcceptedCompletion)
                    })
                    .map(|(_, entry)| entry.request);
                let index = match owner {
                    Some(id) if self.co_rejected.contains(&id) => 0,
                    Some(id) if self.co_lost.contains(&id) => 1,
                    Some(id) if self.co_completion_discarded.contains(&id) => 2,
                    _ => 3,
                };
                self.coverage.latched_by_cause[index] += 1;
            }
        }
        assert!(
            self.certain_from.iter().all(Option::is_some),
            "seed {}: a target never healed",
            self.seed
        );
    }
}

/// An urgent typed STOP with a halt's dispatch deadline.
/// A STOP as the owner admits it: within a halt (its siblings share
/// `order`) or alone.
fn stop(target: u8, now: Instant, budget: Duration, order: u64) -> RuntimeRequest {
    let mut request = urgent_command(target, CancellationPolicy::Supported);
    let context = request.context_mut();
    context.submission_order = order;
    context.retry = RetryPolicy {
        movement_not_executable: false,
        total_budget: budget,
        ..retrying()
    };
    context.dispatch_deadline = Some(now + budget);
    request
}

fn socket_at(index: usize) -> ViscaSocket {
    if index == 0 {
        ViscaSocket::S1
    } else {
        ViscaSocket::S2
    }
}

fn socket_index(socket: ViscaSocket) -> usize {
    usize::from(socket != ViscaSocket::S1)
}

fn next_random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
