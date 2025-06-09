//! High-level extension trait for preset management operations.

// Crate imports
use crate::{
    command::preset::{PresetAction, PresetCommand, PresetNumber},
    error::Error as ViscaError,
    Response, Transport,
};

/// Extension trait providing high-level preset management methods.
pub trait ViscaPresetExt: Transport {
    /// Set (save) the current camera position to a preset using a simple u8 preset number.
    ///
    /// This is a convenience method that accepts a u8 instead of `PresetNumber`.
    ///
    /// # Arguments
    /// * `preset_id` - The preset number to save (typically 0-89)
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if `preset_id` is invalid,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaPresetExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Save current position to preset 1
    /// client.set_preset(1)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_preset(&mut self, preset_id: u8) -> Result<(), ViscaError> {
        let preset_number = PresetNumber::new(preset_id)?;
        self.save_preset_number(preset_number)
    }

    /// Recall a saved preset position using a simple u8 preset number.
    ///
    /// This is a convenience method that accepts a u8 instead of `PresetNumber`.
    ///
    /// # Arguments
    /// * `preset_id` - The preset number to recall (typically 0-89)
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if `preset_id` is invalid,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaPresetExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Return to preset position 1
    /// client.recall_preset(1)?;
    /// # Ok(())
    /// # }
    /// ```
    fn recall_preset(&mut self, preset_id: u8) -> Result<(), ViscaError> {
        let preset_number = PresetNumber::new(preset_id)?;
        self.recall_preset_number(preset_number)
    }

    /// Reset a preset to its default state using a simple u8 preset number.
    ///
    /// This is a convenience method that accepts a u8 instead of `PresetNumber`.
    ///
    /// # Arguments
    /// * `preset_id` - The preset number to reset (typically 0-89)
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if `preset_id` is invalid,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    fn reset_preset(&mut self, preset_id: u8) -> Result<(), ViscaError> {
        let preset_number = PresetNumber::new(preset_id)?;
        self.reset_preset_number(preset_number)
    }
    /// Save the current camera position (pan/tilt/zoom/focus) to a preset.
    ///
    /// # Arguments
    /// * `preset_number` - Preset slot number
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaPresetExt, PresetNumber};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Position camera as desired, then save to preset 1
    /// let preset = PresetNumber::new(1)?;
    /// client.save_preset_number(preset)?;
    ///
    /// // Save another position to preset 2
    /// let preset2 = PresetNumber::new(2)?;
    /// client.save_preset_number(preset2)?;
    /// # Ok(())
    /// # }
    /// ```
    fn save_preset_number(&mut self, preset_number: PresetNumber) -> Result<(), ViscaError> {
        let command = PresetCommand {
            action: PresetAction::Set,
            preset_number,
        };
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Recall a saved preset position.
    ///
    /// The camera will move to the saved pan/tilt/zoom/focus position.
    ///
    /// # Arguments
    /// * `preset_number` - Preset slot number
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaPresetExt, PresetNumber};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Return to preset position 1
    /// let preset1 = PresetNumber::new(1)?;
    /// client.recall_preset_number(preset1)?;
    ///
    /// // Move to home position (preset 0 is often home)
    /// let home = PresetNumber::new(0)?;
    /// client.recall_preset_number(home)?;
    /// # Ok(())
    /// # }
    /// ```
    fn recall_preset_number(&mut self, preset_number: PresetNumber) -> Result<(), ViscaError> {
        let command = PresetCommand {
            action: PresetAction::Recall,
            preset_number,
        };
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Reset a preset to its default state.
    ///
    /// # Arguments
    /// * `preset_number` - Preset slot number
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaPresetExt, PresetNumber};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Clear preset 1
    /// let preset1 = PresetNumber::new(1)?;
    /// client.reset_preset(preset1)?;
    /// # Ok(())
    /// # }
    /// ```
    fn reset_preset_number(&mut self, preset_number: PresetNumber) -> Result<(), ViscaError> {
        let command = PresetCommand {
            action: PresetAction::Reset,
            preset_number,
        };
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }
}

/// Implement the trait for all types that implement `ViscaTransportExt`
impl<T: Transport> ViscaPresetExt for T {}
