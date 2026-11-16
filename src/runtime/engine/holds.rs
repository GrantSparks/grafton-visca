//! Closed mutations of a raw correlation hold table.
//!
//! The scope parameter is a capability: a stream hold table accepts only
//! `StreamScope`, whose vocabulary deliberately has no pre-ACK hold.
use std::{collections::BTreeMap, marker::PhantomData, time::Instant};

use super::{stream_ledger::StreamLedger, RawHold, RawHoldKey, RawHoldScope, RequestId};
use crate::{target::TargetSlot, PayloadShape, ViscaSocket};

#[derive(Debug, Clone, Copy)]
pub(super) enum StreamScope {
    Socket(ViscaSocket),
    Inquiry(PayloadShape),
    AllResponses,
}

pub(super) trait Scope: Copy {
    fn raw(self) -> RawHoldScope;
}
impl Scope for RawHoldScope {
    fn raw(self) -> Self {
        self
    }
}
impl Scope for StreamScope {
    fn raw(self) -> RawHoldScope {
        match self {
            Self::Socket(socket) => RawHoldScope::Socket(socket),
            Self::Inquiry(shape) => RawHoldScope::Inquiry(shape),
            Self::AllResponses => RawHoldScope::AllResponses,
        }
    }
}

#[derive(Debug)]
pub(super) struct HoldTable<S> {
    entries: BTreeMap<RawHoldKey, RawHold>,
    scope: PhantomData<S>,
}
impl<S: Scope> HoldTable<S> {
    pub(super) fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            scope: PhantomData,
        }
    }
    #[cfg(test)]
    pub(super) fn get(&self, target: TargetSlot, scope: RawHoldScope) -> Option<RawHold> {
        self.entries.get(&RawHoldKey::new(target, scope)).copied()
    }
    pub(super) fn entries(&self) -> &BTreeMap<RawHoldKey, RawHold> {
        &self.entries
    }
    /// Extend conservatively and collapse identity when owners overlap.
    pub(super) fn extend(
        &mut self,
        target: TargetSlot,
        scope: S,
        until: Instant,
        owner: Option<RequestId>,
    ) {
        self.entries
            .entry(RawHoldKey::new(target, scope.raw()))
            .and_modify(|hold| {
                hold.until = hold.until.max(until);
                if hold.owner != owner {
                    hold.owner = None;
                }
            })
            .or_insert(RawHold { until, owner });
    }
    pub(super) fn release_socket(
        &mut self,
        target: TargetSlot,
        socket: ViscaSocket,
    ) -> Option<RawHold> {
        self.entries
            .remove(&RawHoldKey::new(target, RawHoldScope::Socket(socket)))
    }
    /// A paid reply leaves only ordinary reply skew, even if its former debt
    /// window was longer than that skew.
    pub(super) fn reset_paid_reply_skew(
        &mut self,
        target: TargetSlot,
        shape: PayloadShape,
        until: Instant,
    ) {
        if let Some(hold) = self
            .entries
            .get_mut(&RawHoldKey::new(target, RawHoldScope::Inquiry(shape)))
        {
            hold.until = until;
        }
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }
    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl HoldTable<RawHoldScope> {
    pub(super) fn expire(&mut self, now: Instant) {
        self.entries.retain(|_, hold| hold.until > now);
    }
}
impl HoldTable<StreamScope> {
    /// Evidence is mandatory: omitting it cannot turn a stream socket into a
    /// time-expiring datagram hold.
    pub(super) fn expire(&mut self, now: Instant, ledger: &StreamLedger) {
        self.entries
            .retain(|key, hold| hold.until > now || released_by_evidence(*key, Some(ledger)));
    }
}

/// One definition serves expiry, wake planning and retained-input release.
/// A stream socket waits for evidence, and a still-owed inquiry reply latches
/// its shape instead of reopening it at the window deadline.
pub(super) fn released_by_evidence(key: RawHoldKey, ledger: Option<&StreamLedger>) -> bool {
    ledger.is_some_and(|ledger| matches!(key.scope, RawHoldScope::Socket(_))
        || matches!(key.scope, RawHoldScope::Inquiry(shape) if ledger.owes_reply(key.target, shape)))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::{num::NonZeroU64, time::Duration};
    fn id(value: u64) -> RequestId {
        RequestId::from_nonzero(NonZeroU64::new(value).unwrap())
    }
    #[test]
    fn extension_collapses_owners_and_socket_release_is_exact() {
        let now = Instant::now();
        let target = TargetSlot::ALL[0];
        let mut holds = HoldTable::<StreamScope>::new();
        holds.extend(
            target,
            StreamScope::Socket(ViscaSocket::S1),
            now + Duration::from_secs(2),
            Some(id(1)),
        );
        holds.extend(
            target,
            StreamScope::Socket(ViscaSocket::S1),
            now,
            Some(id(2)),
        );
        assert_eq!(
            holds.get(target, RawHoldScope::Socket(ViscaSocket::S1)),
            Some(RawHold {
                until: now + Duration::from_secs(2),
                owner: None
            })
        );
        assert!(holds.release_socket(target, ViscaSocket::S2).is_none());
        assert!(holds.release_socket(target, ViscaSocket::S1).is_some());
        assert!(holds.is_empty());
    }
    #[test]
    fn datagram_holds_expire_but_stream_sockets_require_evidence() {
        let now = Instant::now();
        let target = TargetSlot::ALL[0];
        let mut datagram = HoldTable::<RawHoldScope>::new();
        datagram.extend(target, RawHoldScope::PreAck, now, None);
        datagram.expire(now);
        assert!(datagram.is_empty());
        let mut stream = HoldTable::<StreamScope>::new();
        stream.extend(target, StreamScope::Socket(ViscaSocket::S1), now, None);
        stream.expire(now, &StreamLedger::new());
        assert!(!stream.is_empty());
    }
    #[test]
    fn paid_reply_resets_to_skew_instead_of_extending_the_old_window() {
        let now = Instant::now();
        let target = TargetSlot::ALL[0];
        let shape = PayloadShape::Nibbles(2);
        let mut holds = HoldTable::<StreamScope>::new();
        holds.extend(
            target,
            StreamScope::Inquiry(shape),
            now + Duration::from_secs(20),
            None,
        );
        holds.reset_paid_reply_skew(target, shape, now + Duration::from_secs(1));
        assert_eq!(
            holds
                .get(target, RawHoldScope::Inquiry(shape))
                .unwrap()
                .until,
            now + Duration::from_secs(1)
        );
    }
    #[test]
    fn an_owed_reply_keeps_its_shape_until_evidence_pays_it() {
        let now = Instant::now();
        let target = TargetSlot::ALL[0];
        let shape = PayloadShape::Nibbles(2);
        let mut ledger = StreamLedger::new();
        ledger.push(target, id(1), super::super::Owes::Reply(shape));
        ledger.mark_written(target, id(1), false);
        ledger.retire(target, id(1), |_| (now, Duration::ZERO));
        let mut holds = HoldTable::<StreamScope>::new();
        holds.extend(target, StreamScope::Inquiry(shape), now, Some(id(1)));
        holds.expire(now, &ledger);
        assert!(!holds.is_empty());
        ledger.clear();
        holds.expire(now, &ledger);
        assert!(holds.is_empty());
    }
}
