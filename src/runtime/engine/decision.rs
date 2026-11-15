//! Boundary-local effects and deferred frame evidence, returned by value.
//!
//! A decision never survives an external-input or scheduler boundary. Each
//! ledger target can settle its one tracked dispute, and the current frame
//! can additionally settle the two ambiguous socketless candidates.

use smallvec::SmallVec;

use super::{
    stream_ledger::{Retro, RetroFrames, MAX_RETRO_FRAMES},
    Effect, RequestId, TargetSlot,
};

const MAX_TURN_RETRO_FRAMES: usize = TargetSlot::COUNT * MAX_RETRO_FRAMES + 2;
const MAX_DISPLACED_OWNERS: usize = TargetSlot::COUNT * 2;

#[derive(Debug, Default)]
pub(super) struct DeferredEvidence {
    pub(super) retro: SmallVec<[Retro; MAX_TURN_RETRO_FRAMES]>,
    pub(super) displaced: SmallVec<[RequestId; MAX_DISPLACED_OWNERS]>,
}
impl DeferredEvidence {
    pub(super) fn is_empty(&self) -> bool {
        self.retro.is_empty() && self.displaced.is_empty()
    }
}

#[derive(Debug, Default)]
pub(super) struct Decision {
    effects: Vec<Effect>,
    deferred: DeferredEvidence,
}
impl Decision {
    pub(super) fn new() -> Self {
        Self::default()
    }
    pub(super) fn insert(&mut self, index: usize, effect: Effect) {
        self.effects.insert(index, effect);
    }
    pub(super) fn has_deferred(&self) -> bool {
        !self.deferred.is_empty()
    }
    pub(super) fn clear_deferred(&mut self) {
        self.deferred = DeferredEvidence::default();
    }
    pub(super) fn push(&mut self, effect: Effect) {
        self.effects.push(effect);
    }
    pub(super) fn len(&self) -> usize {
        self.effects.len()
    }
    pub(super) fn as_slice(&self) -> &[Effect] {
        &self.effects
    }
    pub(super) fn defer_frames(&mut self, frames: RetroFrames) {
        self.deferred.retro.extend(frames);
    }
    pub(super) fn defer_frame(&mut self, frame: Retro) {
        self.deferred.retro.push(frame);
    }
    pub(super) fn displace(&mut self, request: RequestId) {
        if !self.deferred.displaced.contains(&request) {
            self.deferred.displaced.push(request);
        }
    }
    pub(super) fn take_deferred(&mut self) -> DeferredEvidence {
        std::mem::take(&mut self.deferred)
    }

    /// The caller completes deferred evidence before extracting outward effects.
    /// Keep extraction at the engine's single finalization method.
    pub(super) fn into_effects(self) -> Vec<Effect> {
        self.effects
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::runtime::engine::{stream_ledger::Evidence, GenerationTicket};

    #[test]
    fn all_target_disputes_and_socketless_candidates_remain_inline() {
        let mut decision = Decision::new();
        for index in 1..=MAX_TURN_RETRO_FRAMES {
            decision.defer_frame(Retro {
                request: RequestId::from_nonzero(
                    std::num::NonZeroU64::new(index as u64).expect("bounded nonzero index"),
                ),
                generation: GenerationTicket(1),
                evidence: Evidence::Error(0x41),
            });
        }
        let deferred = decision.take_deferred();
        assert_eq!(deferred.retro.len(), 51);
        assert!(!deferred.retro.spilled());
        assert!(decision.take_deferred().is_empty());
    }
}
