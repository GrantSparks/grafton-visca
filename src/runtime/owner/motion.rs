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
        let current = &mut bank[target];
        for (index, value) in current.iter_mut().enumerate() {
            if axes.bits() & (1 << index) != 0 {
                *value = token;
            }
        }
        MotionStamp {
            registry: Arc::clone(self),
            target,
            axes,
            generations: *current,
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
    pub(super) fn admit(
        mut self,
        verdict: Result<crate::runtime::engine::RequestId, Error>,
    ) -> Result<(crate::runtime::engine::RequestId, MotionStamp), Error> {
        let id = verdict?;
        let current = &mut self.bank[self.target];
        for (index, value) in current.iter_mut().enumerate() {
            if self.axes.bits() & (1 << index) != 0 {
                *value = self.token;
            }
        }
        let stamp = MotionStamp {
            registry: Arc::clone(self.registry),
            target: self.target,
            axes: self.axes,
            generations: *current,
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
        let current = bank[self.target];
        let superseded = current
            .iter()
            .zip(self.generations)
            .enumerate()
            .any(|(index, (now, then))| self.axes.bits() & (1 << index) != 0 && *now != then);
        if superseded {
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
        let current = bank[self.target];
        if current
            .iter()
            .zip(self.generations)
            .enumerate()
            .any(|(index, (now, then))| self.axes.bits() & (1 << index) != 0 && *now != then)
        {
            Err(Error::SettlementSuperseded {
                operation: id,
                axes: self.axes,
            })
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
pub(crate) struct Admitted {
    pub(crate) id: super::RequestId,
    pub(crate) motion: Option<MotionStamp>,
}
impl Admitted {
    #[cfg(test)]
    pub(crate) fn get(&self) -> u64 {
        self.id.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_refusal_releases_the_motion_lock_without_advancing_generations() {
        let registry = Arc::new(MotionRegistry::default());
        let target = TargetSlot::CAMERA_1;
        let original = registry.establish(target, AffectedAxes::ZOOM, 1);
        let admission = registry.begin_admission(target, AffectedAxes::ZOOM, 2);
        assert!(registry.generations.try_lock().is_err());
        assert!(matches!(
            admission.admit(Err(Error::RuntimeQueueFull { capacity: 1 })),
            Err(Error::RuntimeQueueFull { capacity: 1 })
        ));
        assert!(registry.generations.try_lock().is_ok());
        assert!(original.check(OperationId::from_raw(1)).is_ok());
    }
}
