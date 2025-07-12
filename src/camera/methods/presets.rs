//! Preset methods for cameras using the new GAT architecture.

use crate::{
    camera::unified::Camera,
    capabilities::ValidationError,
    command::preset::{PresetCommand, PresetNumber, PresetAction},
    Error, Response,
};

/// Presets operations.
pub trait PresetsOps: Sized {

    /// Recall a preset position.
    #[cfg(feature = "tokio")]
    async fn preset_recall(&self, preset: PresetNumber) -> Result<(), Error>;

    /// Recall a preset position. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn preset_recall_blocking(&mut self, preset: PresetNumber) -> Result<(), Error>;

    /// Set current position as a preset.
    #[cfg(feature = "tokio")]
    async fn preset_set(&self, preset: PresetNumber) -> Result<(), Error>;

    /// Set current position as a preset. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn preset_set_blocking(&mut self, preset: PresetNumber) -> Result<(), Error>;
}

impl PresetsOps for Camera {
    #[cfg(feature = "tokio")]
    async fn preset_recall(&self, preset: PresetNumber) -> Result<(), Error> {
        // Validate preset number (0 is valid - it's the home position)
        if preset.value() > self.max_presets() {
            return Err(Error::ValidationError(ValidationError::InvalidValue {
                parameter: "preset",
                message: format!(
                    "Preset {} is invalid, must be 0-{}",
                    preset.value(),
                    self.max_presets()
                ),
            }));
        }

        let command = PresetCommand {
            action: PresetAction::Recall,
            preset_number: preset,
        };
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn preset_recall_blocking(&mut self, preset: PresetNumber) -> Result<(), Error> {
        
            // Validate preset number (0 is valid - it's the home position)
            if preset.value() > self.max_presets() {
                return Err(Error::ValidationError(ValidationError::InvalidValue {
                    parameter: "preset",
                    message: format!(
                        "Preset {} is invalid, must be 0-{}",
                        preset.value(),
                        self.max_presets()
                    ),
                }));
            }

            let command = PresetCommand {
                action: PresetAction::Recall,
                preset_number: preset,
            };
            let response = self.send_command_blocking(&command)?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    async fn preset_set(&self, preset: PresetNumber) -> Result<(), Error> {
        // Validate preset number (0 is valid - it's the home position)
        if preset.value() > self.max_presets() {
            return Err(Error::ValidationError(ValidationError::InvalidValue {
                parameter: "preset",
                message: format!(
                    "Preset {} is invalid, must be 0-{}",
                    preset.value(),
                    self.max_presets()
                ),
            }));
        }

        let command = PresetCommand {
            action: PresetAction::Set,
            preset_number: preset,
        };
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[cfg(not(feature = "tokio"))]
    fn preset_set_blocking(&mut self, preset: PresetNumber) -> Result<(), Error> {
        
            // Validate preset number (0 is valid - it's the home position)
            if preset.value() > self.max_presets() {
                return Err(Error::ValidationError(ValidationError::InvalidValue {
                    parameter: "preset",
                    message: format!(
                        "Preset {} is invalid, must be 0-{}",
                        preset.value(),
                        self.max_presets()
                    ),
                }));
            }

            let command = PresetCommand {
                action: PresetAction::Set,
                preset_number: preset,
            };
            let response = self.send_command_blocking(&command)?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
}

