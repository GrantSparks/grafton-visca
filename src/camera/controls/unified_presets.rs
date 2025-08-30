//! Mode-parametrized presets control trait using the Mode trait system.

use crate::{
    command::preset::{PresetAction, PresetCommand, PresetNumber},
    mode::Mode,
    Error,
};

/// Unified presets control trait that works with both blocking and async modes.
///
/// This trait uses the Mode trait system to provide a single API surface
/// that works correctly in both blocking and async contexts. The return
/// types adapt automatically based on the Mode parameter.
///
/// # Examples
///
/// ```rust,ignore
/// use grafton_visca::{Camera, mode::{Async, Blocking}};
/// use grafton_visca::camera::controls::unified_presets::UnifiedPresetsControl;
/// use grafton_visca::command::preset::PresetNumber;
///
/// // Async usage
/// let async_camera: Camera<Async, Profile, Transport, Executor> = ...;
/// let preset = PresetNumber::new(1)?;
/// async_camera.preset_recall(preset).await?; // Returns a future
///
/// // Blocking usage  
/// let blocking_camera: Camera<Blocking, Profile, Transport, ()> = ...;
/// let preset = PresetNumber::new(1)?;
/// blocking_camera.preset_recall(preset).await?; // Returns immediately via Ready<T>
/// ```
pub trait UnifiedPresetsControl<M>
where
    M: Mode,
{
    /// Recall a preset position.
    fn preset_recall(&self, preset: PresetNumber) -> M::Ret<Result<(), Error>>;

    /// Set current position as a preset.
    fn preset_set(&self, preset: PresetNumber) -> M::Ret<Result<(), Error>>;

    /// Reset/clear a preset.
    fn preset_reset(&self, preset: PresetNumber) -> M::Ret<Result<(), Error>>;
}

// Implementation for the unified camera type
impl<M, P, Tr, Exec> UnifiedPresetsControl<M> for crate::camera::unified::Camera<M, P, Tr, Exec>
where
    M: Mode + 'static,
    P: crate::capabilities::Profile + Default,
    Tr: Send + Sync,
{
    fn preset_recall(&self, preset: PresetNumber) -> M::Ret<Result<(), Error>> {
        let cmd = PresetCommand {
            action: PresetAction::Recall,
            preset_number: preset,
        };
        self.send_command(&cmd)
    }

    fn preset_set(&self, preset: PresetNumber) -> M::Ret<Result<(), Error>> {
        let cmd = PresetCommand {
            action: PresetAction::Set,
            preset_number: preset,
        };
        self.send_command(&cmd)
    }

    fn preset_reset(&self, preset: PresetNumber) -> M::Ret<Result<(), Error>> {
        let cmd = PresetCommand {
            action: PresetAction::Reset,
            preset_number: preset,
        };
        self.send_command(&cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::{Async, Blocking};

    #[tokio::test]
    async fn test_unified_presets_control_concept() {
        // This test demonstrates the concept - actual implementation would need
        // real camera instances

        // The key insight is that both async and blocking modes can be awaited:
        // - Async returns actual futures
        // - Blocking returns Ready<T> which immediately resolves

        // Example usage (conceptual):
        // let preset = PresetNumber::new(1)?;
        // async_camera.preset_recall(preset).await?;
        // blocking_camera.preset_recall(preset).await?;
        // Both work with the same method signature!
    }

    #[test]
    fn test_preset_number_construction() {
        // Test that PresetNumber can be constructed properly
        let preset = PresetNumber::new(1);
        assert!(preset.is_ok());

        // Preset 0 is actually valid in the current implementation
        let preset = PresetNumber::new(0);
        assert!(preset.is_ok());

        let preset = PresetNumber::new(255);
        assert!(preset.is_ok());

        // Note: PresetNumber::new takes a u8, so values above 255 would cause a compile error
        // All u8 values (0-255) are valid preset numbers
    }

    #[test]
    fn test_mode_markers_are_zero_sized() {
        use std::mem::size_of;
        assert_eq!(size_of::<Async>(), 0);
        assert_eq!(size_of::<Blocking>(), 0);
    }
}
