//! Bounded owner-ordered motion attribution, shared by halt and settlement.
use crate::{AffectedAxes, CameraId, Error, OperationId, Settlement};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
pub(super) struct MotionRegistry {
    // One opaque generation token per registered target/axis. Tokens are the
    // unique ingress ordinals; equality, never numeric order, establishes attribution.
    generations: Mutex<[[u64; 5]; 9]>,
}

#[derive(Debug, Clone)]
pub(crate) struct MotionStamp {
    registry: Arc<MotionRegistry>,
    target: CameraId,
    axes: AffectedAxes,
    generations: [u64; 5],
}

impl MotionRegistry {
    /// Engine admission and generation publication are one boundary against
    /// settlement commits, even before the owner drains the Admitted effect.
    pub(super) fn admit(
        self: &Arc<Self>,
        target: CameraId,
        axes: AffectedAxes,
        token: u64,
        admit: impl FnOnce() -> Vec<super::Effect>,
    ) -> (Vec<super::Effect>, Option<MotionStamp>) {
        let mut bank = self
            .generations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let effects = admit();
        let stamp = effects
            .iter()
            .any(|effect| matches!(effect, super::Effect::Admitted { .. }))
            .then(|| {
                let current = &mut bank[usize::from(target.id())];
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
            });
        (effects, stamp)
    }

    pub(super) fn establish(
        self: &Arc<Self>,
        target: CameraId,
        axes: AffectedAxes,
        token: u64,
    ) -> MotionStamp {
        let mut bank = self
            .generations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let current = &mut bank[usize::from(target.id())];
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
        let current = bank[usize::from(self.target.id())];
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
        let current = bank[usize::from(self.target.id())];
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
