//! High-level extension trait for preset management operations.

// Crate imports
use crate::{
    command::preset::{PresetAction, PresetCommand, PresetNumber},
    error::ViscaError,
    ViscaDevice, ViscaResponse,
};

/// Extension trait providing high-level preset management methods.
pub trait ViscaPresetExt: ViscaDevice {
    /// Save the current camera position (pan/tilt/zoom/focus) to a preset.
    ///
    /// # Arguments
    /// * `preset_number` - Preset slot number (0-254)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaPresetExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Position camera as desired, then save to preset 1
    /// client.save_preset(1)?;
    ///
    /// // Save another position to preset 2
    /// client.save_preset(2)?;
    /// # Ok(())
    /// # }
    /// ```
    fn save_preset(&mut self, preset_number: u8) -> Result<(), ViscaError> {
        let command = PresetCommand {
            action: PresetAction::Set,
            preset_number: PresetNumber::new(preset_number)?,
        };
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Recall a saved preset position.
    ///
    /// The camera will move to the saved pan/tilt/zoom/focus position.
    ///
    /// # Arguments
    /// * `preset_number` - Preset slot number (0-254)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaPresetExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Return to preset position 1
    /// client.recall_preset(1)?;
    ///
    /// // Move to home position (preset 0 is often home)
    /// client.recall_preset(0)?;
    /// # Ok(())
    /// # }
    /// ```
    fn recall_preset(&mut self, preset_number: u8) -> Result<(), ViscaError> {
        let command = PresetCommand {
            action: PresetAction::Recall,
            preset_number: PresetNumber::new(preset_number)?,
        };
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Reset a preset to its default state.
    ///
    /// # Arguments
    /// * `preset_number` - Preset slot number (0-254)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaPresetExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Clear preset 1
    /// client.reset_preset(1)?;
    ///
    /// // Reset all presets (preset 255 typically means all)
    /// client.reset_preset(255)?;
    /// # Ok(())
    /// # }
    /// ```
    fn reset_preset(&mut self, preset_number: u8) -> Result<(), ViscaError> {
        let command = PresetCommand {
            action: PresetAction::Reset,
            preset_number: PresetNumber::new(preset_number)?,
        };
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }
}

/// Implement the trait for all types that implement `ViscaTransportExt`
impl<T: ViscaDevice> ViscaPresetExt for T {}
