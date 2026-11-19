//! Bounded owner-ordered motion attribution, shared by halt and settlement.
use crate::{
    target::{PerTarget, TargetSlot},
    AffectedAxes, Error, OperationId, Settlement,
};
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Debug, Default)]
pub(super) struct MotionRegistry {
    // Each target/axis tracks the current admission and whether it has written.
    // The unique ingress ordinal identifies that admission.
    generations: Mutex<PerTarget<[AxisGeneration; 5]>>,
}

#[derive(Debug, Clone, Copy, Default)]
enum AxisGeneration {
    #[default]
    Unattributed,
    Admitted(u64),
    Written(u64),
}

#[derive(Debug, Clone)]
pub(crate) struct MotionStamp {
    registry: Arc<MotionRegistry>,
    target: TargetSlot,
    axes: AffectedAxes,
    token: u64,
}

fn stamp(
    bank: &mut PerTarget<[AxisGeneration; 5]>,
    target: TargetSlot,
    axes: AffectedAxes,
    token: u64,
) {
    for (index, generation) in bank[target].iter_mut().enumerate() {
        if axes.bits() & (1 << index) != 0 {
            *generation = AxisGeneration::Admitted(token);
        }
    }
}

fn superseded(bank: &PerTarget<[AxisGeneration; 5]>, stamp: &MotionStamp) -> bool {
    bank[stamp.target]
        .iter()
        .enumerate()
        .any(|(index, generation)| {
            stamp.axes.bits() & (1 << index) != 0
                && !matches!(
                    generation,
                    AxisGeneration::Admitted(token) | AxisGeneration::Written(token)
                        if *token == stamp.token
                )
        })
}

impl MotionRegistry {
    /// Hold only the admission/generation boundary against settlement commits.
    pub(super) fn begin_admission(
        self: &Arc<Self>,
        target: TargetSlot,
        axes: AffectedAxes,
        token: u64,
    ) -> MotionAdmission<'_> {
        MotionAdmission {
            registry: self,
            bank: self
                .generations
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            target,
            axes,
            token,
        }
    }

    pub(super) fn fence(&self, target: TargetSlot, axes: AffectedAxes, token: u64) {
        let mut bank = self
            .generations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        stamp(&mut bank, target, axes, token);
    }

    /// Record a matched transport receipt whose bytes may have reached the
    /// camera. A conflicting write before the current operation writes cannot
    /// invalidate its proof. A conflict afterward clears attribution without
    /// reviving the older writer or letting a retry restore the invalidated stamp.
    pub(super) fn possibly_written(&self, target: TargetSlot, axes: AffectedAxes, token: u64) {
        let mut bank = self
            .generations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for (index, generation) in bank[target].iter_mut().enumerate() {
            if axes.bits() & (1 << index) != 0 {
                *generation = match *generation {
                    AxisGeneration::Admitted(current) if current == token => {
                        AxisGeneration::Written(token)
                    }
                    AxisGeneration::Written(current) if current != token => {
                        AxisGeneration::Unattributed
                    }
                    state => state,
                };
            }
        }
    }

    #[cfg(test)]
    pub(super) fn establish(
        self: &Arc<Self>,
        target: TargetSlot,
        axes: AffectedAxes,
        token: u64,
    ) -> MotionStamp {
        let mut bank = self
            .generations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        stamp(&mut bank, target, axes, token);
        MotionStamp {
            registry: Arc::clone(self),
            target,
            axes,
            token,
        }
    }
}

/// The generations lock may cover the synchronous admission verdict only.
/// Consuming this guard publishes successful admission and releases the lock
/// before owner bookkeeping, due work or dispatch can begin.
pub(super) struct MotionAdmission<'a> {
    registry: &'a Arc<MotionRegistry>,
    bank: MutexGuard<'a, PerTarget<[AxisGeneration; 5]>>,
    target: TargetSlot,
    axes: AffectedAxes,
    token: u64,
}

impl MotionAdmission<'_> {
    pub(super) fn admit<T>(mut self, verdict: Result<T, Error>) -> Result<T, Error> {
        let id = verdict?;
        self.publish();
        Ok(id)
    }

    fn publish(&mut self) {
        stamp(&mut self.bank, self.target, self.axes, self.token)
    }

    pub(super) fn admit_operation<T>(
        mut self,
        verdict: Result<T, Error>,
    ) -> Result<(T, MotionStamp), Error> {
        let id = verdict?;
        self.publish();
        let stamp = MotionStamp {
            registry: Arc::clone(self.registry),
            target: self.target,
            axes: self.axes,
            token: self.token,
        };
        Ok((id, stamp))
    }
}

impl MotionStamp {
    /// Comparison and evidence caching share the admission and write lock.
    /// Cached evidence survives conflicting admissions and writes.
    pub(crate) fn commit(
        &self,
        id: OperationId,
        result: Result<Settlement, Error>,
        cached: &mut Option<Result<Settlement, Error>>,
    ) -> Result<Settlement, Error> {
        let bank = self
            .registry
            .generations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if superseded(&bank, self) {
            let error = Error::SettlementSuperseded {
                operation: id,
                axes: self.axes,
            };
            *cached = Some(Err(error.clone()));
            return Err(error);
        }
        if let Ok(evidence) = result {
            *cached = Some(Ok(evidence));
        }
        result
    }

    pub(crate) fn check(&self, id: OperationId) -> Result<(), Error> {
        let bank = self
            .registry
            .generations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if superseded(&bank, self) {
            Err(Error::SettlementSuperseded {
                operation: id,
                axes: self.axes,
            })
        } else {
            Ok(())
        }
    }
}

/// Settlement evidence owned only by a targeted receipt. The stamp supplies
/// the affected axes used for both attribution and the public verdict.
#[derive(Debug)]
pub(crate) struct TargetedObservation {
    motion: MotionStamp,
    pub(super) settled: Option<Result<Settlement, Error>>,
}

impl TargetedObservation {
    pub(super) fn new(motion: MotionStamp) -> Self {
        Self {
            motion,
            settled: None,
        }
    }

    pub(super) fn axes(&self) -> AffectedAxes {
        self.motion.axes
    }

    pub(super) fn check(&self, id: OperationId) -> Result<(), Error> {
        self.motion.check(id)
    }

    pub(super) fn commit_position_observation(
        &mut self,
        id: OperationId,
        result: Result<Settlement, Error>,
    ) -> Result<Settlement, Error> {
        self.motion.commit(id, result, &mut self.settled)
    }

    pub(super) fn commit_completion(&mut self, evidence: Settlement) -> Settlement {
        self.settled = Some(Ok(evidence));
        evidence
    }
}

#[derive(Debug)]
pub(crate) struct Admitted {
    pub(crate) identity: super::AdmittedId,
}
/// A successful targeted admission owns its exact motion generation.
#[derive(Debug)]
pub(crate) struct AdmittedOperation {
    pub(super) admitted: Admitted,
    pub(super) motion: MotionStamp,
}

impl Admitted {
    #[cfg(test)]
    pub(crate) fn get(&self) -> u64 {
        self.id().get()
    }

    #[cfg(test)]
    pub(crate) const fn id(&self) -> super::RequestId {
        self.identity.request_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_and_check_agree_for_every_axis_subset_target_and_generation() -> crate::Result<()> {
        let id = OperationId::from_raw(1);
        for original_bits in 1..=31 {
            let original_axes = AffectedAxes::from_bits(original_bits)?;
            for changed_bits in 1..=31 {
                let changed_axes = AffectedAxes::from_bits(changed_bits)?;
                for changed_target in [TargetSlot::CAMERA_1, TargetSlot::CAMERA_2] {
                    // Tokens are identities: a numerically smaller replacement
                    // supersedes just as a larger token does.
                    for token in [7, 3, 11] {
                        let registry = Arc::new(MotionRegistry::default());
                        let original = registry.establish(TargetSlot::CAMERA_1, original_axes, 7);
                        registry
                            .begin_admission(changed_target, changed_axes, token)
                            .admit(Ok(()))?;
                        let expected = changed_target == TargetSlot::CAMERA_1
                            && original_bits & changed_bits != 0
                            && token != 7;
                        assert_eq!(original.check(id).is_err(), expected);
                        let evidence = Settlement::profile_completion(original_axes);
                        let mut cached = None;
                        let result = original.commit(id, Ok(evidence), &mut cached);
                        assert_eq!(result.is_err(), expected);
                        if expected {
                            assert!(matches!(
                                result,
                                Err(Error::SettlementSuperseded { operation, axes })
                                    if operation == id && axes == original_axes
                            ));
                            assert!(matches!(
                                cached,
                                Some(Err(Error::SettlementSuperseded { .. }))
                            ));
                        } else {
                            assert_eq!(result?, evidence);
                            assert!(matches!(cached, Some(Ok(value)) if value == evidence));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn only_an_overlapping_write_after_the_current_write_can_break_the_proof() {
        let registry = Arc::new(MotionRegistry::default());
        let target = TargetSlot::CAMERA_1;
        let axes = AffectedAxes::MOVEMENT;
        let old = registry.establish(target, AffectedAxes::ZOOM, 1);
        let mut preset = TargetedObservation::new(registry.establish(target, axes, 2));
        let id = OperationId::from_raw(2);

        // Priority or a retry can dispatch work out of admission order.
        registry.possibly_written(target, AffectedAxes::ZOOM, 1);
        assert!(preset.check(id).is_ok(), "the preset has not written yet");
        registry.possibly_written(target, axes, 2);
        registry.possibly_written(TargetSlot::CAMERA_2, AffectedAxes::ZOOM, 1);
        registry.possibly_written(target, AffectedAxes::IRIS, 1);
        assert!(preset.check(id).is_ok(), "disjoint targets and axes");
        let samples = Settlement::observed_stable(
            axes,
            std::time::Duration::from_millis(25),
            crate::camera::MovementTolerance::default(),
        );

        // The final sample has passed its pre-inquiry check and returned,
        // but the overlapping write wins the lock before the proof commits.
        registry.possibly_written(target, AffectedAxes::ZOOM, 1);
        assert!(matches!(
            preset.commit_position_observation(id, Ok(samples)),
            Err(Error::SettlementSuperseded { operation, axes: affected })
                if operation == id && affected == axes
        ));
        registry.possibly_written(target, axes, 2);
        assert!(
            preset.check(id).is_err(),
            "a later retry cannot revive a proof"
        );
        assert!(
            old.check(OperationId::from_raw(1)).is_err(),
            "an out-of-order write cannot revive the older admission"
        );
    }

    #[test]
    fn engine_refusal_releases_the_motion_lock_without_advancing_generations() {
        let registry = Arc::new(MotionRegistry::default());
        let target = TargetSlot::CAMERA_1;
        let original = registry.establish(target, AffectedAxes::ZOOM, 1);
        let admission = registry.begin_admission(target, AffectedAxes::ZOOM, 2);
        assert!(registry.generations.try_lock().is_err());
        assert!(matches!(
            admission.admit::<()>(Err(Error::RuntimeQueueFull { capacity: 1 })),
            Err(Error::RuntimeQueueFull { capacity: 1 })
        ));
        assert!(registry.generations.try_lock().is_ok());
        assert!(original.check(OperationId::from_raw(1)).is_ok());
    }
}
