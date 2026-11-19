//! Bounded owner-ordered motion attribution, shared by halt and settlement.
use crate::{
    target::{PerTarget, TargetSlot},
    AffectedAxes, Error, OperationId, Settlement,
};
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Debug, Default)]
pub(super) struct MotionRegistry {
    // One opaque generation token per registered target/axis. Tokens are the
    // unique ingress ordinals; equality, never numeric order, establishes attribution.
    generations: Mutex<PerTarget<[u64; 5]>>,
}

#[derive(Debug, Clone)]
pub(crate) struct MotionStamp {
    registry: Arc<MotionRegistry>,
    target: TargetSlot,
    axes: AffectedAxes,
    generations: [u64; 5],
}

fn stamp(bank: &mut PerTarget<[u64; 5]>, target: TargetSlot, axes: AffectedAxes, token: u64) {
    let current = &mut bank[target];
    for (index, value) in current.iter_mut().enumerate() {
        if axes.bits() & (1 << index) != 0 {
            *value = token;
        }
    }
}

fn superseded(bank: &PerTarget<[u64; 5]>, stamp: &MotionStamp) -> bool {
    bank[stamp.target]
        .iter()
        .zip(stamp.generations)
        .enumerate()
        .any(|(index, (now, then))| stamp.axes.bits() & (1 << index) != 0 && *now != then)
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
            generations: bank[target],
        }
    }
}

/// The generations lock may cover the synchronous admission verdict only.
/// Consuming this guard publishes successful admission and releases the lock
/// before owner bookkeeping, due work or dispatch can begin.
pub(super) struct MotionAdmission<'a> {
    registry: &'a Arc<MotionRegistry>,
    bank: MutexGuard<'a, PerTarget<[u64; 5]>>,
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
            generations: self.bank[self.target],
        };
        Ok((id, stamp))
    }
}

impl MotionStamp {
    /// Comparison and successful evidence caching share admission's lock.
    /// Once cached, later admissions cannot retroactively invalidate evidence.
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
