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
    AdmissionSlot, CorrelationKind, CorrelationOwner, Entry, Error, GenerationTicket, Lane,
    Lifecycle, Phase, RequestId, RuntimeRequest, SequenceRecord, TargetSlot, TransmissionMeta,
    ViscaSocket, MAX_SEQUENCE_HISTORY,
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
    pub(super) order: u64,
    pub(super) submitted_at: Instant,
    pub(super) generation: GenerationTicket,
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
        attempt: u32,
        error: Error,
        effect_possible: bool,
    },
    PossibleEffect,
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

#[derive(Clone)]
struct IndexFacts {
    ready: Option<(Lane, usize)>,
    socket: Option<(TargetSlot, ViscaSocket)>,
    history: SmallVec<[SequenceRecord; MAX_SEQUENCE_HISTORY]>,
}
impl IndexFacts {
    fn of(entry: &Entry) -> Self {
        Self {
            ready: matches!(entry.lifecycle().phase(), Phase::Ready).then(|| {
                (
                    entry.request.lane(),
                    entry.request.context().control.class.priority_index(),
                )
            }),
            socket: entry
                .lifecycle
                .active_socket()
                .map(|socket| (entry.request.context().target, socket)),
            history: entry
                .sequence_history
                .iter()
                .copied()
                .filter(|record| {
                    entry.lifecycle.active().is_some()
                        && super::correlation_phase_compatible(entry, record.kind)
                })
                .collect(),
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
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }
    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub(super) fn get(&self, id: &RequestId) -> Option<&Entry> {
        self.entries.get(id)
    }
    pub(super) fn iter(&self) -> btree_map::Iter<'_, RequestId, Entry> {
        self.entries.iter()
    }
    pub(super) fn values(&self) -> btree_map::Values<'_, RequestId, Entry> {
        self.entries.values()
    }
    pub(super) fn active_entries(&self) -> impl Iterator<Item = (&RequestId, &Entry)> {
        self.entries
            .iter()
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
            admission_order: facts.order,
            submitted_at: facts.submitted_at,
            attempt: 0,
            last_error: None,
            generation: facts.generation,
            current_sequence: None,
            sequence_history: SmallVec::new(),
            slot: facts.slot,
            effect_possible: false,
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
            EntryChange::Retry {
                to,
                attempt,
                error,
                effect_possible,
            } => {
                entry.lifecycle = entry.lifecycle.transition(to);
                entry.attempt = attempt;
                entry.last_error = Some(error);
                entry.effect_possible = effect_possible;
            }
            EntryChange::Written { receipt, kind } => {
                if let TransmissionMeta::Sony { sequence } = receipt {
                    if kind == CorrelationKind::Request {
                        entry.current_sequence = Some(sequence);
                    }
                    let record = SequenceRecord { sequence, kind };
                    if !entry.sequence_history.contains(&record) {
                        if entry.sequence_history.len() == MAX_SEQUENCE_HISTORY {
                            entry.sequence_history.remove(0);
                        }
                        entry.sequence_history.push(record);
                    }
                }
            }
            EntryChange::PossibleEffect => {
                entry.effect_possible |= entry.reply_family() != super::SendingReply::Inquiry
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
        for record in &facts.history {
            self.add_sequence(id, *record);
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
        for record in &facts.history {
            self.remove_sequence(id, *record, &[]);
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
        for record in &before.history {
            if !after.history.contains(record) {
                self.remove_sequence(id, *record, &after.history);
            }
        }
        for record in &after.history {
            if !before.history.contains(record) {
                self.add_sequence(id, *record);
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
    fn remove_sequence(
        &mut self,
        id: RequestId,
        record: SequenceRecord,
        retained: &[SequenceRecord],
    ) {
        let matches =
            |owner: &CorrelationOwner| !(owner.request == id && owner.kind == record.kind);
        if let Some(owners) = self.sequences.get_mut(&record.sequence) {
            owners.retain(|owner| matches(owner));
            if owners.is_empty() {
                self.sequences.remove(&record.sequence);
            }
        }
        let lower = record.sequence as u16;
        if retained
            .iter()
            .any(|other| other.kind == record.kind && other.sequence as u16 == lower)
        {
            return;
        }
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
    type IntoIter = btree_map::Iter<'a, RequestId, Entry>;
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
