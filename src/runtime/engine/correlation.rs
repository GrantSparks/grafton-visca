//! The correlation regime selected once when a session is constructed.
use std::{collections::BTreeMap, sync::LazyLock, time::Instant};

use super::{
    holds::{self, HoldTable, Scope, StreamScope},
    CompletionSockets, EnvelopeKind, PendingSocketlessCompletions, RawHold, RawHoldKey,
    RawHoldScope, RawReleaseGate, RequestId, StreamLedger, TransportKind,
};
use crate::target::{PerTarget, TargetSlot};

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct CompletionLearning {
    /// Largest dispatched or camera-proven physical socket capacity. Retuning
    /// cannot erase the possible execution of an older owed ACK.
    pub(super) socket_capacity_seen: Option<crate::CommandSocketCount>,
    pub(super) completion_sockets: CompletionSockets,
    pub(super) pending_socketless_completions: Option<PendingSocketlessCompletions>,
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
