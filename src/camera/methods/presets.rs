//! Preset methods for cameras using the new GAT architecture.

use crate::{command::preset::PresetNumber, Error};

/// Presets operations (async).
#[cfg(feature = "async")]
pub trait PresetsOps: Sized {
    /// Recall a preset position.
    async fn preset_recall(&self, preset: PresetNumber) -> Result<(), Error>;

    /// Set current position as a preset.
    async fn preset_set(&self, preset: PresetNumber) -> Result<(), Error>;

    /// Reset/clear a preset.
    async fn preset_reset(&self, preset: PresetNumber) -> Result<(), Error>;
}

/// Presets operations (blocking).
#[cfg(not(feature = "async"))]
pub trait PresetsOpsBlocking: Sized {
    /// Recall a preset position.
    fn preset_recall(&self, preset: PresetNumber) -> Result<(), Error>;

    /// Set current position as a preset.
    fn preset_set(&self, preset: PresetNumber) -> Result<(), Error>;

    /// Reset/clear a preset.
    fn preset_reset(&self, preset: PresetNumber) -> Result<(), Error>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T> PresetsOps for crate::camera::Camera<crate::camera::AsyncMode, P, T>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + 'static,
{
    async fn preset_recall(&self, preset: PresetNumber) -> Result<(), Error> {
        self.preset_recall(preset).await
    }

    async fn preset_set(&self, preset: PresetNumber) -> Result<(), Error> {
        self.preset_set(preset).await
    }

    async fn preset_reset(&self, preset: PresetNumber) -> Result<(), Error> {
        self.preset_reset(preset).await
    }
}

// Blocking implementation for Camera with BlockingMode
#[cfg(not(feature = "async"))]
impl<P, T> PresetsOpsBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T>
where
    P: crate::capabilities::Profile,
    T: crate::transport::BlockingTransport,
{
    fn preset_recall(&self, preset: PresetNumber) -> Result<(), Error> {
        self.preset_recall(preset)
    }

    fn preset_set(&self, preset: PresetNumber) -> Result<(), Error> {
        self.preset_set(preset)
    }

    fn preset_reset(&self, preset: PresetNumber) -> Result<(), Error> {
        self.preset_reset(preset)
    }
}
