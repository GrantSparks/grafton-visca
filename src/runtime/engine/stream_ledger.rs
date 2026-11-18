//! The raw byte-stream correlation ledger (#795).
//!
//! A byte stream (TCP or serial) delivers every byte it accepted, in order,
//! and a camera answers each request it reads with one *first answer*,
//! immediately and in read order: an ACK-bearing command an ACK or a
//! rejection, an inquiry its reply or a rejection, a socket cancellation
//! `0x04`/`0x05`, a `CompletionOnly` command a rejection (or, once it has run,
//! its completion). Raw VISCA answers carry no request identity, so on a raw
//! stream session the engine keeps, per camera, every written request whose
//! first answer has not arrived, in write order. Each first-answer frame
//! resolves against the oldest entry that can legally produce it.
//!
//! An entry whose request ends without its answer stays as a *debt*: when the
//! answer arrives it is discarded instead of reaching a later request. A debt
//! that outlives its request's ambiguity window *latches* its overlap class
//! (for inquiries) or ordinary command lane until the answer arrives; it never blocks a STOP and
//! never poisons the session.
//!
//! Write order also heals: a first answer resolved against the entry written
//! at `o` proves that every request written before `o` was answered already,
//! so a debt older than `o` will never be paid and is retired
//! (`settle_in`). A `CompletionOnly` debt is the exception:
//! its completion arrives only once it has run, not in write order, so it is
//! retired only by its own answer. Its rejection would have come in write
//! order, though, so from then on it owes only its completion
//! ([`Owes::AcceptedCompletion`]). A completion settles nothing.
//!
//! A `CompletionOnly` command that may still be rejected is exclusive for
//! ordinary work on its target: while it is unanswered, no inquiry and no
//! ordinary command is written there (its debt holds both lanes). Only a STOP
//! can be written behind it, and a socketless error then could be either
//! one's — the ledger's one ambiguity. It binds to neither; the ledger keeps
//! the union of the debts either explanation leaves, and assigns the error
//! once decisive evidence proves which request sent it, following the
//! dispute through at most [`MAX_DISPUTE_HANDOFFS`] hand-offs
//! ([`StreamLedger::answer`]).
//!
//! Debts are indistinguishable once owed, so consecutive debts with the same
//! owed answer form one *run* with a member count. "Consecutive" is judged
//! among the entries that can compete for an answer: cancellations for one
//! socket among themselves, and every other kind among the others. A run sits
//! at its oldest member's position and pays its members oldest first, which is
//! exactly the order separate entries would pay in. Runs keep memory bounded
//! by [`max_entries_per_target`] however many STOPs a non-answering camera is
//! sent.
//!
//! The model assumes these camera facts (`docs/architecture_2_0.md`, "The raw
//! byte-stream correlation ledger"):
//!
//! - an inquiry reply has the decoder-declared payload shape;
//! - a socket-0 error (`z0 60 02 FF`, `z0 60 03 FF`, or any other) is a first
//!   answer, given before a command is accepted (`docs/visca_reference.md`
//!   §6.3 lists socket 0 only for the syntax and buffer-full errors; an
//!   inquiry answers on socket 0, §6.2);
//! - an error for an executing command names its socket (`z0 6y 04 FF`,
//!   `z0 6y 41 FF`), and a rejection names the socket the camera allocated,
//!   never a busy one (the PTZOptics G2 bench, 2026-10-04, names the next
//!   free socket of its rotation in every rejection);
//! - every written request receives exactly one first answer (a
//!   `CompletionOnly` command's completion may never come);
//! - a camera whose completions name their sockets names them in every
//!   completion of a socketed command. The engine learns this per camera
//!   from its completions and, until it has, treats a socketless completion
//!   conservatively.
//!
//! Under them, every frame binds only to the request that sent it; a
//! disputed frame binds to none, so a request caught in a dispute can end
//! unconfirmed although the camera answered it. When a camera violates them:
//! a socketless execution error or a second answer binds to the oldest legal
//! entry, paying a debt or reaching a live request as a false rejection or
//! ACK; a request never answered pays itself with the next answer of its
//! class, so requests written behind it end unconfirmed (and a waiting one
//! can receive a later request's answer) until a lane latches, write order
//! settles it, or the session is reopened; a `CompletionOnly` command whose
//! completion never comes latches the command lane until the session is
//! reopened; and a camera that names sockets in only some completions can
//! have one socketless completion taken for an outstanding `CompletionOnly`
//! command's before the engine learns that it omits them.

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use smallvec::SmallVec;

use crate::{
    protocol::response::CameraErrorCode,
    target::{PerTarget, TargetSlot},
    PayloadShape, ViscaSocket,
};

use super::{inquiry_shape::InquiryShapes, Lane, RequestId};

/// Command debts at which ordinary command correlation latches. Urgent
/// ACK-bearing STOPs and cancellations continue, and exact inquiry declarations
/// have their separate bounded reservation policy. No unpaid answer is dropped.
pub(crate) const MAX_DEBTS_PER_TARGET: usize = 32;

/// Bounds one camera's queue of live requests and coalesced debt runs.
///
/// Let D=32, A=admitted ordinary+control requests, C=16 exact raw inquiry
/// declarations, and P=MAX_DISPUTE_HANDOFFS+2=5 pinned dispute entries. Choose
/// the last non-inquiry/non-cancellation write that was not an urgent ACK
/// command (or the empty origin). Its prefix held fewer than D debt members
/// plus at most A live entries: at most D-1+A rows, forever. No later operation
/// increases that prefix; retiring/coalescing only preserves or removes rows.
///
/// The suffix contains at most A live non-cancellation entries, C inquiry debts
/// (overlap exclusion makes each a single member), P pinned entries, and urgent
/// ACK debts. Removing those separators leaves at most A+C+P+1 ACK runs, hence
/// at most 2A+2C+2P+1 suffix rows. Cancellations remain socket-specific and add
/// at most 2A+2 rows. Total: D+5A+2C+2P+2 = 5A+76. Inquiry declarations never
/// merge through overlap: every non-cancellation still competes for socketless
/// errors, so even disjoint inquiry data remains an ACK-run separator.
///
/// The engine audits both this bound and its supporting one-overlapping-inquiry
/// invariant. Admission derives the C reservation union from live entries,
/// ledger evidence and skew holds, including queued requests; it cannot forget
/// an unpaid declaration by ending its logical request.
pub(crate) const fn max_entries_per_target(admissible: usize) -> usize {
    match super::checked_stream_ledger_bound(admissible) {
        Some(bound) => bound,
        None => usize::MAX,
    }
}

/// Checked form used by session preflight before allocating or opening I/O.
pub(super) const fn checked_max_entries_per_target(admissible: usize) -> Option<usize> {
    let Some(live) = admissible.checked_mul(5) else {
        return None;
    };
    live.checked_add(
        MAX_DEBTS_PER_TARGET
            + 2
            + 2 * (MAX_DISPUTE_HANDOFFS + 2)
            + 2 * super::inquiry_shape::MAX_RAW_INQUIRY_SHAPE_CLASSES_PER_TARGET,
    )
}

/// The first answer a written request is owed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Owes {
    /// An ACK-bearing command: an ACK, or an error rejecting it.
    Ack,
    /// A `CompletionOnly` command: an error rejecting it, or its completion.
    /// An accepted one answers nothing until it has run, so a later request's
    /// immediate answer may overtake it.
    Completion,
    /// A `CompletionOnly` debt that write order proves accepted (a later
    /// request's first answer arrived, and a rejection would have come
    /// first): only its completion.
    AcceptedCompletion,
    /// A `NoReply` command: nothing, or an error rejecting it. Only STOPs
    /// are written behind it while its hold (#700) lasts. Only write order
    /// retires it (any later first answer settles it, as does a later
    /// request's completion or execution error): a stall can delay its
    /// rejection past any window. It latches no lane, but past its window
    /// `CompletionOnly` commands to its camera fail rather than wait.
    Rejection,
    /// An inquiry: its reply, or an error rejecting it.
    Reply(PayloadShape),
    /// A socket cancellation packet: `0x04` or `0x05` naming its socket.
    Cancel(ViscaSocket),
}

impl Owes {
    /// Whether a debt of this kind holds `lane` back. A `CompletionOnly` debt
    /// that may still be rejected holds both: its target is exclusive until
    /// it is answered or proven accepted.
    const fn holds(self, lane: Lane) -> bool {
        match (self, lane) {
            (Self::Completion, _) => true,
            _ => matches!(
                (self.lane(), lane),
                (Some(Lane::Command), Lane::Command) | (Some(Lane::Inquiry), Lane::Inquiry)
            ),
        }
    }

    /// The lane a debt of this kind latches, if any. A cancellation's answer
    /// can only be confused with another cancellation's, so it never latches.
    pub(crate) const fn lane(self) -> Option<Lane> {
        match self {
            Self::Ack | Self::Completion | Self::AcceptedCompletion => Some(Lane::Command),
            Self::Reply(_) => Some(Lane::Inquiry),
            Self::Rejection | Self::Cancel(_) => None,
        }
    }

    /// Whether some answer can resolve against both kinds, so their relative
    /// order matters. A cancellation's `0x04`/`0x05` names its socket and
    /// resolves nothing else; every other first answer may (a socketless
    /// error) resolve against any non-cancellation entry.
    fn competes_with(self, other: Self) -> bool {
        match (self, other) {
            (Self::Cancel(socket), Self::Cancel(other)) => socket == other,
            (Self::Cancel(_), _) | (_, Self::Cancel(_)) => false,
            _ => true,
        }
    }
}

/// Whether a lane of a camera owes an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LaneState {
    /// Nothing is owed.
    Clear,
    /// An answer is owed inside its window: the lane's new work waits.
    Window,
    /// An owed answer outlived its window: the lane's new work fails unwritten,
    /// and nothing expires or wakes the owner, until the answer arrives.
    Latched,
}

/// Whether an entry's request still awaits its answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Standing {
    /// The request is live and awaits this answer.
    Live,
    /// The request ended without this answer.
    Owed {
        /// End of the owing request's ambiguity window.
        window_end: Instant,
        /// Set by the first due pass at or after `window_end`.
        latched: bool,
        /// For an ACK debt: the owing command's completion deadline. Once its
        /// ACK arrives the command is executing on the camera, and the socket
        /// it names is held this long for its completion.
        completion: Duration,
    },
}

/// One written request whose first answer has not arrived, or a run of
/// consecutive debts owing the same answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Outstanding {
    /// The write order of the entry (of a run, its oldest member).
    pub(crate) order: u64,
    /// The write order of a run's newest member (of a single entry, `order`).
    pub(crate) newest: u64,
    /// The request that wrote the entry. A run keeps the request that opened
    /// it; it only tags the socket hold a paid ACK leaves.
    pub(crate) request: RequestId,
    pub(crate) owes: Owes,
    /// Whether the write completed. A request that ends before its staged
    /// write left owes nothing.
    pub(crate) written: bool,
    pub(crate) standing: Standing,
    /// The answers the entry owes: 1, or a run's member count.
    pub(crate) members: u64,
    /// Whether a disputed answer may have been this entry's
    /// ([`StreamLedger::answer`]).
    pub(crate) disputed: bool,
    /// Whether a tracked dispute names this entry: it keeps its identity
    /// rather than merge into a run.
    pinned: bool,
}

impl Outstanding {
    pub(crate) const fn is_debt(&self) -> bool {
        matches!(self.standing, Standing::Owed { .. })
    }

    const fn latched(&self) -> bool {
        matches!(self.standing, Standing::Owed { latched: true, .. })
    }

    /// Appends the newer debt `newer`, owing the same answer, to this run.
    /// The run latches when its first member's window ends and holds a paid
    /// ACK's socket for the longest completion deadline, so it is never
    /// laxer than its members would be apart; it may stay latched after its
    /// first member is paid.
    fn absorb(&mut self, newer: Self) {
        if let (
            Standing::Owed {
                window_end,
                latched,
                completion,
            },
            Standing::Owed {
                window_end: newer_end,
                latched: newer_latched,
                completion: newer_completion,
            },
        ) = (&mut self.standing, newer.standing)
        {
            *window_end = (*window_end).min(newer_end);
            *latched |= newer_latched;
            *completion = (*completion).max(newer_completion);
        }
        self.members = self.members.saturating_add(newer.members);
        self.newest = self.newest.max(newer.newest);
        self.disputed |= newer.disputed;
    }
}

/// Which first-answer frame is being resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Answer<'a> {
    /// `z0 4y FF`.
    Ack,
    /// Decoded inquiry data; its structure supplies the only extra evidence.
    Reply(&'a [u8]),
    /// `z0 6y 04|05 FF` naming `y`: only a cancellation packet's answer.
    CancelReply(ViscaSocket),
    /// A rejection naming a free socket: the socket the camera allocated for
    /// the command it rejected. Inquiries are never allocated a socket.
    NamedRejection,
    /// A socketless error: the rejection of any request.
    SocketlessError,
    /// A completion with no live owner of its socket: only a
    /// `CompletionOnly` command's.
    Completion,
}

/// One target's write-ordered queue.
type Queue = VecDeque<Outstanding>;

/// Hand-offs a dispute follows before the ledger stops tracking which
/// explanation is true ([`StreamLedger::answer`]).
pub(crate) const MAX_DISPUTE_HANDOFFS: usize = 3;

/// The opening error plus an ACK and its named follow for each hand-off.
/// The first step cannot gain a follow because its evidence is an error.
pub(crate) const MAX_RETRO_FRAMES: usize = 1 + 2 * MAX_DISPUTE_HANDOFFS;
pub(crate) type RetroFrames = SmallVec<[Retro; MAX_RETRO_FRAMES]>;

/// The oldest-first answer queue of every raw stream target.
#[derive(Debug, Default)]
struct LedgerTarget {
    queue: Queue,
    /// The target's open dispute, if any.
    dispute: Option<Dispute>,
}

#[derive(Debug)]
pub(crate) struct StreamLedger {
    targets: PerTarget<LedgerTarget>,
    next_order: u64,
    /// Disputes, hand-offs, and frames that paid a disputed debt, for the
    /// randomized model.
    #[cfg(test)]
    pub(crate) disputes_seen: u64,
}

/// The entry a frame resolves against (`resolve_in`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Resolution {
    /// No entry can legally have sent it.
    Nothing,
    /// The oldest entry that can legally have sent it.
    Oldest(Outstanding),
    /// Either of two entries could have: `first`, whose answer may already
    /// have come (or a `CompletionOnly` or `NoReply` entry that may still be
    /// rejected), or `second` behind it.
    Ambiguous {
        first: Outstanding,
        second: Outstanding,
    },
}

/// What a first-answer frame said, kept to apply once the request that sent
/// it is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Evidence {
    Ack(Option<ViscaSocket>),
    Error(CameraErrorCode),
    /// A completion or error naming a socket, which follows an ACK.
    Named(NamedFrame),
    Other,
}

/// A completion or error naming a socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NamedFrame {
    Completion(ViscaSocket),
    Error(ViscaSocket, CameraErrorCode),
}

impl NamedFrame {
    const fn socket(self) -> ViscaSocket {
        match self {
            Self::Completion(socket) | Self::Error(socket, _) => socket,
        }
    }
}

/// A frame to apply to a request once a dispute proved it was that
/// request's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Retro {
    pub(crate) request: RequestId,
    pub(crate) evidence: Evidence,
}

/// A request a dispute may have answered, with the frame that answered it
/// in that explanation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Step {
    order: u64,
    request: RequestId,
    evidence: Evidence,
    /// A completion or error that named the socket this step's ACK named,
    /// before the dispute was proven: the request's own if the command was
    /// accepted.
    follow: Option<NamedFrame>,
    /// Audit of unpaid members since this step selected its candidate run.
    #[cfg(test)]
    remaining_members: u64,
}

/// A tracked step whose ACK names a socket and has no follow yet.
#[derive(Clone, Copy)]
pub(super) struct FollowCandidate(usize);

/// The opening frame is always an error, so it cannot have an ACK follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpeningStep {
    order: u64,
    request: RequestId,
    evidence: Evidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Proof {
    Either,
    AcceptOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opening {
    CompletionOnly { proof: Proof },
    NoReply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DisputePhase {
    Active,
    Dormant,
}

/// The opening error and the frames handed off since it. Keeping the opening
/// mode closed prevents a NoReply dispute from acquiring acceptance-only proof.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Dispute {
    command: OpeningStep,
    opening: Opening,
    steps: SmallVec<[Step; MAX_DISPUTE_HANDOFFS + 1]>,
    phase: DisputePhase,
}

impl Dispute {
    fn names(&self, order: u64) -> bool {
        self.command.order == order || self.steps.iter().any(|step| step.order == order)
    }
    const fn no_reply(&self) -> bool {
        matches!(self.opening, Opening::NoReply)
    }
    const fn only_accepts(&self) -> bool {
        matches!(
            self.opening,
            Opening::CompletionOnly {
                proof: Proof::AcceptOnly
            }
        )
    }
}

#[derive(Debug, Clone, Copy)]
enum DisputeEvent {
    Unwritten {
        command: bool,
    },
    Answer {
        entry: Outstanding,
        alone: bool,
        handed_to: Option<Step>,
    },
    SettledBefore,
    Completion {
        order: u64,
    },
    Touched,
    WindowEnded,
}

#[derive(Debug, Clone, Copy)]
enum Kept {
    Unchanged,
    AcceptOnly,
    Dormant,
}

#[derive(Debug, Clone, Copy)]
enum Verdict {
    Keep(Kept),
    HandOff(Step),
    Accept,
    Reject,
    Untrack,
}

/// The one decision table for all evidence that can change a dispute. Its
/// executor below is the only place that takes and reinstalls tracked state.
fn verdict(dispute: &Dispute, event: DisputeEvent, queue: &Queue) -> Verdict {
    let candidate = dispute.steps.last().map(|step| step.order);
    let retired = candidate.is_some_and(|order| !queue.iter().any(|entry| entry.order == order));
    match event {
        DisputeEvent::Unwritten { command: true } => Verdict::Accept,
        DisputeEvent::Unwritten { command: false } if dispute.only_accepts() => Verdict::Untrack,
        DisputeEvent::Unwritten { command: false } => Verdict::Reject,
        DisputeEvent::Answer { entry, .. } if entry.order == dispute.command.order => {
            Verdict::Accept
        }
        DisputeEvent::Answer { entry, .. } if dispute.only_accepts() => {
            if Some(entry.order) == candidate {
                Verdict::Untrack
            } else {
                Verdict::Keep(Kept::Unchanged)
            }
        }
        DisputeEvent::Answer {
            entry,
            alone,
            handed_to,
        } => {
            if Some(entry.order) != candidate {
                if retired {
                    Verdict::Accept
                } else {
                    Verdict::Keep(Kept::Unchanged)
                }
            } else if alone {
                Verdict::Reject
            } else if entry.members > 1 {
                // The paid run member is one frame; its remaining members
                // still include the candidate for the opening disputed frame.
                Verdict::Keep(Kept::Unchanged)
            } else {
                match handed_to {
                    Some(step) if dispute.steps.len() <= MAX_DISPUTE_HANDOFFS => {
                        Verdict::HandOff(step)
                    }
                    _ => Verdict::Untrack,
                }
            }
        }
        DisputeEvent::SettledBefore if retired && !dispute.only_accepts() => Verdict::Accept,
        DisputeEvent::Completion { order } if order == dispute.command.order => Verdict::Accept,
        DisputeEvent::Touched if dispute.no_reply() => Verdict::Untrack,
        DisputeEvent::Touched => Verdict::Keep(Kept::AcceptOnly),
        DisputeEvent::WindowEnded => Verdict::Keep(Kept::Dormant),
        DisputeEvent::SettledBefore | DisputeEvent::Completion { .. } => {
            Verdict::Keep(Kept::Unchanged)
        }
    }
}

fn settle(
    queue: &mut Queue,
    tracked: &mut Option<Dispute>,
    event: DisputeEvent,
    answered: &mut Answered,
) {
    let Some(mut dispute) = tracked.take() else {
        return;
    };
    match verdict(&dispute, event, queue) {
        Verdict::Keep(change) => {
            match change {
                Kept::Unchanged => {}
                Kept::AcceptOnly => {
                    dispute.opening = Opening::CompletionOnly {
                        proof: Proof::AcceptOnly,
                    }
                }
                Kept::Dormant => dispute.phase = DisputePhase::Dormant,
            }
            *tracked = Some(dispute);
        }
        Verdict::HandOff(step) => {
            dispute.steps.push(step);
            dispute.phase = DisputePhase::Active;
            *tracked = Some(dispute);
        }
        Verdict::Accept => accept(queue, &dispute, answered),
        Verdict::Reject => reject(queue, &dispute, answered),
        Verdict::Untrack => {}
    }
    // Callers update pins after their whole ledger mutation. In particular,
    // Touched inside answer must not coalesce candidates while that same
    // frame is still being resolved.
}

/// How a first-answer frame resolved.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Resolved {
    /// No entry can have sent it.
    #[default]
    Nothing,
    /// The live entry's request receives it.
    Live(Outstanding),
    /// It paid this debt and is discarded.
    Debt(Outstanding),
    /// The ledger's one ambiguity: it binds to no request
    /// ([`StreamLedger::answer`]). The two entries either of which could
    /// have sent it, as they stood when it arrived.
    Disputed([Outstanding; 2]),
}

/// What a first-answer frame did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Answered {
    pub(crate) resolved: Resolved,
    /// Exact declarations of owed inquiry replies paid or retired.
    pub(crate) replies_settled: InquiryShapes,
    /// Frames a proven dispute assigns to their requests, in arrival order.
    pub(crate) retro: RetroFrames,
    /// Whether a proven dispute retired a `CompletionOnly` entry as rejected.
    pub(crate) completion_only_retired: bool,
    /// A newly tracked dispute, with whether it could retire a written
    /// CompletionOnly command.
    pub(super) completion_dispute: Option<bool>,
}

impl StreamLedger {
    pub(crate) fn new() -> Self {
        Self {
            targets: PerTarget::default(),
            next_order: 0,
            #[cfg(test)]
            disputes_seen: 0,
        }
    }

    fn queue(&self, target: TargetSlot) -> &Queue {
        &self.targets[target].queue
    }

    fn queue_mut(&mut self, target: TargetSlot) -> &mut Queue {
        &mut self.targets[target].queue
    }

    /// Appends a request whose write is about to start.
    pub(crate) fn push(&mut self, target: TargetSlot, request: RequestId, owes: Owes) -> u64 {
        let order = self.next_order;
        self.next_order = self.next_order.wrapping_add(1);
        self.queue_mut(target).push_back(Outstanding {
            order,
            newest: order,
            request,
            owes,
            written: false,
            standing: Standing::Live,
            members: 1,
            disputed: false,
            pinned: false,
        });
        order
    }

    /// Records that the live entry of `request` with this kind was written.
    pub(crate) fn mark_written(&mut self, target: TargetSlot, request: RequestId, cancel: bool) {
        if let Some(entry) = self.queue_mut(target).iter_mut().find(|entry| {
            entry.request == request
                && entry.standing == Standing::Live
                && matches!(entry.owes, Owes::Cancel(_)) == cancel
        }) {
            entry.written = true;
        }
    }

    /// The entries `request` still awaits.
    pub(crate) fn live(
        &self,
        target: TargetSlot,
        request: RequestId,
    ) -> impl Iterator<Item = &Outstanding> {
        self.queue(target)
            .iter()
            .filter(move |entry| entry.request == request && entry.standing == Standing::Live)
    }

    /// Ends the live entries of `request`: one never written leaves the
    /// ledger, and a written one becomes a debt whose window `window` supplies
    /// for its kind. Debts are never dropped.
    ///
    /// A frame can resolve against an entry whose write result has not come
    /// yet (an answer may overtake it). If the request then ends unwritten,
    /// it was a staged write that never left, so that frame was not its
    /// answer, and a tracked dispute naming it is decided: if it is the
    /// `CompletionOnly` command, the command sent nothing and the disputed
    /// frames were its steps' (accepted); if it is a step, that step sent
    /// nothing and the command was rejected — unless the dispute was demoted
    /// (accept-only): its steps were chosen through entries already
    /// disputed, so a step's identity is inferred rather than proven, and the
    /// dispute is untracked instead. Returns what the dispute assigns.
    pub(crate) fn retire(
        &mut self,
        target: TargetSlot,
        request: RequestId,
        mut window: impl FnMut(Owes) -> (Instant, Duration),
    ) -> Answered {
        let state = &mut self.targets[target];
        let queue = &mut state.queue;
        let unwritten = |entry: &Outstanding| {
            entry.request == request && entry.standing == Standing::Live && !entry.written
        };
        let mut answered = Answered::default();
        let decided = state.dispute.as_ref().and_then(|dispute| {
            queue
                .iter()
                .find(|entry| unwritten(entry) && dispute.names(entry.order))
                .map(|entry| entry.order == dispute.command.order)
        });
        if let Some(command) = decided {
            settle(
                queue,
                &mut state.dispute,
                DisputeEvent::Unwritten { command },
                &mut answered,
            );
            pin(queue, state.dispute.as_ref());
        }
        queue.retain(|entry| !unwritten(entry));
        for entry in queue
            .iter_mut()
            .filter(|entry| entry.request == request && entry.standing == Standing::Live)
        {
            let (window_end, completion) = window(entry.owes);
            entry.standing = Standing::Owed {
                window_end,
                latched: false,
                completion,
            };
        }
        coalesce(queue);
        answered
    }

    /// Resolves a first-answer frame, `evidence` saying what it was, and
    /// applies it: the oldest entry that can legally have sent it takes it,
    /// and every first answer but a completion settles the entries written
    /// before that entry (`settle_in`).
    ///
    /// A frame is *disputed* when its oldest candidate may already have been
    /// answered — a disputed live entry, or (for a socketless error) a
    /// `CompletionOnly` or `NoReply` entry that may still be rejected — and
    /// another candidate is behind it. Behind such a command nothing but
    /// STOPs is written, one waiting at a time, so this is that command's
    /// rejection or the STOP's answer, and no length of time can tell which: a stall delays the
    /// STOP's ACK without bound. The frame binds to neither, and the ledger
    /// keeps the union of the debts either explanation leaves: the command
    /// owes only its completion ([`Owes::AcceptedCompletion`]; a `NoReply`
    /// command, nothing), and the STOP keeps owing its answer, *disputed*. No other ACK-bearing request is
    /// written behind a disputed live entry ([`Self::forbids_crossing`]).
    ///
    /// Only decisive evidence settles which explanation is true, and the
    /// proven side then receives the disputed error ([`Answered::retro`]):
    ///
    /// - The STOP's own answer, when no other entry could have sent it,
    ///   proves the command was rejected: its entry is retired and the error
    ///   is its rejection.
    /// - The command's completion, or write order (a later request answered
    ///   while the STOP's answer had not come), proves it was accepted: the
    ///   error was the STOP's.
    /// - A frame that pays the STOP's debt while another request could have
    ///   sent it is *handed off*: it was the STOP's late answer if the
    ///   command was rejected, or the next waiting request's own answer if
    ///   not, so that request is disputed in its turn, and the frame is kept
    ///   for it. After [`MAX_DISPUTE_HANDOFFS`] hand-offs, or when another
    ///   dispute touches the requests involved, the ledger stops tracking the
    ///   explanations: the disputed entries keep only their union of debts,
    ///   and if the command was in fact rejected its debt latches the command
    ///   lane — the documented endgame.
    ///
    /// An extra debt only discards a future answer; it never binds one to the
    /// wrong request.
    /// Inspects first-answer candidates without consuming their evidence.
    pub(crate) fn candidates(
        &self,
        target: TargetSlot,
        answer: Answer<'_>,
    ) -> [Option<Outstanding>; 2] {
        match resolve_in(self.queue(target), answer) {
            Resolution::Nothing => [None, None],
            Resolution::Oldest(entry) => [Some(entry), None],
            Resolution::Ambiguous { first, second } => [Some(first), Some(second)],
        }
    }

    /// Returns a uniquely attributable first answer, excluding runs and
    /// staged writes. This can prove an earlier frame was an execution error.
    pub(super) fn sole_written_candidate(
        &self,
        target: TargetSlot,
        answer: Answer<'_>,
    ) -> Option<Outstanding> {
        let mut legal_entries = self
            .queue(target)
            .iter()
            .filter(|entry| legal(answer, entry.owes));
        let candidate = legal_entries.next()?;
        (candidate.written && candidate.members == 1 && legal_entries.next().is_none())
            .then_some(*candidate)
    }

    pub(crate) fn answer(
        &mut self,
        target: TargetSlot,
        answer: Answer<'_>,
        evidence: Evidence,
    ) -> Answered {
        let state = &mut self.targets[target];
        let queue = &mut state.queue;
        let mut answered = Answered {
            resolved: Resolved::Nothing,
            replies_settled: InquiryShapes::default(),
            retro: SmallVec::new(),
            completion_only_retired: false,
            completion_dispute: None,
        };
        match resolve_in(queue, answer) {
            Resolution::Nothing => {}
            Resolution::Oldest(entry) => {
                let alone = entry.members == 1
                    && queue
                        .iter()
                        .filter(|other| legal(answer, other.owes))
                        .count()
                        == 1;
                answered.replies_settled = apply_in(queue, entry);
                #[cfg(test)]
                if let Some(dispute) = state.dispute.as_mut() {
                    for step in dispute
                        .steps
                        .iter_mut()
                        .filter(|step| step.order == entry.order)
                    {
                        step.remaining_members = step.remaining_members.saturating_sub(1);
                    }
                }
                // A frame that pays a disputed debt may have been the next
                // request's own answer (past the rest of the paid entry, if
                // a run: its members are alike).
                let handed_to = if entry.is_debt() && entry.disputed {
                    #[cfg(test)]
                    {
                        self.disputes_seen += 1;
                    }
                    dispute_behind(queue, entry.order, answer, evidence)
                } else {
                    None
                };
                settle(
                    queue,
                    &mut state.dispute,
                    DisputeEvent::Answer {
                        entry,
                        alone,
                        handed_to,
                    },
                    &mut answered,
                );
                answered.resolved = if entry.is_debt() {
                    Resolved::Debt(entry)
                } else {
                    Resolved::Live(entry)
                };
            }
            Resolution::Ambiguous { first, second } => {
                let queue = &mut state.queue;
                let candidates = [first, second];
                let first = first.order;
                let opened = state.dispute.is_none();
                // A second dispute while one is tracked: only the command's
                // completion can still prove it (accepted).
                settle(
                    queue,
                    &mut state.dispute,
                    DisputeEvent::Touched,
                    &mut answered,
                );
                let head_owes = queue
                    .iter()
                    .find(|entry| entry.order == first)
                    .map(|entry| entry.owes);
                let completion_only = queue
                    .iter()
                    .find(|entry| {
                        entry.order == first
                            && matches!(entry.owes, Owes::Completion | Owes::Rejection)
                    })
                    .copied();
                // Each entry behind the first that the frame could be the own
                // answer of is disputed, through the first that certainly
                // still owes it (a `NoReply` command's, which may answer
                // nothing, passes it on): the dispute's STOP.
                let stop = dispute_behind(queue, first, answer, evidence);
                if let (true, Some(completion_only), Some(stop), Evidence::Error(code)) =
                    (opened, completion_only, stop, evidence)
                {
                    answered.completion_dispute =
                        Some(head_owes == Some(Owes::Completion) && completion_only.written);
                    state.dispute = Some(Dispute {
                        command: OpeningStep {
                            order: completion_only.order,
                            request: completion_only.request,
                            evidence: Evidence::Error(code),
                        },
                        opening: if head_owes == Some(Owes::Rejection) {
                            Opening::NoReply
                        } else {
                            Opening::CompletionOnly {
                                proof: Proof::Either,
                            }
                        },
                        steps: std::iter::once(stop).collect(),
                        phase: DisputePhase::Active,
                    });
                }
                let tracked = state.dispute.is_some();
                // A `NoReply` command's rejection, had it come, came first
                // either way: it owes nothing more (one member of a run of
                // them: the others still may).
                if head_owes == Some(Owes::Rejection) {
                    take_in(queue, first);
                }
                for entry in queue.iter_mut() {
                    if entry.order == first && entry.owes == Owes::Completion {
                        entry.owes = Owes::AcceptedCompletion;
                        entry.pinned = tracked;
                    } else if stop.is_some_and(|stop| stop.order == entry.order) {
                        entry.pinned = tracked;
                    }
                }
                coalesce(queue);
                #[cfg(test)]
                {
                    self.disputes_seen += 1;
                }
                answered.resolved = Resolved::Disputed(candidates);
            }
        }
        pin(&mut state.queue, state.dispute.as_ref());
        answered
    }

    /// Settles what was written before `order` (`settle_in`): a completion
    /// or execution error of the request written there arrived, which
    /// follows that request's first answer and so every earlier one. Write
    /// order retiring a tracked dispute's candidate proves it accepted, as
    /// for a first answer.
    pub(crate) fn settle_before(&mut self, target: TargetSlot, order: u64) -> Answered {
        let state = &mut self.targets[target];
        let queue = &mut state.queue;
        let mut answered = Answered {
            resolved: Resolved::Nothing,
            replies_settled: settle_in(queue, order),
            retro: SmallVec::new(),
            completion_only_retired: false,
            completion_dispute: None,
        };
        settle(
            queue,
            &mut state.dispute,
            DisputeEvent::SettledBefore,
            &mut answered,
        );
        pin(&mut state.queue, state.dispute.as_ref());
        answered
    }

    /// Finds the tracked hand-off whose ACK names this socket, without
    /// claiming its possible terminal frame.
    pub(super) fn follow_candidate(
        &self,
        target: TargetSlot,
        socket: ViscaSocket,
    ) -> Option<FollowCandidate> {
        self.targets[target]
            .dispute
            .as_ref()?
            .steps
            .iter()
            .position(|step| step.follow.is_none() && step.evidence == Evidence::Ack(Some(socket)))
            .map(FollowCandidate)
    }

    /// Records the terminal of a tracked ACK. Once the dispute is accepted,
    /// both frames are assigned to the same request.
    pub(super) fn record_follow(
        &mut self,
        target: TargetSlot,
        candidate: FollowCandidate,
        frame: NamedFrame,
    ) {
        let Some(step) = self.targets[target]
            .dispute
            .as_mut()
            .and_then(|dispute| dispute.steps.get_mut(candidate.0))
        else {
            unreachable!("the follow candidate came from the current dispute");
        };
        debug_assert!(step.follow.is_none());
        debug_assert_eq!(step.evidence, Evidence::Ack(Some(frame.socket())));
        step.follow = Some(frame);
    }

    /// Whether a tracked dispute may already hold `request`'s answer: the
    /// disputed frame, or one handed off since, may have been its own.
    pub(crate) fn awaits_dispute(&self, target: TargetSlot, request: RequestId) -> bool {
        self.targets[target]
            .dispute
            .as_ref()
            .is_some_and(|dispute| dispute.steps.iter().any(|step| step.request == request))
    }

    /// Checks that every tracked dispute names a `CompletionOnly` entry still
    /// owed and follows no more than [`MAX_DISPUTE_HANDOFFS`] hand-offs.
    pub(crate) fn check_disputes(&self) -> Result<(), &'static str> {
        for (_, state) in self.targets.iter() {
            let queue = &state.queue;
            let dispute = &state.dispute;
            let Some(dispute) = dispute else {
                continue;
            };
            if !dispute.no_reply()
                && !queue.iter().any(|entry| {
                    entry.order == dispute.command.order && entry.owes == Owes::AcceptedCompletion
                })
            {
                return Err("a tracked dispute names no owed CompletionOnly entry");
            }
            if dispute.steps.is_empty() || dispute.steps.len() > MAX_DISPUTE_HANDOFFS + 1 {
                return Err("a tracked dispute exceeds its hand-offs");
            }
            #[cfg(test)]
            for step in &dispute.steps {
                let remaining = queue
                    .iter()
                    .find(|entry| entry.order == step.order)
                    .map_or(0, |entry| entry.members);
                if remaining < step.remaining_members {
                    return Err("a dispute run lost a member without an assigned frame");
                }
            }
        }
        Ok(())
    }

    /// Takes the oldest `CompletionOnly` entry's completion: a socketless
    /// completion dropped while it was unknown whether this camera names its
    /// sockets in completions was that command's, now that the camera is
    /// known to. Returns the entry (a live one's request receives it).
    ///
    /// That completion proves the command accepted, so a tracked dispute
    /// over it is decided as by any completion of it: the disputed frames
    /// were its steps', which the returned [`Answered`] assigns.
    pub(crate) fn take_completion(
        &mut self,
        target: TargetSlot,
    ) -> Option<(Outstanding, Answered)> {
        let state = &mut self.targets[target];
        let queue = &mut state.queue;
        let entry = queue
            .iter()
            .find(|entry| matches!(entry.owes, Owes::Completion | Owes::AcceptedCompletion))
            .copied()?;
        let mut answered = Answered::default();
        settle(
            queue,
            &mut state.dispute,
            DisputeEvent::Completion { order: entry.order },
            &mut answered,
        );
        pin(queue, state.dispute.as_ref());
        take_in(queue, entry.order);
        Some((entry, answered))
    }

    /// Marks the entry `answer` would resolve as disputed, without paying it,
    /// and returns whether there was one: a frame that may have been its
    /// answer was taken for another request's (a rejection naming a socket
    /// that an ended or executing command may still hold, which could be
    /// that command's execution error or this one's rejection).
    pub(crate) fn dispute(&mut self, target: TargetSlot, answer: Answer<'_>) -> bool {
        let queue = self.queue_mut(target);
        // When the frame is already ambiguous between waiting requests (the
        // oldest one disputed), it is theirs as much as the oldest one's:
        // disputing that one passes it on to the next, too.
        let entry = match resolve_in(queue, answer) {
            Resolution::Oldest(entry) | Resolution::Ambiguous { first: entry, .. } => entry,
            Resolution::Nothing => return false,
        };
        if let Some(disputed) = queue.iter_mut().find(|other| other.order == entry.order) {
            disputed.disputed = true;
        }
        // One whose answer may already have come passes the frame on.
        if entry.disputed {
            dispute_behind(queue, entry.order, answer, Evidence::Other);
        }
        // Another dispute over a tracked request: only the command's
        // completion can still prove it (accepted).
        let state = &mut self.targets[target];
        if state
            .dispute
            .as_ref()
            .is_some_and(|dispute| dispute.names(entry.order))
        {
            settle(
                &mut state.queue,
                &mut state.dispute,
                DisputeEvent::Touched,
                &mut Answered::default(),
            );
            pin(&mut state.queue, state.dispute.as_ref());
        }
        true
    }

    /// Whether nothing may be written to cross another unacknowledged
    /// command on `target`, and no inquiry may be written there: a disputed
    /// entry is live (its own answer must stay decisive), a dispute is
    /// tracked (no second dispute may involve its requests), or a STOP
    /// already waits behind a `CompletionOnly` entry that may still be
    /// rejected (so that any dispute it causes involves one STOP).
    pub(crate) fn forbids_crossing(&self, target: TargetSlot) -> bool {
        let queue = self.queue(target);
        let disputed_live = queue.iter().any(|entry| entry.disputed && !entry.is_debt());
        let unproven = queue
            .iter()
            .any(|entry| matches!(entry.owes, Owes::Completion | Owes::Rejection));
        let waiting_ack = queue
            .iter()
            .any(|entry| entry.owes == Owes::Ack && !entry.is_debt());
        let tracked = self.targets[target]
            .dispute
            .as_ref()
            .is_some_and(|dispute| dispute.phase != DisputePhase::Dormant);
        disputed_live || tracked || (unproven && waiting_ack)
    }

    /// Whether a `CompletionOnly` command's answer is outstanding on
    /// `target`.
    /// Whether a `NoReply` command's possible rejection is still owed on
    /// `target` past its window: `CompletionOnly` commands there fail
    /// rather than wait.
    pub(crate) fn rejection_latched(&self, target: TargetSlot) -> bool {
        self.queue(target)
            .iter()
            .any(|entry| entry.owes == Owes::Rejection && entry.latched())
    }

    /// Whether a `NoReply` command's possible rejection is owed on
    /// `target`: until a later first answer settles it, a `CompletionOnly`
    /// command's socketless rejection could not be told from it.
    pub(crate) fn owes_rejection(&self, target: TargetSlot) -> bool {
        self.queue(target)
            .iter()
            .any(|entry| entry.owes == Owes::Rejection)
    }

    pub(crate) fn owes_completion(&self, target: TargetSlot) -> bool {
        self.queue(target)
            .iter()
            .any(|entry| matches!(entry.owes, Owes::Completion | Owes::AcceptedCompletion))
    }

    /// The answers `target` owes for requests that already ended.
    pub(crate) fn debts(&self, target: TargetSlot) -> u64 {
        self.queue(target)
            .iter()
            .filter(|entry| entry.is_debt())
            .map(|entry| entry.members)
            .sum()
    }

    /// The entries (runs and live requests) `target`'s queue holds.
    pub(crate) fn len(&self, target: TargetSlot) -> usize {
        self.queue(target).len()
    }

    pub(crate) fn owes_reply(&self, target: TargetSlot, shape: PayloadShape) -> bool {
        self.queue(target)
            .iter()
            .any(|entry| entry.is_debt() && entry.owes == Owes::Reply(shape))
    }

    /// Exact declarations currently represented by written or staged inquiries.
    /// This view contributes to admission's derived reservation union; no shape
    /// class can disappear merely because its logical request has ended.
    pub(crate) fn inquiry_shapes(
        &self,
        target: TargetSlot,
    ) -> impl Iterator<Item = PayloadShape> + '_ {
        self.queue(target)
            .iter()
            .filter_map(|entry| match entry.owes {
                Owes::Reply(shape) => Some(shape),
                _ => None,
            })
    }

    /// Only an overlapping inquiry debt holds an inquiry back. The global
    /// command-debt cap does not turn an unrelated inquiry debt into a match:
    /// admission separately bounds exact declarations, preserving memory while
    /// allowing a disjoint position poll to recover a stalled mode inquiry.
    /// CompletionOnly debt and unresolved command disputes remain target-wide.
    pub(crate) fn inquiry_state(&self, target: TargetSlot, shape: PayloadShape) -> LaneState {
        if self.targets[target]
            .dispute
            .as_ref()
            .is_some_and(|dispute| dispute.phase == DisputePhase::Dormant && !dispute.no_reply())
        {
            return LaneState::Latched;
        }
        self.queue(target)
            .iter()
            .filter(|entry| {
                entry.is_debt()
                    && match entry.owes {
                        Owes::Reply(owed) => owed.overlaps(shape),
                        Owes::Completion => true,
                        _ => false,
                    }
            })
            .fold(LaneState::Clear, |state, entry| {
                if entry.latched() {
                    LaneState::Latched
                } else if state == LaneState::Clear {
                    LaneState::Window
                } else {
                    state
                }
            })
    }

    /// Whether `target` owes [`MAX_DEBTS_PER_TARGET`] debts: every lane
    /// latches, and nothing but STOPs and cancellations is written, until
    /// debts are paid.
    fn capped(&self, target: TargetSlot) -> bool {
        self.debts(target) >= MAX_DEBTS_PER_TARGET as u64
    }

    /// Whether `lane` on `target` owes an answer, and whether it has latched.
    /// At [`MAX_DEBTS_PER_TARGET`] every lane latches until debts are paid.
    pub(crate) fn lane_state(&self, target: TargetSlot, lane: Lane) -> LaneState {
        if self.capped(target) {
            return LaneState::Latched;
        }
        // A dormant dispute's `CompletionOnly` command, never proven
        // accepted, holds both lanes (its command lane through its debt).
        if lane == Lane::Inquiry
            && self.targets[target]
                .dispute
                .as_ref()
                .is_some_and(|dispute| {
                    dispute.phase == DisputePhase::Dormant && !dispute.no_reply()
                })
        {
            return LaneState::Latched;
        }
        self.queue(target)
            .iter()
            .filter(|entry| entry.owes.holds(lane) && entry.is_debt())
            .fold(LaneState::Clear, |state, entry| {
                if entry.latched() {
                    LaneState::Latched
                } else if state == LaneState::Clear {
                    LaneState::Window
                } else {
                    state
                }
            })
    }

    /// Latches every debt whose window has ended. Nothing is forgotten by
    /// time: a stall can outlast any window. A tracked dispute whose requests
    /// have all ended, and whose windows have, goes *dormant*: it falls to
    /// the documented latch — it no longer holds anything back, and the
    /// camera's lanes latch (all inquiry classes too, while a `CompletionOnly`
    /// command is not proven accepted, like any whose completion never
    /// came), so work they refuse fails at once rather than waiting — while
    /// a late answer can still settle it and reopen them.
    pub(crate) fn latch_due(&mut self, now: Instant) {
        for (_, state) in self.targets.iter_mut() {
            let queue = &state.queue;
            let dispute = &mut state.dispute;
            let ended = dispute.as_ref().is_some_and(|dispute| {
                queue
                    .iter()
                    .filter(|entry| dispute.names(entry.order))
                    .all(|entry| {
                        matches!(entry.standing, Standing::Owed { window_end, .. } if window_end <= now)
                    })
            });
            if ended {
                settle(
                    &mut state.queue,
                    dispute,
                    DisputeEvent::WindowEnded,
                    &mut Answered::default(),
                );
            }
        }
        for entry in self
            .targets
            .iter_mut()
            .flat_map(|(_, state)| &mut state.queue)
        {
            if let Standing::Owed {
                window_end,
                latched,
                ..
            } = &mut entry.standing
            {
                if *window_end <= now && latches(entry.owes) {
                    *latched = true;
                }
            }
        }
    }

    /// The earliest unlatched window end: when a lane may latch.
    pub(crate) fn wake(&self) -> Option<Instant> {
        self.targets
            .iter()
            .flat_map(|(_, state)| &state.queue)
            .filter_map(|entry| match entry.standing {
                Standing::Owed {
                    window_end,
                    latched: false,
                    ..
                } if latches(entry.owes) => Some(window_end),
                _ => None,
            })
            .min()
    }

    #[cfg(test)]
    pub(crate) fn clear(&mut self) {
        self.targets = PerTarget::default();
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.targets
            .iter()
            .all(|(_, state)| state.queue.is_empty() && state.dispute.is_none())
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = (TargetSlot, &Outstanding)> {
        self.targets
            .iter()
            .flat_map(|(target, state)| state.queue.iter().map(move |entry| (target, entry)))
    }
}

/// Whether a debt owing `owes` latches when its window ends: one that holds
/// a lane, or a `NoReply` command's possible rejection (which holds back
/// only `CompletionOnly` commands).
const fn latches(owes: Owes) -> bool {
    owes.lane().is_some() || matches!(owes, Owes::Rejection)
}

/// Pins the entries a tracked dispute names, and only those, so that they
/// keep their identity; entries it no longer names may merge again.
fn pin(queue: &mut Queue, dispute: Option<&Dispute>) {
    let named = |order: u64| dispute.is_some_and(|dispute| dispute.names(order));
    let mut released = false;
    for entry in queue.iter_mut() {
        let pinned = named(entry.order);
        released |= entry.pinned && !pinned;
        entry.pinned = pinned;
    }
    if released {
        coalesce(queue);
    }
}

/// Disputes the entries behind `after` that the frame `answer` could have
/// been the own answer of: each one through the first that certainly still
/// owes it. An entry that may answer nothing (a `NoReply` command's) or whose
/// answer may already have come (a disputed one) passes the frame on.
/// Returns the first entry that must answer, which a tracked dispute hands
/// off to.
fn dispute_behind(
    queue: &mut Queue,
    after: u64,
    answer: Answer<'_>,
    evidence: Evidence,
) -> Option<Step> {
    let mut handed_to = None;
    for entry in queue.iter_mut() {
        if entry.order <= after || !legal(answer, entry.owes) {
            continue;
        }
        let passes_on = entry.owes == Owes::Rejection || entry.disputed;
        entry.disputed = true;
        if entry.owes != Owes::Rejection && handed_to.is_none() {
            handed_to = Some(Step {
                order: entry.order,
                request: entry.request,
                evidence,
                follow: None,
                #[cfg(test)]
                remaining_members: entry.members,
            });
        }
        if !passes_on {
            break;
        }
    }
    handed_to
}

/// A dispute proved the `CompletionOnly` command accepted: the disputed frame
/// and every frame handed off since answered the requests in its steps,
/// which owe nothing more and receive them.
fn accept(queue: &mut Queue, dispute: &Dispute, answered: &mut Answered) {
    for step in &dispute.steps {
        let before = queue
            .iter()
            .find(|entry| entry.order == step.order)
            .map_or(0, |entry| entry.members);
        take_in(queue, step.order);
        let after = queue
            .iter()
            .find(|entry| entry.order == step.order)
            .map_or(0, |entry| entry.members);
        debug_assert_eq!(
            after,
            before.saturating_sub(1),
            "one disputed frame pays exactly one member"
        );
        if let Some(entry) = queue.iter_mut().find(|entry| entry.order == step.order) {
            entry.disputed = false;
            entry.pinned = false;
        }
        for evidence in std::iter::once(step.evidence).chain(step.follow.map(Evidence::Named)) {
            answered.retro.push(Retro {
                request: step.request,
                evidence,
            });
        }
    }
    coalesce(queue);
}

/// A dispute proved the `CompletionOnly` command rejected: the disputed
/// error was its rejection, and its entry is retired.
fn reject(queue: &mut Queue, dispute: &Dispute, answered: &mut Answered) {
    // A `NoReply` command's entry is gone, and it already ended.
    if dispute.no_reply() {
        return;
    }
    take_in(queue, dispute.command.order);
    answered.retro.push(Retro {
        request: dispute.command.request,
        evidence: dispute.command.evidence,
    });
    answered.completion_only_retired = true;
}

/// The oldest entry of `queue` that `answer` can legally resolve, nothing
/// when none can, or the two it is ambiguous between. Ambiguity arises only through a
/// `CompletionOnly` entry that may still be rejected: an accepted one answers
/// nothing until it has run, so a socketless error behind it could also be
/// the answer of a later request (another member of the same run is no
/// different). A `CompletionOnly` command earns no socket, so a named
/// rejection is never its.
fn resolve_in(queue: &Queue, answer: Answer<'_>) -> Resolution {
    let mut candidates = queue.iter().filter(|entry| legal(answer, entry.owes));
    let Some(oldest) = candidates.next().copied() else {
        return Resolution::Nothing;
    };
    // A disputed live entry's answer may already have come, and so may a
    // `CompletionOnly` command's rejection.
    let uncertain = (oldest.disputed && !oldest.is_debt())
        || (answer == Answer::SocketlessError
            && matches!(oldest.owes, Owes::Completion | Owes::Rejection));
    if uncertain {
        if let Some(second) = candidates.next() {
            return Resolution::Ambiguous {
                first: oldest,
                second: *second,
            };
        }
    }
    Resolution::Oldest(oldest)
}

/// Whether `answer` can be a first answer for an entry owing `owes`.
fn legal(answer: Answer<'_>, owes: Owes) -> bool {
    match (answer, owes) {
        (Answer::Ack, Owes::Ack) => true,
        (Answer::Reply(payload), Owes::Reply(shape)) => shape.admits(payload),
        (Answer::CancelReply(socket), Owes::Cancel(owed)) => socket == owed,
        (Answer::NamedRejection, Owes::Ack) => true,
        (Answer::SocketlessError, owes) => {
            !matches!(owes, Owes::Cancel(_) | Owes::AcceptedCompletion)
        }
        (Answer::Completion, Owes::Completion | Owes::AcceptedCompletion) => true,
        _ => false,
    }
}

/// Applies an answer resolved against `entry`: settles what was written
/// before it (a completion too: it follows the request's own first answer,
/// and so every earlier one), then takes it. Returns the exact declarations of
/// owed inquiry replies paid or retired.
fn apply_in(queue: &mut Queue, entry: Outstanding) -> InquiryShapes {
    // Settle first: taking the entry may merge the runs it separated.
    let mut retired = settle_in(queue, entry.order);
    take_in(queue, entry.order);
    if let (true, Owes::Reply(shape)) = (entry.is_debt(), entry.owes) {
        assert!(
            retired.try_insert(shape),
            "inquiry declaration capacity invariant"
        );
    }
    retired
}

/// Takes one answer from the entry written at `order` (its answer arrived):
/// a run's oldest member, or the whole entry.
fn take_in(queue: &mut Queue, order: u64) {
    let Some(index) = queue.iter().position(|entry| entry.order == order) else {
        return;
    };
    if queue[index].members > 1 {
        queue[index].members -= 1;
    } else {
        queue.remove(index);
        coalesce(queue);
    }
}

/// Settles every debt written wholly before `order` once a first answer
/// resolved against the entry written at `order`: the camera answers in
/// write order, so those debts' first answers would have come first. Each is
/// retired, except a `CompletionOnly` entry (debt or live), whose completion
/// comes only once it has run: that one now owes only its completion. Returns
/// the exact declarations of owed inquiry replies retired. A run that also has members
/// written after `order` (one that merged past it) is kept whole: keeping a
/// debt only discards a future answer, never misattributes one.
fn settle_in(queue: &mut Queue, order: u64) -> InquiryShapes {
    let mut replies = InquiryShapes::default();
    queue.retain_mut(|entry| {
        if entry.newest >= order {
            return true;
        }
        // A live `CompletionOnly` command was not rejected either: it now
        // owes only its completion. A disputed live entry's answer must have
        // been the disputed one, so it owes nothing more. Other live entries
        // run to their own deadlines.
        if !entry.is_debt() {
            if entry.owes == Owes::Completion {
                entry.owes = Owes::AcceptedCompletion;
            }
            return !entry.disputed;
        }
        match entry.owes {
            Owes::Completion => {
                entry.owes = Owes::AcceptedCompletion;
                true
            }
            Owes::AcceptedCompletion => true,
            Owes::Reply(shape) => {
                assert!(
                    replies.try_insert(shape),
                    "inquiry declaration capacity invariant"
                );
                false
            }
            Owes::Ack | Owes::Rejection | Owes::Cancel(_) => false,
        }
    });
    coalesce(queue);
    replies
}

/// Merges every debt into the nearest older entry it competes with, when that
/// is a debt owing the same answer. The merged members move only past entries
/// no answer can confuse them with, so resolution order is unchanged. One
/// forward pass leaves no mergeable pair.
fn coalesce(queue: &mut Queue) {
    let mut index = 1;
    while index < queue.len() {
        let entry = queue[index];
        let into = (entry.is_debt() && !entry.pinned)
            .then(|| {
                (0..index)
                    .rev()
                    .find(|older| queue[*older].owes.competes_with(entry.owes))
            })
            .flatten()
            .filter(|older| {
                queue[*older].is_debt() && !queue[*older].pinned && queue[*older].owes == entry.owes
            });
        if let Some(older) = into {
            queue[older].absorb(entry);
            queue.remove(index);
        } else {
            index += 1;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::num::NonZeroU64;

    use super::*;

    const TARGET: TargetSlot = TargetSlot::ALL[0];

    fn request(id: u64) -> RequestId {
        RequestId::from_nonzero(NonZeroU64::new(id).unwrap())
    }

    /// Writes one entry per kind in order, then ends every request unanswered.
    fn owed(kinds: &[Owes]) -> StreamLedger {
        let mut ledger = StreamLedger::new();
        let now = Instant::now();
        for (index, owes) in (1..).zip(kinds) {
            let cancel = matches!(owes, Owes::Cancel(_));
            ledger.push(TARGET, request(index), *owes);
            ledger.mark_written(TARGET, request(index), cancel);
        }
        for index in (1..).take(kinds.len()) {
            ledger.retire(TARGET, request(index), |_| (now, Duration::ZERO));
        }
        ledger
    }

    fn evidence(answer: Answer<'_>) -> Evidence {
        match answer {
            Answer::Ack => Evidence::Ack(None),
            Answer::SocketlessError | Answer::NamedRejection => Evidence::Error(0x41.into()),
            _ => Evidence::Other,
        }
    }

    fn shape(ledger: &StreamLedger) -> Vec<(Owes, u64)> {
        ledger
            .queue(TARGET)
            .iter()
            .map(|entry| (entry.owes, entry.members))
            .collect()
    }

    /// A completion or execution error of the request written at `o` follows
    /// that request's first answer, and so every first answer written before
    /// `o`: it settles older `NoReply` and ACK debts, whatever the time.
    #[test]
    fn a_completion_settles_the_debts_written_before_its_request() {
        let reply = Owes::Reply(PayloadShape::Unknown);
        let mut ledger = owed(&[Owes::Rejection, Owes::Ack, reply]);
        assert_eq!(ledger.debts(TARGET), 3);
        let last = ledger
            .queue(TARGET)
            .back()
            .map(|entry| entry.order)
            .unwrap();
        let answered = ledger.settle_before(TARGET, last);
        assert!(answered.retro.is_empty() && answered.replies_settled.is_empty());
        assert_eq!(shape(&ledger), [(reply, 1)]);
    }

    /// A named rejection that a busy socket's owner or a waiting command
    /// could have earned is disputed rather than given to the owner, also
    /// when the oldest waiting command is itself disputed already (randomized
    /// model, a camera that omits the socket in completions, seed 371: the
    /// rejection reached the executing owner as an error after its ACK).
    #[test]
    fn a_rejection_already_ambiguous_between_waiting_commands_is_disputed() {
        let mut ledger = StreamLedger::new();
        for id in [1, 2] {
            ledger.push(TARGET, request(id), Owes::Ack);
            ledger.mark_written(TARGET, request(id), false);
        }
        // The first such rejection disputes the oldest waiting command; the
        // next is ambiguous between the two, and disputes the second too.
        assert!(ledger.dispute(TARGET, Answer::NamedRejection));
        assert!(ledger.dispute(TARGET, Answer::NamedRejection));
        assert!(ledger.queue(TARGET).iter().all(|entry| entry.disputed));
    }

    /// A completion learned later (`take_completion`) is the command's own,
    /// so it proves the command accepted: the dispute is decided, the
    /// disputed error goes to the STOP, and the STOP's entry leaves the
    /// ledger rather than stay as a debt disputing later requests.
    #[test]
    fn a_learned_completion_proves_its_dispute_accepted() {
        let mut ledger = StreamLedger::new();
        for (id, owes) in [(1, Owes::Completion), (2, Owes::Ack)] {
            ledger.push(TARGET, request(id), owes);
            ledger.mark_written(TARGET, request(id), false);
        }
        let answered = ledger.answer(
            TARGET,
            Answer::SocketlessError,
            Evidence::Error(0x41.into()),
        );
        assert!(matches!(answered.resolved, Resolved::Disputed(_)));
        assert!(ledger.awaits_dispute(TARGET, request(2)));
        let (paid, decided) = ledger.take_completion(TARGET).unwrap();
        assert_eq!(paid.request, request(1));
        assert_eq!(
            decided.retro.as_slice(),
            [Retro {
                request: request(2),
                evidence: Evidence::Error(0x41.into()),
            }]
        );
        assert!(!ledger.awaits_dispute(TARGET, request(2)));
        assert!(ledger.is_empty());
        ledger.check_disputes().unwrap();
    }

    #[test]
    fn accepting_a_dispute_pays_one_member_of_a_stop_debt_run() {
        for count in [2, 3, 17] {
            let kinds: Vec<_> = std::iter::once(Owes::Completion)
                .chain(std::iter::repeat_n(Owes::Ack, count))
                .collect();
            let mut ledger = owed(&kinds);
            let disputed = ledger.answer(
                TARGET,
                Answer::SocketlessError,
                Evidence::Error(0x41.into()),
            );
            assert!(matches!(disputed.resolved, Resolved::Disputed(_)));
            ledger.check_disputes().unwrap();
            ledger.answer(TARGET, Answer::Completion, Evidence::Other);
            assert_eq!(shape(&ledger), [(Owes::Ack, count as u64 - 1)]);
            assert!(ledger
                .queue(TARGET)
                .iter()
                .all(|entry| !entry.disputed && !entry.pinned));
            ledger.push(TARGET, request(99), Owes::Ack);
            ledger.mark_written(TARGET, request(99), false);
            for _ in 1..count {
                let paid = ledger.answer(TARGET, Answer::Ack, Evidence::Ack(None));
                assert!(matches!(paid.resolved, Resolved::Debt(_)));
            }
            let own = ledger.answer(TARGET, Answer::Ack, Evidence::Ack(None));
            assert!(matches!(own.resolved, Resolved::Live(entry) if entry.request == request(99)));
            ledger.check_disputes().unwrap();
        }
    }

    #[test]
    fn the_dispute_audit_detects_a_member_lost_without_a_frame() {
        let mut ledger = owed(&[Owes::Completion, Owes::Ack, Owes::Ack]);
        ledger.answer(
            TARGET,
            Answer::SocketlessError,
            Evidence::Error(0x41.into()),
        );
        ledger.check_disputes().unwrap();
        ledger
            .queue_mut(TARGET)
            .iter_mut()
            .find(|entry| entry.owes == Owes::Ack)
            .unwrap()
            .members -= 1;
        assert_eq!(
            ledger.check_disputes(),
            Err("a dispute run lost a member without an assigned frame")
        );
    }

    #[test]
    fn run_member_answers_preserve_the_unpaid_disputed_member() {
        for paid in 1..4 {
            let mut ledger = owed(&[
                Owes::Completion,
                Owes::Ack,
                Owes::Ack,
                Owes::Ack,
                Owes::Ack,
                Owes::Ack,
            ]);
            ledger.answer(
                TARGET,
                Answer::SocketlessError,
                Evidence::Error(0x41.into()),
            );
            ledger.push(TARGET, request(99), Owes::Ack);
            ledger.mark_written(TARGET, request(99), false);
            for _ in 0..paid {
                ledger.answer(TARGET, Answer::Ack, Evidence::Ack(None));
                ledger.check_disputes().unwrap();
            }
            ledger.answer(TARGET, Answer::Completion, Evidence::Other);
            assert_eq!(ledger.debts(TARGET), 4 - paid);
            for _ in paid..4 {
                assert!(matches!(
                    ledger
                        .answer(TARGET, Answer::Ack, Evidence::Ack(None))
                        .resolved,
                    Resolved::Debt(_)
                ));
            }
            assert!(
                matches!(ledger.answer(TARGET, Answer::Ack, Evidence::Ack(None)).resolved, Resolved::Live(entry) if entry.request == request(99))
            );
        }
    }

    /// Cancellations for different sockets never compete, so alternating
    /// ones still form one run per socket; competing kinds split runs.
    #[test]
    fn runs_form_only_among_competing_entries() {
        let (s1, s2) = (Owes::Cancel(ViscaSocket::S1), Owes::Cancel(ViscaSocket::S2));
        let ledger = owed(&[s1, s2, s1, s2, s1, Owes::Ack, s2, Owes::Ack]);
        assert_eq!(shape(&ledger), [(s1, 3), (s2, 3), (Owes::Ack, 2)]);

        let reply = Owes::Reply(PayloadShape::Unknown);
        let ledger = owed(&[Owes::Ack, Owes::Ack, reply, Owes::Ack, Owes::Completion]);
        assert_eq!(
            shape(&ledger),
            [
                (Owes::Ack, 2),
                (reply, 1),
                (Owes::Ack, 1),
                (Owes::Completion, 1)
            ]
        );
    }

    /// A first answer settles the debts written before it (an inquiry reply
    /// retires the ACK owed ahead of it), and the runs it separated merge and
    /// pay oldest first.
    #[test]
    fn a_first_answer_settles_older_debts_and_runs_pay_in_order() {
        let reply = Owes::Reply(PayloadShape::Unknown);
        let mut ledger = owed(&[Owes::Ack, reply, Owes::Ack, Owes::Ack]);
        assert_eq!(shape(&ledger), [(Owes::Ack, 1), (reply, 1), (Owes::Ack, 2)]);
        let answered = ledger.answer(TARGET, Answer::Reply(&[]), evidence(Answer::Reply(&[])));
        assert!(matches!(answered.resolved, Resolved::Debt(entry) if entry.owes == reply));
        assert_eq!(
            answered.replies_settled.iter().collect::<Vec<_>>(),
            vec![PayloadShape::Unknown]
        );
        assert_eq!(shape(&ledger), [(Owes::Ack, 2)]);
        for remaining in (0..2).rev() {
            let answered = ledger.answer(TARGET, Answer::Ack, evidence(Answer::Ack));
            assert!(matches!(answered.resolved, Resolved::Debt(_)));
            assert_eq!(ledger.debts(TARGET), remaining);
        }
        assert!(ledger.is_empty());
    }

    /// An error behind a `CompletionOnly` debt is disputed only when a
    /// different request could have sent it: within one run it pays a member
    /// either way. Across requests it binds to neither; the
    /// `CompletionOnly` entry owes only its completion and the other is
    /// disputed. Decisive evidence then assigns the error: the STOP's own
    /// ACK, when nothing else could have sent it, proves the command
    /// rejected; the command's completion proves it accepted.
    #[test]
    fn a_disputed_error_is_assigned_by_decisive_evidence() {
        let mut ledger = owed(&[Owes::Completion, Owes::Completion]);
        assert_eq!(shape(&ledger), [(Owes::Completion, 2)]);
        let answered = ledger.answer(
            TARGET,
            Answer::SocketlessError,
            Evidence::Error(0x02.into()),
        );
        assert!(matches!(answered.resolved, Resolved::Debt(_)));

        for (proof, retro_to) in [(Answer::Ack, 1), (Answer::Completion, 2)] {
            let mut ledger = owed(&[Owes::Completion, Owes::Ack]);
            let answered = ledger.answer(
                TARGET,
                Answer::SocketlessError,
                Evidence::Error(0x02.into()),
            );
            assert!(matches!(answered.resolved, Resolved::Disputed(_)));
            assert_eq!(
                shape(&ledger),
                [(Owes::AcceptedCompletion, 1), (Owes::Ack, 1)]
            );
            assert_eq!(ledger.lane_state(TARGET, Lane::Inquiry), LaneState::Clear);
            assert_eq!(ledger.lane_state(TARGET, Lane::Command), LaneState::Window);
            let answered = ledger.answer(TARGET, proof, evidence(proof));
            assert!(matches!(answered.resolved, Resolved::Debt(_)), "{proof:?}");
            assert_eq!(
                answered.retro.as_slice(),
                [Retro {
                    request: request(retro_to),
                    evidence: Evidence::Error(0x02.into()),
                }],
                "{proof:?}"
            );
            assert_eq!(answered.completion_only_retired, proof == Answer::Ack);
            assert!(ledger.is_empty(), "{proof:?}");
        }
    }

    /// A frame that pays the disputed STOP's debt while a later STOP could
    /// have sent it is handed off: the later STOP is disputed in turn, and
    /// the frame is assigned to it if the command proves accepted.
    #[test]
    fn a_dispute_is_handed_off_to_the_next_waiting_stop() {
        let mut ledger = owed(&[Owes::Completion, Owes::Ack]);
        ledger.answer(
            TARGET,
            Answer::SocketlessError,
            Evidence::Error(0x02.into()),
        );
        ledger.push(TARGET, request(9), Owes::Ack);
        ledger.mark_written(TARGET, request(9), false);
        // While a dispute is tracked nothing crosses, and no inquiry is
        // written.
        assert!(ledger.forbids_crossing(TARGET));
        let socket = Some(ViscaSocket::S2);
        assert!(matches!(
            ledger
                .answer(TARGET, Answer::Ack, Evidence::Ack(socket))
                .resolved,
            Resolved::Debt(_)
        ));
        assert!(ledger.forbids_crossing(TARGET));
        let answered = ledger.answer(TARGET, Answer::Completion, Evidence::Other);
        assert_eq!(
            answered
                .retro
                .iter()
                .map(|retro| (retro.request, retro.evidence))
                .collect::<Vec<_>>(),
            [
                (request(2), Evidence::Error(0x02.into())),
                (request(9), Evidence::Ack(socket)),
            ]
        );
        assert!(ledger.is_empty());
    }

    #[test]
    fn the_largest_dispute_decision_fits_its_exact_inline_bound() {
        let mut ledger = owed(&[Owes::Completion, Owes::Ack]);
        ledger.answer(
            TARGET,
            Answer::SocketlessError,
            Evidence::Error(0x41.into()),
        );
        for (id, socket) in [
            (3, ViscaSocket::S1),
            (4, ViscaSocket::S2),
            (5, ViscaSocket::S1),
        ] {
            ledger.push(TARGET, request(id), Owes::Ack);
            ledger.mark_written(TARGET, request(id), false);
            ledger.answer(TARGET, Answer::Ack, Evidence::Ack(Some(socket)));
            let candidate = ledger.follow_candidate(TARGET, socket).unwrap();
            ledger.record_follow(TARGET, candidate, NamedFrame::Completion(socket));
            ledger.retire(TARGET, request(id), |_| {
                (Instant::now(), Duration::from_secs(1))
            });
        }
        let answered = ledger.answer(TARGET, Answer::Completion, Evidence::Other);
        assert_eq!(answered.retro.len(), MAX_RETRO_FRAMES);
        assert_eq!(MAX_RETRO_FRAMES, 7);
        assert!(!answered.retro.spilled());
        assert_eq!(answered.retro[0].evidence, Evidence::Error(0x41.into()));
        assert_eq!(
            answered.retro.last().map(|retro| retro.evidence),
            Some(Evidence::Named(NamedFrame::Completion(ViscaSocket::S1)))
        );
    }

    #[test]
    fn disjoint_live_inquiries_keep_their_own_replies_in_either_order() {
        for reverse in [false, true] {
            let mut ledger = StreamLedger::new();
            for (id, shape) in [(1, PayloadShape::Bytes(1)), (2, PayloadShape::Nibbles(4))] {
                ledger.push(TARGET, request(id), Owes::Reply(shape));
                ledger.mark_written(TARGET, request(id), false);
            }
            let replies: [(u64, &[u8]); 2] = if reverse {
                [(2, &[0, 1, 2, 3]), (1, &[0x80])]
            } else {
                [(1, &[0x80]), (2, &[0, 1, 2, 3])]
            };
            for (id, payload) in replies {
                let answered = ledger.answer(TARGET, Answer::Reply(payload), Evidence::Other);
                assert!(
                    matches!(answered.resolved, Resolved::Live(entry) if entry.request == request(id))
                );
                assert!(answered.replies_settled.is_empty());
            }
            assert!(ledger.is_empty());
        }
    }

    #[test]
    fn disjoint_reply_retires_older_debt_but_malformed_nibbles_retire_nothing() {
        let mut ledger = owed(&[Owes::Reply(PayloadShape::Bytes(1))]);
        ledger.latch_due(Instant::now());
        assert_eq!(
            ledger.inquiry_state(TARGET, PayloadShape::Unknown),
            LaneState::Latched
        );
        assert_eq!(
            ledger.inquiry_state(TARGET, PayloadShape::Nibbles(4)),
            LaneState::Clear
        );
        ledger.push(TARGET, request(2), Owes::Reply(PayloadShape::Nibbles(4)));
        ledger.mark_written(TARGET, request(2), false);
        let invalid = ledger.answer(TARGET, Answer::Reply(&[0, 0, 0, 0x10]), Evidence::Other);
        assert_eq!(invalid.resolved, Resolved::Nothing);
        assert!(invalid.replies_settled.is_empty());
        assert_eq!(ledger.len(TARGET), 2);
        let valid = ledger.answer(TARGET, Answer::Reply(&[0, 1, 2, 3]), Evidence::Other);
        assert!(matches!(valid.resolved, Resolved::Live(entry) if entry.request == request(2)));
        assert_eq!(
            valid.replies_settled.iter().collect::<Vec<_>>(),
            [PayloadShape::Bytes(1)]
        );
        assert!(ledger.is_empty());
    }

    #[test]
    fn shape_legality_never_weakens_socketless_error_order_or_run_separators() {
        let byte = Owes::Reply(PayloadShape::Bytes(4));
        let nibble = Owes::Reply(PayloadShape::Nibbles(4));
        let mut ledger = owed(&[Owes::Ack, byte, Owes::Ack, nibble, Owes::Ack]);
        // Even disjoint data classes compete for an unkeyed error, so an
        // inquiry is always a real separator between command debt runs.
        assert_eq!(ledger.len(TARGET), 5);
        assert!(
            matches!(ledger.answer(TARGET, Answer::SocketlessError, Evidence::Error(2.into())).resolved,
            Resolved::Debt(entry) if entry.request == request(1))
        );
        // Bytes(4) admits this; Nibbles(4) does not. Equality/merging keeps
        // the actual declarations instead of their overlapping union.
        let answered = ledger.answer(TARGET, Answer::Reply(&[0x80, 1, 2, 3]), Evidence::Other);
        assert!(matches!(answered.resolved, Resolved::Debt(entry) if entry.owes == byte));
        assert_eq!(
            answered.replies_settled.iter().collect::<Vec<_>>(),
            [PayloadShape::Bytes(4)]
        );
        assert!(ledger.owes_reply(TARGET, PayloadShape::Nibbles(4)));
        assert!(!ledger.owes_reply(TARGET, PayloadShape::Bytes(4)));
    }
}
