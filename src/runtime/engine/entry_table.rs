//! Authoritative live entries and the indexes changed with them.
//!
//! This module intentionally exposes no mutable entry or index view. Every
//! indexed change enters through admission, a closed transition, or removal.

use std::{
    collections::{btree_map, BTreeMap, VecDeque},
    ops::{Index, IndexMut},
    time::Instant,
};

use smallvec::SmallVec;

use super::{
    AttemptFacts, CorrelationKind, CorrelationOwner, Entry, ExecutingCancellation, Lifecycle,
    RequestId, RequestedCancellation, ResolvingCancellation, RuntimeRequest, SendingReply,
    SequenceRecord, TargetSlot, TransmissionId, TransmissionMeta, ViscaSocket,
};
use crate::target::PerTarget;

/// Two physical sockets, indexed by the protocol value rather than an integer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SocketSlots<T> {
    s1: T,
    s2: T,
}
impl<T> Index<ViscaSocket> for SocketSlots<T> {
    type Output = T;
    fn index(&self, socket: ViscaSocket) -> &T {
        match socket {
            ViscaSocket::S1 => &self.s1,
            ViscaSocket::S2 => &self.s2,
        }
    }
}
impl<T> IndexMut<ViscaSocket> for SocketSlots<T> {
    fn index_mut(&mut self, socket: ViscaSocket) -> &mut T {
        match socket {
            ViscaSocket::S1 => &mut self.s1,
            ViscaSocket::S2 => &mut self.s2,
        }
    }
}

/// Each Sony request replays one sequence; cancellation has its own sequence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct SequenceSlots {
    request: Option<u32>,
    cancellation: Option<u32>,
}

impl Index<CorrelationKind> for SequenceSlots {
    type Output = Option<u32>;

    fn index(&self, kind: CorrelationKind) -> &Self::Output {
        match kind {
            CorrelationKind::Request => &self.request,
            CorrelationKind::Cancellation => &self.cancellation,
        }
    }
}

impl IndexMut<CorrelationKind> for SequenceSlots {
    fn index_mut(&mut self, kind: CorrelationKind) -> &mut Self::Output {
        match kind {
            CorrelationKind::Request => &mut self.request,
            CorrelationKind::Cancellation => &mut self.cancellation,
        }
    }
}

impl SequenceSlots {
    pub(super) fn records(self) -> impl DoubleEndedIterator<Item = SequenceRecord> {
        [CorrelationKind::Request, CorrelationKind::Cancellation]
            .into_iter()
            .filter_map(move |kind| self[kind].map(|sequence| SequenceRecord { sequence, kind }))
    }

    fn record(&mut self, kind: CorrelationKind, sequence: u32) {
        // Fresh allocation is planned only while this correlation slot is empty.
        // Replays carry `Known` and never change either sequence index.
        self[kind].get_or_insert(sequence);
    }
}

/// One phase, with a closed placement for the camera-proven displacement window.
#[derive(Debug, Clone, Copy)]
pub(super) enum EntryPlacement {
    Active(Lifecycle),
    Displaced(DisplacedSocketLifecycle),
}

/// A privately constructed proof that the displaced logical phase was socketed.
#[derive(Debug, Clone, Copy)]
pub(super) struct DisplacedSocketLifecycle(Lifecycle);

impl EntryPlacement {
    pub(super) const fn logical(self) -> Lifecycle {
        match self {
            Self::Active(lifecycle) => lifecycle,
            Self::Displaced(phase) => phase.0,
        }
    }
    pub(super) const fn active(self) -> Option<Lifecycle> {
        match self {
            Self::Active(lifecycle) => Some(lifecycle),
            Self::Displaced(_) => None,
        }
    }
    fn active_socket(self) -> Option<ViscaSocket> {
        self.active().and_then(|lifecycle| lifecycle.socket())
    }
    fn displace(self) -> Self {
        match self {
            Self::Active(lifecycle) if lifecycle.socket().is_some() => {
                Self::Displaced(DisplacedSocketLifecycle(lifecycle))
            }
            placement => placement,
        }
    }
    fn transition(self, to: Lifecycle) -> Self {
        match self {
            Self::Displaced(_) if to.socket().is_some() => {
                Self::Displaced(DisplacedSocketLifecycle(to))
            }
            _ => Self::Active(to),
        }
    }
}

/// Successors which cannot install a socket identity.
#[derive(Debug, Clone, Copy)]
pub(crate) enum UnsocketedLifecycle {
    Ready {
        previous_attempt: Option<AttemptFacts>,
    },
    Sending {
        transmission: TransmissionId,
        started_at: Instant,
        attempt: AttemptFacts,
        reply: SendingReply,
    },
    AwaitingAck {
        sent_at: Instant,
        deadline: Instant,
        attempt: AttemptFacts,
        cancellation: Option<RequestedCancellation>,
    },
    AwaitingCompletion {
        sent_at: Instant,
        deadline: Instant,
        attempt: AttemptFacts,
    },
    ExecutingDisplaced {
        started_at: Instant,
        deadline: Instant,
        attempt: AttemptFacts,
    },
    AwaitingReply {
        sent_at: Instant,
        deadline: Instant,
        attempt: AttemptFacts,
    },
}
impl UnsocketedLifecycle {
    fn lifecycle(self) -> Lifecycle {
        match self {
            Self::Ready { previous_attempt } => Lifecycle::Ready { previous_attempt },
            Self::Sending {
                transmission,
                started_at,
                attempt,
                reply,
            } => Lifecycle::Sending {
                transmission,
                started_at,
                attempt,
                reply,
            },
            Self::AwaitingAck {
                sent_at,
                deadline,
                attempt,
                cancellation,
            } => Lifecycle::AwaitingAck {
                sent_at,
                deadline,
                attempt,
                cancellation,
            },
            Self::AwaitingCompletion {
                sent_at,
                deadline,
                attempt,
            } => Lifecycle::AwaitingCompletion {
                sent_at,
                deadline,
                attempt,
            },
            Self::ExecutingDisplaced {
                started_at,
                deadline,
                attempt,
            } => Lifecycle::ExecutingDisplaced {
                started_at,
                deadline,
                attempt,
            },
            Self::AwaitingReply {
                sent_at,
                deadline,
                attempt,
            } => Lifecycle::AwaitingReply {
                sent_at,
                deadline,
                attempt,
            },
        }
    }
}

/// A socket-preserving successor; the table supplies its existing identity.
#[derive(Debug, Clone, Copy)]
pub(crate) enum SocketedLifecycle {
    Executing {
        started_at: Instant,
        deadline: Instant,
        attempt: AttemptFacts,
        cancellation: ExecutingCancellation,
    },
    AwaitingCancellationResolution {
        deadline: Instant,
        attempt: AttemptFacts,
        cancellation: ResolvingCancellation,
    },
}
impl SocketedLifecycle {
    fn lifecycle(self, socket: ViscaSocket) -> Lifecycle {
        match self {
            Self::Executing {
                started_at,
                deadline,
                attempt,
                cancellation,
            } => Lifecycle::Executing {
                socket,
                started_at,
                deadline,
                attempt,
                cancellation,
            },
            Self::AwaitingCancellationResolution {
                deadline,
                attempt,
                cancellation,
            } => Lifecycle::AwaitingCancellationResolution {
                socket,
                deadline,
                attempt,
                cancellation,
            },
        }
    }
}

/// Proof minted only for a free entry-table socket, consumed on installation.
#[derive(Debug)]
pub(super) struct SocketClaim<'a> {
    table: &'a mut EntryTable,
    request: RequestId,
    socket: ViscaSocket,
}

impl SocketClaim<'_> {
    pub(super) fn install(self, phase: SocketedLifecycle) -> Option<Transitioned> {
        self.table
            .change_lifecycle(self.request, phase.lifecycle(self.socket))
    }

    #[cfg(test)]
    pub(super) fn socket(self) -> ViscaSocket {
        self.socket
    }
}

/// A complete change to one live entry. Index updates are part of the change.
#[derive(Debug)]
pub(super) enum EntryChange {
    Lifecycle(UnsocketedLifecycle),
    Socketed(SocketedLifecycle),
    Written {
        receipt: TransmissionMeta,
        kind: CorrelationKind,
    },
    Retry {
        ready_at: Instant,
        previous_attempt: AttemptFacts,
        prior: super::PriorAttempts,
    },
    CloseObserver,
    DisplaceSocket,
}

impl From<UnsocketedLifecycle> for EntryChange {
    fn from(phase: UnsocketedLifecycle) -> Self {
        Self::Lifecycle(phase)
    }
}
impl From<SocketedLifecycle> for EntryChange {
    fn from(phase: SocketedLifecycle) -> Self {
        Self::Socketed(phase)
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Transitioned {
    pub(super) from: Lifecycle,
    pub(super) to: Lifecycle,
}

#[derive(Clone, Copy)]
struct IndexFacts {
    ready: Option<usize>,
    socket: Option<(TargetSlot, ViscaSocket)>,
    sequences: SequenceSlots,
}
impl IndexFacts {
    fn of(entry: &Entry) -> Self {
        let mut sequences = entry.sequences;
        for kind in [CorrelationKind::Request, CorrelationKind::Cancellation] {
            if entry.lifecycle.active().is_none()
                || !super::correlation_phase_compatible(entry, kind)
            {
                sequences[kind] = None;
            }
        }
        Self {
            ready: entry
                .lifecycle()
                .is_ready()
                .then(|| entry.request.priority_index()),
            socket: entry
                .lifecycle
                .active_socket()
                .map(|socket| (entry.request.context().target, socket)),
            sequences,
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct EntryTable {
    entries: BTreeMap<RequestId, Entry>,
    ready: [VecDeque<RequestId>; 4],
    sequences: BTreeMap<u32, SmallVec<[CorrelationOwner; 2]>>,
    lower_sequences: BTreeMap<u16, SmallVec<[CorrelationOwner; 2]>>,
    sockets: PerTarget<SocketSlots<Option<RequestId>>>,
}
impl EntryTable {
    pub(super) fn new() -> Self {
        Self::default()
    }
    #[cfg(any(test, debug_assertions))]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }
    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub(super) fn get(&self, id: &RequestId) -> Option<&Entry> {
        self.entries.get(id)
    }
    #[cfg(not(all(test, debug_assertions)))]
    pub(super) fn iter(&self) -> btree_map::Iter<'_, RequestId, Entry> {
        self.entries.iter()
    }
    #[cfg(all(test, debug_assertions))]
    pub(super) fn iter(&self) -> CountingIter<btree_map::Iter<'_, RequestId, Entry>> {
        CountingIter(self.entries.iter())
    }
    #[cfg(not(all(test, debug_assertions)))]
    pub(super) fn values(&self) -> btree_map::Values<'_, RequestId, Entry> {
        self.entries.values()
    }
    #[cfg(all(test, debug_assertions))]
    pub(super) fn values(&self) -> CountingIter<btree_map::Values<'_, RequestId, Entry>> {
        CountingIter(self.entries.values())
    }
    pub(super) fn active_entries(&self) -> impl Iterator<Item = (&RequestId, &Entry)> {
        self.iter()
            .filter(|(_, entry)| entry.lifecycle.active().is_some())
    }
    pub(super) fn ready_candidates(&self) -> impl Iterator<Item = RequestId> + '_ {
        let candidates = self.ready.iter().rev().flatten().copied();
        #[cfg(all(test, debug_assertions))]
        let candidates = CountingIter(candidates);
        candidates
    }
    #[cfg(test)]
    pub(super) fn ready(&self, priority: usize) -> &VecDeque<RequestId> {
        &self.ready[priority]
    }
    pub(super) fn socket_owner(
        &self,
        target: TargetSlot,
        socket: ViscaSocket,
    ) -> Option<RequestId> {
        self.sockets[target][socket]
    }
    pub(super) fn exact_owners(&self, sequence: u32) -> &[CorrelationOwner] {
        self.sequences
            .get(&sequence)
            .map_or(&[], SmallVec::as_slice)
    }
    pub(super) fn lower_owners(&self, sequence: u16) -> &[CorrelationOwner] {
        self.lower_sequences
            .get(&sequence)
            .map_or(&[], SmallVec::as_slice)
    }

    pub(super) fn admit(&mut self, id: RequestId, request: RuntimeRequest, submitted_at: Instant) {
        debug_assert!(!self.entries.contains_key(&id));
        let entry = Entry {
            request,
            lifecycle: EntryPlacement::Active(Lifecycle::Ready {
                previous_attempt: None,
            }),
            submitted_at,
            prior: None,
            sequences: SequenceSlots::default(),
        };
        let indexed = IndexFacts::of(&entry);
        self.entries.insert(id, entry);
        self.add_indexes(id, &indexed);
    }

    pub(super) fn claim_socket(
        &mut self,
        request: RequestId,
        candidates: impl IntoIterator<Item = ViscaSocket>,
    ) -> Option<SocketClaim<'_>> {
        let target = self.entries.get(&request)?.request.context().target;
        let socket = candidates.into_iter().find(|socket| {
            self.socket_owner(target, *socket)
                .is_none_or(|owner| owner == request)
        })?;
        Some(SocketClaim {
            table: self,
            request,
            socket,
        })
    }

    fn change_lifecycle(&mut self, id: RequestId, to: Lifecycle) -> Option<Transitioned> {
        let entry = self.entries.get_mut(&id)?;
        let before = IndexFacts::of(entry);
        let from = entry.lifecycle();
        entry.lifecycle = entry.lifecycle.transition(to);
        let after = IndexFacts::of(entry);
        let changed = Transitioned { from, to };
        self.update_indexes(id, &before, &after);
        Some(changed)
    }

    pub(super) fn transition(
        &mut self,
        id: RequestId,
        change: EntryChange,
    ) -> Option<Transitioned> {
        let entry = self.entries.get_mut(&id)?;
        let before = IndexFacts::of(entry);
        let from = entry.lifecycle();
        match change {
            EntryChange::DisplaceSocket => entry.lifecycle = entry.lifecycle.displace(),
            EntryChange::Lifecycle(to) => {
                entry.lifecycle = entry.lifecycle.transition(to.lifecycle())
            }
            EntryChange::Socketed(to) => {
                let socket = entry.lifecycle().socket()?;
                entry.lifecycle = entry.lifecycle.transition(to.lifecycle(socket));
            }
            EntryChange::Retry {
                ready_at,
                previous_attempt,
                prior,
            } => {
                entry.lifecycle = entry.lifecycle.transition(Lifecycle::Backoff {
                    ready_at,
                    previous_attempt,
                });
                entry.prior = Some(prior);
            }
            EntryChange::Written { receipt, kind } => {
                if let TransmissionMeta::Allocated { sequence } = receipt {
                    entry.sequences.record(kind, sequence);
                }
            }
            EntryChange::CloseObserver => {
                let mut to = entry.lifecycle();
                to.close_observer();
                entry.lifecycle = entry.lifecycle.transition(to);
            }
        }
        let after = IndexFacts::of(entry);
        let result = Transitioned {
            from,
            to: entry.lifecycle(),
        };
        self.update_indexes(id, &before, &after);
        Some(result)
    }

    /// Test-only corruption of semantic request facts while keeping indexes
    /// structurally correct, so retained cross-domain audits can be exercised.
    #[cfg(test)]
    pub(super) fn replace_request_for_audit(&mut self, id: RequestId, request: RuntimeRequest) {
        let Some(entry) = self.entries.get_mut(&id) else {
            return;
        };
        let before = IndexFacts::of(entry);
        entry.request = request;
        let after = IndexFacts::of(entry);
        self.update_indexes(id, &before, &after);
    }

    pub(super) fn remove(&mut self, id: &RequestId) -> Option<Entry> {
        let entry = self.entries.remove(id)?;
        self.remove_indexes(*id, &IndexFacts::of(&entry));
        Some(entry)
    }
    fn add_indexes(&mut self, id: RequestId, facts: &IndexFacts) {
        if let Some(priority) = facts.ready {
            self.ready[priority].push_back(id);
        }
        if let Some((target, socket)) = facts.socket {
            self.sockets[target][socket] = Some(id);
        }
        for record in facts.sequences.records() {
            self.add_sequence(id, record);
        }
    }
    fn remove_indexes(&mut self, id: RequestId, facts: &IndexFacts) {
        if let Some(priority) = facts.ready {
            let queue = &mut self.ready[priority];
            if let Some(index) = queue.iter().position(|queued| *queued == id) {
                queue.remove(index);
            }
        }
        if let Some((target, socket)) = facts.socket {
            self.sockets[target][socket] = None;
        }
        for record in facts.sequences.records() {
            self.remove_sequence(id, record);
        }
    }
    fn update_indexes(&mut self, id: RequestId, before: &IndexFacts, after: &IndexFacts) {
        if before.ready != after.ready {
            if let Some(priority) = before.ready {
                let queue = &mut self.ready[priority];
                if let Some(index) = queue.iter().position(|queued| *queued == id) {
                    queue.remove(index);
                }
            }
            if let Some(priority) = after.ready {
                self.ready[priority].push_back(id);
            }
        }
        if before.socket != after.socket {
            if let Some((target, socket)) = before.socket {
                self.sockets[target][socket] = None;
            }
            if let Some((target, socket)) = after.socket {
                self.sockets[target][socket] = Some(id);
            }
        }
        for record in before.sequences.records() {
            if after.sequences[record.kind] != Some(record.sequence) {
                self.remove_sequence(id, record);
            }
        }
        for record in after.sequences.records() {
            if before.sequences[record.kind] != Some(record.sequence) {
                self.add_sequence(id, record);
            }
        }
    }
    fn add_sequence(&mut self, id: RequestId, record: SequenceRecord) {
        let owner = CorrelationOwner {
            request: id,
            kind: record.kind,
        };
        add_unique_owner(self.sequences.entry(record.sequence).or_default(), owner);
        add_unique_owner(
            self.lower_sequences
                .entry(record.sequence as u16)
                .or_default(),
            owner,
        );
    }
    fn remove_sequence(&mut self, id: RequestId, record: SequenceRecord) {
        let matches =
            |owner: &CorrelationOwner| !(owner.request == id && owner.kind == record.kind);
        if let Some(owners) = self.sequences.get_mut(&record.sequence) {
            owners.retain(|owner| matches(owner));
            if owners.is_empty() {
                self.sequences.remove(&record.sequence);
            }
        }
        let lower = record.sequence as u16;
        if let Some(owners) = self.lower_sequences.get_mut(&lower) {
            owners.retain(|owner| matches(owner));
            if owners.is_empty() {
                self.lower_sequences.remove(&lower);
            }
        }
    }
}

impl<'a> IntoIterator for &'a EntryTable {
    type Item = (&'a RequestId, &'a Entry);
    #[cfg(not(all(test, debug_assertions)))]
    type IntoIter = btree_map::Iter<'a, RequestId, Entry>;
    #[cfg(all(test, debug_assertions))]
    type IntoIter = CountingIter<btree_map::Iter<'a, RequestId, Entry>>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

fn add_unique_owner(owners: &mut SmallVec<[CorrelationOwner; 2]>, owner: CorrelationOwner) {
    if !owners.contains(&owner) {
        owners.push(owner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_socket_slots_keep_both_values_independent() {
        let mut slots = SocketSlots {
            s1: Some(1),
            s2: Some(2),
        };
        slots[ViscaSocket::S1] = None;
        assert_eq!(slots[ViscaSocket::S1], None);
        assert_eq!(slots[ViscaSocket::S2], Some(2));
    }
}

#[cfg(all(test, debug_assertions))]
thread_local! {
    static ENTRY_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
#[cfg(all(test, debug_assertions))]
fn record_entry_visit() {
    ENTRY_VISITS.with(|visits| visits.set(visits.get() + 1));
}
#[cfg(all(test, debug_assertions))]
pub(super) fn reset_entry_visits() {
    ENTRY_VISITS.with(|visits| visits.set(0));
}
#[cfg(all(test, debug_assertions))]
pub(super) fn entry_visits() -> usize {
    ENTRY_VISITS.with(std::cell::Cell::get)
}

// Debug count tests instrument full-table and ready-candidate iterators.
// Release timing uses the production iterators without a counter or TLS branch.
#[cfg(all(test, debug_assertions))]
#[derive(Clone)]
pub(super) struct CountingIter<I>(I);
#[cfg(all(test, debug_assertions))]
impl<I: Iterator> Iterator for CountingIter<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        let next = self.0.next();
        if next.is_some() {
            record_entry_visit();
        }
        next
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
#[cfg(all(test, debug_assertions))]
impl<I: DoubleEndedIterator> DoubleEndedIterator for CountingIter<I> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let next = self.0.next_back();
        if next.is_some() {
            record_entry_visit();
        }
        next
    }
}
#[cfg(all(test, debug_assertions))]
impl<I: ExactSizeIterator> ExactSizeIterator for CountingIter<I> {}
