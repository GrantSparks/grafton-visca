//! High-level extension trait for preset management operations.

use crate::command::preset::{PresetAction, PresetCommand};
use crate::error::ViscaError;
use crate::transport_ext::ViscaTransportExt;

/// Extension trait providing high-level preset management methods.
pub trait ViscaPresetExt: ViscaTransportExt {
    /// Save the current camera position (pan/tilt/zoom/focus) to a preset.
    ///
    /// # Arguments
    /// * `preset_number` - Preset slot number (0-254)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaPresetExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Position camera as desired, then save to preset 1
    /// transport.save_preset(1)?;
    ///
    /// // Save another position to preset 2
    /// transport.save_preset(2)?;
    /// # Ok(())
    /// # }
    /// ```
    fn save_preset(&mut self, preset_number: u8) -> Result<(), ViscaError> {
        let command = PresetCommand {
            action: PresetAction::Set,
            preset_number,
        };
        self.send_command(&command)?;
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
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaPresetExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Return to preset position 1
    /// transport.recall_preset(1)?;
    ///
    /// // Move to home position (preset 0 is often home)
    /// transport.recall_preset(0)?;
    /// # Ok(())
    /// # }
    /// ```
    fn recall_preset(&mut self, preset_number: u8) -> Result<(), ViscaError> {
        let command = PresetCommand {
            action: PresetAction::Recall,
            preset_number,
        };
        self.send_command(&command)?;
        Ok(())
    }

    /// Reset a preset to its default state.
    ///
    /// # Arguments
    /// * `preset_number` - Preset slot number (0-254)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaPresetExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Clear preset 1
    /// transport.reset_preset(1)?;
    ///
    /// // Reset all presets (preset 255 typically means all)
    /// transport.reset_preset(255)?;
    /// # Ok(())
    /// # }
    /// ```
    fn reset_preset(&mut self, preset_number: u8) -> Result<(), ViscaError> {
        let command = PresetCommand {
            action: PresetAction::Reset,
            preset_number,
        };
        self.send_command(&command)?;
        Ok(())
    }
}

/// Implement the trait for all types that implement ViscaTransportExt
impl<T: ViscaTransportExt> ViscaPresetExt for T {}
