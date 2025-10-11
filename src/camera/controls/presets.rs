//! Preset control implementation for PTZ cameras.
//!
//! This module provides preset position management functionality including:
//! - Storing camera positions (pan, tilt, zoom, focus, etc.) as numbered presets
//! - Recalling stored presets to quickly move to predefined positions
//! - Clearing/resetting preset slots for reuse
//! - Support for multiple preset slots (typically 0-255 depending on camera)
//!
//! Presets are a fundamental feature of PTZ cameras that allow operators to
//! quickly move between commonly used camera positions. Each preset stores
//! the complete camera state including pan/tilt position, zoom level,
//! focus position, and potentially other camera settings.
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{
    camera::ViscaClient,
    command::preset::{PresetAction, PresetCommand, PresetNumber},
    mode::Mode,
    Error,
};

/// Preset operations for PTZ cameras.
///
/// This trait provides preset control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # Preset Management
///
/// Presets store the complete camera state and allow quick positioning:
/// - **Set**: Save current camera position to a preset slot
/// - **Recall**: Move camera to a previously saved preset position
/// - **Reset**: Clear a preset slot to free it for reuse
///
/// # Preset Numbers
///
/// Most cameras support multiple preset slots (commonly 0-255), though the
/// exact range varies by camera model. Check your camera documentation for
/// the supported preset range.
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// // Save current position as preset 1
/// camera.preset_set(PresetNumber::new(1)?)?;
///
/// // Move to preset 1
/// camera.preset_recall(PresetNumber::new(1)?)?;
///
/// // Clear preset 1
/// camera.preset_reset(PresetNumber::new(1)?)?;
/// ```
///
/// ## Async mode
/// ```ignore
/// // Save current position as preset 1
/// camera.preset_set(PresetNumber::new(1)?).await?;
///
/// // Move to preset 1
/// camera.preset_recall(PresetNumber::new(1)?).await?;
///
/// // Clear preset 1
/// camera.preset_reset(PresetNumber::new(1)?).await?;
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait PresetsControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Recall a preset position.
    ///
    /// Moves the camera to a previously saved preset position. This command
    /// will restore all saved camera parameters including pan, tilt, zoom,
    /// focus, and other settings that were stored with the preset.
    ///
    /// # Parameters
    /// - `preset`: The preset number to recall
    ///
    /// # Errors
    /// Returns an error if the preset number is invalid, the preset is empty,
    /// or the command fails to send or receive a response.
    fn preset_recall(
        &self,
        preset: PresetNumber,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set current position as a preset.
    ///
    /// Saves the current camera state (pan, tilt, zoom, focus, and other settings)
    /// to the specified preset slot. This will overwrite any existing preset
    /// stored in that slot.
    ///
    /// # Parameters
    /// - `preset`: The preset number to store the current position in
    ///
    /// # Errors
    /// Returns an error if the preset number is invalid or the command
    /// fails to send or receive a response.
    fn preset_set(
        &self,
        preset: PresetNumber,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Reset/clear a preset.
    ///
    /// Clears the specified preset slot, making it empty and available for reuse.
    /// After resetting, the preset will no longer contain any saved position data.
    ///
    /// # Parameters
    /// - `preset`: The preset number to reset/clear
    ///
    /// # Errors
    /// Returns an error if the preset number is invalid or the command
    /// fails to send or receive a response.
    fn preset_reset(
        &self,
        preset: PresetNumber,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> PresetsControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn preset_recall(
        &self,
        preset: PresetNumber,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        let cmd = PresetCommand {
            action: PresetAction::Recall,
            preset_number: preset,
        };
        self.execute_with_opts(cmd, opts)
    }

    fn preset_set(
        &self,
        preset: PresetNumber,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        let cmd = PresetCommand {
            action: PresetAction::Set,
            preset_number: preset,
        };
        self.execute_with_opts(cmd, opts)
    }

    fn preset_reset(
        &self,
        preset: PresetNumber,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        let cmd = PresetCommand {
            action: PresetAction::Reset,
            preset_number: preset,
        };
        self.execute_with_opts(cmd, opts)
    }
}
