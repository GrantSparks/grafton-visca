//! Preset methods for cameras using the new GAT architecture.

use crate::{
    capabilities::ValidationError,
    command::{
        preset::{PresetAction, PresetCommand, PresetNumber},
        Response,
    },
    Error,
};

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
pub trait PresetsOpsBlocking: Sized {
    /// Recall a preset position.
    fn preset_recall(&self, preset: PresetNumber) -> Result<(), Error>;

    /// Set current position as a preset.
    fn preset_set(&self, preset: PresetNumber) -> Result<(), Error>;

    /// Reset/clear a preset.
    fn preset_reset(&self, preset: PresetNumber) -> Result<(), Error>;
}

// Async implementation
#[cfg(feature = "async")]
impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> PresetsOps
    for crate::camera::generic::Camera<P, T>
{
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

    async fn preset_reset(&self, preset: PresetNumber) -> Result<(), Error> {
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
            action: PresetAction::Reset,
            preset_number: preset,
        };
        let response = self.send_command(&command).await?;
        match response {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blocking implementation
impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> PresetsOpsBlocking
    for crate::camera::generic::Camera<P, T>
{
    fn preset_recall(&self, preset: PresetNumber) -> Result<(), Error> {
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

    fn preset_set(&self, preset: PresetNumber) -> Result<(), Error> {
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

    fn preset_reset(&self, preset: PresetNumber) -> Result<(), Error> {
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
            action: PresetAction::Reset,
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
