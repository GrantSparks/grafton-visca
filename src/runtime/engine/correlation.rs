//! The correlation regime selected once when a session is constructed.
use std::{collections::BTreeMap, sync::LazyLock, time::Instant};

use super::{
    holds::{self, HoldTable, Scope, StreamScope},
    CompletionSockets, EnvelopeKind, PendingSocketlessCompletions, RawHold, RawHoldKey,
    RawHoldScope, RawReleaseGate, RequestId, StreamLedger, TransportKind,
};
use crate::{
    target::{PerTarget, TargetSlot},
    ViscaSocket,
};

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct CompletionLearning {
    /// Largest dispatched or camera-proven physical socket capacity. Retuning
    /// cannot erase the possible execution of an older owed ACK.
    pub(super) socket_capacity_seen: Option<crate::CommandSocketCount>,
    pub(super) terminals: TerminalDebt,
    pub(super) completion_sockets: CompletionSockets,
    pub(super) pending_socketless_completions: Option<PendingSocketlessCompletions>,
    /// Owners a completion dropped as ambiguous could have finished. Exact
    /// evidence can end or replace those owners before the next socketless ACK.
    pub(super) dropped_completion_owners: [Option<RequestId>; 2],
}

/// Whether a received terminal belongs to a known family.
#[derive(Clone, Copy)]
pub(super) enum TerminalFamily {
    Socketed,
    CompletionOnly,
    Either,
}

/// Bounds the number of camera terminals that remain owed, independently of
/// whether the ledger can assign any frame to a particular request.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct TerminalDebt {
    lower: u64,
    upper: TerminalUpper,
    /// Keep a fresh zero proof until the last live socket owner ends.
    pub(super) proved_idle: bool,
    /// The same upper bound under the hypothesis that the tracked socketless
    /// error rejected its CompletionOnly candidate. Advance both bounds so
    /// that a later verdict does not subtract from a count already clipped.
    rejected_completion: Option<TerminalUpper>,
    /// Per socket, the sole possible rejection candidate for an ambiguous
    /// named error. An ACK exhausts these proofs before it may reuse a socket.
    named_errors: [Option<u64>; 2],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TerminalUpper {
    all: u64,
    socketed: u8,
    completion_only: u64,
}

impl TerminalUpper {
    fn normalize(&mut self) {
        self.all = self
            .all
            .min(u64::from(self.socketed).saturating_add(self.completion_only));
        self.socketed = self.socketed.min(u8::try_from(self.all).unwrap_or(u8::MAX));
        self.completion_only = self.completion_only.min(self.all);
    }

    fn permits(self, family: TerminalFamily) -> bool {
        match family {
            TerminalFamily::Socketed => self.socketed != 0,
            TerminalFamily::CompletionOnly => self.completion_only != 0,
            TerminalFamily::Either => self.all != 0,
        }
    }

    fn remove(&mut self, family: TerminalFamily) {
        if !self.permits(family) {
            return;
        }
        self.all -= 1;
        match family {
            TerminalFamily::Socketed => self.socketed -= 1,
            TerminalFamily::CompletionOnly => self.completion_only -= 1,
            TerminalFamily::Either => {}
        }
        self.normalize();
    }
}

impl TerminalDebt {
    fn update_upper(&mut self, change: impl Fn(&mut TerminalUpper)) {
        let before = self.upper.all;
        change(&mut self.upper);
        if let Some(assumed_rejection) = self.rejected_completion.as_mut() {
            change(assumed_rejection);
        }
        self.record_bound_change(before);
    }

    fn record_bound_change(&mut self, before: u64) {
        self.lower = self.lower.min(self.upper.all);
        self.proved_idle = self.upper.all == 0 && (self.proved_idle || before != 0);
    }

    pub(super) fn observe_ack(&mut self, capacity: crate::CommandSocketCount) {
        self.named_errors = [None; 2];
        self.lower = self.lower.saturating_add(1);
        self.update_upper(|upper| {
            upper.all = upper.all.saturating_add(1);
            upper.socketed = (upper.socketed + 1).min(capacity.value());
            upper.normalize();
        });
    }

    pub(super) fn write_completion_only(&mut self) {
        self.lower = self.lower.saturating_add(1);
        self.update_upper(|upper| {
            upper.all = upper.all.saturating_add(1);
            upper.completion_only = upper.completion_only.saturating_add(1);
        });
    }

    pub(super) fn terminal(&mut self, family: TerminalFamily) {
        if self.upper.permits(family) {
            self.lower = self.lower.saturating_sub(1);
        }
        self.update_upper(|upper| upper.remove(family));
    }

    pub(super) fn disputed_socketless_error(&mut self, new_dispute: Option<bool>) {
        self.lower = self.lower.saturating_sub(1);
        if let Some(completion_written) = new_dispute {
            self.rejected_completion = completion_written.then(|| {
                let mut rejected = self.upper;
                rejected.remove(TerminalFamily::CompletionOnly);
                rejected
            });
        }
    }

    pub(super) fn settle_completion_rejection(&mut self) {
        if let Some(rejected) = self.rejected_completion.take() {
            let before = self.upper.all;
            self.upper = rejected;
            self.record_bound_change(before);
        }
    }

    pub(super) fn disputed_named_error(&mut self, socket: ViscaSocket, waiting: Option<u64>) {
        self.lower = self.lower.saturating_sub(1);
        self.named_errors[socket.as_index()] = waiting;
    }

    pub(super) fn awaits_named_proof(&self) -> bool {
        self.named_errors.iter().any(Option::is_some)
    }

    pub(super) fn prove_first_answer(&mut self, order: u64) -> bool {
        let mut proved = 0;
        for waiting in &mut self.named_errors {
            if *waiting == Some(order) {
                *waiting = None;
                proved += 1;
            }
        }
        self.update_upper(|upper| {
            for _ in 0..proved {
                upper.remove(TerminalFamily::Socketed);
            }
        });
        proved != 0
    }
}

#[derive(Debug)]
pub(super) struct StreamCorrelation {
    pub(super) holds: HoldTable<StreamScope>,
    pub(super) ledger: StreamLedger,
    pub(super) learning: PerTarget<CompletionLearning>,
    pub(super) release_gate: Option<RawReleaseGate>,
}

#[derive(Debug)]
pub(super) enum CorrelationMode {
    Sequenced,
    RawDatagram { holds: HoldTable<RawHoldScope> },
    RawStream(Box<StreamCorrelation>),
}

impl CorrelationMode {
    pub(super) fn new(envelope: EnvelopeKind, transport: TransportKind) -> Self {
        match (envelope, transport) {
            (EnvelopeKind::Sony, _) => Self::Sequenced,
            (EnvelopeKind::Raw, TransportKind::Datagram) => Self::RawDatagram {
                holds: HoldTable::new(),
            },
            (EnvelopeKind::Raw, TransportKind::Stream) => {
                Self::RawStream(Box::new(StreamCorrelation::new()))
            }
        }
    }
    pub(super) fn as_ref(&self) -> CorrelationRef<'_> {
        match self {
            Self::Sequenced => CorrelationRef::Sequenced,
            Self::RawDatagram { holds } => CorrelationRef::RawDatagram(holds),
            Self::RawStream(stream) => CorrelationRef::RawStream(stream),
        }
    }
    pub(super) fn as_mut(&mut self) -> CorrelationState<'_> {
        match self {
            Self::Sequenced => CorrelationState::Sequenced,
            Self::RawDatagram { holds } => CorrelationState::RawDatagram(holds),
            Self::RawStream(stream) => CorrelationState::RawStream(stream),
        }
    }
    #[cfg(test)]
    pub(super) fn ledger(&self) -> &StreamLedger {
        self.as_ref().ledger()
    }
    #[cfg(test)]
    pub(super) fn holds(&self) -> &BTreeMap<RawHoldKey, RawHold> {
        self.as_ref().holds()
    }
    #[cfg(test)]
    pub(super) fn learning(&self, target: TargetSlot) -> &CompletionLearning {
        self.as_ref().learning(target)
    }
    #[cfg(test)]
    pub(super) fn stream_mut(&mut self) -> Option<&mut StreamCorrelation> {
        match self {
            Self::RawStream(stream) => Some(stream),
            Self::Sequenced | Self::RawDatagram { .. } => None,
        }
    }
}

/// Borrowed mode selection: mutable state is exposed only after matching its variant.
pub(super) enum CorrelationState<'a> {
    Sequenced,
    RawDatagram(&'a mut HoldTable<RawHoldScope>),
    RawStream(&'a mut StreamCorrelation),
}

impl CorrelationState<'_> {
    pub(super) fn extend_hold(
        &mut self,
        target: TargetSlot,
        scope: StreamScope,
        until: Instant,
        owner: Option<RequestId>,
    ) {
        match self {
            Self::RawDatagram(holds) => holds.extend(target, scope.raw(), until, owner),
            Self::RawStream(stream) => stream.holds.extend(target, scope, until, owner),
            Self::Sequenced => {}
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum CorrelationRef<'a> {
    Sequenced,
    RawDatagram(&'a HoldTable<RawHoldScope>),
    RawStream(&'a StreamCorrelation),
}
impl<'a> CorrelationRef<'a> {
    pub(super) const fn is_raw(self) -> bool {
        !matches!(self, Self::Sequenced)
    }
    pub(super) const fn is_stream(self) -> bool {
        matches!(self, Self::RawStream(_))
    }
    pub(super) fn stream(self) -> Option<&'a StreamCorrelation> {
        match self {
            Self::RawStream(stream) => Some(stream),
            Self::Sequenced | Self::RawDatagram { .. } => None,
        }
    }
    pub(super) fn ledger(self) -> &'a StreamLedger {
        static EMPTY: LazyLock<StreamLedger> = LazyLock::new(StreamLedger::new);
        self.stream().map_or(&EMPTY, |stream| &stream.ledger)
    }
    pub(super) fn holds(self) -> &'a BTreeMap<RawHoldKey, RawHold> {
        static EMPTY: BTreeMap<RawHoldKey, RawHold> = BTreeMap::new();
        match self {
            Self::Sequenced => &EMPTY,
            Self::RawDatagram(holds) => holds.entries(),
            Self::RawStream(stream) => stream.holds.entries(),
        }
    }
    #[cfg(test)]
    pub(super) fn learning(self, target: TargetSlot) -> &'a CompletionLearning {
        static EMPTY: LazyLock<CompletionLearning> = LazyLock::new(CompletionLearning::default);
        self.stream()
            .map_or(&EMPTY, |stream| &stream.learning[target])
    }

    pub(super) fn released_by_evidence(self, key: RawHoldKey) -> bool {
        holds::released_by_evidence(key, self.stream().map(|stream| &stream.ledger))
    }
}

impl StreamCorrelation {
    pub(super) fn new() -> Self {
        Self {
            holds: HoldTable::new(),
            ledger: StreamLedger::new(),
            learning: PerTarget::default(),
            release_gate: None,
        }
    }
    pub(super) fn update_learning(
        &mut self,
        target: TargetSlot,
        update: impl FnOnce(&mut CompletionLearning),
    ) {
        update(&mut self.learning[target]);
    }
    pub(super) fn take_pending(
        &mut self,
        target: TargetSlot,
    ) -> Option<PendingSocketlessCompletions> {
        self.learning[target].pending_socketless_completions.take()
    }

    /// A stream's pre-ACK and completion-only answers remain in its ledger.
    /// Only an exact socket or a physically written NoReply request needs a
    /// separate hold. This scope type cannot express a pre-ACK hold.
    pub(super) fn quarantine(
        &mut self,
        target: TargetSlot,
        id: RequestId,
        lifecycle: super::Lifecycle,
        until: Instant,
    ) {
        use super::{Lifecycle, SendingReply};
        let scope = match lifecycle {
            Lifecycle::Executing { socket, .. }
            | Lifecycle::AwaitingCancellationResolution { socket, .. } => {
                Some(StreamScope::Socket(socket))
            }
            Lifecycle::Sending {
                reply: SendingReply::NoReply,
                ..
            } if self.ledger.live(target, id).any(|entry| entry.written) => {
                Some(StreamScope::AllResponses)
            }
            _ => None,
        };
        if let Some(scope) = scope {
            self.holds.extend(target, scope, until, Some(id));
        }
    }
    pub(super) fn stop_wait_bound(&self, entry: &super::Entry) -> Option<Instant> {
        if !entry.request.is_stop() {
            return None;
        }
        let context = entry.request.context();
        let bound = super::add_duration(
            entry.submitted_at,
            context
                .timeout
                .ack
                .get()
                .saturating_add(context.timeout.ambiguity.get()),
        );
        Some(context.dispatch_deadline.map_or(bound, |deadline| {
            bound.min(super::add_duration(
                entry.submitted_at,
                deadline.saturating_duration_since(entry.submitted_at) / 2,
            ))
        }))
    }
    pub(super) fn owes(&self, entry: &super::Entry) -> Option<super::Owes> {
        match entry.reply_family() {
            super::SendingReply::Inquiry => entry.request.payload_shape().map(super::Owes::Reply),
            super::SendingReply::Stop | super::SendingReply::AckThenCompletion => {
                Some(super::Owes::Ack)
            }
            super::SendingReply::CompletionOnly => Some(super::Owes::Completion),
            super::SendingReply::NoReply => Some(super::Owes::Rejection),
        }
    }
    /// Drain one target's learned completion at a time so the engine applies
    /// its request transition before inspecting the next owed completion.
    pub(super) fn next_learned_completion(
        &mut self,
        target: TargetSlot,
    ) -> Option<(super::Outstanding, super::Answered)> {
        let CompletionSockets::Named { unpaid } = &mut self.learning[target].completion_sockets
        else {
            return None;
        };
        if *unpaid == 0 {
            return None;
        }
        if let Some((entry, answered)) = self.ledger.take_completion(target) {
            *unpaid -= 1;
            return Some((entry, answered));
        }
        *unpaid = 0;
        None
    }
    pub(super) fn learn_socketless_completion(&mut self, target: TargetSlot) -> CompletionSockets {
        if !self.ledger.owes_completion(target) {
            let learning = &mut self.learning[target];
            match &mut learning.completion_sockets {
                CompletionSockets::Unknown {
                    retired: retired @ 1..,
                    ..
                } => *retired -= 1,
                _ => learning.completion_sockets = CompletionSockets::Omitted,
            }
        }
        self.learning[target].completion_sockets
    }
}

/// Datagram quarantine can name PreAck; its stream counterpart cannot.
pub(super) fn quarantine_datagram(
    holds: &mut HoldTable<RawHoldScope>,
    target: TargetSlot,
    id: RequestId,
    lifecycle: super::Lifecycle,
    until: Instant,
) {
    use super::{Lifecycle, SendingReply};
    let scope = match lifecycle {
        Lifecycle::Executing { socket, .. }
        | Lifecycle::AwaitingCancellationResolution { socket, .. } => {
            Some(RawHoldScope::Socket(socket))
        }
        Lifecycle::AwaitingAck { .. } => Some(RawHoldScope::PreAck),
        Lifecycle::AwaitingCompletion { .. } => Some(RawHoldScope::AllResponses),
        Lifecycle::Sending {
            reply: SendingReply::Stop | SendingReply::AckThenCompletion,
            ..
        } => Some(RawHoldScope::PreAck),
        Lifecycle::Sending {
            reply: SendingReply::CompletionOnly | SendingReply::NoReply,
            ..
        } => Some(RawHoldScope::AllResponses),
        _ => None,
    };
    if let Some(scope) = scope {
        holds.extend(target, scope, until, Some(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn construction_selects_each_correlation_regime_once() {
        for transport in [TransportKind::Datagram, TransportKind::Stream] {
            assert!(matches!(
                CorrelationMode::new(EnvelopeKind::Sony, transport),
                CorrelationMode::Sequenced
            ));
        }
        assert!(matches!(
            CorrelationMode::new(EnvelopeKind::Raw, TransportKind::Datagram),
            CorrelationMode::RawDatagram { .. }
        ));
        assert!(matches!(
            CorrelationMode::new(EnvelopeKind::Raw, TransportKind::Stream),
            CorrelationMode::RawStream(_)
        ));
    }
}
