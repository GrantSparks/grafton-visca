//! Preset methods for cameras using the new GAT architecture.

use crate::{command::preset::PresetNumber, impl_camera_ops, Error};

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

// Use macro to generate implementations
impl_camera_ops!(
    async,
    PresetsOps,
    async fn preset_recall(&self, preset: PresetNumber) -> Result<(), Error>;
    async fn preset_set(&self, preset: PresetNumber) -> Result<(), Error>;
    async fn preset_reset(&self, preset: PresetNumber) -> Result<(), Error>;
);

impl_camera_ops!(
    blocking,
    PresetsOpsBlocking,
    fn preset_recall(&self, preset: PresetNumber) -> Result<(), Error>;
    fn preset_set(&self, preset: PresetNumber) -> Result<(), Error>;
    fn preset_reset(&self, preset: PresetNumber) -> Result<(), Error>;
);
