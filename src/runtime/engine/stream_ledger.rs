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
//! that outlives its request's ambiguity window *latches* its lane (inquiries,
//! or ordinary commands) until the answer arrives; it never blocks a STOP and
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
//! The model assumes four camera facts (`docs/architecture_2_0.md`, "The raw
//! byte-stream correlation ledger"):
//!
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

use crate::{CameraId, ViscaSocket};

use super::{GenerationTicket, InquiryRoute, Lane, RequestId};

/// Debts at which a camera's correlation is lost.
///
/// One stall leaves only a few debts: ordinary commands wait while one is
/// owed (so at most one or two command debts), inquiries are single-flight
/// (one reply debt), and each halt adds at most three STOPs. Thirty-two is
/// therefore far beyond any stall a camera answers in the end; reaching it
/// means the camera is not answering at all. No debt is ever dropped — that
/// would let its late answer reach a later request — so at the cap every
/// lane of the camera latches until debts are paid, and its `NoReply`
/// commands (whose possible rejections are owed too) are refused. Only urgent
/// ACK-bearing commands (STOPs), which are never refused, and cancellations
/// add debts beyond it; runs absorb them ([`max_entries_per_target`]).
pub(crate) const MAX_DEBTS_PER_TARGET: usize = 32;

/// The most entries (runs and live requests) one camera's queue can hold,
/// given `admissible`: the requests it can have admitted at once (the
/// session's ordinary capacity plus its control reserve).
///
/// Each admitted request has at most two live entries (its own answer and its
/// cancellation's). Every non-cancellation entry other than an urgent ACK is
/// written below the cap, when its debt runs number fewer than
/// [`MAX_DEBTS_PER_TARGET`]; after the last such write, the urgent ACKs behind
/// it form one run split only by live entries, and each request that was live
/// at that write splits at most one more. That bounds the non-cancellation
/// entries by `MAX_DEBTS_PER_TARGET + 3 * admissible`. Cancellations for one
/// socket are all alike, so their runs are split only by live ones: at most
/// `2 * live + 1` per socket, `2 * admissible + 2` over both. A tracked
/// dispute pins at most `MAX_DISPUTE_HANDOFFS + 2` entries, each of which can
/// split one more run. The bound is independent of how many STOPs are
/// written; the engine checks it as an invariant.
pub(crate) const fn max_entries_per_target(admissible: usize) -> usize {
    MAX_DEBTS_PER_TARGET + 5 * admissible + 2 + 2 * (MAX_DISPUTE_HANDOFFS + 2)
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
    Reply(InquiryRoute),
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
    pub(crate) generation: GenerationTicket,
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
pub(crate) enum Answer {
    /// `z0 4y FF`.
    Ack,
    /// Inquiry data, with the route its content selects (if any).
    Reply(Option<InquiryRoute>),
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

/// The oldest-first answer queue of every raw stream target.
#[derive(Debug)]
pub(crate) struct StreamLedger {
    targets: [Queue; 9],
    /// Per target, the open dispute, if any.
    disputes: [Option<Dispute>; 9],
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
    Error(u8),
    /// A completion or error naming a socket, which follows an ACK.
    Named(NamedFrame),
    Other,
}

/// A completion or error naming a socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NamedFrame {
    Completion(ViscaSocket),
    Error(ViscaSocket, u8),
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
    pub(crate) generation: GenerationTicket,
    pub(crate) evidence: Evidence,
}

/// A request a dispute may have answered, with the frame that answered it
/// in that explanation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Step {
    order: u64,
    request: RequestId,
    generation: GenerationTicket,
    evidence: Evidence,
    /// A completion or error that named the socket this step's ACK named,
    /// before the dispute was proven: the request's own if the command was
    /// accepted.
    follow: Option<NamedFrame>,
}

/// An open dispute: a socketless error that the `CompletionOnly` entry or the
/// STOP behind it sent. Either the error was the command's rejection
/// (*rejected*), or the command was accepted and the error, and every frame
/// handed off since, answered the requests in `steps` (*accepted*).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Dispute {
    completion_only: Step,
    /// Whether the first side is a `NoReply` command, which owes nothing
    /// more once disputed (its entry is gone) and never completes.
    rejection_only: bool,
    /// The disputed error, and each frame handed off since, with the request
    /// it answered if the command was accepted. The last is the request whose
    /// own answer would prove the command rejected.
    steps: SmallVec<[Step; MAX_DISPUTE_HANDOFFS + 1]>,
    /// Whether a second dispute touched its requests: the frames since may
    /// belong to either explanation, so no STOP's answer can prove the
    /// command rejected any more, and neither can write order; only the
    /// command's completion still proves it accepted.
    accept_only: bool,
    /// Whether every request it names has ended and its window with it: it
    /// no longer holds anything back, its camera's lanes latch (the
    /// documented endgame), and only a late answer can still settle it.
    dormant: bool,
}

impl Dispute {
    /// Whether the entry written at `order` is one this dispute names: its
    /// `CompletionOnly` (or `NoReply`) command, or one of its steps.
    fn names(&self, order: u64) -> bool {
        self.completion_only.order == order || self.steps.iter().any(|step| step.order == order)
    }
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
    /// Whether an owed inquiry reply was paid or retired.
    pub(crate) reply_settled: bool,
    /// Frames a proven dispute assigns to their requests, in arrival order.
    pub(crate) retro: SmallVec<[Retro; MAX_DISPUTE_HANDOFFS + 1]>,
    /// Whether a proven dispute retired a `CompletionOnly` entry as rejected.
    pub(crate) completion_only_retired: bool,
}

impl StreamLedger {
    pub(crate) fn new() -> Self {
        Self {
            targets: std::array::from_fn(|_| VecDeque::new()),
            disputes: std::array::from_fn(|_| None),
            next_order: 0,
            #[cfg(test)]
            disputes_seen: 0,
        }
    }

    fn queue(&self, target: CameraId) -> &Queue {
        &self.targets[usize::from(target.id())]
    }

    fn queue_mut(&mut self, target: CameraId) -> &mut Queue {
        &mut self.targets[usize::from(target.id())]
    }

    /// Appends a request whose write is about to start.
    pub(crate) fn push(
        &mut self,
        target: CameraId,
        request: RequestId,
        generation: GenerationTicket,
        owes: Owes,
    ) -> u64 {
        let order = self.next_order;
        self.next_order = self.next_order.wrapping_add(1);
        self.queue_mut(target).push_back(Outstanding {
            order,
            newest: order,
            request,
            generation,
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
    pub(crate) fn mark_written(&mut self, target: CameraId, request: RequestId, cancel: bool) {
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
        target: CameraId,
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
        target: CameraId,
        request: RequestId,
        mut window: impl FnMut(Owes) -> (Instant, Duration),
    ) -> Answered {
        let index = usize::from(target.id());
        let queue = &mut self.targets[index];
        let unwritten = |entry: &Outstanding| {
            entry.request == request && entry.standing == Standing::Live && !entry.written
        };
        let mut answered = Answered::default();
        let decided = self.disputes[index].as_ref().and_then(|dispute| {
            queue
                .iter()
                .find(|entry| unwritten(entry) && dispute.names(entry.order))
                .map(|entry| entry.order == dispute.completion_only.order)
        });
        if let Some(command_unwritten) = decided {
            if let Some(dispute) = self.disputes[index].take() {
                if command_unwritten {
                    // The command sent nothing, so the disputed error was
                    // the STOP's, and each frame handed off since its step's.
                    accept(queue, &dispute, &mut answered);
                } else if !dispute.accept_only {
                    // A step sent nothing, so it never sent the frame that
                    // the command's acceptance would make its own: the
                    // command was rejected.
                    reject(queue, &dispute, &mut answered);
                }
                // A demoted dispute's steps were inferred through entries
                // that were already disputed, not proven, so an unwritten one
                // proves nothing: it is untracked instead.
            }
            pin(queue, None);
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
    pub(crate) fn answer(
        &mut self,
        target: CameraId,
        answer: Answer,
        evidence: Evidence,
    ) -> Answered {
        let index = usize::from(target.id());
        let queue = &mut self.targets[index];
        let mut answered = Answered {
            resolved: Resolved::Nothing,
            reply_settled: false,
            retro: SmallVec::new(),
            completion_only_retired: false,
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
                answered.reply_settled = apply_in(queue, entry);
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
                if let Some(dispute) = self.disputes[index].take() {
                    let candidate = dispute.steps.last().map(|step| step.order);
                    if entry.order == dispute.completion_only.order {
                        accept(queue, &dispute, &mut answered);
                    } else if dispute.accept_only {
                        // Its candidate answered: whose frame that was, no
                        // explanation tells any more; stop tracking.
                        if Some(entry.order) != candidate {
                            self.disputes[index] = Some(dispute);
                        }
                    } else if Some(entry.order) == candidate && alone {
                        reject(queue, &dispute, &mut answered);
                    } else if Some(entry.order) == candidate && entry.members > 1 {
                        // A member of the candidate run paid: its members are
                        // alike, so the run stays the candidate.
                        self.disputes[index] = Some(dispute);
                    } else if Some(entry.order) == candidate {
                        match handed_to {
                            Some(step) if dispute.steps.len() <= MAX_DISPUTE_HANDOFFS => {
                                let mut dispute = dispute;
                                dispute.steps.push(step);
                                // A live request is named again.
                                dispute.dormant = false;
                                self.disputes[index] = Some(dispute);
                            }
                            // Untracked past the cap, or with no candidate.
                            _ => {}
                        }
                    } else if candidate
                        .is_some_and(|order| !queue.iter().any(|entry| entry.order == order))
                    {
                        // Write order retired the candidate: its answer was
                        // the disputed frame.
                        accept(queue, &dispute, &mut answered);
                    } else {
                        self.disputes[index] = Some(dispute);
                    }
                }
                answered.resolved = if entry.is_debt() {
                    Resolved::Debt(entry)
                } else {
                    Resolved::Live(entry)
                };
            }
            Resolution::Ambiguous { first, second } => {
                let queue = &mut self.targets[index];
                let candidates = [first, second];
                let first = first.order;
                let opened = self.disputes[index].is_none();
                // A second dispute while one is tracked: only the command's
                // completion can still prove it (accepted).
                demote(&mut self.disputes[index]);
                let step = |entry: &Outstanding, evidence| Step {
                    order: entry.order,
                    request: entry.request,
                    generation: entry.generation,
                    evidence,
                    follow: None,
                };
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
                    .map(|entry| step(entry, Evidence::Error(0)));
                // Each entry behind the first that the frame could be the own
                // answer of is disputed, through the first that certainly
                // still owes it (a `NoReply` command's, which may answer
                // nothing, passes it on): the dispute's STOP.
                let stop = dispute_behind(queue, first, answer, evidence);
                if let (true, Some(completion_only), Some(stop), Evidence::Error(code)) =
                    (opened, completion_only, stop, evidence)
                {
                    self.disputes[index] = Some(Dispute {
                        completion_only: Step {
                            evidence: Evidence::Error(code),
                            ..completion_only
                        },
                        rejection_only: head_owes == Some(Owes::Rejection),
                        steps: std::iter::once(stop).collect(),
                        accept_only: false,
                        dormant: false,
                    });
                }
                let tracked = self.disputes[index].is_some();
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
        pin(&mut self.targets[index], self.disputes[index].as_ref());
        answered
    }

    /// Settles what was written before `order` (`settle_in`): a completion
    /// or execution error of the request written there arrived, which
    /// follows that request's first answer and so every earlier one. Write
    /// order retiring a tracked dispute's candidate proves it accepted, as
    /// for a first answer.
    pub(crate) fn settle_before(&mut self, target: CameraId, order: u64) -> Answered {
        let index = usize::from(target.id());
        let queue = &mut self.targets[index];
        let mut answered = Answered {
            resolved: Resolved::Nothing,
            reply_settled: settle_in(queue, order),
            retro: SmallVec::new(),
            completion_only_retired: false,
        };
        if let Some(dispute) = self.disputes[index].take() {
            let candidate = dispute.steps.last().map(|step| step.order);
            let retired =
                candidate.is_some_and(|order| !queue.iter().any(|entry| entry.order == order));
            if retired && !dispute.accept_only {
                accept(queue, &dispute, &mut answered);
            } else {
                self.disputes[index] = Some(dispute);
            }
        }
        pin(&mut self.targets[index], self.disputes[index].as_ref());
        answered
    }

    /// Keeps a completion or error naming `frame`'s socket that a tracked
    /// dispute's handed-off ACK named, and returns whether it did: if the
    /// command is proven accepted, that ACK and this frame were the same
    /// request's; if rejected, the ACK was the late answer of the command
    /// owed there, and this frame is that command's. Either way the command
    /// running there has ended, so its socket is free.
    pub(crate) fn follow(&mut self, target: CameraId, frame: NamedFrame) -> bool {
        let Some(dispute) = self.disputes[usize::from(target.id())].as_mut() else {
            return false;
        };
        let Some(step) = dispute.steps.iter_mut().find(|step| {
            step.follow.is_none() && step.evidence == Evidence::Ack(Some(frame.socket()))
        }) else {
            return false;
        };
        step.follow = Some(frame);
        true
    }

    /// Whether a tracked dispute may already hold `request`'s answer: the
    /// disputed frame, or one handed off since, may have been its own.
    pub(crate) fn awaits_dispute(&self, target: CameraId, request: RequestId) -> bool {
        self.disputes[usize::from(target.id())]
            .as_ref()
            .is_some_and(|dispute| dispute.steps.iter().any(|step| step.request == request))
    }

    /// Checks that every tracked dispute names a `CompletionOnly` entry still
    /// owed and follows no more than [`MAX_DISPUTE_HANDOFFS`] hand-offs.
    pub(crate) fn check_disputes(&self) -> Result<(), &'static str> {
        for (queue, dispute) in self.targets.iter().zip(&self.disputes) {
            let Some(dispute) = dispute else {
                continue;
            };
            if !dispute.rejection_only
                && !queue.iter().any(|entry| {
                    entry.order == dispute.completion_only.order
                        && entry.owes == Owes::AcceptedCompletion
                })
            {
                return Err("a tracked dispute names no owed CompletionOnly entry");
            }
            if dispute.steps.is_empty() || dispute.steps.len() > MAX_DISPUTE_HANDOFFS + 1 {
                return Err("a tracked dispute exceeds its hand-offs");
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
    pub(crate) fn take_completion(&mut self, target: CameraId) -> Option<(Outstanding, Answered)> {
        let index = usize::from(target.id());
        let queue = &mut self.targets[index];
        let entry = queue
            .iter()
            .find(|entry| matches!(entry.owes, Owes::Completion | Owes::AcceptedCompletion))
            .copied()?;
        let mut answered = Answered::default();
        if let Some(dispute) =
            self.disputes[index].take_if(|dispute| dispute.completion_only.order == entry.order)
        {
            accept(queue, &dispute, &mut answered);
            pin(queue, None);
        }
        take_in(queue, entry.order);
        Some((entry, answered))
    }

    /// Marks the entry `answer` would resolve as disputed, without paying it,
    /// and returns whether there was one: a frame that may have been its
    /// answer was taken for another request's (a rejection naming a socket
    /// that an ended or executing command may still hold, which could be
    /// that command's execution error or this one's rejection).
    pub(crate) fn dispute(&mut self, target: CameraId, answer: Answer) -> bool {
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
        let index = usize::from(target.id());
        if self.disputes[index]
            .as_ref()
            .is_some_and(|dispute| dispute.names(entry.order))
        {
            demote(&mut self.disputes[index]);
            pin(&mut self.targets[index], self.disputes[index].as_ref());
        }
        true
    }

    /// Whether nothing may be written to cross another unacknowledged
    /// command on `target`, and no inquiry may be written there: a disputed
    /// entry is live (its own answer must stay decisive), a dispute is
    /// tracked (no second dispute may involve its requests), or a STOP
    /// already waits behind a `CompletionOnly` entry that may still be
    /// rejected (so that any dispute it causes involves one STOP).
    pub(crate) fn forbids_crossing(&self, target: CameraId) -> bool {
        let queue = self.queue(target);
        let disputed_live = queue.iter().any(|entry| entry.disputed && !entry.is_debt());
        let unproven = queue
            .iter()
            .any(|entry| matches!(entry.owes, Owes::Completion | Owes::Rejection));
        let waiting_ack = queue
            .iter()
            .any(|entry| entry.owes == Owes::Ack && !entry.is_debt());
        let tracked = self.disputes[usize::from(target.id())]
            .as_ref()
            .is_some_and(|dispute| !dispute.dormant);
        disputed_live || tracked || (unproven && waiting_ack)
    }

    /// Whether a `CompletionOnly` command's answer is outstanding on
    /// `target`.
    /// Whether a `NoReply` command's possible rejection is still owed on
    /// `target` past its window: `CompletionOnly` commands there fail
    /// rather than wait.
    pub(crate) fn rejection_latched(&self, target: CameraId) -> bool {
        self.queue(target)
            .iter()
            .any(|entry| entry.owes == Owes::Rejection && entry.latched())
    }

    /// Whether a `NoReply` command's possible rejection is owed on
    /// `target`: until a later first answer settles it, a `CompletionOnly`
    /// command's socketless rejection could not be told from it.
    pub(crate) fn owes_rejection(&self, target: CameraId) -> bool {
        self.queue(target)
            .iter()
            .any(|entry| entry.owes == Owes::Rejection)
    }

    pub(crate) fn owes_completion(&self, target: CameraId) -> bool {
        self.queue(target)
            .iter()
            .any(|entry| matches!(entry.owes, Owes::Completion | Owes::AcceptedCompletion))
    }

    /// The answers `target` owes for requests that already ended.
    pub(crate) fn debts(&self, target: CameraId) -> u64 {
        self.queue(target)
            .iter()
            .filter(|entry| entry.is_debt())
            .map(|entry| entry.members)
            .sum()
    }

    /// The entries (runs and live requests) `target`'s queue holds.
    pub(crate) fn len(&self, target: CameraId) -> usize {
        self.queue(target).len()
    }

    pub(crate) fn owes_reply(&self, target: CameraId) -> bool {
        self.queue(target)
            .iter()
            .any(|entry| entry.is_debt() && matches!(entry.owes, Owes::Reply(_)))
    }

    /// Whether `target` owes [`MAX_DEBTS_PER_TARGET`] debts: every lane
    /// latches, and nothing but STOPs and cancellations is written, until
    /// debts are paid.
    fn capped(&self, target: CameraId) -> bool {
        self.debts(target) >= MAX_DEBTS_PER_TARGET as u64
    }

    /// Whether `lane` on `target` owes an answer, and whether it has latched.
    /// At [`MAX_DEBTS_PER_TARGET`] every lane latches until debts are paid.
    pub(crate) fn lane_state(&self, target: CameraId, lane: Lane) -> LaneState {
        if self.capped(target) {
            return LaneState::Latched;
        }
        // A dormant dispute's `CompletionOnly` command, never proven
        // accepted, holds both lanes (its command lane through its debt).
        if lane == Lane::Inquiry
            && self.disputes[usize::from(target.id())]
                .as_ref()
                .is_some_and(|dispute| dispute.dormant && !dispute.rejection_only)
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
    /// camera's lanes latch (the inquiry lane too, while a `CompletionOnly`
    /// command is not proven accepted, like any whose completion never
    /// came), so work they refuse fails at once rather than waiting — while
    /// a late answer can still settle it and reopen them.
    pub(crate) fn latch_due(&mut self, now: Instant) {
        for (queue, dispute) in self.targets.iter().zip(self.disputes.iter_mut()) {
            let ended = dispute.as_ref().is_some_and(|dispute| {
                queue
                    .iter()
                    .filter(|entry| dispute.names(entry.order))
                    .all(|entry| {
                        matches!(entry.standing, Standing::Owed { window_end, .. } if window_end <= now)
                    })
            });
            if let (true, Some(dispute)) = (ended, dispute.as_mut()) {
                dispute.dormant = true;
            }
        }
        for entry in self.targets.iter_mut().flatten() {
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
            .flatten()
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

    pub(crate) fn clear(&mut self) {
        for queue in &mut self.targets {
            queue.clear();
        }
        self.disputes = std::array::from_fn(|_| None);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.targets.iter().all(VecDeque::is_empty)
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = (CameraId, &Outstanding)> {
        self.targets.iter().enumerate().flat_map(|(index, queue)| {
            let target = u8::try_from(index)
                .ok()
                .and_then(|id| CameraId::new(id).ok());
            queue
                .iter()
                .filter_map(move |entry| target.map(|target| (target, entry)))
        })
    }
}

/// Whether a debt owing `owes` latches when its window ends: one that holds
/// a lane, or a `NoReply` command's possible rejection (which holds back
/// only `CompletionOnly` commands).
const fn latches(owes: Owes) -> bool {
    owes.lane().is_some() || matches!(owes, Owes::Rejection)
}

/// A second dispute touched a tracked one's requests: it is kept, but only
/// the command's completion can prove it any more. A `NoReply` command never
/// completes, so its dispute is dropped.
fn demote(dispute: &mut Option<Dispute>) {
    match dispute {
        Some(tracked) if !tracked.rejection_only => tracked.accept_only = true,
        _ => *dispute = None,
    }
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
    answer: Answer,
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
                generation: entry.generation,
                evidence,
                follow: None,
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
        queue.retain(|entry| entry.order != step.order);
        for evidence in std::iter::once(step.evidence).chain(step.follow.map(Evidence::Named)) {
            answered.retro.push(Retro {
                request: step.request,
                generation: step.generation,
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
    if dispute.rejection_only {
        return;
    }
    take_in(queue, dispute.completion_only.order);
    answered.retro.push(Retro {
        request: dispute.completion_only.request,
        generation: dispute.completion_only.generation,
        evidence: dispute.completion_only.evidence,
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
fn resolve_in(queue: &Queue, answer: Answer) -> Resolution {
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
fn legal(answer: Answer, owes: Owes) -> bool {
    match (answer, owes) {
        (Answer::Ack, Owes::Ack) => true,
        (Answer::Reply(route), Owes::Reply(owed)) => {
            route.is_none_or(|route| route == owed || route == InquiryRoute::UNKNOWN)
                || owed == InquiryRoute::UNKNOWN
        }
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
/// and so every earlier one), then takes it. Returns whether an owed inquiry
/// reply was paid or retired.
fn apply_in(queue: &mut Queue, entry: Outstanding) -> bool {
    // Settle first: taking the entry may merge the runs it separated.
    let retired_reply = settle_in(queue, entry.order);
    take_in(queue, entry.order);
    let paid_reply = entry.is_debt() && matches!(entry.owes, Owes::Reply(_));
    paid_reply || retired_reply
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
/// whether an owed inquiry reply was retired. A run that also has members
/// written after `order` (one that merged past it) is kept whole: keeping a
/// debt only discards a future answer, never misattributes one.
fn settle_in(queue: &mut Queue, order: u64) -> bool {
    let mut reply = false;
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
            Owes::Reply(_) => {
                reply = true;
                false
            }
            Owes::Ack | Owes::Rejection | Owes::Cancel(_) => false,
        }
    });
    coalesce(queue);
    reply
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

    const TARGET: CameraId = CameraId::CAMERA_1;

    fn request(id: u64) -> RequestId {
        RequestId::from_nonzero(NonZeroU64::new(id).unwrap())
    }

    /// Writes one entry per kind in order, then ends every request unanswered.
    fn owed(kinds: &[Owes]) -> StreamLedger {
        let mut ledger = StreamLedger::new();
        let now = Instant::now();
        for (index, owes) in (1..).zip(kinds) {
            let cancel = matches!(owes, Owes::Cancel(_));
            ledger.push(TARGET, request(index), GenerationTicket(1), *owes);
            ledger.mark_written(TARGET, request(index), cancel);
        }
        for index in (1..).take(kinds.len()) {
            ledger.retire(TARGET, request(index), |_| (now, Duration::ZERO));
        }
        ledger
    }

    fn evidence(answer: Answer) -> Evidence {
        match answer {
            Answer::Ack => Evidence::Ack(None),
            Answer::SocketlessError | Answer::NamedRejection => Evidence::Error(0x41),
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
        let reply = Owes::Reply(InquiryRoute::UNKNOWN);
        let mut ledger = owed(&[Owes::Rejection, Owes::Ack, reply]);
        assert_eq!(ledger.debts(TARGET), 3);
        let last = ledger
            .queue(TARGET)
            .back()
            .map(|entry| entry.order)
            .unwrap();
        let answered = ledger.settle_before(TARGET, last);
        assert!(answered.retro.is_empty() && !answered.reply_settled);
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
            ledger.push(TARGET, request(id), GenerationTicket(1), Owes::Ack);
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
            ledger.push(TARGET, request(id), GenerationTicket(1), owes);
            ledger.mark_written(TARGET, request(id), false);
        }
        let answered = ledger.answer(TARGET, Answer::SocketlessError, Evidence::Error(0x41));
        assert!(matches!(answered.resolved, Resolved::Disputed(_)));
        assert!(ledger.awaits_dispute(TARGET, request(2)));
        let (paid, decided) = ledger.take_completion(TARGET).unwrap();
        assert_eq!(paid.request, request(1));
        assert_eq!(
            decided.retro.as_slice(),
            [Retro {
                request: request(2),
                generation: GenerationTicket(1),
                evidence: Evidence::Error(0x41),
            }]
        );
        assert!(!ledger.awaits_dispute(TARGET, request(2)));
        assert!(ledger.is_empty());
        ledger.check_disputes().unwrap();
    }

    /// Cancellations for different sockets never compete, so alternating
    /// ones still form one run per socket; competing kinds split runs.
    #[test]
    fn runs_form_only_among_competing_entries() {
        let (s1, s2) = (Owes::Cancel(ViscaSocket::S1), Owes::Cancel(ViscaSocket::S2));
        let ledger = owed(&[s1, s2, s1, s2, s1, Owes::Ack, s2, Owes::Ack]);
        assert_eq!(shape(&ledger), [(s1, 3), (s2, 3), (Owes::Ack, 2)]);

        let reply = Owes::Reply(InquiryRoute::UNKNOWN);
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
        let reply = Owes::Reply(InquiryRoute::UNKNOWN);
        let mut ledger = owed(&[Owes::Ack, reply, Owes::Ack, Owes::Ack]);
        assert_eq!(shape(&ledger), [(Owes::Ack, 1), (reply, 1), (Owes::Ack, 2)]);
        let answered = ledger.answer(TARGET, Answer::Reply(None), evidence(Answer::Reply(None)));
        assert!(matches!(answered.resolved, Resolved::Debt(entry) if entry.owes == reply));
        assert!(answered.reply_settled);
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
        let answered = ledger.answer(TARGET, Answer::SocketlessError, Evidence::Error(0x02));
        assert!(matches!(answered.resolved, Resolved::Debt(_)));

        for (proof, retro_to) in [(Answer::Ack, 1), (Answer::Completion, 2)] {
            let mut ledger = owed(&[Owes::Completion, Owes::Ack]);
            let answered = ledger.answer(TARGET, Answer::SocketlessError, Evidence::Error(0x02));
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
                    generation: GenerationTicket(1),
                    evidence: Evidence::Error(0x02),
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
        ledger.answer(TARGET, Answer::SocketlessError, Evidence::Error(0x02));
        ledger.push(TARGET, request(9), GenerationTicket(1), Owes::Ack);
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
                (request(2), Evidence::Error(0x02)),
                (request(9), Evidence::Ack(socket)),
            ]
        );
        assert!(ledger.is_empty());
    }
}
