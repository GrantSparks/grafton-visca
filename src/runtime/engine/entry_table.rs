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
    AdmissionSlot, CorrelationKind, CorrelationOwner, Entry, Lane, Lifecycle, RequestId,
    RuntimeRequest, SequenceRecord, TargetSlot, TransmissionMeta, ViscaSocket,
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
        let slot = &mut self[kind];
        if let Some(previous) = slot {
            // The framing check rejects an inconsistent retry before this
            // mutation. Do not silently replace its authoritative sequence if
            // an internal caller ever violates that protocol.
            assert_eq!(
                *previous, sequence,
                "one Sony sequence per correlation kind"
            );
        } else {
            *slot = Some(sequence);
        }
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

#[derive(Debug, Clone, Copy)]
pub(super) struct AdmissionFacts {
    pub(super) submitted_at: Instant,
    pub(super) slot: AdmissionSlot,
}

/// A complete change to one live entry. Index updates are part of the change.
#[derive(Debug)]
pub(super) enum EntryChange {
    Lifecycle(Lifecycle),
    Written {
        receipt: TransmissionMeta,
        kind: CorrelationKind,
    },
    Retry {
        to: Lifecycle,
        prior: super::PriorAttempts,
    },
    CloseObserver,
    DisplaceSocket,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Transitioned {
    pub(super) from: Lifecycle,
    pub(super) to: Lifecycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SocketConflict {
    pub(super) target: TargetSlot,
    pub(super) socket: ViscaSocket,
    pub(super) owner: RequestId,
}

#[derive(Clone, Copy)]
struct IndexFacts {
    ready: Option<(Lane, usize)>,
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
                .then(|| (entry.request.lane(), entry.request.priority_index())),
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
    commands: [VecDeque<RequestId>; 4],
    inquiries: [VecDeque<RequestId>; 4],
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
    pub(super) fn ready(&self, lane: Lane, priority: usize) -> &VecDeque<RequestId> {
        match lane {
            Lane::Command => &self.commands[priority],
            Lane::Inquiry => &self.inquiries[priority],
        }
    }
    fn ready_mut(&mut self, lane: Lane, priority: usize) -> &mut VecDeque<RequestId> {
        match lane {
            Lane::Command => &mut self.commands[priority],
            Lane::Inquiry => &mut self.inquiries[priority],
        }
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

    pub(super) fn admit(
        &mut self,
        id: RequestId,
        request: RuntimeRequest,
        facts: AdmissionFacts,
    ) -> bool {
        if self.entries.contains_key(&id) {
            return false;
        }
        let entry = Entry {
            request,
            lifecycle: EntryPlacement::Active(Lifecycle::Ready {
                previous_attempt: None,
            }),
            submitted_at: facts.submitted_at,
            prior: None,
            sequences: SequenceSlots::default(),
            slot: facts.slot,
        };
        let indexed = IndexFacts::of(&entry);
        self.entries.insert(id, entry);
        self.add_indexes(id, &indexed);
        true
    }

    pub(super) fn transition(
        &mut self,
        id: RequestId,
        change: EntryChange,
    ) -> Result<Option<Transitioned>, SocketConflict> {
        let Some(entry) = self.entries.get_mut(&id) else {
            return Ok(None);
        };
        let to = match &change {
            EntryChange::Lifecycle(to) | EntryChange::Retry { to, .. } => *to,
            _ => entry.lifecycle(),
        };
        // Resource admission precedes mutation. Displacement cannot reclaim a socket.
        if matches!(
            change,
            EntryChange::Lifecycle(_) | EntryChange::Retry { .. }
        ) && entry.lifecycle.active().is_some()
        {
            if let Some(socket) = to.socket() {
                let target = entry.request.context().target;
                if let Some(owner) = self.sockets[target][socket].filter(|owner| *owner != id) {
                    return Err(SocketConflict {
                        target,
                        socket,
                        owner,
                    });
                }
            }
        }
        let before = IndexFacts::of(entry);
        let from = entry.lifecycle();
        match change {
            EntryChange::DisplaceSocket => entry.lifecycle = entry.lifecycle.displace(),
            EntryChange::Lifecycle(to) => entry.lifecycle = entry.lifecycle.transition(to),
            EntryChange::Retry { to, prior } => {
                entry.lifecycle = entry.lifecycle.transition(to);
                entry.prior = Some(prior);
            }
            EntryChange::Written { receipt, kind } => {
                if let TransmissionMeta::Sony { sequence } = receipt {
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
        Ok(Some(result))
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
        if let Some((lane, priority)) = facts.ready {
            self.ready_mut(lane, priority).push_back(id);
        }
        if let Some((target, socket)) = facts.socket {
            self.sockets[target][socket] = Some(id);
        }
        for record in facts.sequences.records() {
            self.add_sequence(id, record);
        }
    }
    fn remove_indexes(&mut self, id: RequestId, facts: &IndexFacts) {
        if let Some((lane, priority)) = facts.ready {
            let queue = self.ready_mut(lane, priority);
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
            if let Some((lane, priority)) = before.ready {
                let queue = self.ready_mut(lane, priority);
                if let Some(index) = queue.iter().position(|queued| *queued == id) {
                    queue.remove(index);
                }
            }
            if let Some((lane, priority)) = after.ready {
                self.ready_mut(lane, priority).push_back(id);
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

// Debug count tests instrument every full-table iterator. Release timing uses
// the exact production iterators, with no counter or TLS branch in the hot path.
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
